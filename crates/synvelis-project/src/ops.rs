//! `ProjectOp`: project-level domain operations implementing [`Operation`].
//!
//! Ops that create entities draw their IDs from the injected generator
//! inside `apply` (validation stays pure). Ops returning no payload are
//! validated only; the created entity's ID is not reported back — callers
//! that need it read it from the project after apply (F0 simplification).

use crate::{
    controller::Operation,
    error::Error,
    id::{AssetId, ClipId, IdGenerator, LaneId, SourceId, TrackId},
    placement::{ClipExtent, ClipPlacement, ClipPlayback, TimeAnchor},
    project::{Asset, Clip, Lane, Project, Source, SourceState, Track},
    recording_data::ClockEpoch,
    tempo::SignatureDto,
    tempo::TempoEventDto,
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
    /// Remove a clip.
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
        }
        Ok(())
    }
}

/// Clip placement+playback contract checks.
///
/// Only the *rate-independent* contract is enforced here — anchor
/// resolvability is tempo-map dependent (an anchor may re-derive
/// differently or fail after a tempo edit; consumers surface that as
/// `UnresolvableAnchor`, so it is not an op-time error).
///
/// `Absolute` extents carry their own time base: `frames_at(denominator)`
/// treats the declared base as the source's native frame rate — the
/// unit-consistent convention for absolute-extent clips (`1/rate`
/// bases check exactly). `Musical` extents are tempo-derived and
/// `Source` carries no native rate, so only the non-empty window is
/// checked; their handle fit is enforced at resolve time.
fn validate_clip_contract(
    _project: &Project,
    placement: &ClipPlacement,
    playback: &ClipPlayback,
) -> Result<(), Error> {
    if placement.extent.is_empty() {
        return Err(Error::Validation("clip extent is empty".into()));
    }
    match &placement.extent {
        ClipExtent::Absolute(dur) => {
            let media_frames =
                u64::try_from(dur.frames_at(dur.denominator())).map_err(|_| Error::Negative)?;
            playback.validate_window(media_frames)
        }
        ClipExtent::Musical(_) => playback.validate_window(0),
    }
}
