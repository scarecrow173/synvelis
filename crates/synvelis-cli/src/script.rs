//! `--file` classification — the fail-closed script/DSL gate
//! (AUTX-CLI-FILE-01, design-script §4/§5).
//!
//! Extension → kind/language/backend mapping is fixed by the AUTX-00
//! contract:
//!
//! | Extension | `action run` | `workflow run` |
//! | --- | --- | --- |
//! | `.luau` | Action Script (Luau) | Workflow Script (Luau) |
//! | `.yaml`/`.yml` | reject — DSL is workflow-only | Workflow DSL |
//! | `.rhai` | Action Script (Rhai, **`PoC` only**) | Workflow Script (Rhai, **`PoC` only**) |
//! | `.lua` | reject — Lua 5.4 is not supported | reject — Lua 5.4 is not supported |
//! | anything else | reject — unknown extension | reject — unknown extension |
//!
//! Every recognized backend (`luau-vm`, `rhai`) is **not installed** in
//! this build — script *execution* is deferred to synvelis#272/#273 —
//! so a recognized file resolves to a typed [`CliError::Unsupported`]
//! unless `--dry-run` is given, in which case the CLI reports the
//! classification (and, for Workflow DSL, the parsed+validated
//! definition) without executing anything.

use std::path::Path;

use serde_json::json;

use crate::args::Command;
use crate::envelope::{CliEnvelope, EnvelopeStatus};
use crate::error::CliError;

/// The file-kind axis a `--file` argument was classified into.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FileClass {
    /// `.luau` under `action run` — Luau Action Script.
    ActionScriptLuau,
    /// `.rhai` under `action run` — Rhai Action Script (`PoC` only).
    ActionScriptRhai,
    /// `.yaml`/`.yml` under `workflow run` — Workflow DSL.
    WorkflowDsl,
    /// `.luau` under `workflow run` — Luau Workflow Script.
    WorkflowScriptLuau,
    /// `.rhai` under `workflow run` — Rhai Workflow Script (`PoC` only).
    WorkflowScriptRhai,
}

impl FileClass {
    /// The contract names reported in results/errors.
    #[must_use]
    pub const fn names(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::ActionScriptLuau => ("action-script", "luau", "luau-vm"),
            Self::ActionScriptRhai => ("action-script", "rhai", "rhai"),
            Self::WorkflowDsl => ("workflow-dsl", "yaml", "synvelis-automation"),
            Self::WorkflowScriptLuau => ("workflow-script", "luau", "luau-vm"),
            Self::WorkflowScriptRhai => ("workflow-script", "rhai", "rhai"),
        }
    }

    /// Whether this class is a PoC-only coexistence backend.
    #[must_use]
    pub const fn is_poc(self) -> bool {
        matches!(self, Self::ActionScriptRhai | Self::WorkflowScriptRhai)
    }
}

/// Classifies `file` for `command` (`ActionRun`/`WorkflowRun` only).
///
/// # Errors
/// [`CliError::InvalidInput`] — unknown extension or
/// extension/subcommand kind mismatch; [`CliError::Unsupported`] —
/// recognized but unsupported language (Lua 5.4).
pub fn classify(command: &Command, file: &Path) -> Result<FileClass, CliError> {
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let workflow_side = matches!(command, Command::WorkflowRun { .. });
    match ext.as_deref() {
        Some("luau") => Ok(if workflow_side {
            FileClass::WorkflowScriptLuau
        } else {
            FileClass::ActionScriptLuau
        }),
        Some("yaml" | "yml") => {
            if workflow_side {
                Ok(FileClass::WorkflowDsl)
            } else {
                Err(CliError::InvalidInput(
                    "`.yaml`/`.yml` is the Workflow DSL — it is only valid for \
                     `workflow run --file`, not `action run --file`"
                        .to_owned(),
                ))
            }
        }
        Some("rhai") => Ok(if workflow_side {
            FileClass::WorkflowScriptRhai
        } else {
            FileClass::ActionScriptRhai
        }),
        Some("lua") => Err(CliError::Unsupported(
            "Lua 5.4 (`.lua`) is explicitly not supported; the script \
             contract covers Luau (`.luau`), Workflow DSL (`.yaml`/`.yml`), \
             and PoC-only Rhai (`.rhai`)"
                .to_owned(),
        )),
        Some(other) => Err(CliError::InvalidInput(format!(
            "unknown file extension '.{other}' — supported: .luau, .yaml/.yml, .rhai (PoC)"
        ))),
        None => Err(CliError::InvalidInput(
            "file has no extension — cannot classify script kind".to_owned(),
        )),
    }
}

/// Reads and UTF-8-validates the `--file` payload.
///
/// # Errors
/// [`CliError::InvalidInput`] — missing path, non-file, unreadable, or
/// non-UTF-8 content.
pub fn read_source(file: &Path) -> Result<String, CliError> {
    let canonical = file.canonicalize().map_err(|e| {
        CliError::InvalidInput(format!("cannot resolve --file '{}': {e}", file.display()))
    })?;
    if !canonical.is_file() {
        return Err(CliError::InvalidInput(format!(
            "--file '{}' is not a regular file",
            canonical.display()
        )));
    }
    std::fs::read_to_string(&canonical).map_err(|e| {
        CliError::InvalidInput(format!("cannot read --file '{}': {e}", canonical.display()))
    })
}

