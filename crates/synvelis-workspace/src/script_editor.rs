//! Script Editor document model, analyzer-status projection, and
//! view-local state (synvelis#274 / SV-SCRIPT-04).
//!
//! [`ScriptEditorDocument`] is the driver-neutral editor read model:
//! it owns the source text under a monotonically increasing
//! [`SourceRevision`], the pending analysis requests keyed by that
//! revision, the accepted diagnostics/completions projection, the
//! analyzer status vocabulary, and the `ScriptApiModel` verdict rows.
//! It consumes the `synvelis-script` analyzer port
//! ([`AnalyzeRequest`]/[`AnalyzeResult`]/[`AnalyzerError`]) and the
//! `synvelis-automation` [`AnalyzerRegistry`] resolution — it never
//! implements analysis itself and never becomes project/grant/job
//! truth.
//!
//! Stale rejection is consumer-side (F-14): a result is accepted only
//! when its `source_revision` equals the *current* document revision
//! **and** a pending request exists for that revision **and** the
//! definition digests pinned at request time still match the current
//! injection. Anything else is discarded as stale — a delayed
//! asynchronous response can never overwrite newer state.
//!
//! [`ScriptEditorViewState`] is the per-view persisted slice (row
//! selections only) carried in [`crate::ViewState`]; the document
//! itself is runtime state owned by the composition layer.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use synvelis_script::{
    AnalyzeRequest, AnalyzeResult, AnalyzerDescriptor, AnalyzerError, Completion, CompletionKind,
    Cursor, DefinitionInjection, Diagnostic, DiagnosticKind, DiagnosticSeverity, ScriptLanguageId,
    ScriptSource, SourceRevision, TypeDefinition, WantedCapabilities, verify_result_revision,
};

/// Digest pin of the type definitions an analysis was computed
/// against — model/stub drift makes in-flight results stale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefinitionPins {
    /// `ScriptApiModel` digest the stub was generated from.
    pub model_digest: String,
    /// `LuauStub` text digest.
    pub stub_digest: String,
}

impl DefinitionPins {
    /// Pins from a [`DefinitionInjection`].
    #[must_use]
    pub fn from_injection(injection: &DefinitionInjection) -> Self {
        Self {
            model_digest: injection.model_digest.clone(),
            stub_digest: injection.stub_digest.clone(),
        }
    }
}

/// Display row for one diagnostic — plain data so app/theme layers
/// never see `synvelis-script` types.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptDiagnosticRow {
    /// `"error"` / `"warning"` / `"information"` / `"other"`.
    pub severity: String,
    /// `"syntax"` / `"type"` / `"other"`.
    pub kind: String,
    /// Machine-readable diagnostic code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// 0-based start line.
    pub line: u32,
    /// 0-based start byte column.
    pub column: u32,
    /// 0-based end line.
    pub end_line: u32,
    /// 0-based end byte column.
    pub end_column: u32,
    /// Revision the diagnostic was computed against.
    pub source_revision: u64,
}

/// Display row for one completion candidate.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptCompletionRow {
    /// Candidate label.
    pub label: String,
    /// Completion kind label (`"generated-function"`, …).
    pub kind: String,
    /// Detail text (signature etc.).
    pub detail: Option<String>,
    /// Replacement range start (line, column).
    pub replace_line: u32,
    /// Replacement range start column.
    pub replace_column: u32,
    /// Replacement range end (line, column).
    pub end_line: u32,
    /// Replacement range end column.
    pub end_column: u32,
    /// Revision the completion was computed against.
    pub source_revision: u64,
}

/// Display row for one public script-API entry — the verdict surface
/// that shows `Invocable`/`Disabled`/`Planned`/`Retired`/… without
/// leaking `Hidden` entries.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptApiRow {
    /// Script-visible API name.
    pub name: String,
    /// Verdict label (`"invocable"`, `"disabled"`, …).
    pub verdict: String,
    /// Verdict detail (reason / supported versions / …).
    pub detail: Option<String>,
    /// Exact capability ref (`id@major.minor`).
    pub capability: String,
    /// Optional description.
    pub description: Option<String>,
}

fn severity_label(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Information => "information",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Error => "error",
        _ => "other",
    }
}

fn kind_label(kind: DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::Syntax => "syntax",
        DiagnosticKind::Type => "type",
        _ => "other",
    }
}

