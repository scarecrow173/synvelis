//! Change selection → typed write plan + preflight for VCS-05 apply.
//!
//! The plan is built once against the live project and the after
//! snapshot, then executed inside one [`crate::ProjectController`]
//! apply — the controller's scratch-clone + `validate_references`
//! oracle is the final arbiter, so any inconsistency the planner
//! missed still fails unchanged.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::{
    Error, IdGenerator, Operation, Project,
    diff::{ChangeKind, ChangeSet, ObjectKind, SemanticChange, SemanticObjectRef},
    id::{AssetId, SourceId},
    routing::{PortDirection, ProcessorBinding},
    snapshot::{BlobProbe, BlobRead, SnapshotCancel},
};

use super::{ApplyError, entity};

/// One planned entity transplant.
#[derive(Clone, Debug)]
pub(crate) struct PlannedWrite {
    /// The object the change targets.
    pub object: SemanticObjectRef,
    /// The `ChangeSet` change ID this write realizes.
    pub change_id: crate::diff::ChangeId,
    /// The semantic change kind (drives reconcile/protected fields).
    pub change_kind: ChangeKind,
    /// Whole-entity JSON payload (protected fields already adjusted).
    pub payload: Value,
}

/// The single [`Operation`] an apply commits — holds the plan plus
/// the after-project used for membership reconciliation and MIDI
/// note ownership.
pub(crate) struct ApplySelectedOp {
    /// Planned writes in `ChangeSet` order.
    pub writes: Vec<PlannedWrite>,
    /// After-snapshot project projection (read-only source of truth
    /// for membership positions and note ownership).
    pub after: Project,
}

impl Operation for ApplySelectedOp {
    fn validate(&self, project: &Project) -> Result<(), Error> {
        // Cheap existence re-check — the planner already ran these
        // against the same live state; the oracle in `apply` is the
        // authoritative gate.
        for write in &self.writes {
            if write.change_kind == ChangeKind::Added
                && entity::entity_value(project, &write.object)
                    .map_err(|e| apply_err_to_error(&e))?
                    .is_some()
            {
                return Err(Error::Validation(format!(
                    "added object already exists: {}",
                    write.object.stable_id
                )));
            }
        }
        Ok(())
    }

    fn apply(&self, project: &mut Project, ids: &mut dyn IdGenerator) -> Result<(), Error> {
        entity::write_all(project, &self.after, &self.writes, ids)
    }
}

fn apply_err_to_error(e: &ApplyError) -> Error {
    Error::Validation(e.to_string())
}

/// Kinds whose `Removed` changes are deferred (cascade removal is a
/// separate design — every removal fails unchanged in this slice).
fn change_supported(object: &SemanticObjectRef, change: &SemanticChange) -> Result<(), ApplyError> {
    let kind = object.kind;
    if !entity::supported_kind(kind) {
        return Err(ApplyError::Unsupported {
            kind,
            change: change.kind,
            detail: "object kind outside the apply surface".into(),
        });
    }
    match change.kind {
        ChangeKind::Removed => Err(ApplyError::Unsupported {
            kind,
            change: change.kind,
            detail: "removal changes are deferred (cascade design)".into(),
        }),
        ChangeKind::Reordered => {
            if matches!(
                kind,
                ObjectKind::Track | ObjectKind::Lane | ObjectKind::ProcessorChain
            ) {
                Ok(())
            } else {
                Err(ApplyError::Unsupported {
                    kind,
                    change: change.kind,
                    detail: "reorder only applies to ordered membership lists".into(),
                })
            }
        }
        ChangeKind::Reconnected => {
            if kind == ObjectKind::RoutingConnection {
                Ok(())
            } else {
                Err(ApplyError::Unsupported {
                    kind,
                    change: change.kind,
                    detail: "reconnect only applies to routing connections".into(),
                })
            }
        }
        ChangeKind::Replaced => {
            if matches!(
                kind,
                ObjectKind::Clip | ObjectKind::Asset | ObjectKind::MediaSource
            ) {
                Ok(())
            } else {
                Err(ApplyError::Unsupported {
                    kind,
                    change: change.kind,
                    detail: "replace only applies to clip/asset/source content swaps".into(),
                })
            }
        }
        ChangeKind::Moved => {
            if matches!(
                kind,
                ObjectKind::Clip | ObjectKind::MidiNote | ObjectKind::Processor
            ) {
                Ok(())
            } else {
                Err(ApplyError::Unsupported {
                    kind,
                    change: change.kind,
                    detail: "move only applies to clip/midi-note/processor placement".into(),
                })
            }
        }
        ChangeKind::Added | ChangeKind::Modified => Ok(()),
    }
}

