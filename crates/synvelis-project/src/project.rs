//! The `Project` aggregate: all persisted authoring state plus the
//! recording registry, kept in sorted `BTreeMap`s for canonical output.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    id::{
        AssetId, CaptureSegmentId, CaptureStreamId, ClipId, ClockEpochId, ImportId, LaneId,
        ProjectId, RecordingPassId, RecordingSessionId, SourceId, TakeId, TrackId,
    },
    range::{MediaSpan, MusicalRange},
    recording_data::{
        CaptureSegment, CaptureStream, ClockEpoch, RecordingPass, RecordingSession, Take,
    },
    tempo::{ProjectPpqn, TempoMapData},
    time::SourceFrame,
};

/// Where a clip sits on the timeline.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "range", deny_unknown_fields)]
pub enum TimeAnchor {
    /// Musical position (project tick range, ppqn context on `Project`).
    Musical(MusicalRange),
    /// Media position (time-base span).
    Media(MediaSpan),
}

/// A track containing lanes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    /// Stable ID.
    pub id: TrackId,
    /// Display name.
    pub name: String,
    /// Owned lanes, in order.
    pub lane_ids: Vec<LaneId>,
}

/// A lane inside a track.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lane {
    /// Stable ID.
    pub id: LaneId,
    /// Owning track.
    pub track_id: TrackId,
    /// Clips on this lane, in order.
    pub clip_ids: Vec<ClipId>,
}

/// A clip: a positioned reference into a media source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    /// Stable ID.
    pub id: ClipId,
    /// Lane it sits on.
    pub lane_id: LaneId,
    /// Media source it reads.
    pub source_id: SourceId,
    /// Timeline placement.
    pub anchor: TimeAnchor,
    /// Offset into the source where the clip starts reading.
    pub source_offset: SourceFrame,
    /// Muted clips contribute silence — excluded at timeline lowering.
    /// `#[serde(default)]` keeps snapshots written before L1 loadable.
    #[serde(default)]
    pub muted: bool,
}

/// A stored asset (content-addressed media or document).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    /// Stable ID.
    pub id: AssetId,
    /// Content digest (opaque string; algorithm recorded elsewhere).
    pub digest: String,
}

/// Verification/finalization state of a media source reference.
/// Forward-only: `Imported → Verified → Committed → Saved`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum SourceState {
    /// Registered, not yet verified.
    Imported,
    /// Verified against recorded media (VS-D registers this stage).
    Verified,
    /// Media committed/finalized (still distinct from project save).
    Committed,
    /// Fully persisted state acknowledged.
    Saved,
}

impl SourceState {
    /// The single legal successor, or `None` at `Saved` (terminal).
    /// Transitions are strictly one stage forward.
    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Imported => Some(Self::Verified),
            Self::Verified => Some(Self::Committed),
            Self::Committed => Some(Self::Saved),
            Self::Saved => None,
        }
    }
}

/// A media source reference — the Verified Source port value.
///
/// Carries identity + content digest + state only; never media bytes.
/// `content_digest` doubles as the future `CommitIdentity` carrier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Stable ID.
    pub id: SourceId,
    /// Backing asset.
    pub asset_id: AssetId,
    /// Content digest of the referenced media.
    pub content_digest: String,
    /// Verification/finalization stage.
    pub state: SourceState,
}

/// A recording commit that was applied to the project (VS-D idempotency
/// registry entry). Keys are the session identity; `content_hash` carries
/// the committed manifest payload hash so a retry with different content
/// is detected as an integrity conflict rather than applied twice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedRecordingCommit {
    /// Canonical content hash of the committed manifest payload (hex).
    pub content_hash: String,
    /// Sources registered by the commit.
    pub source_ids: Vec<SourceId>,
    /// Takes registered by the commit.
    pub take_ids: Vec<TakeId>,
    /// Clips registered by the commit.
    pub clip_ids: Vec<ClipId>,
}

/// A safe local import that was applied to the project (L1 idempotency
/// registry entry). Keys are the import identity reserved by the import
/// manifest; `content_hash` carries the verified blob digest so a retry
/// with different content is detected as an integrity conflict rather
/// than applied twice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedImport {
    /// Verified content digest of the materialized blob (hex).
    pub content_hash: String,
    /// Asset registered by the import.
    pub asset_id: AssetId,
    /// Source registered by the import.
    pub source_id: SourceId,
}

