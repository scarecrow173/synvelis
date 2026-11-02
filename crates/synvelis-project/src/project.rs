//! The `Project` aggregate: all persisted authoring state plus the
//! recording registry, kept in sorted `BTreeMap`s for canonical output.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    comp::CompRegistry,
    control::{ControlModel, ControlScope},
    error::Error,
    id::{
        AssetId, CaptureSegmentId, CaptureStreamId, ClipId, ClockEpochId, ImportId, LaneId,
        ProjectId, RecordingPassId, RecordingSessionId, SourceId, TakeId, TrackId,
    },
    placement::{ClipPlacement, ClipPlayback},
    range::{MediaSpan, MusicalRange},
    recording_data::{
        CaptureSegment, CaptureStream, ClockEpoch, RecordingPass, RecordingSession, Take,
    },
    tempo::{ProjectPpqn, TempoMapData},
    time::ProjectTime,
};

/// A project-domain half-open range: musical ticks or a media span.
///
/// (Formerly `TimeAnchor` — renamed when `Clip` adopted the normalized
/// anchor+extent placement model; a point anchor is now
/// [`crate::placement::TimeAnchor`].)
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "range", deny_unknown_fields)]
pub enum ProjectRange {
    /// Musical range (project tick range, ppqn context on `Project`).
    Musical(MusicalRange),
    /// Media range (time-base span).
    Media(MediaSpan),
}

/// The time domain a [`ProjectRange`] lives in.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectDomain {
    /// Musical ticks.
    Musical,
    /// Media time base.
    Media,
}

impl ProjectRange {
    /// The domain this range is expressed in.
    #[must_use]
    pub const fn domain(&self) -> ProjectDomain {
        match self {
            Self::Musical(_) => ProjectDomain::Musical,
            Self::Media(_) => ProjectDomain::Media,
        }
    }

    /// Inclusive start as a project-time point.
    #[must_use]
    pub fn start_time(&self) -> ProjectTime {
        match self {
            Self::Musical(r) => ProjectTime::Musical(r.start()),
            Self::Media(s) => ProjectTime::Media(s.start()),
        }
    }

    /// Exclusive end as a project-time point.
    #[must_use]
    pub fn end_time(&self) -> ProjectTime {
        match self {
            Self::Musical(r) => ProjectTime::Musical(r.end()),
            Self::Media(s) => ProjectTime::Media(s.end()),
        }
    }