/// Field-evidence gate for the singleton kinds: `Project` accepts
/// only `name`; `RoutingModel` accepts only `master`. Any other field
/// diff fails closed as [`ApplyError::UnsupportedField`].
fn check_singleton_fields(change: &SemanticChange) -> Result<(), ApplyError> {
    let allowed: &str = match change.object.kind {
        ObjectKind::Project => "name",
        ObjectKind::RoutingModel => "master",
        _ => return Ok(()),
    };
    for field in &change.evidence.fields {
        if field.field != allowed {
            return Err(ApplyError::UnsupportedField {
                kind: change.object.kind,
                field: field.field.clone(),
            });
        }
    }
    Ok(())
}

/// Builds the typed write plan for `selected` change IDs, in
/// `ChangeSet` order. Fails closed on the first unsupported,
/// misidentified, or missing target.
pub(crate) fn plan_changes(
    live: &Project,
    after: &Project,
    changeset: &ChangeSet,
    selected: &[crate::diff::ChangeId],
    cancel: &SnapshotCancel,
) -> Result<Vec<PlannedWrite>, ApplyError> {
    let wanted: BTreeSet<&crate::diff::ChangeId> = selected.iter().collect();
    let mut found: BTreeSet<&crate::diff::ChangeId> = BTreeSet::new();
    let mut planned = Vec::new();
    for change in &changeset.changes {
        if cancel.is_cancelled() {
            return Err(ApplyError::Cancelled);
        }
        if !wanted.contains(&change.id) {
            continue;
        }
        found.insert(&change.id);
        change_supported(&change.object, change)?;
        check_singleton_fields(change)?;
        let is_added = change.kind == ChangeKind::Added;
        // Existence rule: Added must be absent live; everything else
        // must be present live (stale/misidentified targets fail).
        let live_value = entity::entity_value(live, &change.object)?;
        match (is_added, live_value.is_some()) {
            (true, true) => {
                return Err(ApplyError::TargetConflict {
                    object: change.object.clone(),
                });
            }
            (false, false) => {
                return Err(ApplyError::TargetMissing {
                    object: change.object.clone(),
                });
            }
            _ => {}
        }
        let payload = entity::payload_for(live, after, &change.object, change.kind, is_added)?;
        planned.push(PlannedWrite {
            object: change.object.clone(),
            change_id: change.id.clone(),
            change_kind: change.kind,
            payload,
        });
    }
    // Every selected ID must resolve to a ChangeSet change — an
    // unresolvable selection is rejected, never silently dropped.
    for id in selected {
        if !found.contains(id) {
            return Err(ApplyError::UnknownChange {
                change_id: id.clone(),
            });
        }
    }
    Ok(planned)
}

// ---------------------------------------------------------------------------
// preflight
// ---------------------------------------------------------------------------

