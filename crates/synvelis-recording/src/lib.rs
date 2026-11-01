//! `synvelis-recording`: recording-domain behavior — `RecordingOp`, the
//! session state machine, registration validation, and the VS-B Mock
//! Clock/Gate/RT-capture slice — implemented against the
//! `synvelis-project` aggregate. Contains no persisted state of its own;
//! the single store is the `Project` snapshot.

#![warn(missing_docs)]

pub mod bounded;
pub mod capture;
pub mod clock;
pub mod engine;
pub mod gate;
pub mod handoff;
pub mod mock;
pub mod ops;
pub mod plan;
pub mod session;
pub mod validate;

pub use bounded::{BoundedFull, BoundedVec};
pub use capture::{CaptureBlockHeader, CaptureChunk, CaptureSpan, ChunkId, ChunkValidity};
pub use clock::InputBlock;
pub use engine::{CaptureEngine, EngineEndsBack};
pub use gate::{CapturePiece, GateError, GateSchedule, GateScheduler, GateSpan};
pub use handoff::{
    EngineEnds, FAULT_ANY, FAULT_ARITHMETIC, FAULT_CONTRACT, FAULT_EPOCH_DISCONTINUITY,
    FAULT_FREE_EXHAUSTED, FAULT_INPUT_DISCONTINUITY, FAULT_PCM_OVERFLOW, FAULT_QUEUE_FULL,
    FAULT_SCHEDULE_UNDERRUN, FAULT_SPAN_OVERFLOW, FAULT_WRITER_ERROR, FaultLatch, WriterEnds,
    build_handoff,
};
pub use mock::{ChunkLog, InspectionWriter, MockInput, MockPcm, Scenario, StreamRig, verify_log};
pub use ops::{RecordingOp, SynvelisOp};
pub use plan::{
    BufferBudget, CaptureTarget, GateSpec, PreparedRecordingPlan, RecordingRequest, preflight,
};
pub use session::{
    accepts_registration, accepts_take_verification, legal_transition, require_transition,
};
