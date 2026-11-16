//! `script analyze --file` — the headless Script Analyzer verb
//! (AUTX-SCR-ANA-01 / synvelis#274).
//!
//! Pipeline: extension→language classification →
//! [`script::read_source`] → [`host::boot_discovery`] `ScriptStack` →
//! [`LuauStub::generate`] → [`DefinitionInjection::from_stub`] →
//! [`AnalyzerRegistry`] + [`LuauPortAnalyzer`] →
//! [`analyze_with_injection`] → [`verify_revision`] →
//! [`CliEnvelope`].
//!
//! This lane is deliberately *not* a gateway operation: no wire
//! request, no `host::handle`, no claims, no project binding —
//! analyzer success is a static-analysis signal only and mints no
//! execution permission, grant, approval, or project truth. The boot
//! exists solely to capture the live `ScriptStack` (snapshot →
//! surface → `ScriptApiModel` → bindings) so the injected type
//! definitions come from the same canonical model every other
//! consumer uses — never a hand-written or fixture copy.
//!
//! `SourceRevision(1)` is the fixed revision of a one-shot CLI
//! invocation: there is no edit session, hence no stale domain —
//! [`verify_revision`] is still checked as the contract echo guard.

use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Value, json};
use synvelis_automation::{
    AnalyzerRegistry, LuauPortAnalyzer, analyze_with_injection, verify_revision,
};
use synvelis_script::{
    AnalyzeResult, AnalyzerError, Completion, CompletionKind, Cursor, DefinitionInjection,
    Diagnostic, DiagnosticKind, DiagnosticLevel, DiagnosticSeverity, LuauStub, ScriptLanguageId,
    ScriptSource, SourcePosition, SourceRange, SourceRevision, WantedCapabilities,
};

use crate::args::Args;
use crate::commands::{self, CommandResult};
use crate::envelope::{CliEnvelope, EnvelopeStatus};
use crate::error::CliError;
use crate::host;
use crate::script;

/// Runs `script analyze --file <path> [--cursor <byte-offset>]`.
///
/// # Errors
/// [`CliError`] — usage/input rejections (extension, unreadable or
/// non-UTF-8 file, out-of-range cursor), `cli.unsupported` for
/// recognized-but-unserved languages (`.lua`, `.rhai` with no
/// registered analyzer), `cli.internal` for contract drift or
/// adapter invariants. Analyzer *diagnostics* are never errors —
/// they ride inside `result` on a `completed` envelope.
pub fn analyze_command(
    args: &Args,
    file: &Path,
    cursor: Option<usize>,
) -> Result<CommandResult, CliError> {
    commands::reject_dry_run(args, "script analyze")?;
    let language = classify_language(file)?;
    let text = script::read_source(file)?;
    let booted = host::boot_discovery()?;
    let stub = LuauStub::generate(&booted.script.model).map_err(|e| {
        CliError::Internal(format!(
            "cannot generate Luau stub from ScriptApiModel: {e}"
        ))
    })?;
    let injection = DefinitionInjection::from_stub(&stub);
    let mut registry = AnalyzerRegistry::new();
    registry
        .register(Box::new(LuauPortAnalyzer::new()))
        .map_err(|e| CliError::Internal(format!("analyzer registration failed: {e}")))?;
    let revision = SourceRevision(1);
    let result = analyze_with_injection(
        &registry,
        ScriptSource {
            text,
            language: language.clone(),
            revision,
        },
        &injection,
        WantedCapabilities {
            diagnostics: Some(DiagnosticLevel::SyntaxAndType),
            completion: cursor.is_some(),
        },
        cursor.map(|byte_offset| Cursor { byte_offset }),
    )
    .map_err(|e| analyzer_error(&e, &language))?;
    verify_revision(&result, revision).map_err(|e| analyzer_error(&e, &language))?;
    Ok(commands::result(
        envelope(file, &language, &registry, &injection, &result),
        0,
        Some(human(file, &result)),
    ))
}

/// Maps a script-source extension onto the analyzer language id.
///
/// `.yaml`/`.yml` and unknown/missing extensions are [`CliError::
/// InvalidInput`] — the Workflow DSL and non-script files are not
/// analyzer targets. `.lua` is [`CliError::Unsupported`] (Lua 5.4 is
/// explicitly out of contract). `.rhai` is admitted as a language id
/// so the registry can fail closed with `AnalyzerError::
/// Unavailable` — NR-07's missing analyzer is reported in the typed
/// vocabulary, not swallowed by the CLI.
fn classify_language(file: &Path) -> Result<ScriptLanguageId, CliError> {
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("luau") => Ok(ScriptLanguageId::luau()),
        Some("rhai") => Ok(ScriptLanguageId("rhai".to_owned())),
        Some("lua") => Err(CliError::Unsupported(
            "Lua 5.4 (`.lua`) is explicitly not supported; the script \
             analyzer covers Luau (`.luau`) and PoC-only Rhai (`.rhai`)"
                .to_owned(),
        )),
        Some(_) => Err(CliError::InvalidInput(format!(
            "'{}' is not a script source for `script analyze` — \
             supported extensions: .luau, .rhai (.yaml/.yml is the \
             Workflow DSL, not an analyzer target)",
            file.display()
        ))),
        None => Err(CliError::InvalidInput(format!(
            "'{}' has no extension — cannot classify script language",
            file.display()
        ))),
    }
}