/// The recording registry: all session-scoped entities.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingRegistry {
    /// Sessions by ID.
    pub sessions: BTreeMap<RecordingSessionId, RecordingSession>,
    /// Passes by ID.
    pub passes: BTreeMap<RecordingPassId, RecordingPass>,
    /// Capture streams by ID.
    pub streams: BTreeMap<CaptureStreamId, CaptureStream>,
    /// Capture segments by ID.
    pub segments: BTreeMap<CaptureSegmentId, CaptureSegment>,
    /// Takes by ID.
    pub takes: BTreeMap<TakeId, Take>,
    /// Applied recording commits by session ID. `#[serde(default)]`
    /// keeps snapshots written before VS-D loadable.
    #[serde(default)]
    pub applied_commits: BTreeMap<RecordingSessionId, AppliedRecordingCommit>,
}

/// The full persisted project aggregate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// Project identity (survives save/reload unchanged).
    pub project_id: ProjectId,
    /// Display name.
    pub name: String,
    /// PPQN context for all `MusicalTick` values in this project.
    pub ppqn: ProjectPpqn,
    /// Persisted tempo map snapshot.
    pub tempo_map: TempoMapData,
    /// Watermark the ID generator was at when this state was produced.
    pub id_watermark: u128,
    /// Tracks by ID.
    pub tracks: BTreeMap<TrackId, Track>,
    /// Lanes by ID.
    pub lanes: BTreeMap<LaneId, Lane>,
    /// Clips by ID.
    pub clips: BTreeMap<ClipId, Clip>,
    /// Assets by ID.
    pub assets: BTreeMap<AssetId, Asset>,
    /// Media sources by ID.
    pub sources: BTreeMap<SourceId, Source>,
    /// Clock epochs by ID.
    pub clock_epochs: BTreeMap<ClockEpochId, ClockEpoch>,
    /// Recording registry.
    pub recording: RecordingRegistry,
    /// Applied local imports by import ID. `#[serde(default)]`
    /// keeps snapshots written before L1 loadable.
    #[serde(default)]
    pub applied_imports: BTreeMap<ImportId, AppliedImport>,
}

impl Project {
    /// An empty project with the given identity and name.
    #[must_use]
    pub fn new(project_id: ProjectId, name: impl Into<String>) -> Self {
        Self {
            project_id,
            name: name.into(),
            ppqn: ProjectPpqn::default(),
            tempo_map: TempoMapData::default(),
            id_watermark: 0,
            tracks: BTreeMap::new(),
            lanes: BTreeMap::new(),
            clips: BTreeMap::new(),
            assets: BTreeMap::new(),
            sources: BTreeMap::new(),
            clock_epochs: BTreeMap::new(),
            recording: RecordingRegistry::default(),
            applied_imports: BTreeMap::new(),
        }
    }

