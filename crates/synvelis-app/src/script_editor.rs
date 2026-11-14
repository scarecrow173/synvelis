//! SV-SCRIPT-04 (synvelis#274) — driver-neutral `ViewSpec` / intent
//! decode for the Script Editor surface.
//!
//! These builders project the [`synvelis_workspace::script_editor`]
//! read model — itself a pure projection of the `synvelis-script`
//! analyzer port and `ScriptApiModel` verdicts — into
//! `ruviform_ui::view` specs. No GPUI/OS/Native-FFI/VM type appears
//! here or in the projected types, and no view state becomes
//! project/grant/job truth.
//!
//! `ComponentSpec` has no text-editing kind in the current contract,
//! so the headless slice projects the document as read-model labels
//! plus diagnostics/completions/API-verdict tables. Real text input,
//! IME, pixel, and focus behavior is the real-GPUI lane (sv#114/#267).
//!
//! Interactive contract:
//!
//! - Every intent lands on one of the `SV_SCRIPT_*_TARGET` logical
//!   targets and decodes through [`decode_script_editor_command`] —
//!   the single intent path.
//! - Non-executable states never get buttons: with no document there
//!   is no Analyze action at all (fail-closed spec, not just handler).
//!
//! **Dependency direction:** `ruviform_ui::view` +
//! `synvelis_workspace` types only. **RT contract:** non-RT only.

use std::cell::RefCell;
use std::rc::Rc;

use ruviform_ui::UiError;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{
    A11ySpec, AttachEpochAllocator, ButtonSpec, CellValue, ComponentSpec, ComponentVisualState,
    LabelSpec, TableSelectionSpec, TableSpec, ViewHost, ViewSpec,
};
use serde_json::Value;
use synvelis_workspace::{
    ScriptApiRow, ScriptCompletionRow, ScriptDiagnosticRow, ScriptEditorView,
    ScriptEditorViewAction, ScriptEditorViewState,
};

use crate::screens::{key, panel};

/// Logical target of Script Editor commands (analyze, surface
/// toggles). Table selections use the per-table targets below.
pub const SV_SCRIPT_TARGET: &str = "svscript.editor";
/// Logical target of diagnostic-table selections.
pub const SV_SCRIPT_DIAGNOSTICS_TARGET: &str = "svscript.diagnostics";
/// Logical target of completion-table selections.
pub const SV_SCRIPT_COMPLETIONS_TARGET: &str = "svscript.completions";
/// Logical target of API-verdict-table selections.
pub const SV_SCRIPT_API_TARGET: &str = "svscript.api";

// ── screen state ──────────────────────────────────────────────────────

/// Script Editor screen state — the projected editor view plus the
/// persisted view-local selections/toggles. `editor: None` renders
/// the empty-state spec.
#[derive(Clone, Debug, Default)]
pub struct ScriptEditorScreenState {
    /// Latest applied editor projection.
    pub editor: Option<ScriptEditorView>,
    /// Per-view persisted state.
    pub view: ScriptEditorViewState,
}

// ── shared spec helpers ───────────────────────────────────────────────

fn label(key_name: &str, a11y_label: &str, text: String) -> ComponentSpec {
    ComponentSpec::Label(LabelSpec {
        key: key(key_name),
        text,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some(a11y_label.to_string()),
            ..A11ySpec::default()
        },
    })
}

fn action_button(
    key_name: &str,
    label_text: &str,
    target: &str,
    action: &str,
    params: Option<Value>,
) -> ComponentSpec {
    ComponentSpec::Button(ButtonSpec {
        key: key(key_name),
        label: label_text.to_string(),
        target: target.to_string(),
        action: action.to_string(),
        params,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec::default(),
    })
}

fn text_cell(s: impl Into<String>) -> CellValue {
    CellValue::Text(s.into())
}

fn opt_cell(s: Option<String>) -> CellValue {
    CellValue::Text(s.unwrap_or_default())
}

fn position_text(line: u32, column: u32) -> String {
    format!("{}:{}", line + 1, column + 1)
}

// ── spec builders ─────────────────────────────────────────────────────

fn status_text(editor: &ScriptEditorView) -> String {
    let cursor = editor
        .cursor
        .map_or_else(|| "none".to_string(), |c| c.to_string());
    let defs = editor.definitions.as_ref().map_or_else(
        || "none".to_string(),
        |d| {
            format!(
                "model {}… stub {}…",
                &d.model_digest[..12.min(d.model_digest.len())],
                &d.stub_digest[..12.min(d.stub_digest.len())]
            )
        },
    );
    format!(
        "{} — {} — rev {} — cursor {} — analyzer {} ({}) — definitions {}",
        editor.name,
        editor.language,
        editor.revision,
        cursor,
        editor.status.label(),
        editor.status.detail().unwrap_or_default(),
        defs,
    )
}