fn completion_kind_label(kind: CompletionKind) -> &'static str {
    match kind {
        CompletionKind::Property => "property",
        CompletionKind::Binding => "binding",
        CompletionKind::Keyword => "keyword",
        CompletionKind::String => "string",
        CompletionKind::Type => "type",
        CompletionKind::Module => "module",
        CompletionKind::GeneratedFunction => "generated-function",
        CompletionKind::RequirePath => "require-path",
        CompletionKind::HotComment => "hot-comment",
        _ => "other",
    }
}

fn name_rejection_label(reason: synvelis_script::NameRejection) -> &'static str {
    use synvelis_script::NameRejection::{Empty, InvalidIdentifier, ReservedWord, TooLong};
    match reason {
        Empty => "empty",
        TooLong => "too-long",
        InvalidIdentifier => "invalid-identifier",
        ReservedWord => "reserved-word",
    }
}

fn verdict_label(verdict: &synvelis_script::ScriptVerdict) -> (&'static str, Option<String>) {
    use synvelis_script::ScriptVerdict::{
        Disabled, InvalidName, Invocable, Planned, Retired, UnknownCapability, UnresolvedSchema,
        UnsupportedVersion,
    };
    match verdict {
        Invocable => ("invocable", None),
        Disabled { reason } => ("disabled", Some(reason.clone())),
        Planned => ("planned", None),
        Retired => ("retired", None),
        UnsupportedVersion { supported } => (
            "unsupported-version",
            Some(
                supported
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        ),
        UnknownCapability => ("unknown-capability", None),
        UnresolvedSchema { schema } => ("unresolved-schema", Some(schema.clone())),
        InvalidName { reason } => (
            "invalid-name",
            Some(name_rejection_label(*reason).to_owned()),
        ),
    }
}

/// Projects the public surface of a [`synvelis_script::ScriptApiModel`]
/// into display rows. `Hidden` entries are never projected — the same
/// predicate the stub/binding generators use.
#[must_use]
pub fn project_script_api(model: &synvelis_script::ScriptApiModel) -> Vec<ScriptApiRow> {
    model
        .list_public()
        .into_iter()
        .map(|entry| {
            let (verdict, detail) = verdict_label(&entry.verdict);
            ScriptApiRow {
                name: entry.name.clone(),
                verdict: verdict.to_owned(),
                detail,
                capability: entry.requested.to_string(),
                description: entry.description.clone(),
            }
        })
        .collect()
}

/// Analyzer status vocabulary — the display projection of
/// availability/capability/stale states. Distinct from runtime/VM
/// startup status: an unavailable analyzer says nothing about whether
/// the runtime can run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ScriptAnalyzerStatus {
    /// No analysis has run yet (or no analyzer resolved).
    #[default]
    Idle,
    /// Last analysis accepted at the current revision + definitions.
    Ready {
        /// Analyzer implementation id.
        analyzer_id: String,
        /// Analyzer implementation version.
        impl_version: String,
    },
    /// Document text, cursor inputs, or definitions changed since the
    /// last accepted analysis — a re-run is needed.
    Dirty,
    /// An asynchronous result was rejected as stale (revision or
    /// definition-digest drift).
    Stale,
    /// No analyzer registered for the document language (F-18).
    Unavailable,
    /// The analyzer does not advertise a wanted capability (F-13).
    Unsupported {
        /// Capability name.
        capability: String,
    },
    /// The analyzer rejected the request itself
    /// (`InvalidRequest`/`VersionMismatch`/`Internal`).
    Rejected {
        /// Failure kind label.
        kind: String,
        /// Human-readable detail.
        detail: String,
    },
}

impl ScriptAnalyzerStatus {
    /// Stable kebab-case label for theme/status mapping.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Ready { .. } => "ready",
            Self::Dirty => "dirty",
            Self::Stale => "stale",
            Self::Unavailable => "unavailable",
            Self::Unsupported { .. } => "unsupported",
            Self::Rejected { .. } => "rejected",
        }
    }

    /// Extra detail text for display (capability name / reason).
    #[must_use]
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Ready {
                analyzer_id,
                impl_version,
            } => Some(format!("{analyzer_id} {impl_version}")),
            Self::Unsupported { capability } => Some(capability.clone()),
            Self::Rejected { detail, .. } => Some(detail.clone()),
            _ => None,
        }
    }
}

/// Errors produced by the editor document model.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScriptEditorError {
    /// The result's `source_revision` does not match the current
    /// document revision, no pending request exists for it, or the
    /// definition digests drifted — consumer-side stale rejection
    /// (F-14).
    StaleResult,
    /// The document has no pending request for the reported revision —
    /// an unsolicited result is never accepted.
    UnsolicitedResult {
        /// The revision the result claims.
        revision: u64,
    },
}

