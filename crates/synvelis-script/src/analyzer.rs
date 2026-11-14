//! Analyzer port — a thin consumer-side boundary mirroring the
//! RF-SCRIPT-00 `ScriptAnalyzer` contract shape.
//!
//! This module defines *types and a trait only*: no Ruviform dependency,
//! no Luau.Analysis FFI, no VM. [`DefinitionInjection`] adapts a generated
//! [`crate::luau::LuauStub`] into the `type_definitions` slot of an
//! [`AnalyzeRequest`]. Real analyzer implementations (e.g. the Ruviform
//! Luau bridge) satisfy [`ScriptAnalyzer`]; fakes satisfy it in tests.
//!
//! Contract vocabulary preserved from RF-SCRIPT-00:
//!
//! - `source_revision` is carried on every result; stale rejection is the
//!   **consumer's** job via [`verify_result_revision`] (F-14).
//! - `AnalyzerError::Unsupported` for unimplemented capabilities (F-13),
//!   `Unavailable` for missing analyzers (F-18), `StaleResult` for
//!   revision mismatch.
//! - Analyzer success is **never** execution permission — it is a
//!   static-check signal only; Gateway authorization is unchanged.

use std::fmt;

use crate::luau::LuauStub;

/// Consumer-managed source revision — an opaque monotonically
/// increasing value bumped on every edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceRevision(pub u64);

/// Language identifier (contract `ScriptLanguageId` mirror).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScriptLanguageId(pub String);

impl ScriptLanguageId {
    /// The Luau language id — the standard required backend.
    #[must_use]
    pub fn luau() -> Self {
        Self("luau".to_owned())
    }
}

impl fmt::Display for ScriptLanguageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Diagnostic depth an analyzer may advertise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticLevel {
    /// No analysis capability.
    Unsupported,
    /// Syntax diagnostics only.
    Syntax,
    /// Syntax + type diagnostics.
    SyntaxAndType,
}

/// Capabilities an analyzer advertises.
#[derive(Debug, Clone)]
pub struct AnalyzerCapabilities {
    /// Diagnostics depth.
    pub diagnostics: DiagnosticLevel,
    /// Whether completion is provided.
    pub completion: bool,
    /// Maximum accepted source bytes (0 = unlimited).
    pub max_source_bytes: u64,
    /// Supported languages.
    pub supported_languages: Vec<ScriptLanguageId>,
}

/// Backend version: implementation version and contract version are
/// separate; a contract-version mismatch fails closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptBackendVersion {
    /// Implementation version (semver or opaque string).
    pub impl_version: String,
    /// Contract compatibility version.
    pub contract_version: u32,
}

/// Static description of an analyzer implementation.
#[derive(Debug, Clone)]
pub struct AnalyzerDescriptor {
    /// Analyzer implementation id.
    pub analyzer_id: String,
    /// Target language.
    pub language: ScriptLanguageId,
    /// Version (impl/contract separated).
    pub version: ScriptBackendVersion,
    /// Advertised capabilities.
    pub capabilities: AnalyzerCapabilities,
}

/// Capabilities requested for one analysis.
#[derive(Debug, Clone, Default)]
pub struct WantedCapabilities {
    /// Requested diagnostics depth (`None` = not needed).
    pub diagnostics: Option<DiagnosticLevel>,
    /// Whether completion is requested.
    pub completion: bool,
}

/// One consumer-provided type definition package (a `.d.luau`-style
/// source blob with a package name).
#[derive(Debug, Clone)]
pub struct TypeDefinition {
    /// Package name (Luau packageName equivalent).
    pub package_name: String,
    /// Definition source text.
    pub source: String,
}

/// UTF-8 byte offset into `ScriptSource::text`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Byte offset.
    pub byte_offset: usize,
}

/// The script source under analysis.
#[derive(Debug, Clone)]
pub struct ScriptSource {
    /// UTF-8 source text.
    pub text: String,
    /// Language id.
    pub language: ScriptLanguageId,
    /// Consumer-managed revision.
    pub revision: SourceRevision,
}

/// An analysis request — source + injected definitions + wanted
/// capabilities + optional completion cursor.
#[derive(Debug, Clone)]
pub struct AnalyzeRequest {
    /// Source under analysis.
    pub source: ScriptSource,
    /// Language (must match `source.language`).
    pub language: ScriptLanguageId,
    /// Injected type definitions.
    pub type_definitions: Vec<TypeDefinition>,
    /// Wanted capabilities.
    pub wanted: WantedCapabilities,
    /// Completion cursor (`None` = no completion requested).
    pub cursor: Option<Cursor>,
}

/// 0-based line / byte-column position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    /// 0-based line.
    pub line: u32,
    /// Byte column within the line.
    pub column: u32,
}

/// A source range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRange {
    /// Start position.
    pub start: SourcePosition,
    /// End position.
    pub end: SourcePosition,
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticSeverity {
    /// Informational.
    Information,
    /// Warning.
    Warning,
    /// Error.
    Error,
}

/// Diagnostic kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticKind {
    /// Syntax (parse) error.
    Syntax,
    /// Type error.
    Type,
    /// Other (lint etc.).
    Other,
}

