//! The `Project` aggregate: all persisted authoring state plus the
//! recording registry, kept in sorted `BTreeMap`s for canonical output.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    comp::CompRegistry,
    control::{BeatPoint, ControlModel, ControlOwner, MASTER_FADER_RAW, ParameterRole},
    error::Error,
    id::{
        AssetId, CaptureSegmentId, CaptureStreamId, ClipId, ClockEpochId, ImportId, LaneId, NoteId,
        ParameterTargetId, ProjectId, RecordingPassId, RecordingSessionId, SourceId, TakeId,
        TrackId,
    },
    midi::MidiSource,
    monitoring_data::MonitoringData,
    placement::{ClipPlacement, ClipPlayback},
    range::{MediaSpan, MusicalRange},
    recording_data::{
        CaptureSegment, CaptureStream, ClockEpoch, RecordingPass, RecordingSession, Take,
    },
    routing::{RoutingModel, WELL_KNOWN_BASE, is_issuable_raw},
    smf::SmfDocument,
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

/// A clip: a positioned reference into a source — media or MIDI.
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
    /// Source it reads — a media `Source` or a `MidiSource`.
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

/// A note: a clip-local timed entity that anchors note-local
/// expression scope (`ControlOwner::Note`).
///
/// `start`/`len` are clip-relative beats — the note follows its owning
/// clip's musical anchor, so `MoveClip` shifts the evaluation domain
/// without rewriting anything. `key` carries the note's musical
/// identity (0–127) for stable display/dedup purposes; the `PoC` only
/// uses the temporal domain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    /// Stable ID.
    pub id: NoteId,
    /// Owning clip.
    pub clip: ClipId,
    /// Musical key identity (0–127).
    pub key: u8,
    /// Clip-relative start beat.
    pub start: BeatPoint,
    /// Note length in beats (> 0).
    pub len: BeatPoint,
}

/// Algorithm classification of a persisted content-digest string.
///
/// Digest strings stay opaque to the project — the producer records
/// the algorithm. This classification exists so consumers can tell a
/// verified content address from a weak legacy digest: a legacy digest
/// is *never* promoted to a strong identity without byte-level
/// re-verification (synvelis#86).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DigestKind {
    /// `"sha256:<64 hex>"` — the ALIB-02 strong content address that
    /// resolves under the `blobs/sha256/<hex>` namespace (the blob
    /// layer lowercases the hex, so casing variants share one blob).
    Sha256,
    /// Exactly 16 lowercase hex chars — the legacy forward+reverse
    /// CRC32 fold written by `synvelis-player`/`synvelis-recording`
    /// before SHA-256 import. Weak: collisions are possible, so equal
    /// digests never prove equal bytes.
    Crc32Fold,
    /// Any other recorded string — preserved verbatim; the algorithm
    /// is the producer's concern. Treated as weak/opaque: never a
    /// verified content address.
    Opaque,
}

/// Typed view over a persisted digest string — `Asset.digest`,
/// `Source.content_digest`, `AppliedImport.content_hash`.
///
/// Parsing is infallible: unrecognized forms classify
/// [`DigestKind::Opaque`] rather than failing, so every historical
/// snapshot stays readable. Serde (de)serializes the raw string
/// unchanged — the persisted form is never rewritten.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetDigest {
    raw: String,
}

impl AssetDigest {
    /// Classifies `raw` — never fails.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        Self {
            raw: raw.to_owned(),
        }
    }

    /// The persisted string, verbatim.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// The algorithm classification.
    #[must_use]
    pub fn kind(&self) -> DigestKind {
        classify_digest(&self.raw)
    }

    /// `true` only for the strong SHA-256 content address — the sole
    /// form a consumer may treat as verified content identity.
    #[must_use]
    pub fn is_content_addressed(&self) -> bool {
        self.kind() == DigestKind::Sha256
    }
}

