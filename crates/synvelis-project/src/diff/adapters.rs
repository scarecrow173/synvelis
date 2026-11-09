//! Per-domain semantic-diff adapters (VCS-02, synvelis#93).
//!
//! Each [`DomainAdapter`] compares one [`ComparisonDomain`] of the
//! Project aggregate. New Project domains get their own adapter — the
//! engine stays domain-agnostic and Ruviform Graph/Media types are never
//! touched.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::control::MASTER_FADER_RAW;
use crate::id::{NoteId, ParameterTargetId, SourceId};
use crate::midi::MidiNote;
use crate::placement::{ClipExtent, TimeAnchor};
use crate::project::Clip;
use crate::project::Project;
use crate::range::MusicalRange;

use super::engine::{DiffContext, EntityId, EntityKey, FieldClass, diff_map};
use super::types::{
    AmbiguityReason, ChangeEvidence, ChangeKind, ComparisonDomain, DiffError, ObjectKind,
    SemanticChange, SemanticFieldDiff, SemanticTimeRange,
};

/// One domain adapter. Stateless; `diff` reads `ctx.before`/`ctx.after`
/// and appends confirmed changes / unresolved issues to the context.
pub trait DomainAdapter {
    /// The domain this adapter compares.
    fn domain(&self) -> ComparisonDomain;
    /// Runs the domain diff.
    ///
    /// # Errors
    /// Propagates [`DiffError`] on cancellation/budget exhaustion.
    fn diff(&self, ctx: &mut DiffContext<'_>) -> Result<(), DiffError>;
}

/// The built-in adapter set, one per [`ComparisonDomain`].
pub(crate) fn default_adapters() -> Vec<Box<dyn DomainAdapter>> {
    vec![
        Box::new(ProjectMetaAdapter),
        Box::new(ArrangementAdapter),
        Box::new(MediaAdapter),
        Box::new(ControlAdapter),
        Box::new(RoutingAdapter),
        Box::new(RecordingAdapter),
        Box::new(CompingAdapter),
    ]
}

fn no_range<T>(_: Option<&T>, _: Option<&T>) -> Option<SemanticTimeRange> {
    None
}

/// Clip affected range: the stored musical extent when both anchor and
/// extent are musical; `None` for absolute/mixed placements.
fn clip_range(before: Option<&Clip>, after: Option<&Clip>) -> Option<SemanticTimeRange> {
    let clip = after.or(before)?;
    if let (TimeAnchor::Musical(tick), ClipExtent::Musical(dur)) =
        (clip.placement.anchor, clip.placement.extent)
    {
        let end = tick.checked_add(dur.value()).ok()?;
        MusicalRange::try_new(tick, end)
            .ok()
            .map(SemanticTimeRange::Musical)
    } else {
        None
    }
}

/// Order-only list semantics: same multiset in a different sequence →
/// `Reordered`; different membership → `Skip` (child entities already
/// express membership through their own Added/Removed/Moved changes).
fn ordered_list_class(before: &Value, after: &Value) -> FieldClass {
    let (Value::Array(b), Value::Array(a)) = (before, after) else {
        return FieldClass::Modified;
    };
    let mut bs: Vec<String> = b.iter().map(ToString::to_string).collect();
    let mut as_: Vec<String> = a.iter().map(ToString::to_string).collect();
    bs.sort();
    as_.sort();
    if bs == as_ {
        FieldClass::Reordered
    } else {
        FieldClass::Skip
    }
}

/// Field-level change policy. `path` is the dotted canonical-JSON path
/// inside the entity (`placement.anchor`, `name`, ...). Identity-class
/// fields are owner/lineage references whose change means the entity
/// cannot be confirmed as the same object.
pub(crate) fn classify_field(
    ctx: &DiffContext<'_>,
    kind: ObjectKind,
    path: &str,
    before: &Value,
    after: &Value,
) -> FieldClass {
    let top = path.split('.').next().unwrap_or(path);
    // role/time_scope exist only from schema v7; earlier snapshots carry
    // migration-derived values that must not diff as authored content.
    let pre_v7 = ctx.before_id.schema_version < 7 || ctx.after_id.schema_version < 7;
    match kind.domain() {
        ComparisonDomain::Project => FieldClass::Modified,
        ComparisonDomain::Arrangement => classify_arrangement(kind, path, top, before, after),
        ComparisonDomain::Media => classify_media(kind, top),
        ComparisonDomain::Control => classify_control(kind, top, pre_v7),
        ComparisonDomain::Routing => classify_routing(kind, top),
        ComparisonDomain::Recording => classify_recording(kind, top),
        ComparisonDomain::Comping => classify_comping(kind, top, before, after),
    }
}

fn classify_arrangement(
    kind: ObjectKind,
    path: &str,
    top: &str,
    before: &Value,
    after: &Value,
) -> FieldClass {
    match kind {
        ObjectKind::Track => match top {
            "lane_ids" => ordered_list_class(before, after),
            _ => FieldClass::Modified,
        },
        ObjectKind::Lane => match top {
            "track_id" => FieldClass::Identity,
            "clip_ids" => ordered_list_class(before, after),
            _ => FieldClass::Modified,
        },
        ObjectKind::Clip => {
            // Canonical-JSON paths are leaf-dotted (`placement.anchor.
            // musical`), so anchor changes are matched by prefix.
            if path == "lane_id" || path.starts_with("placement.anchor") {
                FieldClass::Moved
            } else if path == "source_id" {
                FieldClass::Replaced
            } else {
                FieldClass::Modified
            }
        }
        ObjectKind::Note => match top {
            "clip" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        _ => FieldClass::Modified,
    }
}

fn classify_media(kind: ObjectKind, top: &str) -> FieldClass {
    match kind {
        ObjectKind::Asset => match top {
            "digest" => FieldClass::Replaced,
            _ => FieldClass::Modified,
        },
        ObjectKind::MediaSource => match top {
            "asset_id" | "content_digest" => FieldClass::Replaced,
            _ => FieldClass::Modified,
        },
        ObjectKind::MidiSource => match top {
            "notes" => FieldClass::Skip,
            _ => FieldClass::Modified,
        },
        _ => FieldClass::Modified,
    }
}

fn classify_control(kind: ObjectKind, top: &str, pre_v7: bool) -> FieldClass {
    match kind {
        ObjectKind::ParameterTarget => match top {
            "owner" => FieldClass::Identity,
            "role" | "time_scope" if pre_v7 => FieldClass::Suppressed("pre-v7-role-timescope"),
            _ => FieldClass::Modified,
        },
        ObjectKind::AutomationLane => match top {
            "target" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        ObjectKind::ModulationSource => match top {
            "owner" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        ObjectKind::ModulationAssignment => match top {
            "source" | "target" | "depth_source" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        _ => FieldClass::Modified,
    }
}

fn classify_routing(kind: ObjectKind, top: &str) -> FieldClass {
    match kind {
        ObjectKind::Bus => match top {
            "strip" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        ObjectKind::Strip => match top {
            "owner" | "chain" => FieldClass::Identity,
            "port_ids" => FieldClass::Skip,
            _ => FieldClass::Modified,
        },
        ObjectKind::Processor => match top {
            "port_ids" => FieldClass::Skip,
            _ => FieldClass::Modified,
        },
        ObjectKind::ProcessorChain => match top {
            "processors" => FieldClass::Skip,
            _ => FieldClass::Modified,
        },
        ObjectKind::SemanticPort => match top {
            "owner" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        ObjectKind::RoutingConnection => match top {
            "source" | "destination" => FieldClass::Reconnected,
            _ => FieldClass::Modified,
        },
        _ => FieldClass::Modified,
    }
}

fn classify_recording(kind: ObjectKind, top: &str) -> FieldClass {
    match kind {
        ObjectKind::RecordingPass => match top {
            "session_id" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        ObjectKind::CaptureStream => match top {
            "session_id" | "lane_id" | "clock_epoch_id" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        ObjectKind::CaptureSegment => match top {
            "stream_id" | "pass_id" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        ObjectKind::Take => match top {
            "session_id" | "pass_id" | "stream_id" | "lane_id" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        _ => FieldClass::Modified,
    }
}

fn classify_comping(kind: ObjectKind, top: &str, before: &Value, after: &Value) -> FieldClass {
    match kind {
        ObjectKind::TakeGroup => match top {
            "owner_lane_id" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        ObjectKind::Comp => match top {
            "take_group_id" => FieldClass::Identity,
            "region_ids" | "transition_ids" => ordered_list_class(before, after),
            _ => FieldClass::Modified,
        },
        ObjectKind::CompRegion => match top {
            "comp_id" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        ObjectKind::CompTransition => match top {
            "comp_id" | "left_region_id" | "right_region_id" => FieldClass::Identity,
            _ => FieldClass::Modified,
        },
        _ => FieldClass::Modified,
    }
}

// ---------------------------------------------------------------------
// Domain adapters
// ---------------------------------------------------------------------

struct ProjectMetaAdapter;

impl DomainAdapter for ProjectMetaAdapter {
    fn domain(&self) -> ComparisonDomain {
        ComparisonDomain::Project
    }

    fn diff(&self, ctx: &mut DiffContext<'_>) -> Result<(), DiffError> {
        let view = |p: &Project| {
            serde_json::json!({
                "name": p.name,
                "ppqn": p.ppqn,
                // revision is bookkeeping, not authored content.
                "tempo_map": {
                    "ppqn": p.tempo_map.ppqn,
                    "tempo_events": p.tempo_map.tempo_events,
                    "signature_events": p.tempo_map.signature_events,
                },
            })
        };
        let before = view(ctx.before);
        let after = view(ctx.after);
        if before == after {
            return Ok(());
        }
        let mut fields = Vec::new();
        super::engine::json_diff(&before, &after, "", &mut fields);
        let mut by_kind: BTreeMap<ChangeKind, Vec<SemanticFieldDiff>> = BTreeMap::new();
        for f in fields {
            match classify_field(ctx, ObjectKind::Project, &f.field, &f.before, &f.after) {
                FieldClass::Identity => {
                    return ctx.emit_ambiguous(
                        ctx.object_ref(ObjectKind::Project, ctx.before_id.project_id.to_tagged()),
                        AmbiguityReason::OwnerConflict,
                    );
                }
                FieldClass::Skip => {}
                FieldClass::Suppressed(d) => ctx.emit_suppressed(ComparisonDomain::Project, d)?,
                class => {
                    if let Some(ck) = class.change_kind() {
                        by_kind.entry(ck).or_default().push(f);
                    }
                }
            }
        }
        let stable_id = ctx.before_id.project_id.to_tagged();
        let object = ctx.object_ref(ObjectKind::Project, stable_id);
        let before_ref = DiffContext::value_ref(&before)?;
        let after_ref = DiffContext::value_ref(&after)?;
        for (ck, flds) in by_kind {
            let id = ctx.change_id(&object, ck, &flds);
            ctx.emit_change(SemanticChange {
                id,
                object: object.clone(),
                kind: ck,
                before: Some(before_ref),
                after: Some(after_ref),
                affected_range: None,
                evidence: ChangeEvidence { fields: flds },
                dependencies: Vec::new(),
            })?;
        }
        Ok(())
    }
}

struct ArrangementAdapter;

impl DomainAdapter for ArrangementAdapter {
    fn domain(&self) -> ComparisonDomain {
        ComparisonDomain::Arrangement
    }

    fn diff(&self, ctx: &mut DiffContext<'_>) -> Result<(), DiffError> {
        diff_map(
            ctx,
            ObjectKind::Track,
            &ctx.before.tracks,
            &ctx.after.tracks,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::Lane,
            &ctx.before.lanes,
            &ctx.after.lanes,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::Clip,
            &ctx.before.clips,
            &ctx.after.clips,
            clip_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::Note,
            &ctx.before.notes,
            &ctx.after.notes,
            no_range,
            false,
        )?;
        Ok(())
    }
}

struct MediaAdapter;

impl DomainAdapter for MediaAdapter {
    fn domain(&self) -> ComparisonDomain {
        ComparisonDomain::Media
    }

    fn diff(&self, ctx: &mut DiffContext<'_>) -> Result<(), DiffError> {
        diff_map(
            ctx,
            ObjectKind::Asset,
            &ctx.before.assets,
            &ctx.after.assets,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::MediaSource,
            &ctx.before.sources,
            &ctx.after.sources,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::MidiSource,
            &ctx.before.midi_sources,
            &ctx.after.midi_sources,
            no_range,
            false,
        )?;
        // SMF documents are keyed by their MIDI source's ID — the
        // document itself is content-addressed by that key.
        diff_map(
            ctx,
            ObjectKind::SmfDocument,
            &ctx.before.smf_documents,
            &ctx.after.smf_documents,
            no_range,
            true,
        )?;
        Self::diff_midi_notes(ctx)
    }
}

impl MediaAdapter {
    /// MIDI-source notes are child entities of `MidiSource` (which skips
    /// its `notes` field). They diff globally by `NoteId`; a note whose
    /// owning source changed is an owner conflict, not a move.
    fn diff_midi_notes(ctx: &mut DiffContext<'_>) -> Result<(), DiffError> {
        fn index(project: &Project) -> (BTreeMap<NoteId, &MidiNote>, BTreeMap<NoteId, SourceId>) {
            let mut notes = BTreeMap::new();
            let mut owners = BTreeMap::new();
            for (sid, src) in &project.midi_sources {
                for (nid, note) in &src.notes {
                    notes.insert(*nid, note);
                    owners.insert(*nid, *sid);
                }
            }
            (notes, owners)
        }
        let (b_notes, b_owners) = index(ctx.before);
        let (a_notes, a_owners) = index(ctx.after);
        ctx.tick(b_notes.len() + a_notes.len())?;
        ctx.entities += b_notes.len() + a_notes.len();
        let mut keys: std::collections::BTreeSet<NoteId> = b_notes.keys().copied().collect();
        keys.extend(a_notes.keys().copied());
        for key in keys {
            if ctx.cancel.is_cancelled() {
                return Err(DiffError::Cancelled);
            }
            let stable_id = key.tagged();
            let raw = key.raw_u128();
            match (b_notes.get(&key), a_notes.get(&key)) {
                (Some(b), Some(a)) => {
                    if ctx.raw_is_ambiguous(raw) {
                        ctx.emit_ambiguous(
                            ctx.object_ref(ObjectKind::MidiNote, stable_id),
                            AmbiguityReason::CrossKindId,
                        )?;
                        continue;
                    }
                    if b_owners.get(&key) != a_owners.get(&key) {
                        ctx.emit_ambiguous(
                            ctx.object_ref(ObjectKind::MidiNote, stable_id),
                            AmbiguityReason::OwnerConflict,
                        )?;
                        continue;
                    }
                    ctx.diff_matched(
                        EntityId {
                            kind: ObjectKind::MidiNote,
                            stable_id,
                            raw,
                        },
                        b,
                        a,
                        no_range,
                        false,
                    )?;
                }
                (Some(b), None) => ctx.diff_presence(
                    EntityId {
                        kind: ObjectKind::MidiNote,
                        stable_id,
                        raw,
                    },
                    Some(*b),
                    None,
                    no_range,
                )?,
                (None, Some(a)) => ctx.diff_presence(
                    EntityId {
                        kind: ObjectKind::MidiNote,
                        stable_id,
                        raw,
                    },
                    None,
                    Some(*a),
                    no_range,
                )?,
                (None, None) => {}
            }
        }
        Ok(())
    }
}

struct ControlAdapter;

impl DomainAdapter for ControlAdapter {
    fn domain(&self) -> ComparisonDomain {
        ComparisonDomain::Control
    }

    fn diff(&self, ctx: &mut DiffContext<'_>) -> Result<(), DiffError> {
        // The bootstrap master-fader target is migration-seeded before
        // schema v8; comparing a pre-v8 projection against an authored
        // one would fabricate a change. Suppress the entity and report
        // `PartialProjection` when it actually differs.
        let suppress_fader = ctx.before_id.schema_version < 8 || ctx.after_id.schema_version < 8;
        let fader_id = ParameterTargetId::new(MASTER_FADER_RAW);
        let (b_targets, a_targets) = if suppress_fader {
            let bf = ctx.before.control.targets.get(&fader_id);
            let af = ctx.after.control.targets.get(&fader_id);
            if !(matches!((bf, af), (Some(b), Some(a)) if b == a)) {
                ctx.emit_suppressed(ComparisonDomain::Control, "pre-v8-master-fader")?;
            }
            (
                filtered_targets(&ctx.before.control.targets, fader_id),
                filtered_targets(&ctx.after.control.targets, fader_id),
            )
        } else {
            (
                ctx.before.control.targets.clone(),
                ctx.after.control.targets.clone(),
            )
        };
        diff_map(
            ctx,
            ObjectKind::ParameterTarget,
            &b_targets,
            &a_targets,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::AutomationLane,
            &ctx.before.control.lanes,
            &ctx.after.control.lanes,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::ModulationSource,
            &ctx.before.control.sources,
            &ctx.after.control.sources,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::ModulationAssignment,
            &ctx.before.control.assignments,
            &ctx.after.control.assignments,
            no_range,
            false,
        )?;
        Ok(())
    }
}

fn filtered_targets(
    targets: &BTreeMap<ParameterTargetId, crate::control::ParameterTarget>,
    exclude: ParameterTargetId,
) -> BTreeMap<ParameterTargetId, crate::control::ParameterTarget> {
    targets
        .iter()
        .filter(|(id, _)| **id != exclude)
        .map(|(id, t)| (*id, t.clone()))
        .collect()
}

struct RoutingAdapter;

impl DomainAdapter for RoutingAdapter {
    fn domain(&self) -> ComparisonDomain {
        ComparisonDomain::Routing
    }

    fn diff(&self, ctx: &mut DiffContext<'_>) -> Result<(), DiffError> {
        // Pre-v6 snapshots carry a migration projection of routing, not
        // authored signal flow — projected IDs are watermark artifacts
        // and sends/FX/returns are unrepresentable. Never fabricate
        // entity-level routing diffs from it.
        if ctx.before_id.schema_version < 6 || ctx.after_id.schema_version < 6 {
            return ctx.emit_suppressed(ComparisonDomain::Routing, "pre-v6-routing");
        }
        // Model-level singleton: the master bus pointer.
        if ctx.before.routing.master != ctx.after.routing.master {
            let stable_id = ctx.before_id.project_id.to_tagged();
            let object = ctx.object_ref(ObjectKind::RoutingModel, stable_id);
            let before_v = Value::String(ctx.before.routing.master.to_tagged());
            let after_v = Value::String(ctx.after.routing.master.to_tagged());
            let fields = vec![SemanticFieldDiff {
                field: "master".to_string(),
                before: before_v.clone(),
                after: after_v.clone(),
            }];
            let id = ctx.change_id(&object, ChangeKind::Modified, &fields);
            ctx.emit_change(SemanticChange {
                id,
                object,
                kind: ChangeKind::Modified,
                before: Some(DiffContext::value_ref(&before_v)?),
                after: Some(DiffContext::value_ref(&after_v)?),
                affected_range: None,
                evidence: ChangeEvidence { fields },
                dependencies: Vec::new(),
            })?;
        }
        diff_map(
            ctx,
            ObjectKind::Bus,
            &ctx.before.routing.buses,
            &ctx.after.routing.buses,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::Strip,
            &ctx.before.routing.strips,
            &ctx.after.routing.strips,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::Processor,
            &ctx.before.routing.processors,
            &ctx.after.routing.processors,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::ProcessorChain,
            &ctx.before.routing.chains,
            &ctx.after.routing.chains,
            no_range,
            false,
        )?;
        Self::diff_chain_membership(ctx)?;
        diff_map(
            ctx,
            ObjectKind::SemanticPort,
            &ctx.before.routing.ports,
            &ctx.after.routing.ports,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::RoutingConnection,
            &ctx.before.routing.connections,
            &ctx.after.routing.connections,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::SendConfig,
            &ctx.before.routing.sends,
            &ctx.after.routing.sends,
            no_range,
            false,
        )?;
        Ok(())
    }
}

impl RoutingAdapter {
    /// `ProcessorChain.processors` membership is the only place a
    /// processor's chain assignment lives (processors carry no back
    /// reference). Same-set order change → `Reordered` on the chain;
    /// chain change on the same processor → `Moved` on the processor.
    fn diff_chain_membership(ctx: &mut DiffContext<'_>) -> Result<(), DiffError> {
        use crate::id::{ChainId, ProcessorId};
        let b_assign: BTreeMap<ProcessorId, ChainId> = ctx
            .before
            .routing
            .chains
            .iter()
            .flat_map(|(cid, chain)| chain.processors.iter().map(move |pid| (*pid, *cid)))
            .collect();
        let a_assign: BTreeMap<ProcessorId, ChainId> = ctx
            .after
            .routing
            .chains
            .iter()
            .flat_map(|(cid, chain)| chain.processors.iter().map(move |pid| (*pid, *cid)))
            .collect();
        Self::diff_chain_moves(ctx, &b_assign, &a_assign)?;
        Self::diff_chain_reorders(ctx)
    }

    /// Emits `Moved` on a processor whose chain assignment changed while
    /// the processor entity itself exists on both sides.
    fn diff_chain_moves(
        ctx: &mut DiffContext<'_>,
        b_assign: &BTreeMap<crate::id::ProcessorId, crate::id::ChainId>,
        a_assign: &BTreeMap<crate::id::ProcessorId, crate::id::ChainId>,
    ) -> Result<(), DiffError> {
        let mut keys: std::collections::BTreeSet<crate::id::ProcessorId> =
            b_assign.keys().copied().collect();
        keys.extend(a_assign.keys().copied());
        for pid in keys {
            if ctx.cancel.is_cancelled() {
                return Err(DiffError::Cancelled);
            }
            let (Some(bc), Some(ac)) = (b_assign.get(&pid), a_assign.get(&pid)) else {
                continue;
            };
            if bc == ac {
                continue;
            }
            if !ctx.before.routing.processors.contains_key(&pid)
                || !ctx.after.routing.processors.contains_key(&pid)
                || ctx.raw_is_ambiguous(pid.raw())
            {
                continue;
            }
            let object = ctx.object_ref(ObjectKind::Processor, pid.tagged());
            let fields = vec![SemanticFieldDiff {
                field: "chain".to_string(),
                before: Value::String(bc.tagged()),
                after: Value::String(ac.tagged()),
            }];
            let id = ctx.change_id(&object, ChangeKind::Moved, &fields);
            let before_ref = DiffContext::value_ref(&ctx.before.routing.processors[&pid])?;
            let after_ref = DiffContext::value_ref(&ctx.after.routing.processors[&pid])?;
            ctx.emit_change(SemanticChange {
                id,
                object,
                kind: ChangeKind::Moved,
                before: Some(before_ref),
                after: Some(after_ref),
                affected_range: None,
                evidence: ChangeEvidence { fields },
                dependencies: Vec::new(),
            })?;
        }
        Ok(())
    }

    /// Emits `Reordered` on a chain whose processor set is unchanged but
    /// whose persisted order differs.
    fn diff_chain_reorders(ctx: &mut DiffContext<'_>) -> Result<(), DiffError> {
        let mut chain_keys: std::collections::BTreeSet<crate::id::ChainId> =
            ctx.before.routing.chains.keys().copied().collect();
        chain_keys.extend(ctx.after.routing.chains.keys().copied());
        for cid in chain_keys {
            let (Some(bc), Some(ac)) = (
                ctx.before.routing.chains.get(&cid),
                ctx.after.routing.chains.get(&cid),
            ) else {
                continue;
            };
            let bs: Vec<String> = bc.processors.iter().map(EntityKey::tagged).collect();
            let as_: Vec<String> = ac.processors.iter().map(EntityKey::tagged).collect();
            let bs_sorted = {
                let mut s = bs.clone();
                s.sort();
                s
            };
            let as_sorted = {
                let mut s = as_.clone();
                s.sort();
                s
            };
            if bs == as_ || bs_sorted != as_sorted {
                continue;
            }
            if ctx.raw_is_ambiguous(cid.raw()) {
                continue;
            }
            let object = ctx.object_ref(ObjectKind::ProcessorChain, cid.tagged());
            let fields = vec![SemanticFieldDiff {
                field: "processors".to_string(),
                before: Value::Array(bs.iter().map(|s| Value::String(s.clone())).collect()),
                after: Value::Array(as_.iter().map(|s| Value::String(s.clone())).collect()),
            }];
            let id = ctx.change_id(&object, ChangeKind::Reordered, &fields);
            let before_ref = DiffContext::value_ref(bc)?;
            let after_ref = DiffContext::value_ref(ac)?;
            ctx.emit_change(SemanticChange {
                id,
                object,
                kind: ChangeKind::Reordered,
                before: Some(before_ref),
                after: Some(after_ref),
                affected_range: None,
                evidence: ChangeEvidence { fields },
                dependencies: Vec::new(),
            })?;
        }
        Ok(())
    }
}

struct RecordingAdapter;

impl DomainAdapter for RecordingAdapter {
    fn domain(&self) -> ComparisonDomain {
        ComparisonDomain::Recording
    }

    fn diff(&self, ctx: &mut DiffContext<'_>) -> Result<(), DiffError> {
        let rec_b = &ctx.before.recording;
        let rec_a = &ctx.after.recording;
        diff_map(
            ctx,
            ObjectKind::ClockEpoch,
            &ctx.before.clock_epochs,
            &ctx.after.clock_epochs,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::RecordingSession,
            &rec_b.sessions,
            &rec_a.sessions,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::RecordingPass,
            &rec_b.passes,
            &rec_a.passes,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::CaptureStream,
            &rec_b.streams,
            &rec_a.streams,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::CaptureSegment,
            &rec_b.segments,
            &rec_a.segments,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::Take,
            &rec_b.takes,
            &rec_a.takes,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::AppliedCommit,
            &rec_b.applied_commits,
            &rec_a.applied_commits,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::AppliedImport,
            &ctx.before.applied_imports,
            &ctx.after.applied_imports,
            no_range,
            false,
        )?;
        Ok(())
    }
}

struct CompingAdapter;

impl DomainAdapter for CompingAdapter {
    fn domain(&self) -> ComparisonDomain {
        ComparisonDomain::Comping
    }

    fn diff(&self, ctx: &mut DiffContext<'_>) -> Result<(), DiffError> {
        let comp_b = &ctx.before.comp;
        let comp_a = &ctx.after.comp;
        diff_map(
            ctx,
            ObjectKind::TakeGroup,
            &comp_b.take_groups,
            &comp_a.take_groups,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::Comp,
            &comp_b.comps,
            &comp_a.comps,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::CompRegion,
            &comp_b.regions,
            &comp_a.regions,
            no_range,
            false,
        )?;
        diff_map(
            ctx,
            ObjectKind::CompTransition,
            &comp_b.transitions,
            &comp_a.transitions,
            no_range,
            false,
        )?;
        Ok(())
    }
}
