//! `synvelis-recording`: recording-domain behavior — `RecordingOp`, the
//! session state machine, registration validation, the VS-B Mock
//! Clock/Gate/RT-capture slice, and the VS-C non-RT staging writer
//! (Raw PCM + Journal + Crash Recovery). Contains no persisted project
//! state of its own; the single store is the `Project` snapshot.

#![warn(missing_docs)]

pub mod bounded;
pub mod capture;
pub mod clock;
pub mod commit;
pub mod crc32;
pub mod engine;
pub mod faultinj;
pub mod gate;
pub mod handoff;
pub mod input;
pub mod journal;
pub mod mock;
pub mod ops;
pub mod plan;
pub mod recovery;
pub mod session;
pub mod staging;
pub mod validate;
pub mod writer;

pub use bounded::{BoundedFull, BoundedVec};
pub use capture::{CaptureBlockHeader, CaptureChunk, CaptureSpan, ChunkId, ChunkValidity};
pub use clock::InputBlock;
pub use commit::{
    CanonicalReceipt, ClipEntry, CommitError, CommitIdentity, CommitMarker, CommitOutcome,
    CommitPayload, CommitReceipt, CommitReport, CommitStatus, DigestScheme, PassEntry,
    PreparedCommit, ReceiptStage, RecordingCommitManifest, RecoverableStream, SegmentEntry,
    StreamEntry, StreamFinalization, StreamFinalizeEvidence, TakeEntry, apply_commit,
    build_payload, commit_manifest_path, commit_marker_path, commit_receipt, commit_session,
    commit_status, load_canonical, load_finalized, payload_hash, prepare_manifest,
    read_commit_manifest, read_commit_marker, reconcile_commit_receipt, validate_commit,
};
pub use crc32::{Crc32, IntegrityDigest, crc32, digest_fold};
pub use engine::{CaptureEngine, EngineEndsBack};
pub use faultinj::{
    FaultWriter, Inject, PublishFaults, PublishInject, StorageWriter, corrupt_byte,
};
pub use gate::{CapturePiece, GateError, GateSchedule, GateScheduler, GateSpan};
pub use handoff::{
    ChunkSink, EngineEnds, FAULT_ANY, FAULT_ARITHMETIC, FAULT_CONTRACT, FAULT_EPOCH_DISCONTINUITY,
    FAULT_FREE_EXHAUSTED, FAULT_INPUT_DISCONTINUITY, FAULT_PCM_OVERFLOW, FAULT_QUEUE_FULL,
    FAULT_SCHEDULE_UNDERRUN, FAULT_SPAN_OVERFLOW, FAULT_WRITER_ERROR, FaultLatch, WriterEnds,
    build_handoff,
};
pub use input::{
    ArmState, ChannelMap, InputAdapter, InputAdapterParts, InputBindError, InputBinding,
    InputSession, InputStreamSpec, InputTapSpec, MonitorState, OpenedInput, TapEngineEnds,
    TapRecord, TapWriterEnds,
};
pub use journal::{
    CheckpointRecord, ChunkRecord, FinalizeOutcome, FinalizeRecord, JournalAppender, JournalRecord,
    JournalSpan, RecordPayload, ReplayOutcome, StopReason, encode_record, replay_journal,
};
pub use mock::{ChunkLog, InspectionWriter, MockInput, MockPcm, Scenario, StreamRig, verify_log};
pub use ops::{RecordingOp, SynvelisOp};
pub use plan::{
    BufferBudget, CaptureTarget, GateSpec, PreparedRecordingPlan, RecordingRequest, preflight,
};
pub use recovery::{
    DiscardCandidate, MediaReferenceProbe, RecoveryReport, ReferenceCheck, ReviewAction,
    ReviewDecision, ReviewError, ReviewOutcome, ReviewRecord, ReviewState, SessionRecovery,
    StreamClass, StreamRecovery, review_stream,
};
pub use session::{
    accepts_registration, accepts_take_verification, legal_transition, require_transition,
};
pub use staging::{
    CANONICAL_MEDIA_FORMAT, CANONICAL_SCHEMA_VERSION, CanonicalSourceRef, DirSyncCapability,
    ManifestStream, StagingArea, StagingCapabilities, StagingError, StagingManifest,
    StagingSessionConfig, StagingStreamConfig, canonical_rel, publish_atomic,
    publish_atomic_faultable, review_rel, stream_config, stream_stem,
};
pub use writer::{
    DirSyncState, DurabilityEvidence, DurableFrontier, FinalizedOutcome, FinalizedSourceRef,
    FinalizedSpan, FinishReport, StreamPaths, StreamState, StreamWriter, WriterError,
    checkpoint_all, finish_all, stream_paths,
};