/// The [`DigestKind`] rule, shared with the blob layer's namespace
/// decision: `"sha256:"` + 64 hex digits is the strong form (the same
/// acceptance `LocalBlobDir`'s object-identity mapping applies),
/// exactly-16 lowercase hex is the legacy CRC fold, everything else
/// is opaque.
fn classify_digest(raw: &str) -> DigestKind {
    if let Some(hex) = raw.strip_prefix("sha256:")
        && hex.len() == 64
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return DigestKind::Sha256;
    }
    if raw.len() == 16
        && raw
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return DigestKind::Crc32Fold;
    }
    DigestKind::Opaque
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
    /// Notes by ID — clip-local entities anchoring `ControlOwner::Note`
    /// evaluation domains (issue #43). `#[serde(default)]` lets a v3
    /// body deserialize; `load()` still requires schema v5.
    #[serde(default)]
    pub notes: BTreeMap<NoteId, Note>,
    /// Assets by ID.
    pub assets: BTreeMap<AssetId, Asset>,
    /// Media sources by ID.
    pub sources: BTreeMap<SourceId, Source>,
    /// MIDI sources by ID — project-owned note sequences sharing the
    /// `SourceId` space, so a clip references a MIDI source through
    /// `Clip::source_id` exactly like a media source (issue #52:
    /// linked clips read the same note set). `#[serde(default)]` lets
    /// a v4 body deserialize; `load()` still requires schema v5.
    #[serde(default)]
    pub midi_sources: BTreeMap<SourceId, MidiSource>,
    /// SMF provenance documents keyed by the `MidiSource` they were
    /// imported into (issue #126). The document owns the source-native
    /// event stream — track/channel ordering, tempo/meta, sysex —
    /// while `MidiSource.notes` is its deterministic note projection.
    /// `#[serde(default)]` keeps pre-v9 bodies loadable; `load()`
    /// still requires schema v9.
    #[serde(default)]
    pub smf_documents: BTreeMap<SourceId, SmfDocument>,
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
    /// The Semantic Routing Model — persisted signal-flow authority
    /// (issue #104, schema v6). `#[serde(default)]` lets a v5 body
    /// *parse*; it is not a migration — the v5→v6 step projects the
    /// legacy routing explicitly (see `crate::migration`).
    #[serde(default)]
    pub routing: RoutingModel,
    /// Authored monitor declarations — per-track Arm/Monitor mode,
    /// channel map and lane binding plus project-wide punch marks
    /// (synvelis#117, schema v12). Session *observations*
    /// (`input_output_ready`, hardware-direct state, latency, live
    /// sessions) are runtime state and are never persisted here.
    /// `#[serde(default)]` keeps pre-v12 bodies loadable.
    #[serde(default)]
    pub monitoring: MonitoringData,
}

impl Project {
    /// `true` when `source_id` resolves to a readable source — a media
    /// [`Source`] or a [`MidiSource`]. Clip references validate against
    /// this union; domains that require media bytes (recording,
    /// comping) keep checking `sources` alone.
    #[must_use]
    pub fn contains_source(&self, source_id: &SourceId) -> bool {
        self.sources.contains_key(source_id) || self.midi_sources.contains_key(source_id)
    }

    /// An empty project with the given identity and name.
    #[must_use]
    pub fn new(project_id: ProjectId, name: impl Into<String>) -> Self {
        let routing = RoutingModel::default();
        // The bootstrap routing model always carries the master bus and
        // its strip — the fader target's owner (synvelis#185).
        let master_strip = routing.buses[&routing.master].strip;
        Self {
            project_id,
            name: name.into(),
            ppqn: ProjectPpqn::default(),
            tempo_map: TempoMapData::default(),
            id_watermark: 0,
            tracks: BTreeMap::new(),
            lanes: BTreeMap::new(),
            clips: BTreeMap::new(),
            notes: BTreeMap::new(),
            assets: BTreeMap::new(),
            sources: BTreeMap::new(),
            midi_sources: BTreeMap::new(),
            smf_documents: BTreeMap::new(),
            clock_epochs: BTreeMap::new(),
            recording: RecordingRegistry::default(),
            comp: CompRegistry::default(),
            applied_imports: BTreeMap::new(),
            control: ControlModel::bootstrap(master_strip),
            routing,
            monitoring: MonitoringData::default(),
        }
    }

