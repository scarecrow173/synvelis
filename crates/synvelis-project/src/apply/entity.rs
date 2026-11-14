//! Entity extraction, canonical fingerprinting, and typed transplant
//! for VCS-05 apply. Everything here moves *whole typed entities*
//! between the after-snapshot projection and the live project —
//! never field-level JSON path patching.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::{
    Error, IdGenerator, Project,
    comp::{Comp, CompRegion, CompTransition, TakeGroup},
    control::{AutomationLane, ModulationAssignment, ModulationSource, ParameterTarget},
    diff::{ChangeKind, ObjectKind, SemanticObjectRef},
    id::{
        AssetId, AutomationLaneId, BusId, ChainId, ClipId, CompId, CompRegionId, CompTransitionId,
        LaneId, ModulationAssignmentId, ModulationSourceId, NoteId, ParameterTargetId, ProcessorId,
        RoutingConnectionId, SemanticPortId, SendId, SourceId, StripId, TakeGroupId, TrackId,
    },
    midi::{MidiNote, MidiSource},
    monitoring_data::{PunchMarks, TrackMonitorDecl},
    project::{Asset, Lane, Note, Source, Track},
    routing::{
        Bus, ChannelStrip, PortOwner, Processor, ProcessorChain, RoutingConnection, SemanticPort,
        SendConfig,
    },
    snapshot::ContentDigest,
};

use super::{ApplyError, plan::PlannedWrite};

// ---------------------------------------------------------------------------
// canonical entity value extraction (fingerprint + transplant payload)
// ---------------------------------------------------------------------------

fn invalid_id(detail: String) -> ApplyError {
    ApplyError::InvalidIntent { detail }
}

fn to_value<T: serde::Serialize>(entity: &T) -> Result<Value, ApplyError> {
    serde_json::to_value(entity).map_err(|e| ApplyError::Source {
        detail: e.to_string(),
    })
}

/// `true` for object kinds inside the VCS apply surface
/// (`apply_selected`/`target_fingerprint`). Everything else fails
/// closed at plan/fingerprint time. Comping entities stay outside:
/// user-selected partial application cannot preserve their domain
/// invariants — they materialize only through the variant adopt path
/// ([`adopt_supported_kind`]), whose candidates are domain-validated.
pub(crate) fn supported_kind(kind: ObjectKind) -> bool {
    matches!(
        kind,
        ObjectKind::Project
            | ObjectKind::Track
            | ObjectKind::Lane
            | ObjectKind::Clip
            | ObjectKind::Note
            | ObjectKind::Asset
            | ObjectKind::MediaSource
            | ObjectKind::MidiSource
            | ObjectKind::MidiNote
            | ObjectKind::ParameterTarget
            | ObjectKind::AutomationLane
            | ObjectKind::ModulationSource
            | ObjectKind::ModulationAssignment
            | ObjectKind::RoutingModel
            | ObjectKind::Bus
            | ObjectKind::Strip
            | ObjectKind::Processor
            | ObjectKind::ProcessorChain
            | ObjectKind::SemanticPort
            | ObjectKind::RoutingConnection
            | ObjectKind::SendConfig
            | ObjectKind::TrackMonitorDecl
            | ObjectKind::PunchMarks
    )
}

/// `true` for object kinds the variant adopt path may materialize —
/// the VCS apply surface plus the comping registry, whose entities
/// arrive only inside domain-validated branch candidates.
pub(crate) fn adopt_supported_kind(kind: ObjectKind) -> bool {
    supported_kind(kind)
        || matches!(
            kind,
            ObjectKind::TakeGroup
                | ObjectKind::Comp
                | ObjectKind::CompRegion
                | ObjectKind::CompTransition
        )
}

