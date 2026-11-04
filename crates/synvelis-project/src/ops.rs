//! `ProjectOp`: project-level domain operations implementing [`Operation`].
//!
//! Ops that create entities draw their IDs from the injected generator
//! inside `apply` (validation stays pure). Ops returning no payload are
//! validated only; the created entity's ID is not reported back — callers
//! that need it read it from the project after apply (F0 simplification).

use crate::{
    control::{
        AutomationLane, AutomationRegion, AutomationTimeScope, BeatRange, ConflictPolicy,
        ControlOwner, ParamMapping, ParameterRole, Polarity, SourceKind, TargetCapability,
        Transform, ValueKind, cut_range_points, dispatch_check, modulation_allowed,
        validate_control_binding, validate_control_owner,
    },
    controller::Operation,
    error::Error,
    id::{
        AssetId, AutomationLaneId, BusId, ChainId, ClipId, IdGenerator, LaneId,
        ModulationAssignmentId, ModulationSourceId, NoteId, ParameterTargetId, ProcessorId,
        RoutingConnectionId, SemanticPortId, SendId, SourceId, StripId, TrackId,
    },
    midi::{MidiDivision, MidiNote, MidiSource},
    placement::{
        ClipExtent, ClipPlacement, ClipPlayback, TimeAnchor, validate_clip_contract,
        validate_midi_clip_contract,
    },
    project::{Asset, Clip, Lane, Note, Project, Source, SourceState, Track},
    range::{MusicalRange, SourceRange},
    recording_data::ClockEpoch,
    routing::{
        Bus, BusKind, ChannelStrip, ConnectionKind, ExplicitChannelMap, PortBinding, PortDirection,
        PortOwner, PortRole, Processor, ProcessorBinding, ProcessorChain, RoutingConnection,
        SemanticPort, SendConfig, SignalDomain, StripOwner,
    },
    tempo::SignatureDto,
    tempo::TempoEventDto,
    time::{MidiTick, MusicalDuration, MusicalTick, SourceFrame},
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
        /// Source the clip reads — media or MIDI.
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
    /// Add a clip-local note (anchor for `ControlOwner::Note`).
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
    /// Create a MIDI source: a project-owned note sequence in
    /// source-native ticks sharing the `SourceId` space (issue #52).
    /// Clips reference it through `Clip::source_id` like a media
    /// source — linked clips then read the same note set.
    AddMidiSource {
        /// Native tick resolution.
        division: MidiDivision,
    },
    /// Remove a MIDI source — only while no clip references it.
    RemoveMidiSource {
        /// Target MIDI source.
        source_id: SourceId,
    },
    /// Add a note to a MIDI source. The note is source payload —
    /// every clip reading the source sees it (linked semantics); the
    /// op names the source directly, so editing never goes "through"
    /// one of the linked clips.
    AddMidiNote {
        /// Owning MIDI source.
        source_id: SourceId,
        /// Musical key identity (0–127).
        key: u8,
        /// Note-on velocity (1–127).
        velocity: u8,
        /// Source-native start tick.
        start: MidiTick,
        /// Note length in source-native ticks (> 0).
        len: MidiTick,
    },
    /// Remove a note from a MIDI source.
    RemoveMidiNote {
        /// Owning MIDI source.
        source_id: SourceId,
        /// Target note.
        note_id: NoteId,
    },
    /// Set a MIDI note's velocity (1–127).
    SetMidiNoteVelocity {
        /// Owning MIDI source.
        source_id: SourceId,
        /// Target note.
        note_id: NoteId,
        /// New velocity.
        velocity: u8,
    },
    /// Move a MIDI note to a new start tick. The duration is
    /// unchanged — a move never rewrites `len`.
    MoveMidiNote {
        /// Owning MIDI source.
        source_id: SourceId,
        /// Target note.
        note_id: NoteId,
        /// New source-native start tick.
        start: MidiTick,
    },
    /// Break a clip's shared-source link: duplicate the MIDI source
    /// it reads (fresh `SourceId`, fresh `NoteId`s) and repoint the
    /// clip at the copy — the explicit linked→independent
    /// transition (issue #52). Independence is never implicit
    /// copy-on-write, and detaching a media-source clip is rejected.
    DetachClipSource {
        /// Clip to detach.
        clip_id: ClipId,
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
    /// Create a parameter target (issue #40, owner/time-scope split per
    /// issue #121). `Note` owner + `NoteLocal` scope resolves to a
    /// note-relative coordinate domain (issue #43).
    AddParameterTarget {
        /// Display name.
        name: String,
        /// Owning entity.
        owner: ControlOwner,
        /// Semantic role — singleton roles are unique per owner.
        role: ParameterRole,
        /// Time coordinate of the target's automation points.
        time_scope: AutomationTimeScope,
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
        /// Owning entity.
        owner: ControlOwner,
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
    // -- semantic routing model (issue #104) -------------------------
    /// Create a group or return bus: the bus, its channel strip, the
    /// strip's chain and its main input/output ports. The master bus is
    /// created at project bootstrap — it cannot be added here.
    AddBus {
        /// `Group` or `Return` (`Master` is rejected).
        kind: crate::routing::BusKind,
        /// Display name.
        name: String,
    },
    /// Remove a bus together with its owned strip, chain and ports.
    /// Connections/sends still referencing those ports reject the
    /// commit at the reference oracle — remove them first (a single
    /// `Composite` is the sanctioned path). The master bus cannot be
    /// removed.
    RemoveBus {
        /// Target bus.
        bus_id: BusId,
    },
    /// Give a track its channel strip (chain + main ports) and wire the
    /// strip's main output into the master's main input — the
    /// deterministic default route matching the v5→v6 migration
    /// projection. Rejected when the track already owns a strip.
    AddTrackStrip {
        /// Owning track.
        track_id: TrackId,
    },
    /// Create a `Main` or `Sidechain` connection between two semantic
    /// ports. `Send` connections are created only by `AddSend` so the
    /// send↔connection 1:1 invariant holds by construction.
    AddConnection {
        /// Source output port.
        source: SemanticPortId,
        /// Destination input port.
        destination: SemanticPortId,
        /// `Main` or `Sidechain` (`Send(_)` is rejected).
        kind: crate::routing::ConnectionKind,
        /// Explicit channel map for differing port layouts.
        channel_map: Option<ExplicitChannelMap>,
    },
    /// Remove a connection. When the connection is a send edge its
    /// `SendConfig` is removed with it (1:1 invariant).
    RemoveConnection {
        /// Target connection.
        connection_id: RoutingConnectionId,
    },
    /// Create a send: the `kind = Send` connection plus its
    /// `SendConfig` in one atomic op (one CAS, one undo step).
    /// `gain_target` is a reference into the control model — the
    /// persisted gain authority lives there, never duplicated.
    AddSend {
        /// Source strip main output port.
        source: SemanticPortId,
        /// Destination strip main input port.
        destination: SemanticPortId,
        /// Pre/post-fader tap.
        tap: crate::routing::SendTap,
        /// Optional send-level control target.
        gain_target: Option<ParameterTargetId>,
    },
    /// Remove a send and its connection.
    RemoveSend {
        /// Target send.
        send_id: SendId,
    },
    /// Change a send's tap point.
    SetSendTap {
        /// Target send.
        send_id: SendId,
        /// New tap.
        tap: crate::routing::SendTap,
    },
    /// Enable or disable a connection — a disabled connection carries
    /// no signal and forms no graph edge.
    SetConnectionEnabled {
        /// Target connection.
        connection_id: RoutingConnectionId,
        /// New enabled state.
        enabled: bool,
    },
    /// Insert a processor into a strip's chain at `index`, creating its
    /// main input/output ports (plus a sidechain input port when
    /// requested).
    InsertProcessor {
        /// Owning strip.
        strip_id: StripId,
        /// Position in the chain (0..=len).
        index: usize,
        /// Display name.
        name: String,
        /// Plugin binding (resolved identity or unresolved marker).
        binding: crate::routing::ProcessorBinding,
        /// Also create a sidechain-role input port.
        sidechain_input: bool,
    },
    /// Remove a processor and its ports. Connections still referencing
    /// those ports reject at the reference oracle — remove them first.
    RemoveProcessor {
        /// Target processor.
        processor_id: ProcessorId,
    },
    /// Move a processor to a new position inside its chain — the
    /// persisted DSP order is meaningful.
    ReorderProcessor {
        /// Owning chain.
        chain_id: ChainId,
        /// Processor to move.
        processor_id: ProcessorId,
        /// New index (0..len).
        new_index: usize,
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
                if !project.contains_source(source_id) {
                    return Err(Error::NotFound(format!("source {source_id}")));
                }
                validate_clip_contract(project, placement, playback)?;
                validate_midi_clip_contract(project, source_id, placement, playback)
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
                let placement = ClipPlacement {
                    anchor: clip.placement.anchor,
                    extent: *extent,
                };
                validate_clip_contract(project, &placement, &clip.playback)?;
                validate_midi_clip_contract(project, &clip.source_id, &placement, &clip.playback)
            }
            Self::SetClipPlayback {
                clip_id, playback, ..
            } => {
                let clip = project
                    .clips
                    .get(clip_id)
                    .ok_or_else(|| Error::NotFound(format!("clip {clip_id}")))?;
                validate_clip_contract(project, &clip.placement, playback)?;
                validate_midi_clip_contract(project, &clip.source_id, &clip.placement, playback)
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
            // -- MIDI source/note model (issue #52) -------------------
            Self::AddMidiSource { division } => division.validate(),
            Self::RemoveMidiSource { source_id } => {
                midi_source(project, source_id)?;
                if project.clips.values().any(|c| c.source_id == *source_id) {
                    return Err(Error::Validation(format!(
                        "midi source {source_id} still referenced by clips"
                    )));
                }
                Ok(())
            }
            Self::AddMidiNote {
                source_id,
                key,
                velocity,
                start,
                len,
            } => {
                midi_source(project, source_id)?;
                MidiNote::validate_fields(*key, *velocity, *start, *len)
            }
            Self::RemoveMidiNote { source_id, note_id } => midi_source(project, source_id)?
                .notes
                .contains_key(note_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("note {note_id}"))),
            Self::SetMidiNoteVelocity {
                source_id,
                note_id,
                velocity,
            } => {
                midi_note(project, source_id, note_id)?;
                if *velocity == 0 || *velocity > 127 {
                    return Err(Error::Validation(format!(
                        "velocity {velocity} outside 1..=127"
                    )));
                }
                Ok(())
            }
            Self::MoveMidiNote {
                source_id,
                note_id,
                start,
            } => {
                let note = midi_note(project, source_id, note_id)?;
                // The moved note must still have a representable end.
                start.checked_add(note.len.value())?;
                Ok(())
            }
            Self::DetachClipSource { clip_id } => {
                let clip = project
                    .clips
                    .get(clip_id)
                    .ok_or_else(|| Error::NotFound(format!("clip {clip_id}")))?;
                if !project.midi_sources.contains_key(&clip.source_id) {
                    return Err(Error::Validation(format!(
                        "clip {clip_id} does not read a midi source"
                    )));
                }
                Ok(())
            }
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
                role,
                time_scope,
                kind,
                mapping,
                capability,
                base,
                ..
            } => {
                validate_control_binding(project, *owner, *role, *time_scope, None)?;
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
                    || project
                        .routing
                        .sends
                        .values()
                        .any(|s| s.gain_target == Some(*target))
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
                validate_control_owner(project, *owner)?;
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
            // -- semantic routing model (issue #104) -----------------
            // Structural checks (existence, ownership preconditions,
            // legal kinds) live here; the full invariant sweep runs
            // once in the post-apply oracle, so an op never duplicates
            // rules the model owns.
            Self::AddBus { kind, .. } => match kind {
                BusKind::Master => Err(Error::Validation(
                    "the master bus is created at project bootstrap".into(),
                )),
                BusKind::Group | BusKind::Return => Ok(()),
            },
            Self::RemoveBus { bus_id } => {
                let bus = project
                    .routing
                    .buses
                    .get(bus_id)
                    .ok_or_else(|| Error::NotFound(format!("bus {bus_id}")))?;
                if bus.kind == BusKind::Master {
                    return Err(Error::Validation("the master bus cannot be removed".into()));
                }
                Ok(())
            }
            Self::AddTrackStrip { track_id } => {
                if !project.tracks.contains_key(track_id) {
                    return Err(Error::NotFound(format!("track {track_id}")));
                }
                if project.routing.strip_of_track(*track_id).is_some() {
                    return Err(Error::Validation(format!(
                        "track {track_id} already owns a strip"
                    )));
                }
                Ok(())
            }
            Self::AddConnection {
                source,
                destination,
                kind,
                ..
            } => {
                if matches!(kind, ConnectionKind::Send(_)) {
                    return Err(Error::Validation(
                        "send connections are created by AddSend".into(),
                    ));
                }
                if !project.routing.ports.contains_key(source) {
                    return Err(Error::NotFound(format!("port {source}")));
                }
                if !project.routing.ports.contains_key(destination) {
                    return Err(Error::NotFound(format!("port {destination}")));
                }
                Ok(())
            }
            Self::RemoveConnection { connection_id } => project
                .routing
                .connections
                .contains_key(connection_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("connection {connection_id}"))),
            Self::AddSend {
                source,
                destination,
                gain_target,
                ..
            } => {
                if !project.routing.ports.contains_key(source) {
                    return Err(Error::NotFound(format!("port {source}")));
                }
                if !project.routing.ports.contains_key(destination) {
                    return Err(Error::NotFound(format!("port {destination}")));
                }
                if let Some(target) = gain_target {
                    let t = project
                        .control
                        .targets
                        .get(target)
                        .ok_or_else(|| Error::NotFound(format!("param {target}")))?;
                    // The send adopts the target at apply (owner →
                    // `Send(send_id)`): only a project-time target can
                    // legally carry a send owner.
                    if !matches!(
                        t.time_scope,
                        AutomationTimeScope::ProjectMusical | AutomationTimeScope::ProjectAbsolute
                    ) {
                        return Err(Error::Validation(format!(
                            "send gain target {target} must be project-time"
                        )));
                    }
                    if project
                        .routing
                        .sends
                        .values()
                        .any(|s| s.gain_target == Some(*target))
                    {
                        return Err(Error::Validation(format!(
                            "param {target} already bound as a send gain"
                        )));
                    }
                }
                Ok(())
            }
            Self::RemoveSend { send_id } | Self::SetSendTap { send_id, .. } => project
                .routing
                .sends
                .contains_key(send_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("send {send_id}"))),
            Self::SetConnectionEnabled { connection_id, .. } => project
                .routing
                .connections
                .contains_key(connection_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("connection {connection_id}"))),
            Self::InsertProcessor {
                strip_id, index, ..
            } => {
                let strip = project
                    .routing
                    .strips
                    .get(strip_id)
                    .ok_or_else(|| Error::NotFound(format!("strip {strip_id}")))?;
                let len = project
                    .routing
                    .chains
                    .get(&strip.chain)
                    .map_or(0, |c| c.processors.len());
                if *index > len {
                    return Err(Error::Validation(format!(
                        "insert index {index} beyond chain length {len}"
                    )));
                }
                Ok(())
            }
            Self::RemoveProcessor { processor_id } => project
                .routing
                .processors
                .contains_key(processor_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("processor {processor_id}"))),
            Self::ReorderProcessor {
                chain_id,
                processor_id,
                new_index,
            } => {
                let chain = project
                    .routing
                    .chains
                    .get(chain_id)
                    .ok_or_else(|| Error::NotFound(format!("chain {chain_id}")))?;
                if !chain.processors.contains(processor_id) {
                    return Err(Error::NotFound(format!(
                        "processor {processor_id} not in chain {chain_id}"
                    )));
                }
                if *new_index >= chain.processors.len() {
                    return Err(Error::Validation(format!(
                        "reorder index {new_index} beyond chain length {}",
                        chain.processors.len()
                    )));
                }
                Ok(())
            }
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
                // Removing the production container removes the strip
                // bound to it plus the strip-owned entities; a
                // connection still referencing those ports fails the
                // post-apply oracle and discards the whole command.
                if let Some(strip_id) = project.routing.strip_of_track(*track_id).map(|s| s.id) {
                    remove_strip_owned(project, strip_id);
                }
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
            // -- MIDI source/note model (issue #52) -------------------
            Self::AddMidiSource { division } => {
                let id = SourceId::new(ids.next());
                project.midi_sources.insert(
                    id,
                    MidiSource {
                        id,
                        division: *division,
                        notes: std::collections::BTreeMap::new(),
                    },
                );
            }
            Self::RemoveMidiSource { source_id } => {
                project.midi_sources.remove(source_id);
            }
            Self::AddMidiNote {
                source_id,
                key,
                velocity,
                start,
                len,
            } => {
                if let Some(source) = project.midi_sources.get_mut(source_id) {
                    let id = NoteId::new(ids.next());
                    source.notes.insert(
                        id,
                        MidiNote {
                            id,
                            key: *key,
                            velocity: *velocity,
                            start: *start,
                            len: *len,
                        },
                    );
                }
            }
            Self::RemoveMidiNote { source_id, note_id } => {
                if let Some(source) = project.midi_sources.get_mut(source_id) {
                    source.notes.remove(note_id);
                }
            }
            Self::SetMidiNoteVelocity {
                source_id,
                note_id,
                velocity,
            } => {
                if let Some(source) = project.midi_sources.get_mut(source_id)
                    && let Some(note) = source.notes.get_mut(note_id)
                {
                    note.velocity = *velocity;
                }
            }
            Self::MoveMidiNote {
                source_id,
                note_id,
                start,
            } => {
                if let Some(source) = project.midi_sources.get_mut(source_id)
                    && let Some(note) = source.notes.get_mut(note_id)
                {
                    note.start = *start;
                }
            }
            Self::DetachClipSource { clip_id } => {
                let linked_id = project.clips.get(clip_id).map(|c| c.source_id);
                if let Some(linked_id) = linked_id
                    && let Some(linked) = project.midi_sources.get(&linked_id).cloned()
                {
                    let new_id = SourceId::new(ids.next());
                    // Fresh NoteIds keep the project-wide uniqueness
                    // invariant — a detached copy is a new note, not
                    // the linked one under a second address.
                    let notes = linked
                        .notes
                        .into_values()
                        .map(|mut note| {
                            note.id = NoteId::new(ids.next());
                            (note.id, note)
                        })
                        .collect();
                    project.midi_sources.insert(
                        new_id,
                        MidiSource {
                            id: new_id,
                            division: linked.division,
                            notes,
                        },
                    );
                    if let Some(clip) = project.clips.get_mut(clip_id) {
                        clip.source_id = new_id;
                    }
                }
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
                role,
                time_scope,
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
                        role: *role,
                        time_scope: *time_scope,
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
                if *policy == ConflictPolicy::Replace
                    && let Some(target) = project.control.lanes.get(lane).map(|l| l.target)
                {
                    // AUT-02 (issue #132): plan the boundary anchors on
                    // the pre-edit merged series before the cut drops
                    // the points they are computed from.
                    let series = crate::control::merged_series(project, target);
                    let (left, right) = crate::control::boundary_anchors(&series, &region.range);
                    // Explicit overwrite: the new region wins the merged
                    // series — every lane of this target loses the
                    // overlapping span.
                    for other in project.control.lanes.values_mut() {
                        if other.target == target {
                            cut_range_points(&mut other.regions, &region.range);
                        }
                    }
                    let mut region = region.clone();
                    // Left boundary: fold `v_old(cut.start)` into the new
                    // region unless the caller already placed an
                    // explicit point at the boundary (E-1 — the caller's
                    // point owns the anchor position).
                    if let Some(anchor_point) = left
                        && region
                            .points
                            .first()
                            .is_none_or(|p| p.pos > anchor_point.pos)
                    {
                        region.points.insert(0, anchor_point);
                    }
                    if let Some(lane_ref) = project.control.lanes.get_mut(lane) {
                        lane_ref.regions.push(region);
                    }
                    // Right boundary: restore `v_old(cut.end)` from the
                    // pre-edit curve so the series outside the replaced
                    // span is unchanged.
                    if let Some(anchor) = right {
                        crate::control::stitch_boundary_anchor(project, target, *lane, &anchor)?;
                    }
                } else if let Some(lane_ref) = project.control.lanes.get_mut(lane) {
                    lane_ref.regions.push(region.clone());
                }
            }
            Self::EraseAutomationRange { lane, range } => {
                // AUT-02 (issue #132): erase preserves the right-side
                // curve the same way; the left Linear approach has no
                // region to host an anchor at `range.start` (E-3).
                let target = project.control.lanes.get(lane).map(|l| l.target);
                let right = target.and_then(|t| {
                    let series = crate::control::merged_series(project, t);
                    crate::control::boundary_anchors(&series, range)
                        .1
                        .map(|a| (t, a))
                });
                if let Some(lane_ref) = project.control.lanes.get_mut(lane) {
                    cut_range_points(&mut lane_ref.regions, range);
                }
                if let Some((target, anchor)) = right {
                    crate::control::stitch_boundary_anchor(project, target, *lane, &anchor)?;
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
            // -- semantic routing model (issue #104) -----------------
            Self::AddBus { kind, name } => {
                let bus_id = BusId::new(ids.next());
                let (strip_id, _) = insert_strip(project, ids, StripOwner::Bus(bus_id));
                project.routing.buses.insert(
                    bus_id,
                    Bus {
                        id: bus_id,
                        kind: *kind,
                        name: name.clone(),
                        strip: strip_id,
                    },
                );
            }
            Self::RemoveBus { bus_id } => {
                if let Some(bus) = project.routing.buses.remove(bus_id) {
                    remove_strip_owned(project, bus.strip);
                }
            }
            Self::AddTrackStrip { track_id } => {
                let (_, main_out) = insert_strip(project, ids, StripOwner::Track(*track_id));
                let master_in = master_main_input(project)
                    .ok_or_else(|| Error::Validation("master strip lacks main input".into()))?;
                let conn = RoutingConnectionId::new(ids.next());
                project.routing.connections.insert(
                    conn,
                    RoutingConnection {
                        id: conn,
                        source: main_out,
                        destination: master_in,
                        kind: ConnectionKind::Main,
                        enabled: true,
                        channel_map: None,
                    },
                );
            }
            Self::AddConnection {
                source,
                destination,
                kind,
                channel_map,
            } => {
                let id = RoutingConnectionId::new(ids.next());
                project.routing.connections.insert(
                    id,
                    RoutingConnection {
                        id,
                        source: *source,
                        destination: *destination,
                        kind: *kind,
                        enabled: true,
                        channel_map: channel_map.clone(),
                    },
                );
            }
            Self::RemoveConnection { connection_id } => {
                if let Some(conn) = project.routing.connections.remove(connection_id)
                    && let ConnectionKind::Send(send_id) = conn.kind
                {
                    project.routing.sends.remove(&send_id);
                    remove_send_owned(project, send_id);
                }
            }
            Self::AddSend {
                source,
                destination,
                tap,
                gain_target,
            } => {
                // One atomic unit: the send edge plus its config — never
                // one without the other.
                let send_id = SendId::new(ids.next());
                let conn = RoutingConnectionId::new(ids.next());
                project.routing.connections.insert(
                    conn,
                    RoutingConnection {
                        id: conn,
                        source: *source,
                        destination: *destination,
                        kind: ConnectionKind::Send(send_id),
                        enabled: true,
                        channel_map: None,
                    },
                );
                project.routing.sends.insert(
                    send_id,
                    SendConfig {
                        id: send_id,
                        connection: conn,
                        tap: *tap,
                        gain_target: *gain_target,
                    },
                );
                // The send owns its gain target by construction: a
                // `Send`-owned target cannot precede the send id, so
                // `gain_target` declares "this target is the send's
                // level" and the binding is sealed here.
                if let Some(t) = gain_target
                    && let Some(target) = project.control.targets.get_mut(t)
                {
                    target.owner = ControlOwner::Send(send_id);
                    target.role = ParameterRole::SendLevel;
                }
            }
            Self::RemoveSend { send_id } => {
                if let Some(send) = project.routing.sends.remove(send_id) {
                    project.routing.connections.remove(&send.connection);
                    remove_send_owned(project, *send_id);
                }
            }
            Self::SetSendTap { send_id, tap } => {
                if let Some(send) = project.routing.sends.get_mut(send_id) {
                    send.tap = *tap;
                }
            }
            Self::SetConnectionEnabled {
                connection_id,
                enabled,
            } => {
                if let Some(conn) = project.routing.connections.get_mut(connection_id) {
                    conn.enabled = *enabled;
                }
            }
            Self::InsertProcessor {
                strip_id,
                index,
                name,
                binding,
                sidechain_input,
            } => {
                let proc_id = ProcessorId::new(ids.next());
                let mut port_ids = Vec::new();
                let port_in = SemanticPortId::new(ids.next());
                port_ids.push(port_in);
                insert_port(
                    project,
                    SemanticPort {
                        id: port_in,
                        owner: PortOwner::Processor(proc_id),
                        direction: PortDirection::Input,
                        role: PortRole::Main,
                        domain: SignalDomain::Audio,
                        binding: processor_port_binding(binding, "in"),
                    },
                );
                let port_out = SemanticPortId::new(ids.next());
                port_ids.push(port_out);
                insert_port(
                    project,
                    SemanticPort {
                        id: port_out,
                        owner: PortOwner::Processor(proc_id),
                        direction: PortDirection::Output,
                        role: PortRole::Main,
                        domain: SignalDomain::Audio,
                        binding: processor_port_binding(binding, "out"),
                    },
                );
                if *sidechain_input {
                    let port_sc = SemanticPortId::new(ids.next());
                    port_ids.push(port_sc);
                    insert_port(
                        project,
                        SemanticPort {
                            id: port_sc,
                            owner: PortOwner::Processor(proc_id),
                            direction: PortDirection::Input,
                            role: PortRole::Sidechain,
                            domain: SignalDomain::Audio,
                            binding: processor_port_binding(binding, "sidechain"),
                        },
                    );
                }
                project.routing.processors.insert(
                    proc_id,
                    Processor {
                        id: proc_id,
                        name: name.clone(),
                        binding: binding.clone(),
                        port_ids,
                    },
                );
                if let Some(strip) = project.routing.strips.get(strip_id)
                    && let Some(chain) = project.routing.chains.get_mut(&strip.chain)
                {
                    chain.processors.insert(*index, proc_id);
                }
            }
            Self::RemoveProcessor { processor_id } => {
                if let Some(proc) = project.routing.processors.remove(processor_id) {
                    for port_id in &proc.port_ids {
                        project.routing.ports.remove(port_id);
                    }
                    for chain in project.routing.chains.values_mut() {
                        chain.processors.retain(|id| id != processor_id);
                    }
                }
            }
            Self::ReorderProcessor {
                chain_id,
                processor_id,
                new_index,
            } => {
                if let Some(chain) = project.routing.chains.get_mut(chain_id)
                    && let Some(old) = chain.processors.iter().position(|id| id == processor_id)
                {
                    chain.processors.remove(old);
                    chain.processors.insert(*new_index, *processor_id);
                }
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

/// Resolves a MIDI source by ID — the `source_id` op parameter
/// requires a MIDI-kind source, not any entry in the `SourceId` space.
fn midi_source<'p>(project: &'p Project, source_id: &SourceId) -> Result<&'p MidiSource, Error> {
    project
        .midi_sources
        .get(source_id)
        .ok_or_else(|| Error::NotFound(format!("midi source {source_id}")))
}

/// Resolves a note inside a MIDI source.
fn midi_note<'p>(
    project: &'p Project,
    source_id: &SourceId,
    note_id: &NoteId,
) -> Result<&'p MidiNote, Error> {
    midi_source(project, source_id)?
        .notes
        .get(note_id)
        .ok_or_else(|| Error::NotFound(format!("note {note_id}")))
}

