//! `ProjectOp`: project-level domain operations implementing [`Operation`].
//!
//! Ops that create entities draw their IDs from the injected generator
//! inside `apply` (validation stays pure). Ops returning no payload are
//! validated only; the created entity's ID is not reported back — callers
//! that need it read it from the project after apply (F0 simplification).

use crate::{
    control::{
        AutomationLane, AutomationRegion, BeatRange, ConflictPolicy, ControlScope, ParamMapping,
        Polarity, SourceKind, TargetCapability, Transform, ValueKind, cut_range_points,
        dispatch_check, modulation_allowed,
    },
    controller::Operation,
    error::Error,
    id::{
        AssetId, AutomationLaneId, ClipId, IdGenerator, LaneId, ModulationAssignmentId,
        ModulationSourceId, NoteId, ParameterTargetId, SourceId, TrackId,
    },
    placement::{ClipExtent, ClipPlacement, ClipPlayback, TimeAnchor, validate_clip_contract},
    project::{Asset, Clip, Lane, Note, Project, Source, SourceState, Track},
    range::{MusicalRange, SourceRange},
    recording_data::ClockEpoch,
    tempo::SignatureDto,
    tempo::TempoEventDto,
    time::{MusicalDuration, MusicalTick, SourceFrame},
};

/// Project-level operations.
#[derive(Clone, Debug)]
pub enum ProjectOp {
    /// Rename the project.
    SetProjectName {
        /// New display name.
        name: String,
    },
    /// Set the project PPQN. F0 only accepts `960` (configurable PPQN is
    /// deferred to issue #19 / ruviform #189).
    SetPpqn {
        /// New PPQN value.
        ppqn: u32,
    },
    /// Add a tempo event (rejects a duplicate tick), bumping the tempo
    /// map revision.
    AddTempoEvent {
        /// Tick at which the tempo takes effect.
        tick: i64,
        /// Milli-BPM fixed point.
        bpm_milli: u32,
    },
    /// Add a time-signature event (rejects a duplicate tick), bumping
    /// the tempo map revision.
    AddSignatureEvent {
        /// Tick at which the signature takes effect.
        tick: i64,
        /// Beats per bar.
        numerator: u8,
        /// Beat unit denominator (power of two).
        denominator: u8,
    },
    /// Create a track.
    AddTrack {
        /// Display name.
        name: String,
    },
    /// Remove a track (only when it has no lanes).
    RemoveTrack {
        /// Target track.
        track_id: TrackId,
    },
    /// Add a lane to a track.
    AddLane {
        /// Owning track.
        track_id: TrackId,
    },
    /// Remove a lane (only when empty and unreferenced).
    RemoveLane {
        /// Target lane.
        lane_id: LaneId,
    },
    /// Place a clip on a lane.
    AddClip {
        /// Lane to hold the clip.
        lane_id: LaneId,
        /// Media source the clip reads.
        source_id: SourceId,
        /// Timeline placement (anchor + extent, the saved authority).
        placement: ClipPlacement,
        /// Source read window and project→source mapping.
        playback: ClipPlayback,
    },
    /// Remove a clip. Notes anchored to the clip are removed with it —
    /// a note cannot outlive its coordinate frame.
    RemoveClip {
        /// Target clip.
        clip_id: ClipId,
    },
    /// Move a clip: change its placement anchor (extent and playback
    /// are unchanged — a move never rewrites the source window).
    MoveClip {
        /// Target clip.
        clip_id: ClipId,
        /// New anchor point.
        anchor: TimeAnchor,
    },
    /// Trim a clip: change its extent. The new length must still fit
    /// the playback window under its mapping.
    SetClipExtent {
        /// Target clip.
        clip_id: ClipId,
        /// New extent.
        extent: ClipExtent,
    },
    /// Change a clip's playback window/mapping (slip, stretch, loop).
    SetClipPlayback {
        /// Target clip.
        clip_id: ClipId,
        /// New playback contract.
        playback: ClipPlayback,
    },
    /// Set a clip's muted flag — muted clips lower to silence.
    SetClipMuted {
        /// Target clip.
        clip_id: ClipId,
        /// New muted state.
        muted: bool,
    },
    /// Add a clip-local note (anchor for `ControlScope::Note`).
    AddNote {
        /// Owning clip.
        clip: ClipId,
        /// Musical key identity (0–127).
        key: u8,
        /// Clip-relative start beat.
        start: crate::control::BeatPoint,
        /// Note length in beats (> 0).
        len: crate::control::BeatPoint,
    },
    /// Remove a note. The post-apply reference oracle rejects the
    /// commit while a note-scoped target or modulation source still
    /// references the note — remove those first.
    RemoveNote {
        /// Target note.
        note: NoteId,
    },
    /// Register an asset.
    RegisterAsset {
        /// Content digest.
        digest: String,
    },
    /// Register a media source (state `Imported`).
    RegisterSource {
        /// Backing asset.
        asset_id: AssetId,
        /// Content digest of the referenced media.
        content_digest: String,
    },
    /// Advance a source's verification state (forward only:
    /// `Imported → Verified → Committed → Saved`).
    SetSourceState {
        /// Target source.
        source_id: SourceId,
        /// New state (must strictly follow the current one).
        state: SourceState,
    },
    /// Register a clock epoch.
    RegisterClockEpoch {
        /// Description of the clock source.
        description: String,
    },
    /// Apply several ops as one atomic transaction — the multi-target
    /// editing-gesture commit (issue #11/#38: all-or-nothing, one undo
    /// step). Nested `Composite` is rejected.
    Composite {
        /// Ops applied in order on the same scratch project.
        ops: Vec<ProjectOp>,
    },
    /// Split a musically anchored clip at `at` into two clips. The new
    /// clip keeps the source and muted flag; `clip_b_source_offset` is
    /// caller-resolved (tick→source-frame rate mapping is not modeled).
    SplitClip {
        /// Target clip.
        clip_id: ClipId,
        /// Split position; must lie strictly inside the clip's
        /// `MusicalRange`.
        at: MusicalTick,
        /// Source offset of the clip after the split (must not precede
        /// the original offset).
        clip_b_source_offset: SourceFrame,
    },
    /// Remove every clip on `lanes` whose `MusicalRange` overlaps
    /// `range`. Media-anchored clips are outside this op's time domain
    /// and are left untouched (no silent domain conversion).
    RemoveClipsInRange {
        /// Lanes to sweep.
        lanes: Vec<LaneId>,
        /// Musical half-open range to remove.
        range: MusicalRange,
    },
    /// Create a parameter target (issue #40). `Note` scope resolves to
    /// a note-relative coordinate domain (issue #43).
    AddParameterTarget {
        /// Display name.
        name: String,
        /// Owning scope (defines the time coordinate of its automation).
        owner: ControlScope,
        /// Discrete/continuous value domain.
        kind: ValueKind,
        /// Normalized→plain mapping.
        mapping: ParamMapping,
        /// Optional plugin capability contract (fixture `ParamInfo`).
        capability: Option<TargetCapability>,
        /// Normalized base value in `[0, 1]`.
        base: f64,
    },
    /// Remove a parameter target (only when no lane or assignment
    /// references it and it is not a depth source).
    RemoveParameterTarget {
        /// Target to remove.
        target: ParameterTargetId,
    },
    /// Set a target's persistent base value — the `Off`-mode edit and
    /// the fallback when no automation/manual value applies. Rejected
    /// on `READONLY` targets.
    SetParameterBase {
        /// Target to edit.
        target: ParameterTargetId,
        /// New normalized base in `[0, 1]`.
        value: f64,
    },
    /// Create an empty automation lane feeding a target.
    AddAutomationLane {
        /// The parameter the lane feeds.
        target: ParameterTargetId,
    },
    /// Remove an automation lane and all its regions.
    RemoveAutomationLane {
        /// Lane to remove.
        lane: AutomationLaneId,
    },
    /// Write a region into a lane. `policy` is the caller's explicit
    /// conflict decision: `Reject` reports competing series as
    /// [`Error::AutomationConflict`]; `Replace` overwrites the merged
    /// series (existing points inside the new range are removed on
    /// every lane of the target). Requires the target to accept
    /// [`ruviform_plugin_core::ParamWriteMeans::TimedEvent`].
    WriteAutomationRegion {
        /// Lane receiving the region.
        lane: AutomationLaneId,
        /// The region to insert.
        region: AutomationRegion,
        /// Explicit conflict policy.
        policy: ConflictPolicy,
    },
    /// Remove every point inside `range` on `lane` (regions are split
    /// at the boundary; empty remains are dropped).
    EraseAutomationRange {
        /// Lane to erase from.
        lane: AutomationLaneId,
        /// Half-open range to clear.
        range: BeatRange,
    },
    /// Create a modulation source.
    AddModulationSource {
        /// Owning scope.
        owner: ControlScope,
        /// Signal definition.
        kind: SourceKind,
    },
    /// Remove a modulation source (only when no assignment uses it).
    RemoveModulationSource {
        /// Source to remove.
        source: ModulationSourceId,
    },
    /// Bind a source to a target. Only `Continuous`, writable targets
    /// accept modulation — a discrete kind or `READONLY` capability is
    /// rejected here, never silently ignored at evaluation.
    AddModulationAssignment {
        /// Signal provider.
        source: ModulationSourceId,
        /// Modulated parameter.
        target: ParameterTargetId,
        /// Normalized increment scale (finite; sign allowed).
        depth: f64,
        /// Optional target whose resolved value scales `depth`.
        depth_source: Option<ParameterTargetId>,
        /// Bipolar/unipolar application.
        polarity: Polarity,
        /// Signal shaping.
        transform: Transform,
        /// Whether the assignment is live.
        enabled: bool,
    },
    /// Update an assignment's depth and/or enabled flag.
    SetModulationAssignment {
        /// Assignment to update.
        id: ModulationAssignmentId,
        /// New depth (unchanged when `None`).
        depth: Option<f64>,
        /// New enabled flag (unchanged when `None`).
        enabled: Option<bool>,
    },
    /// Remove a modulation assignment.
    RemoveModulationAssignment {
        /// Assignment to remove.
        id: ModulationAssignmentId,
    },
}