impl fmt::Display for ScriptEditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleResult => write!(f, "stale analyzer result"),
            Self::UnsolicitedResult { revision } => {
                write!(
                    f,
                    "analyzer result for revision {revision} has no pending request"
                )
            }
        }
    }
}

impl std::error::Error for ScriptEditorError {}

/// The Script Editor document — source + revision + analysis state.
///
/// Construct with [`ScriptEditorDocument::new`], feed edits through
/// [`ScriptEditorDocument::edit`], issue analyses through
/// [`ScriptEditorDocument::begin_analysis`] (which returns the
/// [`AnalyzeRequest`] to hand to `synvelis_automation::analyze_source`),
/// and settle them through [`ScriptEditorDocument::apply_result`] /
/// [`ScriptEditorDocument::apply_error`].
#[derive(Debug)]
pub struct ScriptEditorDocument {
    name: String,
    language: ScriptLanguageId,
    text: String,
    revision: SourceRevision,
    cursor: Option<Cursor>,
    type_definitions: Vec<TypeDefinition>,
    definitions: Option<DefinitionPins>,
    api: Vec<ScriptApiRow>,
    diagnostics: Vec<ScriptDiagnosticRow>,
    completions: Vec<ScriptCompletionRow>,
    status: ScriptAnalyzerStatus,
    /// In-flight request revisions → definition pins at request time.
    pending: BTreeMap<u64, Option<DefinitionPins>>,
}

impl ScriptEditorDocument {
    /// A new empty document for `language`.
    #[must_use]
    pub fn new(name: impl Into<String>, language: ScriptLanguageId) -> Self {
        Self {
            name: name.into(),
            language,
            text: String::new(),
            revision: SourceRevision(0),
            cursor: None,
            type_definitions: Vec::new(),
            definitions: None,
            api: Vec::new(),
            diagnostics: Vec::new(),
            completions: Vec::new(),
            status: ScriptAnalyzerStatus::Idle,
            pending: BTreeMap::new(),
        }
    }

    /// Document display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Document language id.
    #[must_use]
    pub fn language(&self) -> &ScriptLanguageId {
        &self.language
    }

    /// Current source text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Current source revision — bumped on every [`Self::edit`].
    #[must_use]
    pub fn revision(&self) -> SourceRevision {
        self.revision
    }

    /// Current cursor, if any.
    #[must_use]
    pub fn cursor(&self) -> Option<Cursor> {
        self.cursor
    }

    /// Accepted diagnostics (last applied result).
    #[must_use]
    pub fn diagnostics(&self) -> &[ScriptDiagnosticRow] {
        &self.diagnostics
    }

    /// Accepted completions (last applied result).
    #[must_use]
    pub fn completions(&self) -> &[ScriptCompletionRow] {
        &self.completions
    }

    /// Public API verdict rows.
    #[must_use]
    pub fn api(&self) -> &[ScriptApiRow] {
        &self.api
    }

    /// Analyzer status.
    #[must_use]
    pub fn status(&self) -> &ScriptAnalyzerStatus {
        &self.status
    }

    /// Number of pending (in-flight or unsettled) analysis requests.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Replaces the source text and bumps the revision. Any accepted
    /// analysis becomes dirty; in-flight requests stay pending and
    /// will be rejected as stale when they arrive.
    pub fn edit(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.revision = SourceRevision(self.revision.0.saturating_add(1));
        self.completions.clear();
        if matches!(self.status, ScriptAnalyzerStatus::Ready { .. }) {
            self.status = ScriptAnalyzerStatus::Dirty;
        }
    }

    /// Sets the completion cursor (byte offset). Does not bump the
    /// revision — cursor motion is not a source edit.
    pub fn set_cursor(&mut self, cursor: Option<Cursor>) {
        self.cursor = cursor;
    }

    /// Installs the type definitions for subsequent analyses
    /// (`DefinitionInjection` from the current `ScriptApiModel` →
    /// `LuauStub` projection). A digest change invalidates accepted
    /// analysis — status becomes [`ScriptAnalyzerStatus::Dirty`] and
    /// in-flight results computed against the old definitions will be
    /// rejected as stale on arrival.
    pub fn set_definitions(&mut self, injection: &DefinitionInjection) {
        let pins = DefinitionPins::from_injection(injection);
        if self.definitions.as_ref() != Some(&pins) {
            self.definitions = Some(pins);
            self.type_definitions = vec![injection.type_definition()];
            if matches!(self.status, ScriptAnalyzerStatus::Ready { .. }) {
                self.status = ScriptAnalyzerStatus::Dirty;
            }
        }
    }