/// The canonical JSON view of one object inside `project`.
/// `Ok(None)` = the object kind is supported but the entity is absent.
pub(crate) fn entity_value(
    project: &Project,
    object: &SemanticObjectRef,
) -> Result<Option<Value>, ApplyError> {
    macro_rules! keyed {
        ($ty:ty, $map:expr) => {{
            let id = <$ty>::from_tagged(&object.stable_id)
                .map_err(|e| invalid_id(format!("{}: {e}", object.stable_id)))?;
            Ok($map.get(&id).map(to_value).transpose()?)
        }};
    }
    match object.kind {
        ObjectKind::Project => Ok(Some(json!({
            "name": project.name,
            "ppqn": project.ppqn,
            "tempo_map": project.tempo_map,
        }))),
        ObjectKind::RoutingModel => Ok(Some(json!({ "master": project.routing.master }))),
        ObjectKind::Track => keyed!(TrackId, &project.tracks),
        ObjectKind::Lane => keyed!(LaneId, &project.lanes),
        ObjectKind::Clip => keyed!(ClipId, &project.clips),
        ObjectKind::Note => keyed!(NoteId, &project.notes),
        ObjectKind::Asset => keyed!(AssetId, &project.assets),
        ObjectKind::MediaSource => keyed!(SourceId, &project.sources),
        ObjectKind::MidiSource => keyed!(SourceId, &project.midi_sources),
        ObjectKind::MidiNote => {
            let id = NoteId::from_tagged(&object.stable_id)
                .map_err(|e| invalid_id(format!("{}: {e}", object.stable_id)))?;
            let note = project.midi_sources.values().find_map(|s| s.notes.get(&id));
            Ok(note.map(to_value).transpose()?)
        }
        ObjectKind::ParameterTarget => keyed!(ParameterTargetId, &project.control.targets),
        ObjectKind::AutomationLane => keyed!(AutomationLaneId, &project.control.lanes),
        ObjectKind::ModulationSource => keyed!(ModulationSourceId, &project.control.sources),
        ObjectKind::ModulationAssignment => {
            keyed!(ModulationAssignmentId, &project.control.assignments)
        }
        ObjectKind::Bus => keyed!(BusId, &project.routing.buses),
        ObjectKind::Strip => keyed!(StripId, &project.routing.strips),
        ObjectKind::Processor => keyed!(ProcessorId, &project.routing.processors),
        ObjectKind::ProcessorChain => keyed!(ChainId, &project.routing.chains),
        ObjectKind::SemanticPort => keyed!(SemanticPortId, &project.routing.ports),
        ObjectKind::RoutingConnection => keyed!(RoutingConnectionId, &project.routing.connections),
        ObjectKind::SendConfig => keyed!(SendId, &project.routing.sends),
        ObjectKind::TrackMonitorDecl => {
            // Keyed-synthetic view: `stable_id` borrows the `TrackId`
            // space — the declaration itself carries no id, so this
            // is not a `keyed!` entity-map lookup.
            let track = TrackId::from_tagged(&object.stable_id)
                .map_err(|e| invalid_id(format!("{}: {e}", object.stable_id)))?;
            Ok(project
                .monitoring
                .tracks
                .get(&track)
                .map(to_value)
                .transpose()?)
        }
        ObjectKind::PunchMarks => {
            // Declaration-presence semantics mirror the diff
            // classifier: all-`None` marks mean "no punch declared",
            // which is the absent-entity encoding (`null`).
            let marks = &project.monitoring.punch;
            if marks.punch_in.is_some() || marks.punch_out.is_some() {
                Ok(Some(to_value(marks)?))
            } else {
                Ok(None)
            }
        }
        ObjectKind::TakeGroup => keyed!(TakeGroupId, &project.comp.take_groups),
        ObjectKind::Comp => keyed!(CompId, &project.comp.comps),
        ObjectKind::CompRegion => keyed!(CompRegionId, &project.comp.regions),
        ObjectKind::CompTransition => keyed!(CompTransitionId, &project.comp.transitions),
        _ => Err(ApplyError::Unsupported {
            kind: object.kind,
            change: ChangeKind::Modified,
            detail: "object kind outside the apply surface".into(),
        }),
    }
}