/// A structured diagnostic carrying the analyzed `source_revision`.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Severity.
    pub severity: DiagnosticSeverity,
    /// Kind.
    pub kind: DiagnosticKind,
    /// Machine-readable code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Source range.
    pub range: SourceRange,
    /// The revision this diagnostic was computed against.
    pub source_revision: SourceRevision,
}

/// Completion kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompletionKind {
    /// Property.
    Property,
    /// Binding (variable/function).
    Binding,
    /// Keyword.
    Keyword,
    /// String.
    String,
    /// Type.
    Type,
    /// Module.
    Module,
    /// Generated function.
    GeneratedFunction,
    /// Require path.
    RequirePath,
    /// Hot comment.
    HotComment,
    /// Other.
    Other,
}

/// A completion candidate carrying the analyzed `source_revision`.
#[derive(Debug, Clone)]
pub struct Completion {
    /// Candidate label.
    pub label: String,
    /// Kind.
    pub kind: CompletionKind,
    /// Detail (e.g. type string).
    pub detail: Option<String>,
    /// Insert text.
    pub insert_text: Option<String>,
    /// Replacement range.
    pub replace_range: SourceRange,
    /// The revision completions were computed against.
    pub source_revision: SourceRevision,
}

/// Analysis result — always carries `source_revision`; `completions` is
/// `Some` only when requested.
#[derive(Debug, Clone)]
pub struct AnalyzeResult {
    /// The analyzed revision.
    pub source_revision: SourceRevision,
    /// Diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Completions (only when requested).
    pub completions: Option<Vec<Completion>>,
}

/// Analyzer-port errors — the RF-SCRIPT-00 fail-closed vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnalyzerError {
    /// No analyzer registered/available (F-18).
    Unavailable,
    /// Requested capability not implemented (F-13).
    Unsupported {
        /// Capability name (e.g. `"completion"`).
        capability: String,
    },
    /// `source_revision` mismatch — consumer-side stale rejection (F-14).
    StaleResult,
    /// Malformed request (UTF-8/cursor/size limits).
    InvalidRequest {
        /// Reason.
        reason: String,
    },
    /// ABI/contract version mismatch.
    VersionMismatch {
        /// Mismatched field.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// Internal analyzer failure.
    Internal {
        /// Reason.
        message: String,
    },
}

impl fmt::Display for AnalyzerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => write!(f, "analyzer unavailable"),
            Self::Unsupported { capability } => {
                write!(f, "unsupported analyzer capability: {capability}")
            }
            Self::StaleResult => write!(f, "stale analyzer result"),
            Self::InvalidRequest { reason } => write!(f, "invalid analyzer request: {reason}"),
            Self::VersionMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "analyzer version mismatch on {field}: expected {expected}, got {actual}"
            ),
            Self::Internal { message } => write!(f, "analyzer internal error: {message}"),
        }
    }
}

impl std::error::Error for AnalyzerError {}

/// The analyzer port — static analysis only, independent of any runtime
/// backend or authorization decision.
pub trait ScriptAnalyzer {
    /// The analyzer's static descriptor.
    fn descriptor(&self) -> &AnalyzerDescriptor;

    /// Analyzes `request` (source + injected definitions).
    ///
    /// # Errors
    /// [`AnalyzerError`] — unavailable / unsupported / invalid /
    /// version-mismatch / internal. Never returns a result for a stale
    /// revision silently — consumers verify via
    /// [`verify_result_revision`].
    fn analyze(&self, request: &AnalyzeRequest) -> Result<AnalyzeResult, AnalyzerError>;
}

/// Consumer-side stale-result rejection (F-14): the result must carry
/// the same `source_revision` it was requested against.
///
/// # Errors
/// [`AnalyzerError::StaleResult`] — revision mismatch.
pub fn verify_result_revision(
    result: &AnalyzeResult,
    expected: SourceRevision,
) -> Result<(), AnalyzerError> {
    if result.source_revision == expected {
        Ok(())
    } else {
        Err(AnalyzerError::StaleResult)
    }
}

/// Definition-injection adapter: packages a generated [`LuauStub`] as
/// the `type_definitions` input an analyzer consumes.
///
/// Carries the stub's `model_digest` so consumers can detect analyzing
/// against a stale definition set (source/stub revision drift → the
/// analyzer's `StaleResult`/version-mismatch vocabulary applies).
#[derive(Debug, Clone)]
pub struct DefinitionInjection {
    /// Package name for the injected definitions.
    pub package_name: String,
    /// The generated stub text.
    pub source: String,
    /// Model digest the definitions were generated from.
    pub model_digest: String,
    /// Stub text digest.
    pub stub_digest: String,
}

impl DefinitionInjection {
    /// Package name used for synvelis-script generated definitions.
    pub const PACKAGE_NAME: &'static str = "synvelis-script";

    /// Builds an injection from a generated stub.
    #[must_use]
    pub fn from_stub(stub: &LuauStub) -> Self {
        Self {
            package_name: Self::PACKAGE_NAME.to_owned(),
            source: stub.text.clone(),
            model_digest: stub.model_digest.clone(),
            stub_digest: stub.stub_digest.clone(),
        }
    }

    /// The [`TypeDefinition`] form for [`AnalyzeRequest::type_definitions`].
    #[must_use]
    pub fn type_definition(&self) -> TypeDefinition {
        TypeDefinition {
            package_name: self.package_name.clone(),
            source: self.source.clone(),
        }
    }
}
