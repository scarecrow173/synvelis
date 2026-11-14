//! Scope → owned-object-set derivation for Creative Branches
//! (synvelis#278, CB-01).
//!
//! [`scope_objects`] is the single model-level authority for "which
//! semantic objects does this scope own". It is used for scope
//! fingerprints (render-reuse / adopt staleness identity), fork-time
//! conflict detection, persisted-state validation, and adopt-time
//! transplant planning. Domain payloads never enter the scope
//! declaration — only typed identity and masks do.
//!
//! Comping entities are owned by their scope for
//! fingerprint/validation purposes and materialize through the apply
//! surface since CB-02 (synvelis#279) — `TakeUsage` adopt writes
//! `TakeGroup`/`Comp`/`CompRegion`/`CompTransition` entities while
//! raw takes and sources stay shared.

use std::collections::BTreeSet;

use serde_json::Value;

use super::types::{
    ArrangementDomainMask, ProcessingRangeRef, StripDomainMask, StripField, TakeUsageTarget,
    VariantScope,
};
use crate::{
    Error, Project,
    apply::entity,
    control::{ControlOwner, ParameterRole},
    diff::{ObjectKind, SemanticObjectRef},
    id::{
        ClipId, CompId, CompRegionId, CompTransitionId, ModulationSourceId, ParameterTargetId,
        ProcessorId, ProjectId, SemanticPortId, StripId, TakeGroupId, TrackId,
    },
    placement::{ClipExtent, ClipPlacement, TimeAnchor},
    range::MusicalRange,
    routing::PortOwner,
    snapshot::ContentDigest,
    time::MusicalTick,
};

fn object(project_id: ProjectId, kind: ObjectKind, stable_id: String) -> SemanticObjectRef {
    SemanticObjectRef {
        project_id,
        kind,
        stable_id,
        lineage: None,
    }
}

// ---------------------------------------------------------------------------
// control closure
// ---------------------------------------------------------------------------