impl Operation for ProjectOp {
    #[allow(clippy::too_many_lines)] // flat match per op variant
    fn validate(&self, project: &Project) -> Result<(), Error> {
        match self {
            Self::SetProjectName { .. }
            | Self::RegisterAsset { .. }
            | Self::AddTrack { .. }
            | Self::RegisterClockEpoch { .. } => Ok(()),
            Self::SetPpqn { ppqn } => {
                if *ppqn == 960 {
                    Ok(())
                } else {
                    Err(Error::Validation(format!(
                        "F0 supports ppqn 960 only (got {ppqn})"
                    )))
                }
            }
            Self::AddTempoEvent { tick, bpm_milli } => {
                if project
                    .tempo_map
                    .tempo_events
                    .iter()
                    .any(|e| e.tick == *tick)
                {
                    return Err(Error::Validation(format!("duplicate tempo tick {tick}")));
                }
                if !(1..=999_999_999).contains(bpm_milli) {
                    return Err(Error::Validation(format!(
                        "bpm_milli {bpm_milli} out of range"
                    )));
                }
                Ok(())
            }
            Self::AddSignatureEvent {
                tick,
                numerator,
                denominator,
            } => {
                if project
                    .tempo_map
                    .signature_events
                    .iter()
                    .any(|e| e.tick == *tick)
                {
                    return Err(Error::Validation(format!(
                        "duplicate signature tick {tick}"
                    )));
                }
                if *numerator == 0 {
                    return Err(Error::Validation("signature numerator is zero".into()));
                }
                if !denominator.is_power_of_two() {
                    return Err(Error::Validation(
                        "signature denominator is not a power of two".into(),
                    ));
                }
                Ok(())
            }
            Self::RemoveTrack { track_id } => {
                let track = project
                    .tracks
                    .get(track_id)
                    .ok_or_else(|| Error::NotFound(format!("track {track_id}")))?;
                if !track.lane_ids.is_empty() {
                    return Err(Error::Validation(format!(
                        "track {track_id} still has lanes"
                    )));
                }
                Ok(())
            }
            Self::AddLane { track_id } => project
                .tracks
                .contains_key(track_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("track {track_id}"))),
            Self::RemoveLane { lane_id } => {
                let lane = project
                    .lanes
                    .get(lane_id)
                    .ok_or_else(|| Error::NotFound(format!("lane {lane_id}")))?;
                if !lane.clip_ids.is_empty() {
                    return Err(Error::Validation(format!("lane {lane_id} has clips")));
                }
                if project
                    .recording
                    .streams
                    .values()
                    .any(|s| s.lane_id == *lane_id)
                {
                    return Err(Error::Validation(format!(
                        "lane {lane_id} referenced by capture stream"
                    )));
                }
                Ok(())
            }
            Self::AddClip {
                lane_id,
                source_id,
                placement,
                playback,
            } => {
                if !project.lanes.contains_key(lane_id) {
                    return Err(Error::NotFound(format!("lane {lane_id}")));
                }
                if !project.sources.contains_key(source_id) {
                    return Err(Error::NotFound(format!("source {source_id}")));
                }
                validate_clip_contract(project, placement, playback)
            }
            Self::MoveClip {
                clip_id, anchor, ..
            } => {
                let clip = project
                    .clips
                    .get(clip_id)
                    .ok_or_else(|| Error::NotFound(format!("clip {clip_id}")))?;
                validate_clip_contract(
                    project,
                    &ClipPlacement {
                        anchor: *anchor,
                        extent: clip.placement.extent,
                    },
                    &clip.playback,
                )
            }
            Self::SetClipExtent {
                clip_id, extent, ..
            } => {
                let clip = project
                    .clips
                    .get(clip_id)
                    .ok_or_else(|| Error::NotFound(format!("clip {clip_id}")))?;
                validate_clip_contract(
                    project,
                    &ClipPlacement {
                        anchor: clip.placement.anchor,
                        extent: *extent,
                    },
                    &clip.playback,
                )
            }
            Self::SetClipPlayback {
                clip_id, playback, ..
            } => {
                let clip = project
                    .clips
                    .get(clip_id)
                    .ok_or_else(|| Error::NotFound(format!("clip {clip_id}")))?;
                validate_clip_contract(project, &clip.placement, playback)
            }
            Self::RemoveClip { clip_id } | Self::SetClipMuted { clip_id, .. } => project
                .clips
                .contains_key(clip_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("clip {clip_id}"))),
            Self::AddNote {
                clip,
                key,
                start,
                len,
            } => {
                let c = project
                    .clips
                    .get(clip)
                    .ok_or_else(|| Error::NotFound(format!("clip {clip}")))?;
                if *key > 127 {
                    return Err(Error::Validation(format!("note key {key} > 127")));
                }
                if len.num() <= 0 || start.num() < 0 {
                    return Err(Error::Validation(
                        "note needs len > 0 and start >= 0".into(),
                    ));
                }
                let ClipExtent::Musical(dur) = c.placement.extent else {
                    return Err(Error::Validation(format!(
                        "note {clip} clip is not musically anchored"
                    )));
                };
                let extent = clip_beats(project, dur.value())?;
                if start.checked_add(*len)? > extent {
                    return Err(Error::Validation(format!(
                        "note exceeds clip {clip} extent"
                    )));
                }
                Ok(())
            }
            Self::RemoveNote { note } => project
                .notes
                .contains_key(note)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("note {note}"))),
            Self::RegisterSource { asset_id, .. } => project
                .assets
                .contains_key(asset_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("asset {asset_id}"))),
            Self::SetSourceState { source_id, state } => {
                let source = project
                    .sources
                    .get(source_id)
                    .ok_or_else(|| Error::NotFound(format!("source {source_id}")))?;
                if source.state.next() != Some(*state) {
                    return Err(Error::IllegalTransition(format!(
                        "source state {:?} -> {state:?} (strictly one stage forward)",
                        source.state
                    )));
                }
                Ok(())
            }
            Self::Composite { ops } => {
                if ops.is_empty() {
                    return Err(Error::Validation("empty composite".into()));
                }
                for op in ops {
                    if matches!(op, Self::Composite { .. }) {
                        return Err(Error::Validation("nested composite".into()));
                    }
                    op.validate(project)?;
                }
                Ok(())
            }
            Self::SplitClip {
                clip_id,
                at,
                clip_b_source_offset,
            } => {
                let clip = project
                    .clips
                    .get(clip_id)
                    .ok_or_else(|| Error::NotFound(format!("clip {clip_id}")))?;
                let (TimeAnchor::Musical(start), ClipExtent::Musical(dur)) =
                    (clip.placement.anchor, clip.placement.extent)
                else {
                    return Err(Error::Validation(format!(
                        "clip {clip_id} is not musically anchored"
                    )));
                };
                let end = start.checked_add(dur.value())?;
                if !(*at > start && *at < end) {
                    return Err(Error::Validation(format!(
                        "split tick {} is not strictly inside clip {clip_id}",
                        at.value()
                    )));
                }
                if *clip_b_source_offset < clip.playback.source_region.start() {
                    return Err(Error::Validation(format!(
                        "clip_b_source_offset precedes source offset of clip {clip_id}"
                    )));
                }
                // A note cut in half would lose its tail domain —
                // refuse rather than silently orphan it.
                let offset = clip_beats(project, at.ticks_since(start)?)?;
                for note in project.notes.values().filter(|n| n.clip == *clip_id) {
                    if note.start < offset && note.start.checked_add(note.len)? > offset {
                        return Err(Error::Validation(format!(
                            "note {} straddles split point of clip {clip_id}",
                            note.id
                        )));
                    }
                }
                Ok(())
            }
            Self::RemoveClipsInRange { lanes, range } => {
                if range.is_empty() {
                    return Err(Error::Validation("empty removal range".into()));
                }
                for lane_id in lanes {
                    if !project.lanes.contains_key(lane_id) {
                        return Err(Error::NotFound(format!("lane {lane_id}")));
                    }
                }
                Ok(())
            }
            // -- parameter control model (issue #40) -------------------
            Self::AddParameterTarget {
                owner,
                kind,
                mapping,
                capability,
                base,
                ..
            } => {
                validate_scope(*owner, project)?;
                kind.validate()?;
                if !base.is_finite() || !(0.0..=1.0).contains(base) {
                    return Err(Error::Validation("param base outside [0,1]".into()));
                }
                if let Some(cap) = capability {
                    cap.param_info("")?;
                    if matches!(mapping, ParamMapping::Exponential) && cap.min_value <= 0.0 {
                        return Err(Error::Validation(
                            "exponential mapping needs min_value > 0".into(),
                        ));
                    }
                }
                Ok(())
            }
            Self::RemoveParameterTarget { target } => {
                if !project.control.targets.contains_key(target) {
                    return Err(Error::NotFound(format!("param {target}")));
                }
                if project.control.lanes.values().any(|l| l.target == *target)
                    || project
                        .control
                        .assignments
                        .values()
                        .any(|a| a.target == *target || a.depth_source == Some(*target))
                {
                    return Err(Error::Validation(format!(
                        "param {target} still referenced"
                    )));
                }
                Ok(())
            }
            Self::SetParameterBase { target, value } => {
                let t = project
                    .control
                    .targets
                    .get(target)
                    .ok_or_else(|| Error::NotFound(format!("param {target}")))?;
                if !value.is_finite() || !(0.0..=1.0).contains(value) {
                    return Err(Error::Validation("param base outside [0,1]".into()));
                }
                if t.is_readonly() {
                    return Err(Error::WriteRejected {
                        means: "SetParameterBase".into(),
                        rejection: "param_readonly".into(),
                    });
                }
                Ok(())
            }
            Self::AddAutomationLane { target } => project
                .control
                .targets
                .contains_key(target)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("param {target}"))),
            Self::RemoveAutomationLane { lane } => project
                .control
                .lanes
                .contains_key(lane)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("automation lane {lane}"))),
            Self::WriteAutomationRegion {
                lane,
                region,
                policy,
            } => {
                let lane_ref = project
                    .control
                    .lanes
                    .get(lane)
                    .ok_or_else(|| Error::NotFound(format!("automation lane {lane}")))?;
                let target = project
                    .control
                    .targets
                    .get(&lane_ref.target)
                    .ok_or_else(|| Error::NotFound(format!("param {}", lane_ref.target)))?;
                region.validate()?;
                dispatch_check(target, ruviform_plugin_core::ParamWriteMeans::TimedEvent)?;
                if *policy == ConflictPolicy::Reject {
                    for other in project.control.lanes.values() {
                        if other.target != lane_ref.target {
                            continue;
                        }
                        for existing in &other.regions {
                            if existing.range.overlaps(&region.range) {
                                return Err(Error::AutomationConflict {
                                    target: lane_ref.target.to_string(),
                                    detail: format!(
                                        "lane {lane} region {:?} overlaps lane {} region {:?}",
                                        region.range, other.id, existing.range
                                    ),
                                });
                            }
                        }
                    }
                }
                Ok(())
            }
            Self::EraseAutomationRange { lane, range } => {
                if range.is_empty() {
                    return Err(Error::Validation("empty erase range".into()));
                }
                project
                    .control
                    .lanes
                    .contains_key(lane)
                    .then_some(())
                    .ok_or_else(|| Error::NotFound(format!("automation lane {lane}")))
            }
            Self::AddModulationSource { owner, kind } => {
                validate_scope(*owner, project)?;
                kind.validate()
            }
            Self::RemoveModulationSource { source } => {
                if !project.control.sources.contains_key(source) {
                    return Err(Error::NotFound(format!("mod source {source}")));
                }
                if project
                    .control
                    .assignments
                    .values()
                    .any(|a| a.source == *source)
                {
                    return Err(Error::Validation(format!(
                        "mod source {source} still assigned"
                    )));
                }
                Ok(())
            }
            Self::AddModulationAssignment {
                source,
                target,
                depth,
                depth_source,
                ..
            } => {
                if !project.control.sources.contains_key(source) {
                    return Err(Error::NotFound(format!("mod source {source}")));
                }
                let t = project
                    .control
                    .targets
                    .get(target)
                    .ok_or_else(|| Error::NotFound(format!("param {target}")))?;
                if !depth.is_finite() {
                    return Err(Error::Validation("modulation depth not finite".into()));
                }
                if !modulation_allowed(t.kind) {
                    return Err(Error::Validation(format!(
                        "param {target} kind rejects continuous modulation"
                    )));
                }
                // Modulation reaches the plugin through block-rate
                // writes — `READONLY` rejects, `AUTOMATABLE` is not
                // required (timed events are the sample-accurate path).
                dispatch_check(t, ruviform_plugin_core::ParamWriteMeans::BlockBoundary)?;
                if let Some(ds) = depth_source {
                    if *ds == *target {
                        return Err(Error::Validation(
                            "depth source cannot be the target itself".into(),
                        ));
                    }
                    if !project.control.targets.contains_key(ds) {
                        return Err(Error::NotFound(format!("depth source param {ds}")));
                    }
                }
                Ok(())
            }
            Self::SetModulationAssignment { id, depth, .. } => {
                if !project.control.assignments.contains_key(id) {
                    return Err(Error::NotFound(format!("mod assignment {id}")));
                }
                if let Some(d) = depth
                    && !d.is_finite()
                {
                    return Err(Error::Validation("modulation depth not finite".into()));
                }
                Ok(())
            }
            Self::RemoveModulationAssignment { id } => project
                .control
                .assignments
                .contains_key(id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("mod assignment {id}"))),
        }
    }

    #[allow(clippy::too_many_lines)] // flat match per op variant
    fn apply(&self, project: &mut Project, ids: &mut dyn IdGenerator) -> Result<(), Error> {
        match self {
            Self::SetProjectName { name } => project.name.clone_from(name),
            Self::SetPpqn { ppqn } => project.ppqn = crate::tempo::ProjectPpqn(*ppqn),
            Self::AddTempoEvent { tick, bpm_milli } => {
                project.tempo_map.tempo_events.push(TempoEventDto {
                    tick: *tick,
                    bpm_milli: *bpm_milli,
                });
                project.tempo_map.revision += 1;
            }
            Self::AddSignatureEvent {
                tick,
                numerator,
                denominator,
            } => {
                project.tempo_map.signature_events.push(SignatureDto {
                    tick: *tick,
                    numerator: *numerator,
                    denominator: *denominator,
                });
                project.tempo_map.revision += 1;
            }
            Self::AddTrack { name } => {
                let id = TrackId::new(ids.next());
                project.tracks.insert(
                    id,
                    Track {
                        id,
                        name: name.clone(),
                        lane_ids: Vec::new(),
                    },
                );
            }
            Self::RemoveTrack { track_id } => {
                project.tracks.remove(track_id);
            }
            Self::AddLane { track_id } => {
                let id = LaneId::new(ids.next());
                project.lanes.insert(
                    id,
                    Lane {
                        id,
                        track_id: *track_id,
                        clip_ids: Vec::new(),
                    },
                );
                if let Some(track) = project.tracks.get_mut(track_id) {
                    track.lane_ids.push(id);
                }
            }
            Self::RemoveLane { lane_id } => {
                if let Some(lane) = project.lanes.remove(lane_id)
                    && let Some(track) = project.tracks.get_mut(&lane.track_id)
                {
                    track.lane_ids.retain(|id| id != lane_id);
                }
            }
            Self::AddClip {
                lane_id,
                source_id,
                placement,
                playback,
            } => {
                let id = ClipId::new(ids.next());
                project.clips.insert(
                    id,
                    Clip {
                        id,
                        lane_id: *lane_id,
                        source_id: *source_id,
                        placement: *placement,
                        playback: *playback,
                        muted: false,
                    },
                );
                if let Some(lane) = project.lanes.get_mut(lane_id) {
                    lane.clip_ids.push(id);
                }
            }
            Self::RemoveClip { clip_id } => {
                if let Some(clip) = project.clips.remove(clip_id)
                    && let Some(lane) = project.lanes.get_mut(&clip.lane_id)
                {
                    lane.clip_ids.retain(|id| id != clip_id);
                }
                project.notes.retain(|_, note| note.clip != *clip_id);
            }
            Self::AddNote {
                clip,
                key,
                start,
                len,
            } => {
                let id = NoteId::new(ids.next());
                project.notes.insert(
                    id,
                    Note {
                        id,
                        clip: *clip,
                        key: *key,
                        start: *start,
                        len: *len,
                    },
                );
            }
            Self::RemoveNote { note } => {
                project.notes.remove(note);
            }
            Self::MoveClip { clip_id, anchor } => {
                if let Some(clip) = project.clips.get_mut(clip_id) {
                    clip.placement.anchor = *anchor;
                }
            }
            Self::SetClipExtent { clip_id, extent } => {
                if let Some(clip) = project.clips.get_mut(clip_id) {
                    clip.placement.extent = *extent;
                }
            }
            Self::SetClipPlayback { clip_id, playback } => {
                if let Some(clip) = project.clips.get_mut(clip_id) {
                    clip.playback = *playback;
                }
            }
            Self::SetClipMuted { clip_id, muted } => {
                if let Some(clip) = project.clips.get_mut(clip_id) {
                    clip.muted = *muted;
                }
            }
            Self::RegisterAsset { digest } => {
                let id = AssetId::new(ids.next());
                project.assets.insert(
                    id,
                    Asset {
                        id,
                        digest: digest.clone(),
                    },
                );
            }
            Self::RegisterSource {
                asset_id,
                content_digest,
            } => {
                let id = SourceId::new(ids.next());
                project.sources.insert(
                    id,
                    Source {
                        id,
                        asset_id: *asset_id,
                        content_digest: content_digest.clone(),
                        state: SourceState::Imported,
                    },
                );
            }
            Self::SetSourceState { source_id, state } => {
                if let Some(source) = project.sources.get_mut(source_id) {
                    source.state = *state;
                }
            }
            Self::RegisterClockEpoch { description } => {
                let id = crate::id::ClockEpochId::new(ids.next());
                project.clock_epochs.insert(
                    id,
                    ClockEpoch {
                        id,
                        description: description.clone(),
                    },
                );
            }
            Self::Composite { ops } => {
                // `project` is the controller's scratch clone — a mid-way
                // failure is discarded with it, keeping the commit atomic.
                for op in ops {
                    op.validate(project)?;
                    op.apply(project, ids)?;
                }
            }
            Self::SplitClip {
                clip_id,
                at,
                clip_b_source_offset,
            } => {
                let (lane_id, source_id, muted, start, end, playback) = {
                    let clip = project
                        .clips
                        .get(clip_id)
                        .ok_or_else(|| Error::NotFound(format!("clip {clip_id}")))?;
                    let (TimeAnchor::Musical(start), ClipExtent::Musical(dur)) =
                        (clip.placement.anchor, clip.placement.extent)
                    else {
                        return Err(Error::Validation(format!(
                            "clip {clip_id} is not musically anchored"
                        )));
                    };
                    (
                        clip.lane_id,
                        clip.source_id,
                        clip.muted,
                        start,
                        start.checked_add(dur.value())?,
                        clip.playback,
                    )
                };
                let mut b_playback = playback;
                b_playback.source_region =
                    SourceRange::try_new(*clip_b_source_offset, playback.source_region.end())?;
                let b_extent = MusicalDuration::new(end.ticks_since(*at)?)?;
                let id = ClipId::new(ids.next());
                project.clips.insert(
                    id,
                    Clip {
                        id,
                        lane_id,
                        source_id,
                        placement: ClipPlacement {
                            anchor: TimeAnchor::Musical(*at),
                            extent: ClipExtent::Musical(b_extent),
                        },
                        playback: b_playback,
                        muted,
                    },
                );
                if let Some(lane) = project.lanes.get_mut(&lane_id) {
                    lane.clip_ids.push(id);
                }
                if let Some(clip) = project.clips.get_mut(clip_id) {
                    clip.placement.extent =
                        ClipExtent::Musical(MusicalDuration::new(at.ticks_since(start)?)?);
                }
                // Notes fully past the split point follow clip B with
                // re-based starts; straddles were refused in validate.
                let offset = clip_beats(project, at.ticks_since(start)?)?;
                for note in project.notes.values_mut().filter(|n| n.clip == *clip_id) {
                    if note.start >= offset {
                        note.clip = id;
                        note.start = note.start.checked_sub(offset)?;
                    }
                }
            }
            Self::RemoveClipsInRange { lanes, range } => {
                let doomed: Vec<ClipId> = lanes
                    .iter()
                    .flat_map(|lane_id| {
                        project
                            .lanes
                            .get(lane_id)
                            .into_iter()
                            .flat_map(|lane| lane.clip_ids.iter().copied())
                    })
                    .filter(|clip_id| {
                        project.clips.get(clip_id).is_some_and(|c| {
                            matches!(
                                (c.placement.anchor, c.placement.extent),
                                (TimeAnchor::Musical(start), ClipExtent::Musical(dur))
                                    if MusicalRange::try_new(
                                        start,
                                        start
                                            .checked_add(dur.value())
                                            .unwrap_or(MusicalTick::new(i64::MAX)),
                                    )
                                    .is_ok_and(|r| r.overlaps(range))
                            )
                        })
                    })
                    .collect();
                for clip_id in doomed {
                    Self::RemoveClip { clip_id }.apply(project, ids)?;
                }
            }
            // -- parameter control model (issue #40) -------------------
            Self::AddParameterTarget {
                name,
                owner,
                kind,
                mapping,
                capability,
                base,
            } => {
                let id = ParameterTargetId::new(ids.next());
                project.control.targets.insert(
                    id,
                    crate::control::ParameterTarget {
                        id,
                        owner: *owner,
                        name: name.clone(),
                        kind: *kind,
                        mapping: *mapping,
                        capability: *capability,
                        base: *base,
                    },
                );
            }
            Self::RemoveParameterTarget { target } => {
                project.control.targets.remove(target);
            }
            Self::SetParameterBase { target, value } => {
                if let Some(t) = project.control.targets.get_mut(target) {
                    t.base = *value;
                }
            }
            Self::AddAutomationLane { target } => {
                let id = AutomationLaneId::new(ids.next());
                project.control.lanes.insert(
                    id,
                    AutomationLane {
                        id,
                        target: *target,
                        regions: Vec::new(),
                    },
                );
            }
            Self::RemoveAutomationLane { lane } => {
                project.control.lanes.remove(lane);
            }
            Self::WriteAutomationRegion {
                lane,
                region,
                policy,
            } => {
                if *policy == ConflictPolicy::Replace {
                    let target = project.control.lanes.get(lane).map(|l| l.target);
                    // Explicit overwrite: the new region wins the merged
                    // series — every lane of this target loses the
                    // overlapping span.
                    for other in project.control.lanes.values_mut() {
                        if Some(other.target) == target {
                            cut_range_points(&mut other.regions, &region.range);
                        }
                    }
                }
                if let Some(lane_ref) = project.control.lanes.get_mut(lane) {
                    lane_ref.regions.push(region.clone());
                }
            }
            Self::EraseAutomationRange { lane, range } => {
                if let Some(lane_ref) = project.control.lanes.get_mut(lane) {
                    cut_range_points(&mut lane_ref.regions, range);
                }
            }
            Self::AddModulationSource { owner, kind } => {
                let id = ModulationSourceId::new(ids.next());
                project.control.sources.insert(
                    id,
                    crate::control::ModulationSource {
                        id,
                        owner: *owner,
                        kind: kind.clone(),
                    },
                );
            }
            Self::RemoveModulationSource { source } => {
                project.control.sources.remove(source);
            }
            Self::AddModulationAssignment {
                source,
                target,
                depth,
                depth_source,
                polarity,
                transform,
                enabled,
            } => {
                let id = ModulationAssignmentId::new(ids.next());
                project.control.assignments.insert(
                    id,
                    crate::control::ModulationAssignment {
                        id,
                        source: *source,
                        target: *target,
                        depth: *depth,
                        depth_source: *depth_source,
                        polarity: *polarity,
                        transform: *transform,
                        enabled: *enabled,
                    },
                );
            }
            Self::SetModulationAssignment { id, depth, enabled } => {
                if let Some(a) = project.control.assignments.get_mut(id) {
                    if let Some(d) = depth {
                        a.depth = *d;
                    }
                    if let Some(e) = enabled {
                        a.enabled = *e;
                    }
                }
            }
            Self::RemoveModulationAssignment { id } => {
                project.control.assignments.remove(id);
            }
        }
        Ok(())
    }
}