/// The result of a `--dry-run` file validation: classification plus,
/// for Workflow DSL, the parsed+validated definition summary.
///
/// # Errors
/// [`CliError::InvalidInput`] when a Workflow DSL source fails to
/// parse/validate.
pub fn dry_run_result(class: FileClass, source: Option<&str>) -> Result<CliEnvelope, CliError> {
    let (kind, language, backend) = class.names();
    let mut result = json!({
        "kind": kind,
        "language": language,
        "backend_requirement": backend,
        "poc_only": class.is_poc(),
        "backend_available": false,
        "executed": false,
        "dry_run": true,
    });
    if class == FileClass::WorkflowDsl
        && let Some(source) = source
    {
        let definition = synvelis_automation::parse_workflow_yaml(source).map_err(|e| {
            CliError::InvalidInput(format!("workflow DSL failed to parse/validate: {e}"))
        })?;
        result["workflow_id"] = json!(definition.workflow_id);
        result["version"] = json!(definition.version);
        result["steps"] = json!(definition.steps.len());
        result["inputs"] = json!(definition.inputs.len());
        result["outputs"] = json!(definition.outputs.len());
        result["definition_valid"] = json!(true);
    }
    let mut envelope = CliEnvelope::bare(EnvelopeStatus::Completed);
    envelope.result = Some(result);
    Ok(envelope)
}

/// The fail-closed rejection for a recognized file whose execution
/// path is not wired in this build.
#[must_use]
pub fn unsupported_reason(class: FileClass) -> String {
    let (kind, language, backend) = class.names();
    if class.is_poc() {
        format!(
            "{kind} ({language}) requires the '{backend}' backend, which is a \
             PoC-only coexistence runtime and is not installed — execution is \
             deferred (synvelis#273 / Ruviform RF#558/#559)"
        )
    } else {
        format!(
            "{kind} ({language}) execution is not wired in this build — the \
             '{backend}' backend lands with synvelis#272/#273; the CLI \
             accepted and classified the file but refuses to guess-execute"
        )
    }
}

/// Runs the file-classification half of `action run --file` /
/// `workflow run --file`. Never executes script content.
///
/// # Errors
/// [`CliError`] — classification rejects (2/5) or DSL parse failure (2).
pub fn run_file_command(
    command: &Command,
    file: &Path,
    dry_run: bool,
) -> Result<CliEnvelope, CliError> {
    let class = classify(command, file)?;
    let source = if class == FileClass::WorkflowDsl && dry_run {
        Some(read_source(file)?)
    } else {
        // Still validate readability up-front so a missing file fails
        // the same way in both modes.
        let _ = read_source(file)?;
        None
    };
    if dry_run {
        return dry_run_result(class, source.as_deref());
    }
    Err(CliError::Unsupported(unsupported_reason(class)))
}

/// Value-level helper for tests: the extension classifier without IO.
#[cfg(test)]
pub(crate) fn classify_ext(workflow_side: bool, ext: &str) -> Result<FileClass, CliError> {
    match ext.to_ascii_lowercase().as_str() {
        "luau" => Ok(if workflow_side {
            FileClass::WorkflowScriptLuau
        } else {
            FileClass::ActionScriptLuau
        }),
        "yaml" | "yml" if workflow_side => Ok(FileClass::WorkflowDsl),
        "yaml" | "yml" => Err(CliError::InvalidInput("kind mismatch".to_owned())),
        "rhai" => Ok(if workflow_side {
            FileClass::WorkflowScriptRhai
        } else {
            FileClass::ActionScriptRhai
        }),
        "lua" => Err(CliError::Unsupported("lua 5.4".to_owned())),
        other => Err(CliError::InvalidInput(format!("unknown .{other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_matrix() {
        assert_eq!(
            classify_ext(false, "luau").unwrap(),
            FileClass::ActionScriptLuau
        );
        assert_eq!(
            classify_ext(true, "luau").unwrap(),
            FileClass::WorkflowScriptLuau
        );
        assert_eq!(classify_ext(true, "yaml").unwrap(), FileClass::WorkflowDsl);
        assert_eq!(classify_ext(true, "yml").unwrap(), FileClass::WorkflowDsl);
        assert!(matches!(
            classify_ext(false, "yaml"),
            Err(CliError::InvalidInput(_))
        ));
        assert_eq!(
            classify_ext(false, "rhai").unwrap(),
            FileClass::ActionScriptRhai
        );
        assert!(matches!(
            classify_ext(false, "lua"),
            Err(CliError::Unsupported(_))
        ));
        assert!(matches!(
            classify_ext(true, "txt"),
            Err(CliError::InvalidInput(_))
        ));
    }

    #[test]
    fn names_and_poc_flags() {
        assert_eq!(
            FileClass::ActionScriptLuau.names(),
            ("action-script", "luau", "luau-vm")
        );
        assert!(FileClass::WorkflowScriptRhai.is_poc());
        assert!(!FileClass::WorkflowDsl.is_poc());
        assert_ne!(FileClass::WorkflowDsl.names().2, "");
    }
}