/// Creates a strip with its empty chain and main input/output ports —
/// the shared construction for `AddBus`/`AddTrackStrip` and the v5→v6
/// migration shape. Returns the strip ID and its main output port.
fn insert_strip(
    project: &mut Project,
    ids: &mut dyn IdGenerator,
    owner: StripOwner,
) -> (StripId, SemanticPortId) {
    let strip_id = StripId::new(ids.next());
    let chain_id = ChainId::new(ids.next());
    let port_in = SemanticPortId::new(ids.next());
    let port_out = SemanticPortId::new(ids.next());
    project.routing.chains.insert(
        chain_id,
        ProcessorChain {
            id: chain_id,
            processors: Vec::new(),
        },
    );
    project.routing.strips.insert(
        strip_id,
        ChannelStrip {
            id: strip_id,
            owner,
            chain: chain_id,
            port_ids: vec![port_in, port_out],
        },
    );
    for (id, direction) in [
        (port_in, PortDirection::Input),
        (port_out, PortDirection::Output),
    ] {
        project.routing.ports.insert(
            id,
            SemanticPort {
                id,
                owner: PortOwner::Strip(strip_id),
                direction,
                role: PortRole::Main,
                domain: SignalDomain::Audio,
                binding: PortBinding::Internal,
            },
        );
    }
    (strip_id, port_out)
}

