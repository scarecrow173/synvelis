//! `synvelis-project`: the Synvelis project aggregate and its single
//! writer, [`ProjectController`].
//!
//! Owns every persisted value type (stable IDs, time types, ranges,
//! tempo map snapshot, project entities, recording registry POD data),
//! the [`Operation`]/[`Command`] contract, revision/undo semantics, and
//! the [`ProjectStore`] persistence port. Recording-domain *behavior*
//! (ops, transition rules, registration validation) lives in
//! `synvelis-recording`, which depends on this crate — never the reverse.

#![warn(missing_docs)]

pub mod comp;
pub mod controller;
pub mod error;
pub mod id;
pub mod ops;
pub mod placement;
pub mod project;
pub mod range;
pub mod recording_data;
pub mod store;
pub mod tempo;
pub mod time;

pub use comp::{
    Comp, CompCandidateRef, CompOp, CompProvenance, CompRegion, CompRegistry, CompTransition,
    EditorDisplayTarget, EditorEditingTarget, EffectiveComp, EffectiveSegment, EffectiveTransition,
    FadeShape, SourceBinding, SourceVersionRef, TakeGroup, project_comp,
};
pub use controller::{Command, Operation, ProjectController, ProjectRevision, SavedState};
pub use error::Error;
pub use id::{
    AssetId, CaptureSegmentId, CaptureStreamId, ClipId, ClockEpochId, CompId, CompRegionId,
    CompTransitionId, CounterIdGenerator, IdGenerator, ImportId, LaneId, ProjectId,
    RecordingPassId, RecordingSessionId, SourceId, TakeGroupId, TakeId, TrackId,
};
pub use ops::ProjectOp;
pub use placement::{
    ClipExtent, ClipPlacement, ClipPlayback, PlaybackMapping, ResolvedClip, StretchRatio,
    TimeAnchor,
};
pub use project::{
    AppliedImport, AppliedRecordingCommit, Asset, Clip, Lane, Project, ProjectDomain, ProjectRange,
    RecordingRegistry, Source, SourceState, Track,
};
pub use range::{MediaSpan, MusicalRange, Range, SessionRange, SourceRange};
pub use recording_data::{
    CaptureSegment, CaptureStream, ClockEpoch, RecordingPass, RecordingSession, SessionState, Take,
    TakeState,
};
pub use store::{
    DirectoryBundleStore, InMemoryStore, ProjectStore, SCHEMA_VERSION, SnapshotEnvelope,
    snapshot_name,
};
pub use tempo::{
    ProjectPpqn, SignatureDto, TempoEventDto, TempoMapData, TempoMapRef, TickTimeMapping,
};
pub use time::{
    MediaDuration, MediaTimeValue, MusicalDuration, MusicalTick, ProjectTime, SessionFrame,
    SourceFrame,
};