/// Collects every control entity reachable from `owners`: targets owned
/// by them, automation lanes on those targets, modulation sources owned
/// by them, and assignments touching any of the collected entities.
fn control_closure(
    project: &Project,
    owners: &BTreeSet<ControlOwner>,
    out: &mut BTreeSet<SemanticObjectRef>,
) {
    let mut target_ids: BTreeSet<ParameterTargetId> = BTreeSet::new();
    let mut source_ids: BTreeSet<ModulationSourceId> = BTreeSet::new();
    for target in project.control.targets.values() {
        if owners.contains(&target.owner) {
            target_ids.insert(target.id);
            out.insert(object(
                project.project_id,
                ObjectKind::ParameterTarget,
                target.id.to_tagged(),
            ));
        }
    }
    for lane in project.control.lanes.values() {
        if target_ids.contains(&lane.target) {
            out.insert(object(
                project.project_id,
                ObjectKind::AutomationLane,
                lane.id.to_tagged(),
            ));
        }
    }
    for source in project.control.sources.values() {
        if owners.contains(&source.owner) {
            source_ids.insert(source.id);
            out.insert(object(
                project.project_id,
                ObjectKind::ModulationSource,
                source.id.to_tagged(),
            ));
        }
    }
    for assignment in project.control.assignments.values() {
        if target_ids.contains(&assignment.target)
            || assignment
                .depth_source
                .is_some_and(|d| target_ids.contains(&d))
            || source_ids.contains(&assignment.source)
        {
            out.insert(object(
                project.project_id,
                ObjectKind::ModulationAssignment,
                assignment.id.to_tagged(),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// scope objects
// ---------------------------------------------------------------------------

/// The owned object set of `scope` inside `project`, in deterministic
/// `(kind, stable_id)` order.
///
/// # Errors
/// [`Error::NotFound`]/[`Error::Validation`] when the scope's root
/// references do not resolve or a [`VariantScope::MidiClip`] source is
/// shared with another clip (ambiguous ownership — rejected).
pub(crate) fn scope_objects(
    project: &Project,
    scope: &VariantScope,
) -> Result<BTreeSet<SemanticObjectRef>, Error> {
    let mut out = BTreeSet::new();
    match scope {
        VariantScope::AudioClip { clip } => audio_clip_objects(project, *clip, &mut out)?,
        VariantScope::TakeUsage { target } => take_usage_objects(project, target, &mut out)?,
        VariantScope::MidiClip { clip } => midi_clip_objects(project, *clip, &mut out)?,
        VariantScope::InstrumentSlot { processor } => {
            if !project.routing.processors.contains_key(processor) {
                return Err(Error::NotFound(format!("processor {processor}")));
            }
            processor_objects(project, *processor, &mut out);
        }
        VariantScope::ProcessingRange { range } => {
            processing_range_objects(project, range, &mut out)?;
        }
        VariantScope::StripFields { strip, fields } => {
            strip_fields_objects(project, *strip, fields, &mut out)?;
        }
        VariantScope::Arrangement {
            tracks,
            time,
            fields,
        } => {
            arrangement_objects(project, tracks, time, *fields, &mut out)?;
        }
    }
    Ok(out)
}

/// One audio clip plus its clip-local notes and clip-owned control
/// closure.
fn audio_clip_objects(
    project: &Project,
    clip: ClipId,
    out: &mut BTreeSet<SemanticObjectRef>,
) -> Result<(), Error> {
    let pid = project.project_id;
    if !project.clips.contains_key(&clip) {
        return Err(Error::NotFound(format!("clip {clip}")));
    }
    out.insert(object(pid, ObjectKind::Clip, clip.to_tagged()));
    for note in project.notes.values().filter(|n| n.clip == clip) {
        out.insert(object(pid, ObjectKind::Note, note.id.to_tagged()));
    }
    let mut owners = BTreeSet::new();
    owners.insert(ControlOwner::Clip(clip));
    control_closure(project, &owners, out);
    Ok(())
}

/// One take group plus its comps, regions and transitions. Takes and
/// sources stay shared raw material — never scoped.
fn take_usage_objects(
    project: &Project,
    target: &TakeUsageTarget,
    out: &mut BTreeSet<SemanticObjectRef>,
) -> Result<(), Error> {
    let pid = project.project_id;
    let group = project
        .comp
        .take_groups
        .get(&target.take_group)
        .ok_or_else(|| Error::NotFound(format!("take group {}", target.take_group)))?;
    out.insert(object(pid, ObjectKind::TakeGroup, group.id.to_tagged()));
    for comp in project
        .comp
        .comps
        .values()
        .filter(|c| c.take_group_id == target.take_group)
    {
        out.insert(object(pid, ObjectKind::Comp, comp.id.to_tagged()));
        for region in project
            .comp
            .regions
            .values()
            .filter(|r| r.comp_id == comp.id)
        {
            out.insert(object(pid, ObjectKind::CompRegion, region.id.to_tagged()));
        }
        for transition in project
            .comp
            .transitions
            .values()
            .filter(|t| t.comp_id == comp.id)
        {
            out.insert(object(
                pid,
                ObjectKind::CompTransition,
                transition.id.to_tagged(),
            ));
        }
    }
    Ok(())
}

/// One MIDI clip plus its exclusively-owned `MidiSource` note content
/// and clip-local notes. A source shared with another clip makes
/// ownership ambiguous — rejected, never guessed.
fn midi_clip_objects(
    project: &Project,
    clip: ClipId,
    out: &mut BTreeSet<SemanticObjectRef>,
) -> Result<(), Error> {
    let pid = project.project_id;
    let clip_entity = project
        .clips
        .get(&clip)
        .ok_or_else(|| Error::NotFound(format!("clip {clip}")))?;
    out.insert(object(pid, ObjectKind::Clip, clip.to_tagged()));
    if let Some(source) = project.midi_sources.get(&clip_entity.source_id) {
        let shared = project
            .clips
            .values()
            .any(|c| c.id != clip && c.source_id == clip_entity.source_id);
        if shared {
            return Err(Error::Validation(format!(
                "midi source {} is shared with another clip",
                clip_entity.source_id
            )));
        }
        out.insert(object(
            pid,
            ObjectKind::MidiSource,
            clip_entity.source_id.to_tagged(),
        ));
        for note in source.notes.values() {
            out.insert(object(pid, ObjectKind::MidiNote, note.id.to_tagged()));
        }
    }
    for note in project.notes.values().filter(|n| n.clip == clip) {
        out.insert(object(pid, ObjectKind::Note, note.id.to_tagged()));
    }
    let mut owners = BTreeSet::new();
    owners.insert(ControlOwner::Clip(clip));
    control_closure(project, &owners, out);
    Ok(())
}

/// A contiguous processor range: every processor in `start..=end`,
/// their ports and control closures, plus connections wholly inside
/// the range (boundary connections stay shared).
fn processing_range_objects(
    project: &Project,
    range: &ProcessingRangeRef,
    out: &mut BTreeSet<SemanticObjectRef>,
) -> Result<(), Error> {
    let pid = project.project_id;
    for processor in range_processors(project, range)? {
        processor_objects(project, processor, out);
    }
    let owned_ports: BTreeSet<SemanticPortId> = out
        .iter()
        .filter(|o| o.kind == ObjectKind::SemanticPort)
        .filter_map(|o| SemanticPortId::from_tagged(&o.stable_id).ok())
        .collect();
    for conn in project.routing.connections.values() {
        if owned_ports.contains(&conn.source) && owned_ports.contains(&conn.destination) {
            out.insert(object(
                pid,
                ObjectKind::RoutingConnection,
                conn.id.to_tagged(),
            ));
        }
    }
    Ok(())
}

/// Selected field families of one channel strip: the strip entity,
/// optionally its whole insert chain, and the channel-field control
/// closure filtered by the declared mask.
fn strip_fields_objects(
    project: &Project,
    strip: StripId,
    fields: &StripDomainMask,
    out: &mut BTreeSet<SemanticObjectRef>,
) -> Result<(), Error> {
    let pid = project.project_id;
    let strip_entity = project
        .routing
        .strips
        .get(&strip)
        .ok_or_else(|| Error::NotFound(format!("strip {strip}")))?;
    out.insert(object(pid, ObjectKind::Strip, strip.to_tagged()));
    if fields.owns(StripField::Inserts) {
        let chain = project
            .routing
            .chains
            .get(&strip_entity.chain)
            .ok_or_else(|| Error::NotFound(format!("chain {}", strip_entity.chain)))?;
        for processor in &chain.processors {
            processor_objects(project, *processor, out);
        }
        // Connections wholly inside the insert chain are owned;
        // chain-boundary connections stay shared.
        let chain_ports: BTreeSet<SemanticPortId> = out
            .iter()
            .filter(|o| o.kind == ObjectKind::SemanticPort)
            .filter_map(|o| SemanticPortId::from_tagged(&o.stable_id).ok())
            .collect();
        for conn in project.routing.connections.values() {
            if chain_ports.contains(&conn.source) && chain_ports.contains(&conn.destination) {
                out.insert(object(
                    pid,
                    ObjectKind::RoutingConnection,
                    conn.id.to_tagged(),
                ));
            }
        }
    }
    // Channel-field control closure: the matching singleton roles on
    // the strip, filtered by the declared mask.
    let mut target_ids: BTreeSet<ParameterTargetId> = BTreeSet::new();
    for target in project.control.targets.values() {
        if target.owner != ControlOwner::Strip(strip) {
            continue;
        }
        let field = match target.role {
            ParameterRole::Fader => StripField::Fader,
            ParameterRole::Pan => StripField::Pan,
            ParameterRole::Mute => StripField::Mute,
            ParameterRole::Solo => StripField::Solo,
            _ => continue,
        };
        if fields.owns(field) {
            target_ids.insert(target.id);
            out.insert(object(
                pid,
                ObjectKind::ParameterTarget,
                target.id.to_tagged(),
            ));
        }
    }
    for lane in project.control.lanes.values() {
        if target_ids.contains(&lane.target) {
            out.insert(object(pid, ObjectKind::AutomationLane, lane.id.to_tagged()));
        }
    }
    let mut source_ids: BTreeSet<ModulationSourceId> = BTreeSet::new();
    for source in project.control.sources.values() {
        if source.owner == ControlOwner::Strip(strip) {
            source_ids.insert(source.id);
            out.insert(object(
                pid,
                ObjectKind::ModulationSource,
                source.id.to_tagged(),
            ));
        }
    }
    for assignment in project.control.assignments.values() {
        if target_ids.contains(&assignment.target)
            || assignment
                .depth_source
                .is_some_and(|d| target_ids.contains(&d))
            || source_ids.contains(&assignment.source)
        {
            out.insert(object(
                pid,
                ObjectKind::ModulationAssignment,
                assignment.id.to_tagged(),
            ));
        }
    }
    Ok(())
}

/// A musical-time selection across tracks: the overlapping clips (when
/// `clips` is selected), their clip-local notes (when `notes` is
/// selected — implying clip ownership), and each clip's control
/// closure.
fn arrangement_objects(
    project: &Project,
    tracks: &[TrackId],
    time: &MusicalRange,
    fields: ArrangementDomainMask,
    out: &mut BTreeSet<SemanticObjectRef>,
) -> Result<(), Error> {
    let pid = project.project_id;
    for clip_id in arrangement_clips(project, tracks, time)? {
        if fields.clips || fields.notes {
            out.insert(object(pid, ObjectKind::Clip, clip_id.to_tagged()));
        }
        if fields.notes {
            for note in project.notes.values().filter(|n| n.clip == clip_id) {
                out.insert(object(pid, ObjectKind::Note, note.id.to_tagged()));
            }
        }
        let mut owners = BTreeSet::new();
        owners.insert(ControlOwner::Clip(clip_id));
        control_closure(project, &owners, out);
    }
    Ok(())
}

/// Entities owned by one processor: itself, every port it owns, and
/// its processor-owned control closure.
fn processor_objects(
    project: &Project,
    processor: ProcessorId,
    out: &mut BTreeSet<SemanticObjectRef>,
) {
    let pid = project.project_id;
    out.insert(object(pid, ObjectKind::Processor, processor.to_tagged()));
    for port in project.routing.ports.values() {
        if port.owner == PortOwner::Processor(processor) {
            out.insert(object(pid, ObjectKind::SemanticPort, port.id.to_tagged()));
        }
    }
    let mut owners = BTreeSet::new();
    owners.insert(ControlOwner::Processor(processor));
    control_closure(project, &owners, out);
}

/// Processors of `range` in chain order (inclusive `start..=end`).
fn range_processors(
    project: &Project,
    range: &ProcessingRangeRef,
) -> Result<Vec<ProcessorId>, Error> {
    let chain = project
        .routing
        .chains
        .get(&range.chain)
        .ok_or_else(|| Error::NotFound(format!("chain {}", range.chain)))?;
    let start = chain
        .processors
        .iter()
        .position(|p| *p == range.start)
        .ok_or_else(|| Error::NotFound(format!("processor {}", range.start)))?;
    let end = chain
        .processors
        .iter()
        .position(|p| *p == range.end)
        .ok_or_else(|| Error::NotFound(format!("processor {}", range.end)))?;
    if start > end {
        return Err(Error::Validation(format!(
            "processing range end {} precedes start {}",
            range.end, range.start
        )));
    }
    Ok(chain.processors[start..=end].to_vec())
}

/// Clips on `tracks` whose musical placement overlaps `time`.
///
/// Only `Musical` anchor + `Musical` extent clips participate — an
/// absolute-anchored clip has no stable musical span to compare
/// against the scope's musical range and is excluded (documented CB-01
/// limitation; real adapters refine this per domain).
fn arrangement_clips(
    project: &Project,
    tracks: &[TrackId],
    time: &MusicalRange,
) -> Result<Vec<ClipId>, Error> {
    let mut out = Vec::new();
    for clip in project.clips.values() {
        let Some(lane) = project.lanes.get(&clip.lane_id) else {
            continue;
        };
        let Some(track) = project.tracks.get(&lane.track_id) else {
            continue;
        };
        if !tracks.contains(&track.id) {
            continue;
        }
        let Some(start) = musical_anchor(&clip.placement.anchor) else {
            continue;
        };
        let Some(end) = musical_extent_end(&clip.placement)? else {
            continue;
        };
        if start.value() < time.end().value() && time.start().value() < end.value() {
            out.push(clip.id);
        }
    }
    out.sort();
    Ok(out)
}

fn musical_anchor(anchor: &TimeAnchor) -> Option<MusicalTick> {
    match anchor {
        TimeAnchor::Musical(t) => Some(*t),
        TimeAnchor::Absolute(_) => None,
    }
}

fn musical_extent_end(placement: &ClipPlacement) -> Result<Option<MusicalTick>, Error> {
    let (Some(start), ClipExtent::Musical(extent)) =
        (musical_anchor(&placement.anchor), placement.extent)
    else {
        return Ok(None);
    };
    Ok(Some(start.checked_add(extent.value())?))
}

// ---------------------------------------------------------------------------
// fingerprint / value extraction
// ---------------------------------------------------------------------------

/// Canonical JSON view of one scope-owned object — the apply-surface
/// extractor plus the comping kinds the apply surface does not yet
/// transplant. `Ok(None)` = supported kind, absent entity.
pub(crate) fn scope_value(
    project: &Project,
    object: &SemanticObjectRef,
) -> Result<Option<Value>, Error> {
    match object.kind {
        ObjectKind::TakeGroup => {
            let id = TakeGroupId::from_tagged(&object.stable_id)?;
            Ok(project
                .comp
                .take_groups
                .get(&id)
                .map(serde_json::to_value)
                .transpose()?)
        }
        ObjectKind::Comp => {
            let id = CompId::from_tagged(&object.stable_id)?;
            Ok(project
                .comp
                .comps
                .get(&id)
                .map(serde_json::to_value)
                .transpose()?)
        }
        ObjectKind::CompRegion => {
            let id = CompRegionId::from_tagged(&object.stable_id)?;
            Ok(project
                .comp
                .regions
                .get(&id)
                .map(serde_json::to_value)
                .transpose()?)
        }
        ObjectKind::CompTransition => {
            let id = CompTransitionId::from_tagged(&object.stable_id)?;
            Ok(project
                .comp
                .transitions
                .get(&id)
                .map(serde_json::to_value)
                .transpose()?)
        }
        _ => entity::entity_value(project, object).map_err(|e| Error::Validation(e.to_string())),
    }
}

/// Scope-content fingerprint — the canonical digest over the scope's
/// owned object set. Identical scoped content yields identical digests
/// across branches, candidates and the live project: this is the
/// render-reuse key and the adopt target hash.
///
/// # Errors
/// [`Error::Validation`] when an owned object cannot be extracted.
pub(crate) fn scope_fingerprint(
    project: &Project,
    scope: &VariantScope,
) -> Result<ContentDigest, Error> {
    let objects = scope_objects(project, scope)?;
    let mut parts = Vec::with_capacity(objects.len());
    for object in &objects {
        let value = scope_value(project, object)?;
        let kind =
            serde_json::to_string(&object.kind).map_err(|e| Error::Validation(e.to_string()))?;
        let text = match &value {
            Some(v) => serde_json::to_string(v).map_err(|e| Error::Validation(e.to_string()))?,
            None => "null".to_owned(),
        };
        parts.push(format!("{kind}|{}|{text}", object.stable_id));
    }
    Ok(ContentDigest::sha256(parts.join("\n").as_bytes()))
}

/// Live scope digest for intents — see [`scope_fingerprint`].
///
/// # Errors
/// Propagates [`scope_fingerprint`] errors.
pub fn scope_digest(project: &Project, scope: &VariantScope) -> Result<ContentDigest, Error> {
    scope_fingerprint(project, scope)
}

/// `true` when two scopes own at least one common object inside
/// `project` — the persisted/at-fork overlap oracle. Disjoint scopes
/// coexist; any shared object makes the scopes conflicting.
///
/// # Errors
/// Propagates [`scope_objects`] errors (invalid scope references).
pub(crate) fn scopes_conflict(
    project: &Project,
    a: &VariantScope,
    b: &VariantScope,
) -> Result<bool, Error> {
    let oa = scope_objects(project, a)?;
    let ob = scope_objects(project, b)?;
    Ok(!oa.is_disjoint(&ob))
}

/// Structural validation of a scope against `project`: root references
/// resolve, ranges are ordered, selections are non-empty.
///
/// # Errors
/// [`Error::NotFound`]/[`Error::Validation`].
pub(crate) fn validate_scope_structure(
    project: &Project,
    scope: &VariantScope,
) -> Result<(), Error> {
    match scope {
        VariantScope::AudioClip { clip } | VariantScope::MidiClip { clip } => {
            if !project.clips.contains_key(clip) {
                return Err(Error::NotFound(format!("clip {clip}")));
            }
        }
        VariantScope::TakeUsage { target } => {
            if !project.comp.take_groups.contains_key(&target.take_group) {
                return Err(Error::NotFound(format!("take group {}", target.take_group)));
            }
        }
        VariantScope::InstrumentSlot { processor } => {
            if !project.routing.processors.contains_key(processor) {
                return Err(Error::NotFound(format!("processor {processor}")));
            }
        }
        VariantScope::ProcessingRange { range } => {
            range_processors(project, range)?;
        }
        VariantScope::StripFields { strip, fields } => {
            if !project.routing.strips.contains_key(strip) {
                return Err(Error::NotFound(format!("strip {strip}")));
            }
            if !fields.any() {
                return Err(Error::Validation("empty strip domain mask".into()));
            }
        }
        VariantScope::Arrangement {
            tracks,
            time,
            fields,
        } => {
            if tracks.is_empty() {
                return Err(Error::Validation("arrangement scope has no tracks".into()));
            }
            let mut dedup = BTreeSet::new();
            for track in tracks {
                if !dedup.insert(*track) {
                    return Err(Error::Validation(format!("duplicate track {track}")));
                }
                if !project.tracks.contains_key(track) {
                    return Err(Error::NotFound(format!("track {track}")));
                }
            }
            if time.is_empty() {
                return Err(Error::Validation(
                    "arrangement scope has empty time range".into(),
                ));
            }
            if !fields.any() {
                return Err(Error::Validation("empty arrangement domain mask".into()));
            }
        }
    }
    Ok(())
}
