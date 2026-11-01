//! Recording-domain persisted entities (POD only; behavior lives in
//! `synvelis-recording`). These are part of the `Project` aggregate's
//! serialized state per the persistence architecture (#17), keeping a
//! single store, single writer, and full undo coverage.

use serde::{Deserialize, Serialize};

use crate::{
    id::{
        CaptureSegmentId, CaptureStreamId, ClockEpochId, LaneId, RecordingPassId,
        RecordingSessionId, SourceId, TakeId,
    },
    range::{SessionRange, SourceRange},
};

/// Recording session lifecycle. `Created → Open → Closed → Finalized` is
/// the only legal forward path (enforced by `synvelis-recording`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum SessionState {
    /// Session exists but is not yet accepting registrations.
    Created,
    /// Accepting pass/stream/segment/take registrations.
    Open,
    /// Registration closed; takes may still be marked verified.
    Closed,
    /// Finalized; immutable for recording purposes.
    Finalized,
}

/// A recording session (one recording engagement's registry root).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingSession {
    /// Stable ID.
    pub id: RecordingSessionId,
    /// Display name (immutable in F0).
    pub name: String,
    /// Lifecycle state.
    pub state: SessionState,
}

/// A pass within a session (one take round).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingPass {
    /// Stable ID.
    pub id: RecordingPassId,
    /// Owning session.
    pub session_id: RecordingSessionId,
    /// Ordinal, unique within the session.
    pub index: u32,
}

/// One capture stream bound to a lane and a clock epoch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureStream {
    /// Stable ID.
    pub id: CaptureStreamId,
    /// Owning session.
    pub session_id: RecordingSessionId,
    /// Lane this stream feeds.
    pub lane_id: LaneId,
    /// Clock epoch the session frames are measured in.
    pub clock_epoch_id: ClockEpochId,
}

/// A contiguous captured interval: session-time span ↔ source-time span.
/// Both ranges must be non-empty (validated on registration).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureSegment {
    /// Stable ID.
    pub id: CaptureSegmentId,
    /// Stream that produced it (also determines the session).
    pub stream_id: CaptureStreamId,
    /// Pass during which it was captured.
    pub pass_id: RecordingPassId,
    /// Session-time span `[start, end)`.
    pub session_range: SessionRange,
    /// Source-time span `[start, end)`.
    pub source_range: SourceRange,
    /// Media source holding the captured material.
    pub source_id: SourceId,
}

/// Take lifecycle relative to media verification.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum TakeState {
    /// Registered but media not yet verified.
    Raw,
    /// Media verified (separate stage from project save/finalize).
    Verified,
}

/// A take: the user-facing recording result referencing segments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Take {
    /// Stable ID.
    pub id: TakeId,
    /// Owning session.
    pub session_id: RecordingSessionId,
    /// Pass in which it was taken.
    pub pass_id: RecordingPassId,
    /// Stream that produced it.
    pub stream_id: CaptureStreamId,
    /// Lane it occupies.
    pub lane_id: LaneId,
    /// Media source it reads from.
    pub source_id: SourceId,
    /// Segments composing it (non-empty, all in the same session).
    pub segment_ids: Vec<CaptureSegmentId>,
    /// Verification state.
    pub state: TakeState,
}

/// A clock epoch marker (device/timeline clock identity for streams).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockEpoch {
    /// Stable ID.
    pub id: ClockEpochId,
    /// Human-readable description of the clock source.
    pub description: String,
}