fn source_text(editor: &ScriptEditorView) -> String {
    if editor.text.is_empty() {
        "(empty source)".to_string()
    } else {
        editor.text.clone()
    }
}

fn diagnostic_detail(state: &ScriptEditorScreenState, editor: &ScriptEditorView) -> String {
    let Some(index) = state.view.selected_diagnostic else {
        return "no selection".to_string();
    };
    let Some(row) = editor.diagnostics.get(index) else {
        return format!("selection {index} outside diagnostics");
    };
    format!(
        "{} — {} — {} — rev {}",
        row.code, row.message, row.severity, row.source_revision
    )
}

fn completion_detail(state: &ScriptEditorScreenState, editor: &ScriptEditorView) -> String {
    let Some(index) = state.view.selected_completion else {
        return "no selection".to_string();
    };
    let Some(row) = editor.completions.get(index) else {
        return format!("selection {index} outside completions");
    };
    format!(
        "{} — {} — rev {}",
        row.label,
        row.detail.as_deref().unwrap_or(""),
        row.source_revision
    )
}

fn api_detail(state: &ScriptEditorScreenState, editor: &ScriptEditorView) -> String {
    let Some(index) = state.view.selected_api else {
        return "no selection".to_string();
    };
    let Some(row) = editor.api.get(index) else {
        return format!("selection {index} outside api");
    };
    format!(
        "{} — {} — {}",
        row.name,
        row.verdict,
        row.detail.as_deref().unwrap_or("")
    )
}

fn diagnostics_table(rows: &[ScriptDiagnosticRow]) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("svscript.diagnostics.table"),
        columns: vec![
            "Severity".to_string(),
            "Kind".to_string(),
            "Code".to_string(),
            "Message".to_string(),
            "Line".to_string(),
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    text_cell(row.severity.clone()),
                    text_cell(row.kind.clone()),
                    text_cell(row.code.clone()),
                    text_cell(row.message.clone()),
                    text_cell(position_text(row.line, row.column)),
                ]
                .into_iter()
                .map(Some)
                .collect()
            })
            .collect(),
        selection: Some(TableSelectionSpec {
            target: SV_SCRIPT_DIAGNOSTICS_TARGET.to_string(),
        }),
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Script diagnostics".to_string()),
            ..A11ySpec::default()
        },
    })
}

fn completions_table(rows: &[ScriptCompletionRow]) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("svscript.completions.table"),
        columns: vec![
            "Label".to_string(),
            "Kind".to_string(),
            "Detail".to_string(),
            "Replace".to_string(),
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    text_cell(row.label.clone()),
                    text_cell(row.kind.clone()),
                    opt_cell(row.detail.clone()),
                    text_cell(position_text(row.replace_line, row.replace_column)),
                ]
                .into_iter()
                .map(Some)
                .collect()
            })
            .collect(),
        selection: Some(TableSelectionSpec {
            target: SV_SCRIPT_COMPLETIONS_TARGET.to_string(),
        }),
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Script completions".to_string()),
            ..A11ySpec::default()
        },
    })
}

fn api_table(rows: &[ScriptApiRow]) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("svscript.api.table"),
        columns: vec![
            "API".to_string(),
            "Verdict".to_string(),
            "Capability".to_string(),
            "Detail".to_string(),
        ],
        rows: rows
            .iter()
            .map(|row| {
                vec![
                    text_cell(row.name.clone()),
                    text_cell(row.verdict.clone()),
                    text_cell(row.capability.clone()),
                    opt_cell(row.detail.clone()),
                ]
                .into_iter()
                .map(Some)
                .collect()
            })
            .collect(),
        selection: Some(TableSelectionSpec {
            target: SV_SCRIPT_API_TARGET.to_string(),
        }),
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Script API verdicts".to_string()),
            ..A11ySpec::default()
        },
    })
}

fn editor_buttons(state: &ScriptEditorScreenState) -> Vec<ComponentSpec> {
    let mut buttons = vec![action_button(
        "svscript.editor.analyze",
        "Analyze",
        SV_SCRIPT_TARGET,
        "analyze",
        None,
    )];
    let api_label = if state.view.show_api {
        "Hide API"
    } else {
        "Show API"
    };
    buttons.push(action_button(
        "svscript.editor.api-toggle",
        api_label,
        SV_SCRIPT_TARGET,
        "toggle-api",
        None,
    ));
    buttons
}

