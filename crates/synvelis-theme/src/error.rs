//! Error types for the `synvelis-theme` crate.

use std::fmt;

/// Kind of a single token-level validation problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemKind {
    /// The token's `$type` is not in the accepted initial profile.
    UnknownType,
    /// The token's `$value` does not match the shape required by its
    /// resolved `$type`.
    BadValue,
    /// An alias reference is syntactically malformed (unbalanced or
    /// nested braces, empty target). Target existence is not checked
    /// — a reference may legitimately live in another document/layer;
    /// resolution is the interpreter's responsibility.
    BadRef,
    /// A same-document alias cycle was detected.
    CyclicRef,
    /// The required `$extensions["synvelis.productTheme"]` envelope is
    /// missing or malformed.
    MissingMeta,
    /// The document schema/version is unsupported.
    Schema,
}

/// A single token-level validation problem with its JSON path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenProblem {
    /// Dotted JSON path of the offending token/group (e.g. `color.bg`).
    pub path: String,
    /// Classification of the problem.
    pub kind: ProblemKind,
    /// Human-readable detail.
    pub detail: String,
}

impl fmt::Display for TokenProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}: {}", self.path, self.kind, self.detail)
    }
}

/// Error type for all `synvelis-theme` operations.
#[derive(Debug)]
pub enum ThemeError {
    /// Document is not valid JSON / not a JSON object at root, or the
    /// envelope schema is unsupported.
    Schema(String),
    /// One or more token-level validation problems were found.
    Validation(Vec<TokenProblem>),
    /// Filesystem or JSON parse failure while loading a document.
    Io(String),
    /// A dependency contract is not yet available (e.g. the ruviform
    /// #385 `ResolvedTheme` seam) — the caller must not treat this as
    /// a silent fallback.
    Unsupported(String),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::Schema(msg) => write!(f, "theme schema error: {msg}"),
            ThemeError::Validation(problems) => {
                write!(f, "theme validation failed ({} problem(s))", problems.len())
            }
            ThemeError::Io(msg) => write!(f, "theme I/O error: {msg}"),
            ThemeError::Unsupported(msg) => write!(f, "theme unsupported: {msg}"),
        }
    }
}

impl std::error::Error for ThemeError {}
