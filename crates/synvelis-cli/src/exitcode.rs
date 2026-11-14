//! Deterministic outcome → process exit-code mapping (AUTX-03).
//!
//! | Code | Meaning |
//! | --- | --- |
//! | 0 | `Completed` (also `ProposalReady`/`ApprovalRequired` — the
//!   invoked capability finished its synchronous work; the proposal /
//!   approval lifecycle continues under separate invocations) |
//! | 2 | `InvalidInput` — malformed request/arguments/targets |
//! | 3 | `Denied` — authentication, authorization, approval, or
//!   self-declared-authority rejection |
//! | 4 | `Conflict` — revision/idempotency/digest/proposal-state
//!   conflict, stale pinned state |
//! | 5 | `Unsupported` — unknown capability/schema/version, missing
//!   executor, unavailable capability, unwired script backend |
//! | 6 | `ExecutionFailure` — preparation rejected against the pinned
//!   base state |
//! | 7 | `Indeterminate` — outcome cannot be determined; `Accepted`
//!   also lands here because the terminal result is still pending |
//! | 8 | `ConfirmedCancelled` — reserved for confirmed-cancel
//!   verdicts (no cancel verb exists in this slice) |
//! | 9 | `Unavailable`/`SetupFailure` — project/store/journal/policy
//!   could not be opened |
//! | 10 | `InternalError` — internal invariant failure |
//!
//! `Accepted`/`Running`-class outcomes are never reported as
//! `completed`/exit 0: an accepted async operation maps to `accepted`
//! status + exit 7 so automation cannot mistake submission for
//! completion. Status queries and wait results stay distinct — there
//! is no `--wait` in this slice.

use synvelis_gateway::{GatewayError, GatewayOutcome};

use crate::envelope::EnvelopeStatus;

/// Maps a [`GatewayOutcome`] to its envelope status and exit code.
///
/// Only [`GatewayOutcome::Completed`] yields exit 0 —
/// `ProposalReady`/`ApprovalRequired` are also successful *responses*
/// (the invoked capability finished; follow-up lives in the proposal
/// store), while `Accepted` is explicitly non-terminal → 7.
#[must_use]
pub fn outcome_exit(outcome: &GatewayOutcome<serde_json::Value>) -> (EnvelopeStatus, u8) {
    match outcome {
        GatewayOutcome::Completed { .. } => (EnvelopeStatus::Completed, 0),
        GatewayOutcome::ProposalReady { .. } => (EnvelopeStatus::ProposalReady, 0),
        GatewayOutcome::ApprovalRequired { .. } => (EnvelopeStatus::ApprovalRequired, 0),
        GatewayOutcome::Accepted { .. } => (EnvelopeStatus::Accepted, 7),
    }
}

/// Maps a [`GatewayError`] to the deterministic exit code.
///
/// The mapping is per-variant (not per-category) so ambiguous
/// category members land where their retry semantics belong:
/// `ConsistencyNotAllowed`/`CursorInvalid` are request-shape problems
/// (2), while stale pinned state (`RevisionConflict`,
/// `SnapshotExpired`, `CursorExpired`, `DigestMismatch`,
/// `ProposalUnavailable`, `IdempotencyConflict`, `DuplicateRequest`)
/// is a state conflict (4).
#[must_use]
pub fn error_exit(error: &GatewayError) -> u8 {
    use GatewayError as E;
    match error {
        // ── 2 InvalidInput: request/argument/target/schema-shape ──
        E::MalformedRequest { .. }
        | E::ImplicitTargetRejected { .. }
        | E::InvalidId { .. }
        | E::UnknownResourceKind { .. }
        | E::WrongResourceKind { .. }
        | E::WrongTimeDomain { .. }
        | E::TargetRequired
        | E::UnexpectedTarget
        | E::TargetShapeMismatch { .. }
        | E::SchemaViolation { .. }
        | E::ConsistencyNotAllowed { .. }
        | E::CursorInvalid { .. }
        | E::LimitExceeded { .. } => 2,
        // ── 3 Denied: caller/authN/authZ/approval/self-authority ──
        E::SelfDeclaredAuthority { .. }
        | E::Authentication
        | E::Denied { .. }
        | E::ApprovalInvalid { .. } => 3,
        // ── 4 Conflict: pinned/live state disagreement ──
        E::RevisionConflict { .. }
        | E::SnapshotExpired
        | E::CursorExpired
        | E::DigestMismatch { .. }
        | E::ProposalUnavailable { .. }
        | E::DuplicateRequest { .. }
        | E::IdempotencyConflict => 4,
        // ── 5 Unsupported: resolution/executor/availability ──
        E::UnknownCapability { .. }
        | E::UnsupportedCapabilityVersion { .. }
        | E::UnknownSchema { .. }
        | E::UnsupportedSchemaVersion { .. }
        | E::IncompatibleSchema { .. }
        | E::Unavailable { .. }
        | E::CapabilityConflict { .. }
        | E::IncoherentDescriptor { .. }
        | E::SchemaConflict { .. }
        | E::NoExecutor { .. } => 5,
        // ── 6 ExecutionFailure: rejected at preparation ──
        E::PreparationRejected { .. } => 6,
        // ── 7 Indeterminate: recovery-class ambiguity ──
        E::Indeterminate { .. } => 7,
        // ── 10 InternalError ──
        E::Internal { .. } => 10,
    }
}

/// Exit code for a gateway [`synvelis_gateway::ErrorEnvelope`].
#[must_use]
pub fn envelope_exit(error: &synvelis_gateway::ErrorEnvelope) -> u8 {
    error_exit(&error.code)
}