/// Canonical fingerprint over `objects` against `project` — see
/// [`super::target_fingerprint`].
///
/// Absent entities encode as `null` rather than erroring: `Added`
/// changes legitimately have no live entity, and the same encoding
/// applies on both sides of the comparison. Presence rules
/// (`TargetMissing`/`TargetConflict`) are enforced separately by the
/// planner and by the caller-visible target check.
pub(crate) fn fingerprint(
    project: &Project,
    objects: &BTreeSet<SemanticObjectRef>,
) -> Result<ContentDigest, ApplyError> {
    let mut parts = Vec::with_capacity(objects.len());
    for object in objects {
        if !supported_kind(object.kind) {
            return Err(ApplyError::Unsupported {
                kind: object.kind,
                change: ChangeKind::Modified,
                detail: "object kind outside the apply surface".into(),
            });
        }
        let value = entity_value(project, object)?;
        let kind = serde_json::to_string(&object.kind).map_err(|e| ApplyError::Source {
            detail: e.to_string(),
        })?;
        let text = match &value {
            Some(v) => serde_json::to_string(v).map_err(|e| ApplyError::Source {
                detail: e.to_string(),
            })?,
            None => "null".to_owned(),
        };
        parts.push(format!("{kind}|{}|{text}", object.stable_id));
    }
    Ok(ContentDigest::sha256(parts.join("\n").as_bytes()))
}

// ---------------------------------------------------------------------------
// transplant payload preparation
// ---------------------------------------------------------------------------

/// Membership-list fields that are NEVER transplanted from the
/// after-entity: parent membership is expressed by the child changes
/// themselves and reconciled explicitly. `Reordered` changes are the
/// single exception — the ordered list IS the change.
fn protected_fields(kind: ObjectKind) -> &'static [&'static str] {
    match kind {
        ObjectKind::Track => &["lane_ids"],
        ObjectKind::Lane => &["clip_ids"],
        ObjectKind::MidiSource => &["notes"],
        ObjectKind::Strip | ObjectKind::Processor => &["port_ids"],
        ObjectKind::ProcessorChain => &["processors"],
        _ => &[],
    }
}

/// Builds the transplant payload for one planned change: the full
/// after-entity JSON with protected membership fields reset — to the
/// live value for existing entities, to empty for `Added` (children
/// selected in the same apply populate them during reconcile).
/// `Reordered` keeps the after value (it is the change itself).
pub(crate) fn payload_for(
    live: &Project,
    after: &Project,
    object: &SemanticObjectRef,
    change_kind: ChangeKind,
    is_added: bool,
) -> Result<Value, ApplyError> {
    if object.kind == ObjectKind::Project {
        // Only `name` is applicable — ppqn/tempo changes are rejected
        // earlier at field-evidence check time; the payload carries
        // just the name.
        return Ok(json!({ "name": after.name }));
    }
    if object.kind == ObjectKind::RoutingModel {
        return Ok(json!({ "master": after.routing.master }));
    }
    let mut payload = entity_value(after, object)?.ok_or_else(|| ApplyError::Source {
        detail: format!("after snapshot lacks {}", object.stable_id),
    })?;
    if change_kind == ChangeKind::Reordered {
        return Ok(payload);
    }
    let Some(obj) = payload.as_object_mut() else {
        return Err(ApplyError::Source {
            detail: "entity payload is not an object".into(),
        });
    };
    for field in protected_fields(object.kind) {
        let replacement = if is_added {
            json!([])
        } else {
            entity_value(live, object)?
                .and_then(|v| v.get(*field).cloned())
                .unwrap_or_else(|| json!([]))
        };
        obj.insert((*field).to_owned(), replacement);
    }
    // `notes` is a map, not a list — fix the Added default.
    if object.kind == ObjectKind::MidiSource
        && is_added
        && let Some(notes) = obj.get_mut("notes")
    {
        *notes = json!({});
    }
    Ok(payload)
}

// ---------------------------------------------------------------------------
// write path (Operation::apply body)
// ---------------------------------------------------------------------------

fn from_value<T: serde::de::DeserializeOwned>(value: Value, what: &str) -> Result<T, Error> {
    serde_json::from_value(value).map_err(|e| Error::Validation(format!("{what}: {e}")))
}

