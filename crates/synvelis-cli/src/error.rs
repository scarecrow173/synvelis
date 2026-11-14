//! CLI-originated typed errors.
//!
//! Gateway failures arrive as [`synvelis_gateway::ErrorEnvelope`]
//! already; this enum covers failures the CLI itself produces before
//! or outside the gateway pipeline (argument parsing, file
//! classification, project boot). Each variant serializes into the
//! same envelope `error` shape as gateway errors — `code`, `category`,
//! `retry`, `message` — with `cli.*`-namespaced codes so consumers can
//! distinguish adapter-side rejection from gateway-side rejection.
//!
//! `category`/`retry` reuse the gateway [`ErrorCategory`] /
//! [`RetryAdvice`] vocabularies; the authoritative signal remains the
//! process exit code ([`crate::exitcode`]).

/// A failure produced by the CLI adapter itself (not the gateway).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliError {
    /// Malformed invocation: unknown command/flag, missing required
    /// option, unparsable value. Exit 2.
    Usage(String),
    /// Well-formed invocation rejected by CLI-side validation: unknown
    /// file extension, file/subcommand kind mismatch, missing project
    /// resource. Exit 2.
    InvalidInput(String),
    /// Recognized input whose execution path is not wired in this
    /// build (script backends, render provider, deferred workflow
    /// execution). Exit 5.
    Unsupported(String),
    /// Environment/store could not be opened or read (project bundle,
    /// policy log, commit journal, IO). Exit 9.
    Setup(String),
    /// An invariant inside the adapter failed. Exit 10.
    Internal(String),
}

impl CliError {
    /// The process exit code for this error (see [`crate::exitcode`]).
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) | Self::InvalidInput(_) => 2,
            Self::Unsupported(_) => 5,
            Self::Setup(_) => 9,
            Self::Internal(_) => 10,
        }
    }

    /// The `cli.*`-namespaced error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Usage(_) => "cli.usage",
            Self::InvalidInput(_) => "cli.invalid-input",
            Self::Unsupported(_) => "cli.unsupported",
            Self::Setup(_) => "cli.setup-failed",
            Self::Internal(_) => "cli.internal",
        }
    }

    /// The message and the matching gateway-vocabulary category/retry
    /// pair for the envelope `error` object.
    #[must_use]
    pub fn parts(&self) -> (&'static str, &'static str, &'static str, &str) {
        match self {
            Self::Usage(m) => ("cli.usage", "validation", "never", m),
            Self::InvalidInput(m) => ("cli.invalid-input", "validation", "after-correction", m),
            Self::Unsupported(m) => ("cli.unsupported", "capability", "never", m),
            Self::Setup(m) => ("cli.setup-failed", "internal", "transient", m),
            Self::Internal(m) => ("cli.internal", "internal", "never", m),
        }
    }

    /// Serializes into the envelope `error` value — same field names as
    /// the gateway [`synvelis_gateway::ExternalError`] projection.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        let (code, category, retry, message) = self.parts();
        serde_json::json!({
            "code": code,
            "category": category,
            "retry": retry,
            "message": message,
        })
    }

    /// Human-readable one-line rendering for stderr/text mode.
    #[must_use]
    pub fn display(&self) -> String {
        let (_, _, _, message) = self.parts();
        format!("{message} (exit {})", self.exit_code())
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display())
    }
}

impl std::error::Error for CliError {}