    /// Validates every internal reference. Called on load so a corrupt or
    /// tampered snapshot is rejected before it replaces live state.
    ///
    /// Also re-validates the persisted *temporal contract*: the
    /// persisted tempo map must compile (every musical anchor/extent/
    /// comp range resolves through it) and every clip's placement/
    /// playback pair must satisfy `validate_clip_contract`, so a
    /// snapshot that can no longer produce valid lowered state is
    /// rejected at load rather than resurrecting invalid clips.
    ///
    /// # Errors
    /// [`Error::NotFound`]/[`Error::WrongOwner`]/[`Error::Validation`].
    #[allow(clippy::too_many_lines)] // sequential per-entity checks
    pub fn validate_references(&self) -> Result<(), Error> {
        // The persisted tempo map must compile — a malformed map would
        // silently strand every musical-time consumer.
        crate::tempo::TickTimeMapping::new(&self.tempo_map)?;
        // PPQN coherence (issue #125): the project setting and the
        // tempo map's tick context must agree and stay inside the SMF
        // 15-bit selectable range — a snapshot that splits them gives
        // every musical-tick consumer a different denominator.
        if !crate::ppqn::ppqn_in_range(self.ppqn.0) {
            return Err(Error::Validation(format!(
                "project ppqn {} outside selectable range {}..={}",
                self.ppqn.0,
                crate::ppqn::PPQN_MIN,
                crate::ppqn::PPQN_MAX
            )));
        }
        if self.tempo_map.ppqn != self.ppqn.0 {
            return Err(Error::Validation(format!(
                "tempo map ppqn {} disagrees with project ppqn {}",
                self.tempo_map.ppqn, self.ppqn.0
            )));
        }
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
            if !self.contains_source(&clip.source_id) {
                return Err(Error::NotFound(format!("source {}", clip.source_id)));
            }
            crate::placement::validate_clip_contract(self, &clip.placement, &clip.playback)?;
            crate::placement::validate_midi_clip_contract(
                self,
                &clip.source_id,
                &clip.placement,
                &clip.playback,
            )?;
        }
        for (key, note) in &self.notes {
            if *key != note.id {
                return Err(Error::WrongOwner(format!(
                    "note {} stored under foreign key {key}",
                    note.id
                )));
            }
            if !self.clips.contains_key(&note.clip) {
                return Err(Error::NotFound(format!("clip {}", note.clip)));
            }
            if note.len.num() <= 0 || note.start.num() < 0 {
                return Err(Error::Validation(format!(
                    "note {} needs len > 0 and start >= 0",
                    note.id
                )));
            }
            if note.key > 127 {
                return Err(Error::Validation(format!(
                    "note {} key {} > 127",
                    note.id, note.key
                )));
            }
        }
        for source in self.sources.values() {
            if !self.assets.contains_key(&source.asset_id) {
                return Err(Error::NotFound(format!("asset {}", source.asset_id)));
            }
        }
        self.validate_digests()?;
        // -- MIDI source/note model (issue #52) ----------------------
        // The `SourceId` space is shared across `sources` and
        // `midi_sources`, and the `NoteId` space across clip-local
        // `notes` and every MIDI source: a crafted snapshot must not
        // name the same ID in two places — clip/note addressing would
        // silently depend on which collection a consumer reads.
        let mut midi_note_ids = std::collections::BTreeSet::new();
        for (source_id, source) in &self.midi_sources {
            if *source_id != source.id {
                return Err(Error::WrongOwner(format!(
                    "midi source {} stored under foreign key {source_id}",
                    source.id
                )));
            }
            if self.sources.contains_key(source_id) {
                return Err(Error::Validation(format!(
                    "source {source_id} names both a media source and a midi source"
                )));
            }
            source.validate()?;
            for note_id in source.notes.keys() {
                if !midi_note_ids.insert(*note_id) {
                    return Err(Error::Validation(format!(
                        "note {note_id} exists in more than one midi source"
                    )));
                }
                if self.notes.contains_key(note_id) {
                    return Err(Error::Validation(format!(
                        "note {note_id} exists as both clip-local note and midi note"
                    )));
                }
            }
        }
        // -- SMF provenance documents (issue #126) -------------------
        // Every document must name a live MIDI source, agree with its
        // division, and project to exactly the stored note multiset —
        // a crafted snapshot cannot keep provenance and notes
        // divergent (note edits on SMF sources are op-rejected, so any
        // divergence here is foreign state).
        for (source_id, doc) in &self.smf_documents {
            let source = self.midi_sources.get(source_id).ok_or_else(|| {
                Error::NotFound(format!("midi source {source_id} (smf document)"))
            })?;
            doc.validate()?;
            if source.division.ticks_per_quarter() != doc.division {
                return Err(Error::Validation(format!(
                    "smf document for {source_id} division {} != midi source division {}",
                    doc.division,
                    source.division.ticks_per_quarter()
                )));
            }
            let mut projected: Vec<(u8, u8, i64, i64)> = doc
                .paired_notes()
                .map_err(|e| Error::Validation(format!("smf document for {source_id}: {e}")))?
                .iter()
                .map(|s| (s.key, s.velocity, s.start.value(), s.len.value()))
                .collect();
            let mut actual: Vec<(u8, u8, i64, i64)> = source
                .notes
                .values()
                .map(|n| (n.key, n.velocity, n.start.value(), n.len.value()))
                .collect();
            projected.sort_unstable();
            actual.sort_unstable();
            if projected != actual {
                return Err(Error::Validation(format!(
                    "smf document for {source_id} note projection diverges from midi source notes"
                )));
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
        // -- monitor declarations (synvelis#117, schema v12) --------
        // A declaration names a live track; its optional lane must be
        // one of that track's lanes, and the channel map must be
        // non-empty. Punch marks carry no references.
        for (track_id, decl) in &self.monitoring.tracks {
            let track = self.tracks.get(track_id).ok_or_else(|| {
                Error::NotFound(format!("track {track_id} (monitor declaration)"))
            })?;
            if let Some(lane) = decl.lane
                && !track.lane_ids.contains(&lane)
            {
                return Err(Error::Validation(format!(
                    "monitor declaration lane {lane} is not a lane of track {track_id}"
                )));
            }
            if decl.channel_map.is_empty() {
                return Err(Error::Validation(format!(
                    "monitor declaration for track {track_id} has an empty channel map"
                )));
            }
        }
        crate::comp::validate_registry(self)?;
        // -- parameter control model (issue #40, issue #121) -----------
        let ctl = &self.control;
        // The bootstrap master-strip fader is structural (synvelis#185):
        // `Project::new` seeds it, ops cannot remove it, and the v7→v8
        // migration restores it — a snapshot without it is foreign
        // state, not an older legal shape. The oracle also pins the
        // target's *role and owner*: the well-known ID is the app's
        // lookup key for "the master strip's fader", so an entry that
        // merely squats on the ID while pointing elsewhere is just as
        // foreign. Cosmetic fields (name/kind/mapping/base) stay
        // unconstrained — the v7→v8 migration legitimately re-keys a
        // pre-existing user fader with its own values.
        let fader_id = ParameterTargetId::new(MASTER_FADER_RAW);
        let fader = ctl.targets.get(&fader_id).ok_or_else(|| {
            Error::Validation("project lacks the bootstrap master-strip fader target".into())
        })?;
        if let Some(bus) = self.routing.buses.get(&self.routing.master)
            && (fader.role != ParameterRole::Fader || fader.owner != ControlOwner::Strip(bus.strip))
        {
            return Err(Error::Validation(
                "bootstrap master-strip fader target has a foreign role or owner".into(),
            ));
        }
        for target in ctl.targets.values() {
            crate::control::validate_control_binding(
                self,
                target.owner,
                target.role,
                target.time_scope,
                Some(target.id),
            )?;
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
            crate::control::validate_control_owner(self, source.owner)?;
            source
                .kind
                .validate()
                .map_err(|e| Error::Validation(format!("mod source {}: {e}", source.id)))?;
        }
        // Send gain bindings must reference targets the send itself
        // owns (issue #121 owner mapping).
        for send in self.routing.sends.values() {
            if let Some(target_id) = send.gain_target {
                let target = ctl
                    .targets
                    .get(&target_id)
                    .ok_or_else(|| Error::NotFound(format!("param {target_id}")))?;
                if target.owner != ControlOwner::Send(send.id) {
                    return Err(Error::Validation(format!(
                        "send {} gain target {} is not send-owned",
                        send.id, target_id
                    )));
                }
            }
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
        // -- semantic routing model (issue #104) ----------------------
        self.routing.validate(self)
    }

    /// Digest/receipt integrity shared by [`Self::validate_references`]
    /// and the v10→v11 migration step (synvelis#86): every persisted
    /// digest string is non-empty, and every applied-import receipt
    /// names live asset and source entities.
    ///
    /// Digest *strings* stay opaque — [`AssetDigest`] owns
    /// classification. Equality between independently recorded digest
    /// fields is deliberately *not* an invariant: a source may record
    /// a different digest than its backing asset.
    ///
    /// # Errors
    /// [`Error::Validation`] on an empty digest;
    /// [`Error::NotFound`] on a receipt naming a missing entity.
    pub(crate) fn validate_digests(&self) -> Result<(), Error> {
        for asset in self.assets.values() {
            if asset.digest.is_empty() {
                return Err(Error::Validation(format!(
                    "asset {} has an empty digest",
                    asset.id
                )));
            }
        }
        for source in self.sources.values() {
            if source.content_digest.is_empty() {
                return Err(Error::Validation(format!(
                    "source {} has an empty content digest",
                    source.id
                )));
            }
        }
        for (import_id, applied) in &self.applied_imports {
            if !self.assets.contains_key(&applied.asset_id) {
                return Err(Error::NotFound(format!(
                    "asset {} (applied import {import_id})",
                    applied.asset_id
                )));
            }
            if !self.sources.contains_key(&applied.source_id) {
                return Err(Error::NotFound(format!(
                    "source {} (applied import {import_id})",
                    applied.source_id
                )));
            }
            if applied.content_hash.is_empty() {
                return Err(Error::Validation(format!(
                    "applied import {import_id} has an empty content hash"
                )));
            }
        }
        Ok(())
    }

    /// Largest generator-issuable raw ID among every serialized entity
    /// key — `None` when the project carries no issued entity. Entity
    /// identity is the map key, so key iteration covers the live IDs.
    /// Reserved well-known raw values (`>= WELL_KNOWN_BASE`, the
    /// bootstrap master entities) are constants the generator never
    /// issues — not "issued" IDs — so they do not push the watermark.
    pub(crate) fn max_issuable_id_raw(&self) -> Option<u128> {
        macro_rules! fold {
            ($max:expr, $map:expr) => {
                $max = $map.keys().fold($max, |acc: Option<u128>, id| {
                    if is_issuable_raw(id.raw()) {
                        Some(acc.map_or_else(|| id.raw(), |m| m.max(id.raw())))
                    } else {
                        acc
                    }
                });
            };
        }
        let mut max: Option<u128> = None;
        fold!(max, self.tracks);
        fold!(max, self.lanes);
        fold!(max, self.clips);
        fold!(max, self.notes);
        fold!(max, self.assets);
        fold!(max, self.sources);
        fold!(max, self.midi_sources);
        fold!(max, self.clock_epochs);
        fold!(max, self.applied_imports);
        fold!(max, self.recording.sessions);
        fold!(max, self.recording.passes);
        fold!(max, self.recording.streams);
        fold!(max, self.recording.segments);
        fold!(max, self.recording.takes);
        fold!(max, self.comp.take_groups);
        fold!(max, self.comp.comps);
        fold!(max, self.comp.regions);
        fold!(max, self.comp.transitions);
        fold!(max, self.control.targets);
        fold!(max, self.control.lanes);
        fold!(max, self.control.sources);
        fold!(max, self.control.assignments);
        // -- semantic routing model (issue #104) ----------------------
        fold!(max, self.routing.buses);
        fold!(max, self.routing.strips);
        fold!(max, self.routing.processors);
        fold!(max, self.routing.chains);
        fold!(max, self.routing.ports);
        fold!(max, self.routing.connections);
        fold!(max, self.routing.sends);
        max
    }

    /// Rejects a state whose `id_watermark` is not the issuance
    /// position *after* every serialized entity. `load` reseeds the
    /// generator from this field, so a forged low watermark would
    /// re-issue a live ID on the next op — silently overwriting that
    /// entity. Checked only at the load boundary: during `apply` the
    /// just-drawn IDs legitimately sit at/above the still-unstamped
    /// watermark. `project_id` is excluded — it is created outside the
    /// generator. A watermark inside the reserved well-known range
    /// would let the generator re-issue bootstrap constants.
    ///
    /// # Errors
    /// [`Error::Validation`] when the watermark cannot be authoritative.
    pub(crate) fn check_id_watermark(&self) -> Result<(), Error> {
        if self.id_watermark >= WELL_KNOWN_BASE {
            return Err(Error::Validation(format!(
                "id_watermark {} entered the reserved range",
                self.id_watermark
            )));
        }
        if let Some(max) = self.max_issuable_id_raw()
            && self.id_watermark <= max
        {
            return Err(Error::Validation(format!(
                "id_watermark {} does not exceed live entity id {max}",
                self.id_watermark
            )));
        }
        Ok(())
    }
}