    /// Installs the public API verdict rows (from
    /// [`project_script_api`]).
    pub fn set_api_rows(&mut self, rows: Vec<ScriptApiRow>) {
        self.api = rows;
    }

    /// Begins an analysis at the current revision: records the pending
    /// request (keyed by revision, pinning the current definition
    /// digests) and returns the [`AnalyzeRequest`] to run through
    /// `synvelis_automation::analyze_source` / `analyze_with_injection`.
    #[must_use]
    pub fn begin_analysis(&mut self, wanted: WantedCapabilities) -> AnalyzeRequest {
        let cursor = self.cursor;
        self.pending
            .insert(self.revision.0, self.definitions.clone());
        AnalyzeRequest {
            source: ScriptSource {
                text: self.text.clone(),
                language: self.language.clone(),
                revision: self.revision,
            },
            language: self.language.clone(),
            type_definitions: self.type_definitions.clone(),
            wanted,
            cursor,
        }
    }

    /// Accepts an [`AnalyzeResult`] — only when it was computed for
    /// the current revision against the current definition digests.
    ///
    /// # Errors
    /// [`ScriptEditorError::StaleResult`] — revision mismatch or
    /// definition-digest drift (status becomes
    /// [`ScriptAnalyzerStatus::Stale`]);
    /// [`ScriptEditorError::UnsolicitedResult`] — no pending request
    /// exists for the claimed revision.
    pub fn apply_result(
        &mut self,
        result: &AnalyzeResult,
        analyzer: &AnalyzerDescriptor,
    ) -> Result<(), ScriptEditorError> {
        let revision = result.source_revision;
        // Consumer-side stale rejection (F-14): the result must match
        // the revision it was requested against *and* that must still
        // be the document's current revision.
        if verify_result_revision(result, self.revision).is_err() {
            self.pending.remove(&revision.0);
            self.status = ScriptAnalyzerStatus::Stale;
            return Err(ScriptEditorError::StaleResult);
        }
        let Some(pending_definitions) = self.pending.remove(&revision.0) else {
            self.status = ScriptAnalyzerStatus::Stale;
            return Err(ScriptEditorError::UnsolicitedResult {
                revision: revision.0,
            });
        };
        if pending_definitions != self.definitions {
            self.status = ScriptAnalyzerStatus::Stale;
            return Err(ScriptEditorError::StaleResult);
        }
        self.diagnostics = result.diagnostics.iter().map(diagnostic_row).collect();
        self.completions = result
            .completions
            .clone()
            .unwrap_or_default()
            .iter()
            .map(completion_row)
            .collect();
        self.status = ScriptAnalyzerStatus::Ready {
            analyzer_id: analyzer.analyzer_id.clone(),
            impl_version: analyzer.version.impl_version.clone(),
        };
        Ok(())
    }

    /// Records an [`AnalyzerError`] for the request issued at
    /// `request_revision` and maps it onto the status vocabulary —
    /// `Unavailable`/`Unsupported`/`StaleResult`/`InvalidRequest`/
    /// `VersionMismatch`/`Internal` all stay typed and visible.
    pub fn apply_error(&mut self, request_revision: SourceRevision, error: &AnalyzerError) {
        self.pending.remove(&request_revision.0);
        self.status = match error {
            AnalyzerError::Unavailable => ScriptAnalyzerStatus::Unavailable,
            AnalyzerError::Unsupported { capability } => ScriptAnalyzerStatus::Unsupported {
                capability: capability.clone(),
            },
            AnalyzerError::StaleResult => ScriptAnalyzerStatus::Stale,
            AnalyzerError::InvalidRequest { reason } => ScriptAnalyzerStatus::Rejected {
                kind: "invalid-request".to_owned(),
                detail: reason.clone(),
            },
            AnalyzerError::VersionMismatch {
                field,
                expected,
                actual,
            } => ScriptAnalyzerStatus::Rejected {
                kind: "version-mismatch".to_owned(),
                detail: format!("{field}: expected {expected}, got {actual}"),
            },
            AnalyzerError::Internal { message } => ScriptAnalyzerStatus::Rejected {
                kind: "internal".to_owned(),
                detail: message.clone(),
            },
            _ => ScriptAnalyzerStatus::Rejected {
                kind: "unknown".to_owned(),
                detail: error.to_string(),
            },
        };
    }