/// Digests introduced by the selection that must verify `Present`
/// (or at least `Unknown`-free) through the blob probe. Only *new*
/// references are probed — assets the live project already references
/// are not re-judged by an unrelated apply.
fn introduced_digests(
    live: &Project,
    after: &Project,
    planned: &[PlannedWrite],
) -> BTreeSet<String> {
    let live_digests: BTreeSet<String> = live
        .assets
        .values()
        .map(|a| a.digest.clone())
        .chain(live.sources.values().map(|s| s.content_digest.clone()))
        .collect();
    let mut out = BTreeSet::new();
    let push = |digest: &str, out: &mut BTreeSet<String>| {
        if !live_digests.contains(digest) {
            out.insert(digest.to_owned());
        }
    };
    for write in planned {
        match write.object.kind {
            ObjectKind::Asset => {
                if let Some(d) = write.payload.get("digest").and_then(Value::as_str) {
                    push(d, &mut out);
                }
            }
            ObjectKind::MediaSource => {
                if let Some(d) = write.payload.get("content_digest").and_then(Value::as_str) {
                    push(d, &mut out);
                }
                // The backing asset's digest (resolved through the
                // after project so Added assets are covered too).
                if let Some(asset_id) = write
                    .payload
                    .get("asset_id")
                    .and_then(Value::as_str)
                    .and_then(|s| AssetId::from_tagged(s).ok())
                    .and_then(|id| after.assets.get(&id))
                    .map(|a| a.digest.clone())
                {
                    push(&asset_id, &mut out);
                }
            }
            ObjectKind::Clip if write.change_kind == ChangeKind::Added => {
                // A new clip on an existing source re-references an
                // already-live digest — skip. A new clip on a new or
                // re-digested source probes the source's digest.
                if let Some(source_id) = write
                    .payload
                    .get("source_id")
                    .and_then(Value::as_str)
                    .and_then(|s| SourceId::from_tagged(s).ok())
                {
                    let digest = after
                        .sources
                        .get(&source_id)
                        .map(|s| s.content_digest.clone())
                        .or_else(|| {
                            after
                                .assets
                                .values()
                                .find(|a| {
                                    after
                                        .sources
                                        .get(&source_id)
                                        .is_some_and(|s| s.asset_id == a.id)
                                })
                                .map(|a| a.digest.clone())
                        });
                    if let Some(d) = digest {
                        push(&d, &mut out);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Asset preflight: every newly introduced digest must probe
/// `Present`; `Missing`/`Unknown` fail closed. With no probe
/// configured every digest is `Unknown` → fail closed.
pub(crate) fn preflight_assets(
    live: &Project,
    planned: &[PlannedWrite],
    probe: Option<&dyn BlobProbe>,
    after: &Project,
) -> Result<(), ApplyError> {
    for digest in introduced_digests(live, after, planned) {
        let status = probe.map_or(BlobRead::Unknown, |p| p.probe(&digest));
        match status {
            BlobRead::Present(_) => {}
            BlobRead::Absent => return Err(ApplyError::MissingAsset { digest }),
            BlobRead::Unknown => return Err(ApplyError::AssetUnknown { digest }),
        }
    }
    Ok(())
}

/// Plugin preflight: a Processor write whose after-binding is
/// `Unresolved` — or `Resolved` but reported unavailable — fails.
/// Pre-existing unresolved bindings that stay unresolved are allowed
/// (they are the snapshot's own honest state, not a new violation).
pub(crate) fn preflight_plugins(
    live: &Project,
    planned: &[PlannedWrite],
    probe: Option<&dyn super::PluginAvailability>,
) -> Result<(), ApplyError> {
    for write in planned {
        if write.object.kind != ObjectKind::Processor {
            continue;
        }
        let proc_id =
            crate::id::ProcessorId::from_tagged(&write.object.stable_id).map_err(|e| {
                ApplyError::InvalidIntent {
                    detail: e.to_string(),
                }
            })?;
        let after_binding: ProcessorBinding =
            serde_json::from_value(write.payload.get("binding").cloned().unwrap_or(Value::Null))
                .map_err(|e| ApplyError::Source {
                    detail: format!("processor binding: {e}"),
                })?;
        let live_binding = live.routing.processors.get(&proc_id).map(|p| &p.binding);
        let became_unresolved = matches!(after_binding, ProcessorBinding::Unresolved { .. })
            && !matches!(live_binding, Some(ProcessorBinding::Unresolved { .. }));
        if write.change_kind == ChangeKind::Added
            && matches!(after_binding, ProcessorBinding::Unresolved { .. })
        {
            return Err(ApplyError::MissingPlugin {
                detail: format!("added processor {proc_id} has unresolved binding"),
            });
        }
        if became_unresolved {
            return Err(ApplyError::MissingPlugin {
                detail: format!("processor {proc_id} binding became unresolved"),
            });
        }
        if let ProcessorBinding::Resolved(plugin) = &after_binding {
            let was_unresolved = matches!(
                live_binding,
                Some(ProcessorBinding::Unresolved { .. }) | None
            );
            if (was_unresolved || write.change_kind == ChangeKind::Added)
                && let Some(probe) = probe
                && !probe.plugin_available(plugin)
            {
                return Err(ApplyError::MissingPlugin {
                    detail: format!(
                        "plugin {}@{} unavailable",
                        plugin.descriptor, plugin.version
                    ),
                });
            }
        }
    }
    Ok(())
}

/// Port preflight: every connection/send endpoint must resolve in the
/// after project, and newly referenced ports must exist either live
/// or inside this same plan. Direction compatibility is checked
/// cheaply here; the routing oracle does the deep validation inside
/// the single-writer apply.
pub(crate) fn preflight_ports(
    live: &Project,
    after: &Project,
    planned: &[PlannedWrite],
) -> Result<(), ApplyError> {
    // Ports materializing inside this apply.
    let mut added_ports: BTreeSet<crate::id::SemanticPortId> = BTreeSet::new();
    for write in planned {
        if write.object.kind == ObjectKind::SemanticPort
            && write.change_kind == ChangeKind::Added
            && let Ok(id) = crate::id::SemanticPortId::from_tagged(&write.object.stable_id)
        {
            added_ports.insert(id);
        }
    }
    let port_exists = |id: &crate::id::SemanticPortId| {
        live.routing.ports.contains_key(id) || added_ports.contains(id)
    };
    for write in planned {
        match write.object.kind {
            ObjectKind::RoutingConnection => {
                let source = write
                    .payload
                    .get("source")
                    .and_then(Value::as_str)
                    .and_then(|s| crate::id::SemanticPortId::from_tagged(s).ok())
                    .ok_or_else(|| ApplyError::InvalidPort {
                        detail: format!("connection {} has no source", write.object.stable_id),
                    })?;
                let dest = write
                    .payload
                    .get("destination")
                    .and_then(Value::as_str)
                    .and_then(|s| crate::id::SemanticPortId::from_tagged(s).ok())
                    .ok_or_else(|| ApplyError::InvalidPort {
                        detail: format!("connection {} has no destination", write.object.stable_id),
                    })?;
                for (label, id) in [("source", &source), ("destination", &dest)] {
                    if !port_exists(id) {
                        return Err(ApplyError::InvalidPort {
                            detail: format!("{label} port {id} does not exist post-apply"),
                        });
                    }
                }
                // Direction sanity against the after project's ports
                // (the post-apply truth for port fields).
                if let (Some(sp), Some(dp)) = (
                    after.routing.ports.get(&source),
                    after.routing.ports.get(&dest),
                ) && (sp.direction != PortDirection::Output
                    || dp.direction != PortDirection::Input)
                {
                    return Err(ApplyError::InvalidPort {
                        detail: format!("connection {} direction mismatch", write.object.stable_id),
                    });
                }
            }
            ObjectKind::SendConfig => {
                let conn = write
                    .payload
                    .get("connection")
                    .and_then(Value::as_str)
                    .and_then(|s| crate::id::RoutingConnectionId::from_tagged(s).ok())
                    .ok_or_else(|| ApplyError::InvalidPort {
                        detail: format!("send {} has no connection", write.object.stable_id),
                    })?;
                let exists = live.routing.connections.contains_key(&conn)
                    || planned.iter().any(|w| {
                        w.object.kind == ObjectKind::RoutingConnection
                            && w.change_kind == ChangeKind::Added
                            && w.object.stable_id == conn.to_tagged()
                    });
                if !exists {
                    return Err(ApplyError::InvalidPort {
                        detail: format!(
                            "send {} references missing connection {conn}",
                            write.object.stable_id
                        ),
                    });
                }
            }
            _ => {}
        }
    }
    // Strip/processor port membership sanity for port writes is left
    // to the oracle — owner refs are identity-classified.
    Ok(())
}