    /// Validates every internal reference. Called on load so a corrupt or
    /// tampered snapshot is rejected before it replaces live state.
    ///
    /// # Errors
    /// [`Error::NotFound`]/[`Error::WrongOwner`]/[`Error::Validation`].
    #[allow(clippy::too_many_lines)] // sequential per-entity checks
    pub fn validate_references(&self) -> Result<(), Error> {
        for lane in self.lanes.values() {
            let track = self
                .tracks
                .get(&lane.track_id)
                .ok_or_else(|| Error::NotFound(format!("track {}", lane.track_id)))?;
            if !track.lane_ids.contains(&lane.id) {
                return Err(Error::WrongOwner(format!(
                    "lane {} not listed in track {}",
                    lane.id, track.id
                )));
            }
        }
        for track in self.tracks.values() {
            for lane_id in &track.lane_ids {
                if !self.lanes.contains_key(lane_id) {
                    return Err(Error::NotFound(format!("lane {lane_id}")));
                }
            }
        }
        for clip in self.clips.values() {
            let lane = self
                .lanes
                .get(&clip.lane_id)
                .ok_or_else(|| Error::NotFound(format!("lane {}", clip.lane_id)))?;
            if !lane.clip_ids.contains(&clip.id) {
                return Err(Error::WrongOwner(format!(
                    "clip {} not listed in lane {}",
                    clip.id, lane.id
                )));
            }
            if !self.sources.contains_key(&clip.source_id) {
                return Err(Error::NotFound(format!("source {}", clip.source_id)));
            }
        }
        for source in self.sources.values() {
            if !self.assets.contains_key(&source.asset_id) {
                return Err(Error::NotFound(format!("asset {}", source.asset_id)));
            }
        }
        let rec = &self.recording;
        for pass in rec.passes.values() {
            if !rec.sessions.contains_key(&pass.session_id) {
                return Err(Error::NotFound(format!("session {}", pass.session_id)));
            }
            if rec
                .passes
                .values()
                .filter(|p| p.session_id == pass.session_id && p.index == pass.index)
                .count()
                != 1
            {
                return Err(Error::Validation(format!(
                    "duplicate pass index {} in session {}",
                    pass.index, pass.session_id
                )));
            }
        }
        for stream in rec.streams.values() {
            if !rec.sessions.contains_key(&stream.session_id) {
                return Err(Error::NotFound(format!("session {}", stream.session_id)));
            }
            if !self.lanes.contains_key(&stream.lane_id) {
                return Err(Error::NotFound(format!("lane {}", stream.lane_id)));
            }
            if !self.clock_epochs.contains_key(&stream.clock_epoch_id) {
                return Err(Error::NotFound(format!(
                    "clock epoch {}",
                    stream.clock_epoch_id
                )));
            }
        }
        for segment in rec.segments.values() {
            let stream = rec
                .streams
                .get(&segment.stream_id)
                .ok_or_else(|| Error::NotFound(format!("stream {}", segment.stream_id)))?;
            let pass = rec
                .passes
                .get(&segment.pass_id)
                .ok_or_else(|| Error::NotFound(format!("pass {}", segment.pass_id)))?;
            if pass.session_id != stream.session_id {
                return Err(Error::WrongOwner(format!(
                    "segment {} crosses sessions",
                    segment.id
                )));
            }
            if !self.sources.contains_key(&segment.source_id) {
                return Err(Error::NotFound(format!("source {}", segment.source_id)));
            }
            if segment.session_range.is_empty() || segment.source_range.is_empty() {
                return Err(Error::Validation(format!(
                    "empty range in segment {}",
                    segment.id
                )));
            }
        }
        for take in rec.takes.values() {
            if !rec.sessions.contains_key(&take.session_id) {
                return Err(Error::NotFound(format!("session {}", take.session_id)));
            }
            let pass = rec
                .passes
                .get(&take.pass_id)
                .ok_or_else(|| Error::NotFound(format!("pass {}", take.pass_id)))?;
            if pass.session_id != take.session_id {
                return Err(Error::WrongOwner(format!("take {} pass", take.id)));
            }
            let stream = rec
                .streams
                .get(&take.stream_id)
                .ok_or_else(|| Error::NotFound(format!("stream {}", take.stream_id)))?;
            if stream.session_id != take.session_id {
                return Err(Error::WrongOwner(format!("take {} stream", take.id)));
            }
            if take.lane_id != stream.lane_id {
                return Err(Error::WrongOwner(format!("take {} lane", take.id)));
            }
            if !self.sources.contains_key(&take.source_id) {
                return Err(Error::NotFound(format!("source {}", take.source_id)));
            }
            if take.segment_ids.is_empty() {
                return Err(Error::Validation(format!(
                    "take {} has no segments",
                    take.id
                )));
            }
            for seg_id in &take.segment_ids {
                let seg = rec
                    .segments
                    .get(seg_id)
                    .ok_or_else(|| Error::NotFound(format!("segment {seg_id}")))?;
                let seg_stream = rec
                    .streams
                    .get(&seg.stream_id)
                    .ok_or_else(|| Error::NotFound(format!("stream {}", seg.stream_id)))?;
                if seg_stream.session_id != take.session_id {
                    return Err(Error::WrongOwner(format!(
                        "take {} segment {seg_id} belongs to another session",
                        take.id
                    )));
                }
            }
        }
        Ok(())
    }
}