/// Removes a strip and the entities it exclusively owns (its chain and
/// its ports). Callers rely on the post-apply oracle to reject a
/// command that would orphan connections or sends still referencing
/// those ports.
fn remove_strip_owned(project: &mut Project, strip_id: StripId) {
    if let Some(strip) = project.routing.strips.remove(&strip_id) {
        project.routing.chains.remove(&strip.chain);
        for port_id in &strip.port_ids {
            project.routing.ports.remove(port_id);
        }
    }
}

/// Cascade: every control entity a removed send owned dies with it —
/// the send's level target cannot outlive the send that owns it, and
/// `gain_target`'s mutual reference makes oracle-rejection a deadlock
/// otherwise. Lanes and assignments feeding the removed targets and
/// assignments driven by the removed sources go with them.
fn remove_send_owned(project: &mut Project, send_id: SendId) {
    let targets: std::collections::BTreeSet<ParameterTargetId> = project
        .control
        .targets
        .values()
        .filter(|t| t.owner == ControlOwner::Send(send_id))
        .map(|t| t.id)
        .collect();
    let sources: std::collections::BTreeSet<ModulationSourceId> = project
        .control
        .sources
        .values()
        .filter(|s| s.owner == ControlOwner::Send(send_id))
        .map(|s| s.id)
        .collect();
    project
        .control
        .lanes
        .retain(|_, lane| !targets.contains(&lane.target));
    project.control.assignments.retain(|_, a| {
        !targets.contains(&a.target)
            && !a.depth_source.is_some_and(|ds| targets.contains(&ds))
            && !sources.contains(&a.source)
    });
    for id in targets {
        project.control.targets.remove(&id);
    }
    for id in sources {
        project.control.sources.remove(&id);
    }
}