    /// Projects the document into the driver-neutral
    /// [`ScriptEditorView`].
    #[must_use]
    pub fn view(&self) -> ScriptEditorView {
        ScriptEditorView {
            name: self.name.clone(),
            language: self.language.0.clone(),
            revision: self.revision.0,
            cursor: self.cursor.map(|c| c.byte_offset),
            text: self.text.clone(),
            diagnostics: self.diagnostics.clone(),
            completions: self.completions.clone(),
            api: self.api.clone(),
            status: self.status.clone(),
            definitions: self.definitions.clone(),
        }
    }
}

fn diagnostic_row(d: &Diagnostic) -> ScriptDiagnosticRow {
    ScriptDiagnosticRow {
        severity: severity_label(d.severity).to_owned(),
        kind: kind_label(d.kind).to_owned(),
        code: d.code.clone(),
        message: d.message.clone(),
        line: d.range.start.line,
        column: d.range.start.column,
        end_line: d.range.end.line,
        end_column: d.range.end.column,
        source_revision: d.source_revision.0,
    }
}

fn completion_row(c: &Completion) -> ScriptCompletionRow {
    ScriptCompletionRow {
        label: c.label.clone(),
        kind: completion_kind_label(c.kind).to_owned(),
        detail: c.detail.clone(),
        replace_line: c.replace_range.start.line,
        replace_column: c.replace_range.start.column,
        end_line: c.replace_range.end.line,
        end_column: c.replace_range.end.column,
        source_revision: c.source_revision.0,
    }
}

/// The driver-neutral Script Editor projection consumed by
/// `synvelis-app`'s `ViewSpec` builders — plain data only.
#[derive(Clone, Debug, Default)]
pub struct ScriptEditorView {
    /// Document display name.
    pub name: String,
    /// Language id text (`"luau"`, …).
    pub language: String,
    /// Current source revision.
    pub revision: u64,
    /// Cursor byte offset, if any.
    pub cursor: Option<usize>,
    /// Source text (read-model projection for headless display).
    pub text: String,
    /// Accepted diagnostics.
    pub diagnostics: Vec<ScriptDiagnosticRow>,
    /// Accepted completions.
    pub completions: Vec<ScriptCompletionRow>,
    /// Public API verdict rows.
    pub api: Vec<ScriptApiRow>,
    /// Analyzer status.
    pub status: ScriptAnalyzerStatus,
    /// Definition digest pins of the last accepted analysis inputs.
    pub definitions: Option<DefinitionPins>,
}

impl ScriptEditorView {
    /// Whether the analyzer status is a failure vocabulary
    /// (unavailable/unsupported/rejected) — used for fail-closed spec
    /// decisions.
    #[must_use]
    pub fn analyzer_failed(&self) -> bool {
        matches!(
            self.status,
            ScriptAnalyzerStatus::Unavailable
                | ScriptAnalyzerStatus::Unsupported { .. }
                | ScriptAnalyzerStatus::Rejected { .. }
        )
    }
}

/// View-local persisted state for the Script Editor surface — row
/// selections only; never document truth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptEditorViewState {
    /// Selected diagnostic row index.
    #[serde(default)]
    pub selected_diagnostic: Option<usize>,
    /// Selected completion row index.
    #[serde(default)]
    pub selected_completion: Option<usize>,
    /// Selected API verdict row index.
    #[serde(default)]
    pub selected_api: Option<usize>,
    /// Whether the API verdict table is shown.
    #[serde(default = "default_true")]
    pub show_api: bool,
}

impl Default for ScriptEditorViewState {
    fn default() -> Self {
        Self {
            selected_diagnostic: None,
            selected_completion: None,
            selected_api: None,
            show_api: true,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Decoded view-local action for the Script Editor surface.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptEditorViewAction {
    /// Diagnostic row selection changed.
    SelectDiagnostic {
        /// Selected row index.
        index: usize,
    },
    /// Completion row selection changed.
    SelectCompletion {
        /// Selected row index.
        index: usize,
    },
    /// API verdict row selection changed.
    SelectApi {
        /// Selected row index.
        index: usize,
    },
    /// Toggle the API verdict table visibility.
    ToggleApi,
}

impl ScriptEditorViewState {
    /// Applies a decoded view-local action.
    pub fn apply(&mut self, action: &ScriptEditorViewAction) {
        match action {
            ScriptEditorViewAction::SelectDiagnostic { index } => {
                self.selected_diagnostic = Some(*index);
            }
            ScriptEditorViewAction::SelectCompletion { index } => {
                self.selected_completion = Some(*index);
            }
            ScriptEditorViewAction::SelectApi { index } => {
                self.selected_api = Some(*index);
            }
            ScriptEditorViewAction::ToggleApi => self.show_api = !self.show_api,
        }
    }
}
