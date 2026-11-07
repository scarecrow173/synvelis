//! Error types for the `synvelis-theme` crate.

use std::fmt;

/// Kind of a single envelope-level validation problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemKind {
    /// The required `$extensions["synvelis.productTheme"]` envelope is
    /// missing, malformed, or violates the product profile (empty
    /// theme/mode/density, wrong layer tag).
    MissingMeta,
    /// The document or preferences schema/version is unsupported.
    Schema,
}

/// A single envelope-level validation problem with its JSON path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenProblem {
    /// Dotted JSON path of the offending field (e.g.
    /// `$extensions.synvelis.productTheme.mode`).
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
    /// Document is not valid JSON / not a JSON object at root, or a
    /// schema version is unsupported.
    Schema(String),
    /// One or more envelope-level validation problems were found.
    Validation(Vec<TokenProblem>),
    /// Filesystem or JSON parse failure while loading a document.
    Io(String),
    /// A real runtime capability is unsupported (e.g. a resolved-theme
    /// feature a driver cannot provide). This is **not** used for
    /// upstream-contract problems — resolution failures are reported
    /// through [`ThemeError::Resolve`].
    Unsupported(String),
    /// The upstream `ruviform-ui-theme` (`rvf-dtcg-profile.v1`)
    /// pipeline rejected a document or a merge: document-level
    /// problems surface as `ResolveError::Document(TokenError)`
    /// (`MissingProfile`, `InvalidValue`, `ReservedKey`, …) and
    /// merge-level problems as the other `ResolveError` variants
    /// (`MissingMode`, `TypeMismatch`, `UnknownReference`,
    /// `AliasCycle`, `DuplicateSource`, `EmptyDocuments`).
    Resolve(ruviform_ui_theme::ResolveError),
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
            ThemeError::Resolve(e) => write!(f, "theme resolution failed: {e}"),
        }
    }
}

impl std::error::Error for ThemeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ThemeError::Resolve(e) => Some(e),
            _ => None,
        }
    }
}