/// The master strip's main input port — the default destination every
/// track strip routes into.
fn master_main_input(project: &Project) -> Option<SemanticPortId> {
    let master = project.routing.buses.get(&project.routing.master)?;
    let strip = project.routing.strips.get(&master.strip)?;
    strip
        .port_ids
        .iter()
        .find(|p| {
            project.routing.ports.get(p).is_some_and(|port| {
                port.direction == PortDirection::Input && port.role == PortRole::Main
            })
        })
        .copied()
}

/// Inserts a semantic port (routing ops draw its ID beforehand).
fn insert_port(project: &mut Project, port: SemanticPort) {
    project.routing.ports.insert(port.id, port);
}

/// The port binding derived from a processor's plugin binding:
/// resolved instances bind to plugin-advertised port identities, an
/// unresolved processor keeps its ports explicitly `Unresolved` —
/// never silently rebound by numeric equality.
fn processor_port_binding(binding: &ProcessorBinding, advertised: &str) -> PortBinding {
    match binding {
        ProcessorBinding::Resolved(reference) => PortBinding::PluginDeclared {
            plugin: reference.descriptor.clone(),
            advertised: advertised.to_string(),
            capability_revision: reference.capability_revision,
        },
        ProcessorBinding::Unresolved { descriptor } => PortBinding::Unresolved {
            plugin: descriptor.clone(),
            advertised: advertised.to_string(),
        },
    }
}