fn insert_pos<T: PartialEq>(after_list: &[T], live_list: &[T], id: &T) -> usize {
    let Some(idx) = after_list.iter().position(|x| x == id) else {
        return live_list.len();
    };
    let count = after_list[..idx]
        .iter()
        .filter(|x| live_list.contains(x))
        .count();
    count.min(live_list.len())
}

/// Writes every planned entity into `project` (the controller's
/// scratch clone), then reconciles parent membership lists against
/// the after-snapshot ordering, and finally reseeds the ID generator
/// past every preserved stable ID so the persisted watermark stays
/// authoritative.
pub(crate) fn write_all(
    project: &mut Project,
    after: &Project,
    writes: &[PlannedWrite],
    ids: &mut dyn IdGenerator,
) -> Result<(), Error> {
    let mut max_new_id = 0u128;
    for write in writes {
        write_entity(project, after, write, &mut max_new_id)?;
    }
    reconcile_membership(project, after, writes)?;
    reseed_past(ids, max_new_id)?;
    Ok(())
}

fn reseed_past(ids: &mut dyn IdGenerator, max_new_id: u128) -> Result<(), Error> {
    if max_new_id == 0 {
        return Ok(());
    }
    if max_new_id >= crate::routing::WELL_KNOWN_BASE {
        return Err(Error::Validation(format!(
            "apply would preserve id raw {max_new_id} inside the reserved well-known range"
        )));
    }
    let needed = max_new_id.saturating_add(1);
    if ids.peek() < needed {
        ids.reseed(needed);
    }
    Ok(())
}

