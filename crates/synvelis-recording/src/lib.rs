//! `synvelis-recording`: recording-domain behavior — `RecordingOp`, the
//! session state machine, and registration validation — implemented
//! against the `synvelis-project` aggregate. Contains no persisted state
//! of its own; the single store is the `Project` snapshot.

#![warn(missing_docs)]

pub mod ops;
pub mod session;
pub mod validate;

pub use ops::{RecordingOp, SynvelisOp};
pub use session::{
    accepts_registration, accepts_take_verification, legal_transition, require_transition,
};
