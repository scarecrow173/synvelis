//! Unified error type for the project domain.

use std::fmt;

/// All fallible domain operations return this error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A value that must be non-negative was negative, or an operation
    /// would have produced a negative value.
    Negative,
    /// Checked arithmetic overflowed the `i64`/`u64`/`i128` domain.
    Overflow,
    /// A half-open range was constructed with `end < start`.
    Inverted,
    /// A malformed `"<kind>-<hex>"` stable-ID string.
    InvalidId(String),
    /// A `Command::expected_revision` did not match the live revision.
    StaleRevision {
        /// Revision carried by the command.
        expected: u64,
        /// Revision actually live at apply time.
        actual: u64,
    },
    /// A referenced entity does not exist.
    NotFound(String),
    /// A legal-looking reference points at an entity owned by another
    /// parent (e.g. a segment of a different recording session).
    WrongOwner(String),
    /// A domain state transition is not legal from the current state.
    IllegalTransition(String),
    /// A general invariant violation described in text.
    Validation(String),
    /// Competing automation on the same parameter at the same
    /// evaluation time — detected, never silently prioritized.
    AutomationConflict {
        /// The target with competing series.
        target: String,
        /// Description of the conflicting lanes/ranges.
        detail: String,
    },
    /// A zero-delay cycle in the control graph (e.g. mutual
    /// depth-source references) — rejected at compile/eval.
    ControlCycle {
        /// The detected dependency path.
        path: String,
    },
    /// A scope's time coordinate cannot be resolved for the
    /// evaluation position (clip range outside, media anchor, or an
    /// unimplemented note entity).
    ScopeUnavailable {
        /// Description of the unavailable scope/coordinate.
        detail: String,
    },
    /// The ruviform param-write contract rejected the requested means
    /// (`check_param_write` — capability honesty, never a silent
    /// misapplication).
    WriteRejected {
        /// The write means that was requested.
        means: String,
        /// The typed rejection from the upstream contract.
        rejection: String,
    },
    /// The snapshot schema version is outside the supported set.
    UnsupportedSchema {
        /// The version found in the snapshot.
        found: u32,
    },
    /// Snapshot bytes failed strict parsing.
    Deserialize(String),
    /// A persistence store operation failed.
    Store(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Negative => write!(f, "value must be non-negative"),
            Self::Overflow => write!(f, "arithmetic overflow"),
            Self::Inverted => write!(f, "range end precedes start"),
            Self::InvalidId(s) => write!(f, "invalid stable id: {s}"),
            Self::StaleRevision { expected, actual } => {
                write!(f, "stale revision: expected {expected}, live {actual}")
            }
            Self::NotFound(s) => write!(f, "referenced entity not found: {s}"),
            Self::WrongOwner(s) => write!(f, "reference owned by another parent: {s}"),
            Self::IllegalTransition(s) => write!(f, "illegal transition: {s}"),
            Self::Validation(s) => write!(f, "validation failed: {s}"),
            Self::AutomationConflict { target, detail } => {
                write!(f, "automation conflict on {target}: {detail}")
            }
            Self::ControlCycle { path } => write!(f, "control cycle: {path}"),
            Self::ScopeUnavailable { detail } => {
                write!(f, "control scope unavailable: {detail}")
            }
            Self::WriteRejected { means, rejection } => {
                write!(f, "param write rejected ({means}): {rejection}")
            }
            Self::UnsupportedSchema { found } => {
                write!(f, "unsupported schema version: {found}")
            }
            Self::Deserialize(s) => write!(f, "snapshot parse failed: {s}"),
            Self::Store(s) => write!(f, "store error: {s}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Deserialize(err.to_string())
    }
}