/// The single `AnalyzerError` → `CliError` authority for this verb.
/// `Unavailable`/`Unsupported` keep the language and capability in
/// the message; `InvalidRequest` is user input; contract drift and
/// internal failures are adapter-side (`cli.internal`, exit 10).
fn analyzer_error(err: &AnalyzerError, language: &ScriptLanguageId) -> CliError {
    match err {
        AnalyzerError::Unavailable => CliError::Unsupported(format!(
            "no analyzer is registered for script language '{language}'"
        )),
        AnalyzerError::Unsupported { capability } => CliError::Unsupported(format!(
            "analyzer for script language '{language}' does not provide {capability}"
        )),
        AnalyzerError::InvalidRequest { reason } => CliError::InvalidInput(format!(
            "invalid analysis request for '{language}': {reason}"
        )),
        _ => CliError::Internal(format!("analyzer failure for '{language}': {err}")),
    }
}

/// The `completed` envelope payload — analyzer descriptor,
/// injection identity, diagnostics, and (when requested)
/// completions, all as plain JSON data.
fn envelope(
    file: &Path,
    language: &ScriptLanguageId,
    registry: &AnalyzerRegistry,
    injection: &DefinitionInjection,
    result: &AnalyzeResult,
) -> CliEnvelope {
    let mut payload = json!({
        "file": file.display().to_string(),
        "language": language.0,
        "source_revision": result.source_revision.0,
        "diagnostics": result
            .diagnostics
            .iter()
            .map(diagnostic_json)
            .collect::<Vec<_>>(),
        "injection": {
            "package_name": injection.package_name,
            "model_digest": injection.model_digest,
            "stub_digest": injection.stub_digest,
        },
    });
    if let Some(descriptor) = registry.descriptor_for(language) {
        payload["analyzer"] = json!({
            "analyzer_id": descriptor.analyzer_id,
            "language": descriptor.language.0,
            "impl_version": descriptor.version.impl_version,
            "contract_version": descriptor.version.contract_version,
            "diagnostics": diagnostic_level_name(descriptor.capabilities.diagnostics),
            "completion": descriptor.capabilities.completion,
            "max_source_bytes": descriptor.capabilities.max_source_bytes,
        });
    }
    if let Some(completions) = &result.completions {
        payload["completions"] = json!(completions.iter().map(completion_json).collect::<Vec<_>>());
    }
    let mut envelope = CliEnvelope::bare(EnvelopeStatus::Completed);
    envelope.result = Some(payload);
    envelope
}

/// Human rendering: one line per diagnostic / completion, or a
/// `no diagnostics` line for a clean source. Diagnostics are result
/// data — they go to stdout under `--format human`; invocation
/// failures stay on the stderr path (`CliError` → `emit_failure`).
fn human(file: &Path, result: &AnalyzeResult) -> String {
    let mut out = String::new();
    for d in &result.diagnostics {
        let _ = writeln!(
            out,
            "{}:{}:{} {} {} {}",
            file.display(),
            d.range.start.line + 1,
            d.range.start.column + 1,
            severity_name(d.severity),
            d.code,
            d.message
        );
    }
    if let Some(completions) = &result.completions {
        for c in completions {
            let _ = writeln!(
                out,
                "{}:{}:{} completion {}",
                file.display(),
                c.replace_range.start.line + 1,
                c.replace_range.start.column + 1,
                c.label
            );
        }
    }
    if result.diagnostics.is_empty() && result.completions.as_ref().is_none_or(Vec::is_empty) {
        let _ = writeln!(out, "{}: no diagnostics", file.display());
    }
    out
}

fn diagnostic_json(d: &Diagnostic) -> Value {
    json!({
        "severity": severity_name(d.severity),
        "kind": diagnostic_kind_name(d.kind),
        "code": d.code,
        "message": d.message,
        "range": range_json(d.range),
        "source_revision": d.source_revision.0,
    })
}

fn completion_json(c: &Completion) -> Value {
    json!({
        "label": c.label,
        "kind": completion_kind_name(c.kind),
        "detail": c.detail,
        "insert_text": c.insert_text,
        "replace_range": range_json(c.replace_range),
        "source_revision": c.source_revision.0,
    })
}

fn range_json(range: SourceRange) -> Value {
    json!({
        "start": position_json(range.start),
        "end": position_json(range.end),
    })
}

fn position_json(p: SourcePosition) -> Value {
    json!({ "line": p.line, "column": p.column })
}

fn severity_name(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Information => "information",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Error => "error",
        _ => "other",
    }
}

fn diagnostic_kind_name(kind: DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::Syntax => "syntax",
        DiagnosticKind::Type => "type",
        _ => "other",
    }
}

fn completion_kind_name(kind: CompletionKind) -> &'static str {
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

fn diagnostic_level_name(level: DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Unsupported => "unsupported",
        DiagnosticLevel::Syntax => "syntax",
        DiagnosticLevel::SyntaxAndType => "syntax-and-type",
        _ => "other",
    }
}