/// Clip-relative beats for a musical tick offset — the shared
/// conversion for note-domain checks (`AddNote` extent, `SplitClip`
/// re-parenting).
fn clip_beats(project: &Project, ticks: i64) -> Result<crate::control::BeatPoint, Error> {
    let ppqn =
        ruviform_media_core::Ppqn::new(u16::try_from(project.ppqn.0).map_err(|_| Error::Overflow)?)
            .map_err(|e| Error::Validation(format!("ppqn: {e:?}")))?;
    Ok(crate::control::BeatPoint::from_beat(
        ruviform_media_core::TickPosition::new(ruviform_media_core::Tick::new(ticks), ppqn)
            .to_beat(),
    ))
}

/// Validates that a `ControlScope` resolves against existing entities.
fn validate_scope(scope: ControlScope, project: &Project) -> Result<(), Error> {
    match scope {
        ControlScope::Track(id) if !project.tracks.contains_key(&id) => {
            Err(Error::NotFound(format!("track {id}")))
        }
        ControlScope::Clip(id) if !project.clips.contains_key(&id) => {
            Err(Error::NotFound(format!("clip {id}")))
        }
        ControlScope::Note(id) if !project.notes.contains_key(&id) => {
            Err(Error::NotFound(format!("note {id}")))
        }
        _ => Ok(()),
    }
}