fn write_entity(
    project: &mut Project,
    after: &Project,
    write: &PlannedWrite,
    max_new_id: &mut u128,
) -> Result<(), Error> {
    let object = &write.object;
    let payload = write.payload.clone();
    macro_rules! keyed_insert {
        ($map:expr, $ty:ty) => {{
            let entity: $ty = from_value(payload, stringify!($ty))?;
            *max_new_id = (*max_new_id).max(entity.id.raw());
            $map.insert(entity.id, entity);
        }};
    }
    match object.kind {
        ObjectKind::Project => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct NameOnly {
                name: String,
            }
            let view: NameOnly = from_value(payload, "project name")?;
            project.name = view.name;
        }
        ObjectKind::RoutingModel => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct MasterOnly {
                master: crate::id::BusId,
            }
            let view: MasterOnly = from_value(payload, "routing master")?;
            project.routing.master = view.master;
        }
        ObjectKind::Track => keyed_insert!(project.tracks, Track),
        ObjectKind::Lane => keyed_insert!(project.lanes, Lane),
        ObjectKind::Clip => keyed_insert!(project.clips, crate::project::Clip),
        ObjectKind::Note => keyed_insert!(project.notes, Note),
        ObjectKind::Asset => keyed_insert!(project.assets, Asset),
        ObjectKind::MediaSource => keyed_insert!(project.sources, Source),
        ObjectKind::MidiSource => keyed_insert!(project.midi_sources, MidiSource),
        ObjectKind::MidiNote => {
            let note: MidiNote = from_value(payload, "midi note")?;
            *max_new_id = (*max_new_id).max(note.id.raw());
            // The owning source is the one the after-snapshot places
            // the note in — live must already contain it (added
            // sources are separate selected changes).
            let owner = after
                .midi_sources
                .iter()
                .find(|(_, s)| s.notes.contains_key(&note.id))
                .map(|(sid, _)| *sid)
                .ok_or_else(|| Error::Validation("midi note has no after owner".into()))?;
            let source = project
                .midi_sources
                .get_mut(&owner)
                .ok_or_else(|| Error::NotFound(format!("midi source {owner}")))?;
            source.notes.insert(note.id, note);
        }
        ObjectKind::ParameterTarget => keyed_insert!(project.control.targets, ParameterTarget),
        ObjectKind::AutomationLane => keyed_insert!(project.control.lanes, AutomationLane),
        ObjectKind::ModulationSource => keyed_insert!(project.control.sources, ModulationSource),
        ObjectKind::ModulationAssignment => {
            keyed_insert!(project.control.assignments, ModulationAssignment);
        }
        ObjectKind::Bus => keyed_insert!(project.routing.buses, Bus),
        ObjectKind::Strip => keyed_insert!(project.routing.strips, ChannelStrip),
        ObjectKind::Processor => {
            let entity: Processor = from_value(payload, "Processor")?;
            *max_new_id = (*max_new_id).max(entity.id.raw());
            // PC-C4-05: this direct entity-write path bypasses
            // `ProjectOp`, so it synchronizes the persisted
            // plugin-instance identity itself — `ensure` semantics keep
            // an unchanged plugin's id, rebind to a different
            // `PluginRef` (incl. `capability_revision`) mints a new
            // one, and `Unresolved` drops the entry. The post-apply
            // `validate_references` re-checks the bijection.
            project.plugin_instances.sync_processor(&entity)?;
            project.routing.processors.insert(entity.id, entity);
        }
        ObjectKind::ProcessorChain => keyed_insert!(project.routing.chains, ProcessorChain),
        ObjectKind::SemanticPort => keyed_insert!(project.routing.ports, SemanticPort),
        ObjectKind::RoutingConnection => {
            keyed_insert!(project.routing.connections, RoutingConnection);
        }
        ObjectKind::SendConfig => keyed_insert!(project.routing.sends, SendConfig),
        ObjectKind::TrackMonitorDecl => {
            // The declaration holds no id of its own: the borrowed
            // `TrackId` key is parsed from `stable_id`. It never
            // feeds `max_new_id` — the Track entity's own write owns
            // reseed when the track itself was added in this apply.
            let track = TrackId::from_tagged(&object.stable_id)
                .map_err(|e| Error::InvalidId(e.to_string()))?;
            let decl: TrackMonitorDecl = from_value(payload, "track monitor decl")?;
            project.monitoring.tracks.insert(track, decl);
        }
        ObjectKind::PunchMarks => {
            let marks: PunchMarks = from_value(payload, "punch marks")?;
            project.monitoring.punch = marks;
        }
        ObjectKind::TakeGroup => keyed_insert!(project.comp.take_groups, TakeGroup),
        ObjectKind::Comp => keyed_insert!(project.comp.comps, Comp),
        ObjectKind::CompRegion => keyed_insert!(project.comp.regions, CompRegion),
        ObjectKind::CompTransition => keyed_insert!(project.comp.transitions, CompTransition),
        _ => {
            return Err(Error::Validation(format!(
                "unsupported apply kind {:?}",
                object.kind
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// membership reconciliation
// ---------------------------------------------------------------------------

/// Re-derives parent membership for every written child so the live
/// project matches the after-snapshot's *selected* containment:
/// clips into lanes, lanes into tracks, processors into chains, ports
/// into strips/processors. Unselected siblings keep their live
/// positions; new members land at their after-order position among
/// the members that exist live.
fn reconcile_membership(
    project: &mut Project,
    after: &Project,
    writes: &[PlannedWrite],
) -> Result<(), Error> {
    for write in writes {
        match write.object.kind {
            ObjectKind::Clip => reconcile_clip(project, after, write)?,
            ObjectKind::Lane => reconcile_lane(project, after, write)?,
            ObjectKind::Processor => reconcile_processor(project, after, write)?,
            ObjectKind::SemanticPort => reconcile_port(project, after, write)?,
            _ => {}
        }
    }
    Ok(())
}

fn reconcile_clip(
    project: &mut Project,
    after: &Project,
    write: &PlannedWrite,
) -> Result<(), Error> {
    let clip_id = ClipId::from_tagged(&write.object.stable_id)
        .map_err(|e| Error::InvalidId(e.to_string()))?;
    let Some(clip) = project.clips.get(&clip_id).cloned() else {
        return Ok(());
    };
    for (lane_id, lane) in &mut project.lanes {
        if *lane_id != clip.lane_id {
            lane.clip_ids.retain(|c| *c != clip.id);
        }
    }
    if let Some(lane) = project.lanes.get_mut(&clip.lane_id)
        && !lane.clip_ids.contains(&clip.id)
    {
        let after_list = after
            .lanes
            .get(&clip.lane_id)
            .map(|l| l.clip_ids.clone())
            .unwrap_or_default();
        let pos = insert_pos(&after_list, &lane.clip_ids, &clip.id);
        lane.clip_ids.insert(pos, clip.id);
    }
    Ok(())
}

fn reconcile_lane(
    project: &mut Project,
    after: &Project,
    write: &PlannedWrite,
) -> Result<(), Error> {
    let lane_id = LaneId::from_tagged(&write.object.stable_id)
        .map_err(|e| Error::InvalidId(e.to_string()))?;
    let Some(lane) = project.lanes.get(&lane_id).cloned() else {
        return Ok(());
    };
    for (track_id, track) in &mut project.tracks {
        if *track_id != lane.track_id {
            track.lane_ids.retain(|l| *l != lane.id);
        }
    }
    if let Some(track) = project.tracks.get_mut(&lane.track_id)
        && !track.lane_ids.contains(&lane.id)
    {
        let after_list = after
            .tracks
            .get(&lane.track_id)
            .map(|t| t.lane_ids.clone())
            .unwrap_or_default();
        let pos = insert_pos(&after_list, &track.lane_ids, &lane.id);
        track.lane_ids.insert(pos, lane.id);
    }
    Ok(())
}

fn reconcile_processor(
    project: &mut Project,
    after: &Project,
    write: &PlannedWrite,
) -> Result<(), Error> {
    let proc_id = ProcessorId::from_tagged(&write.object.stable_id)
        .map_err(|e| Error::InvalidId(e.to_string()))?;
    // The after-snapshot's chain membership is the truth.
    let target = after
        .routing
        .chains
        .iter()
        .find(|(_, c)| c.processors.contains(&proc_id))
        .map(|(cid, c)| (*cid, c.processors.clone()));
    let Some((target_chain, after_list)) = target else {
        return Ok(());
    };
    for (chain_id, chain) in &mut project.routing.chains {
        if *chain_id != target_chain {
            chain.processors.retain(|p| *p != proc_id);
        }
    }
    if let Some(chain) = project.routing.chains.get_mut(&target_chain)
        && !chain.processors.contains(&proc_id)
    {
        let pos = insert_pos(&after_list, &chain.processors, &proc_id);
        chain.processors.insert(pos, proc_id);
    }
    Ok(())
}

fn reconcile_port(
    project: &mut Project,
    after: &Project,
    write: &PlannedWrite,
) -> Result<(), Error> {
    let port_id = SemanticPortId::from_tagged(&write.object.stable_id)
        .map_err(|e| Error::InvalidId(e.to_string()))?;
    let Some(port) = project.routing.ports.get(&port_id).cloned() else {
        return Ok(());
    };
    // A port has exactly one owner — clear stale memberships before
    // inserting into the current one (owner may have changed).
    for strip in project.routing.strips.values_mut() {
        strip.port_ids.retain(|p| *p != port_id);
    }
    for proc in project.routing.processors.values_mut() {
        proc.port_ids.retain(|p| *p != port_id);
    }
    match port.owner {
        PortOwner::Strip(strip_id) => {
            if let Some(strip) = project.routing.strips.get_mut(&strip_id)
                && !strip.port_ids.contains(&port_id)
            {
                let after_list = after
                    .routing
                    .strips
                    .get(&strip_id)
                    .map(|s| s.port_ids.clone())
                    .unwrap_or_default();
                let pos = insert_pos(&after_list, &strip.port_ids, &port_id);
                strip.port_ids.insert(pos, port_id);
            }
        }
        PortOwner::Processor(owner_id) => {
            if let Some(proc) = project.routing.processors.get_mut(&owner_id)
                && !proc.port_ids.contains(&port_id)
            {
                let after_list = after
                    .routing
                    .processors
                    .get(&owner_id)
                    .map(|p| p.port_ids.clone())
                    .unwrap_or_default();
                let pos = insert_pos(&after_list, &proc.port_ids, &port_id);
                proc.port_ids.insert(pos, port_id);
            }
        }
    }
    Ok(())
}