    /// `true` when the range contains no position (`start == end`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Musical(r) => r.is_empty(),
            Self::Media(s) => s.is_empty(),
        }
    }

    /// `true` when `pos` is inside `[start, end)`. A `pos` in the other
    /// domain is never contained.
    #[must_use]
    pub fn contains_time(&self, pos: &ProjectTime) -> bool {
        match (self, pos) {
            (Self::Musical(r), ProjectTime::Musical(t)) => r.contains(*t),
            (Self::Media(s), ProjectTime::Media(m)) => !m.lt(&s.start()) && m.lt(&s.end()),
            _ => false,
        }
    }

    /// `true` when the two ranges share at least one position.
    /// Different domains never overlap.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Musical(a), Self::Musical(b)) => a.overlaps(b),
            (Self::Media(a), Self::Media(b)) => a.start().lt(&b.end()) && b.start().lt(&a.end()),
            _ => false,
        }
    }

    /// `true` when `other` lies entirely within `self` (same domain
    /// required — a range in another domain is never contained).
    #[must_use]
    pub fn contains_range(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Musical(a), Self::Musical(b)) => a.start() <= b.start() && b.end() <= a.end(),
            (Self::Media(a), Self::Media(b)) => !b.start().lt(&a.start()) && !a.end().lt(&b.end()),
            _ => false,
        }
    }

    /// The range length in frames at `rate`. Musical ranges convert
    /// through `map` (required only for the musical domain).
    ///
    /// # Errors
    /// [`Error::Validation`]/[`Error::Overflow`]/[`Error::Negative`].
    pub fn media_frames(
        &self,
        map: Option<&crate::tempo::TickTimeMapping>,
        rate: u32,
    ) -> Result<i64, Error> {
        match self {
            Self::Media(s) => {
                let end = s.end().frames_at(rate);
                let start = s.start().frames_at(rate);
                end.checked_sub(start).ok_or(Error::Overflow)
            }
            Self::Musical(r) => {
                let map =
                    map.ok_or_else(|| Error::Validation("musical range needs a tempo map".into()))?;
                let end = map.tick_to_media(r.end(), 1, rate)?.value();
                let start = map.tick_to_media(r.start(), 1, rate)?.value();
                end.checked_sub(start).ok_or(Error::Overflow)
            }
        }
    }
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
///
/// Placement (anchor point + extent, the saved authority) and playback
/// (source window + mapping) are separate per the normalized model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    /// Stable ID.
    pub id: ClipId,
    /// Lane it sits on.
    pub lane_id: LaneId,
    /// Media source it reads.
    pub source_id: SourceId,
    /// Timeline placement (anchor + extent).
    pub placement: ClipPlacement,
    /// Source read window and project→source mapping.
    pub playback: ClipPlayback,
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
    /// Comping registry (take groups, comps, regions, transitions).
    /// `#[serde(default)]` keeps snapshots written before MD-A loadable.
    #[serde(default)]
    pub comp: CompRegistry,
    /// Applied local imports by import ID. `#[serde(default)]`
    /// keeps snapshots written before L1 loadable.
    #[serde(default)]
    pub applied_imports: BTreeMap<ImportId, AppliedImport>,
    /// The persisted Parameter Control Model — the single source of
    /// truth every Lane / Signal Flow / Inspector projection reads
    /// (issue #40). `#[serde(default)]` keeps schema-2 minimal
    /// snapshots loadable.
    #[serde(default)]
    pub control: ControlModel,
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
            comp: CompRegistry::default(),
            applied_imports: BTreeMap::new(),
            control: ControlModel::default(),
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
        crate::comp::validate_registry(self)?;
        // -- parameter control model (issue #40) ----------------------
        let ctl = &self.control;
        for target in ctl.targets.values() {
            match target.owner {
                ControlScope::Track(id) if !self.tracks.contains_key(&id) => {
                    return Err(Error::NotFound(format!("track {id}")));
                }
                ControlScope::Clip(id) if !self.clips.contains_key(&id) => {
                    return Err(Error::NotFound(format!("clip {id}")));
                }
                _ => {}
            }
            target
                .kind
                .validate()
                .map_err(|e| Error::Validation(format!("param {}: {e}", target.id)))?;
            if !target.base.is_finite() || !(0.0..=1.0).contains(&target.base) {
                return Err(Error::Validation(format!(
                    "param {} base outside [0,1]",
                    target.id
                )));
            }
            if let Some(cap) = target.capability {
                cap.param_info("")?;
            }
        }
        for source in ctl.sources.values() {
            match source.owner {
                ControlScope::Track(id) if !self.tracks.contains_key(&id) => {
                    return Err(Error::NotFound(format!("track {id}")));
                }
                ControlScope::Clip(id) if !self.clips.contains_key(&id) => {
                    return Err(Error::NotFound(format!("clip {id}")));
                }
                _ => {}
            }
            source
                .kind
                .validate()
                .map_err(|e| Error::Validation(format!("mod source {}: {e}", source.id)))?;
        }
        for lane in ctl.lanes.values() {
            if !ctl.targets.contains_key(&lane.target) {
                return Err(Error::NotFound(format!("param {}", lane.target)));
            }
            for region in &lane.regions {
                region
                    .validate()
                    .map_err(|e| Error::Validation(format!("lane {}: {e}", lane.id)))?;
            }
        }
        for assignment in ctl.assignments.values() {
            if !ctl.sources.contains_key(&assignment.source) {
                return Err(Error::NotFound(format!("mod source {}", assignment.source)));
            }
            if !ctl.targets.contains_key(&assignment.target) {
                return Err(Error::NotFound(format!("param {}", assignment.target)));
            }
            if let Some(ds) = assignment.depth_source
                && !ctl.targets.contains_key(&ds)
            {
                return Err(Error::NotFound(format!("depth source param {ds}")));
            }
        }
        // Regions on one target never overlap — competition is a
        // detected conflict, and load must not carry a silently
        // prioritized series.
        if let Some(conflict) = crate::control::detect_conflicts(self).into_iter().next() {
            return Err(Error::AutomationConflict {
                target: conflict.target.to_string(),
                detail: format!(
                    "lanes {} / {} overlap at {:?}",
                    conflict.lane_a, conflict.lane_b, conflict.overlap
                ),
            });
        }
        Ok(())
    }
}