/// The Script Editor `ViewSpec` — read-model labels, three selection
/// tables, and fail-closed command buttons. With no document the spec
/// is a single status label: no Analyze button exists to press.
#[must_use]
pub fn script_editor_screen_spec(state: &ScriptEditorScreenState) -> ViewSpec {
    let Some(editor) = &state.editor else {
        return ViewSpec {
            root: panel(
                "svscript.editor.root",
                Some("Script Editor"),
                vec![label(
                    "svscript.editor.status",
                    "Script editor status",
                    "Script Editor — no document".to_string(),
                )],
            ),
        };
    };
    let mut children = vec![
        label(
            "svscript.editor.status",
            "Script editor status",
            status_text(editor),
        ),
        label(
            "svscript.editor.source",
            "Script source",
            source_text(editor),
        ),
        label(
            "svscript.editor.analyzer",
            "Analyzer status",
            format!(
                "analyzer {} — {}",
                editor.status.label(),
                editor.status.detail().unwrap_or_default()
            ),
        ),
        label(
            "svscript.editor.diag-detail",
            "Selected diagnostic",
            diagnostic_detail(state, editor),
        ),
        label(
            "svscript.editor.completion-detail",
            "Selected completion",
            completion_detail(state, editor),
        ),
        diagnostics_table(&editor.diagnostics),
        completions_table(&editor.completions),
    ];
    if state.view.show_api {
        children.push(label(
            "svscript.editor.api-detail",
            "Selected API entry",
            api_detail(state, editor),
        ));
        children.push(api_table(&editor.api));
    }
    children.push(panel(
        "svscript.editor.buttons",
        None,
        editor_buttons(state),
    ));
    ViewSpec {
        root: panel("svscript.editor.root", Some("Script Editor"), children),
    }
}

// ── mount ─────────────────────────────────────────────────────────────

/// Mounts the Script Editor onto a fresh [`ViewHost`] under the
/// adopted workspace view id — same attach-epoch contract as
/// [`crate::screens::mount_vcs_history_view`].
///
/// # Errors
/// [`UiError::Internal`] when the spec fails validation.
pub fn mount_script_editor_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    instance: ViewInstanceId,
    state: &ScriptEditorScreenState,
) -> Result<ViewHost, UiError> {
    let mut host = ViewHost::new(instance, attach_epochs.borrow_mut().mint());
    host.mount(script_editor_screen_spec(state))
        .map_err(|e| UiError::Internal {
            message: format!("script editor spec invalid: {e}"),
        })?;
    Ok(host)
}

// ── intent decode ─────────────────────────────────────────────────────

/// A decoded Script Editor command.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptEditorCommand {
    /// View-local mutation — updates `ScriptEditorViewState` and the
    /// document's view-local selections only.
    ViewLocal(ScriptEditorViewAction),
    /// Run an analysis pass at the current revision — the composition
    /// layer resolves the analyzer through `AnalyzerRegistry` and
    /// feeds the result back via `ScriptEditorDocument`.
    Analyze,
}

fn params_usize(params: &Value, key: &str) -> Option<usize> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
}

/// Decodes one view intent `(target, action, params)` into a
/// [`ScriptEditorCommand`]. Returns `None` for unknown targets,
/// actions, or malformed params — fail-closed, never guessed.
#[must_use]
pub fn decode_script_editor_command(
    target: &str,
    action: &str,
    params: &Value,
) -> Option<ScriptEditorCommand> {
    match (target, action) {
        (SV_SCRIPT_TARGET, "analyze") => Some(ScriptEditorCommand::Analyze),
        (SV_SCRIPT_TARGET, "toggle-api") => Some(ScriptEditorCommand::ViewLocal(
            ScriptEditorViewAction::ToggleApi,
        )),
        (SV_SCRIPT_DIAGNOSTICS_TARGET, "select") => Some(ScriptEditorCommand::ViewLocal(
            ScriptEditorViewAction::SelectDiagnostic {
                index: params_usize(params, "row")?,
            },
        )),
        (SV_SCRIPT_COMPLETIONS_TARGET, "select") => Some(ScriptEditorCommand::ViewLocal(
            ScriptEditorViewAction::SelectCompletion {
                index: params_usize(params, "row")?,
            },
        )),
        (SV_SCRIPT_API_TARGET, "select") => Some(ScriptEditorCommand::ViewLocal(
            ScriptEditorViewAction::SelectApi {
                index: params_usize(params, "row")?,
            },
        )),
        _ => None,
    }
}
