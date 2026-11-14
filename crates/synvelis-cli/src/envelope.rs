//! The versioned `CliEnvelope` — the single JSON document `synvelis-cli`
//! writes to stdout for every invocation under `--format json` (and the
//! single NDJSON line under `--format ndjson`).
//!
//! Contract (AUTX-03 / synvelis#262):
//!
//! - `schema_version` is the envelope schema version (`1`).
//! - `status` is the coarse outcome: `completed`, `proposal-ready`,
//!   `approval-required`, `accepted`, or `failed`. It never claims
//!   `completed` for `Accepted`/`Running`-class outcomes.
//! - `operation_id`, `project_revision`, `result`, `error`, and
//!   `references` are present only when meaningful.
//! - Exactly one envelope per invocation; progress/logging belongs on
//!   stderr and never mixes with stdout.

use serde::Serialize;
use serde_json::Value;

/// The only `schema_version` this build emits.
pub const ENVELOPE_SCHEMA_VERSION: u32 = 1;

/// Coarse outcome status carried by [`CliEnvelope::status`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EnvelopeStatus {
    /// The invoked capability finished (`GatewayOutcome::Completed`).
    Completed,
    /// A non-destructive proposal was prepared (`ProposalReady`).
    ProposalReady,
    /// The operation awaits an approval receipt (`ApprovalRequired`).
    ApprovalRequired,
    /// The operation was accepted for later completion (`Accepted`) —
    /// *not* a completion.
    Accepted,
    /// The invocation failed; `error` carries the typed failure.
    Failed,
}

impl EnvelopeStatus {
    /// The kebab-case wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::ProposalReady => "proposal-ready",
            Self::ApprovalRequired => "approval-required",
            Self::Accepted => "accepted",
            Self::Failed => "failed",
        }
    }
}

/// The single versioned stdout document.
#[derive(Clone, Debug, Serialize)]
pub struct CliEnvelope {
    /// Envelope schema version — always [`ENVELOPE_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Coarse outcome status.
    pub status: &'static str,
    /// Accepted-operation identity (present for `accepted`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// Live project revision observed after the operation, when the
    /// invocation was project-bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_revision: Option<u64>,
    /// The capability result payload (`completed` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// The typed failure (`failed` only) — gateway
    /// [`synvelis_gateway::ExternalError`] shape or a `cli.*` object.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    /// Opaque follow-up references (`proposal`, `approval`,
    /// `operation`) for non-terminal outcomes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<Value>,
}

impl CliEnvelope {
    /// An envelope with the given status and no optional fields.
    #[must_use]
    pub fn bare(status: EnvelopeStatus) -> Self {
        Self {
            schema_version: ENVELOPE_SCHEMA_VERSION,
            status: status.as_str(),
            operation_id: None,
            project_revision: None,
            result: None,
            error: None,
            references: None,
        }
    }

    /// Serializes the envelope as a single JSON line (no trailing
    /// newline — the emitter adds it).
    ///
    /// # Panics
    /// Never in practice: every field is plain JSON data.
    #[must_use]
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("CliEnvelope always serializes")
    }
}
