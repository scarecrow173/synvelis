//! SV-SCRIPT-04 (synvelis#274) — headless tests for the driver-neutral
//! Script Editor surface in `synvelis-app::script_editor`.
//!
//! Covers the decode contract (unknown targets/actions/malformed
//! params reject), the spec builders' fail-closed shape (no document
//! → no Analyze button; populated document → status/source/tables/
//! buttons), and the fake-driver mount path. The document model and
//! stale-rejection contract itself is covered in
//! `synvelis-workspace/tests/script_editor.rs`; analyzer resolution
//! in `synvelis-automation/tests/script_analyzer.rs`.

use std::cell::RefCell;
use std::rc::Rc;

use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{AttachEpochAllocator, ComponentSpec};
use serde_json::json;
use synvelis_app::script_editor::{
    SV_SCRIPT_API_TARGET, SV_SCRIPT_COMPLETIONS_TARGET, SV_SCRIPT_DIAGNOSTICS_TARGET,
    SV_SCRIPT_TARGET, ScriptEditorCommand, ScriptEditorScreenState, decode_script_editor_command,
    mount_script_editor_view, script_editor_screen_spec,
};
use synvelis_workspace::{
    ScriptAnalyzerStatus, ScriptCompletionRow, ScriptDiagnosticRow, ScriptEditorView,
    ScriptEditorViewState,
};

fn epochs() -> Rc<RefCell<AttachEpochAllocator>> {
    Rc::new(RefCell::new(AttachEpochAllocator::new()))
}

fn instance() -> ViewInstanceId {
    ViewInstanceId::new(88).expect("nonzero")
}

fn populated_view() -> ScriptEditorView {
    ScriptEditorView {
        name: "main".to_string(),
        language: "luau".to_string(),
        revision: 3,
        cursor: Some(42),
        text: "local r = synvelisPlay(project, {}, consistency)\n".to_string(),
        diagnostics: vec![ScriptDiagnosticRow {
            severity: "error".to_string(),
            kind: "type".to_string(),
            code: "unknown-api".to_string(),
            message: "unknown script api \"synvelisNope\"".to_string(),
            line: 0,
            column: 10,
            end_line: 0,
            end_column: 22,
            source_revision: 3,
        }],
        completions: vec![ScriptCompletionRow {
            label: "synvelisPlay".to_string(),
            kind: "generated-function".to_string(),
            detail: Some("synvelisPlay(project, args, consistency): PlayOutput".to_string()),
            replace_line: 0,
            replace_column: 0,
            end_line: 0,
            end_column: 8,
            source_revision: 3,
        }],
        api: vec![],
        status: ScriptAnalyzerStatus::Ready {
            analyzer_id: "synvelis-luau-port".to_string(),
            impl_version: "0.1.0".to_string(),
        },
        definitions: None,
    }
}

fn populated_state() -> ScriptEditorScreenState {
    ScriptEditorScreenState {
        editor: Some(populated_view()),
        view: ScriptEditorViewState::default(),
    }
}

fn find_buttons(spec: &ComponentSpec, out: &mut Vec<(String, String, String)>) {
    if let ComponentSpec::Button(b) = spec {
        out.push((b.key.0.clone(), b.target.clone(), b.action.clone()));
    }
    if let ComponentSpec::Panel(p) = spec {
        for child in &p.children {
            find_buttons(child, out);
        }
    }
}

fn find_labels(spec: &ComponentSpec, out: &mut Vec<String>) {
    if let ComponentSpec::Label(l) = spec {
        out.push(l.text.clone());
    }
    if let ComponentSpec::Panel(p) = spec {
        for child in &p.children {
            find_labels(child, out);
        }
    }
}

fn find_tables(spec: &ComponentSpec, out: &mut Vec<String>) {
    if let ComponentSpec::Table(t) = spec {
        out.push(format!("{:?}", t.selection));
    }
    if let ComponentSpec::Panel(p) = spec {
        for child in &p.children {
            find_tables(child, out);
        }
    }
}

// ── intent decode ─────────────────────────────────────────────────────

#[test]
fn decode_rejects_unknown_targets_and_actions() {
    assert_eq!(
        decode_script_editor_command("syn.view.mixer", "select", &json!({ "row": 0 })),
        None
    );
    assert_eq!(
        decode_script_editor_command(SV_SCRIPT_TARGET, "explode", &json!({})),
        None
    );
    assert_eq!(decode_script_editor_command("", "", &json!({})), None);
}

#[test]
fn decode_rejects_malformed_params() {
    assert_eq!(
        decode_script_editor_command(SV_SCRIPT_DIAGNOSTICS_TARGET, "select", &json!({})),
        None
    );
    assert_eq!(
        decode_script_editor_command(
            SV_SCRIPT_DIAGNOSTICS_TARGET,
            "select",
            &json!({ "row": "x" })
        ),
        None
    );
    assert_eq!(
        decode_script_editor_command(SV_SCRIPT_COMPLETIONS_TARGET, "select", &json!({})),
        None
    );
    assert_eq!(
        decode_script_editor_command(SV_SCRIPT_API_TARGET, "select", &json!({})),
        None
    );
}

#[test]
fn decode_maps_intents_to_commands() {
    assert_eq!(
        decode_script_editor_command(SV_SCRIPT_TARGET, "analyze", &json!({})),
        Some(ScriptEditorCommand::Analyze)
    );
    assert_eq!(
        decode_script_editor_command(SV_SCRIPT_TARGET, "toggle-api", &json!({})),
        Some(ScriptEditorCommand::ViewLocal(
            synvelis_workspace::ScriptEditorViewAction::ToggleApi
        ))
    );
    assert_eq!(
        decode_script_editor_command(SV_SCRIPT_DIAGNOSTICS_TARGET, "select", &json!({ "row": 1 })),
        Some(ScriptEditorCommand::ViewLocal(
            synvelis_workspace::ScriptEditorViewAction::SelectDiagnostic { index: 1 }
        ))
    );
    assert_eq!(
        decode_script_editor_command(SV_SCRIPT_COMPLETIONS_TARGET, "select", &json!({ "row": 2 })),
        Some(ScriptEditorCommand::ViewLocal(
            synvelis_workspace::ScriptEditorViewAction::SelectCompletion { index: 2 }
        ))
    );
    assert_eq!(
        decode_script_editor_command(SV_SCRIPT_API_TARGET, "select", &json!({ "row": 0 })),
        Some(ScriptEditorCommand::ViewLocal(
            synvelis_workspace::ScriptEditorViewAction::SelectApi { index: 0 }
        ))
    );
}

// ── spec builders ─────────────────────────────────────────────────────

#[test]
fn empty_state_has_no_analyze_button() {
    let spec = script_editor_screen_spec(&ScriptEditorScreenState::default());
    let mut buttons = Vec::new();
    find_buttons(&spec.root, &mut buttons);
    assert!(buttons.is_empty(), "no document → no commands: {buttons:?}");
    let mut tables = Vec::new();
    find_tables(&spec.root, &mut tables);
    assert_eq!(tables, Vec::<String>::new());
}

#[test]
fn populated_state_projects_status_source_and_tables() {
    let spec = script_editor_screen_spec(&populated_state());
    let mut buttons = Vec::new();
    find_buttons(&spec.root, &mut buttons);
    assert!(
        buttons
            .iter()
            .any(|(_, target, action)| target == SV_SCRIPT_TARGET && action == "analyze"),
        "analyze button present: {buttons:?}"
    );
    assert!(
        buttons
            .iter()
            .any(|(_, target, action)| target == SV_SCRIPT_TARGET && action == "toggle-api"),
    );

    let mut tables = Vec::new();
    find_tables(&spec.root, &mut tables);
    assert_eq!(tables.len(), 3, "diagnostics + completions + api tables");
}

#[test]
fn api_table_only_when_shown() {
    let mut state = populated_state();
    let mut view = state.editor.take().unwrap();
    view.api = vec![synvelis_workspace::ScriptApiRow {
        name: "play".to_string(),
        verdict: "invocable".to_string(),
        detail: None,
        capability: "synvelis.transport.play@1.0".to_string(),
        description: None,
    }];
    state.editor = Some(view);

    // show_api defaults to true → api table present (3 tables).
    let spec = script_editor_screen_spec(&state);
    let mut tables = Vec::new();
    find_tables(&spec.root, &mut tables);
    assert_eq!(tables.len(), 3);

    state.view.show_api = false;
    let spec = script_editor_screen_spec(&state);
    let mut tables = Vec::new();
    find_tables(&spec.root, &mut tables);
    assert_eq!(tables.len(), 2);
}

#[test]
fn view_state_selection_reaches_detail_labels() {
    let mut state = populated_state();
    // No selection → detail labels report it.
    let spec = script_editor_screen_spec(&state);
    let mut labels = Vec::new();
    find_labels(&spec.root, &mut labels);
    assert!(labels.iter().any(|t| t == "no selection"));

    // A decoded view-local select updates the persisted state and the
    // spec's detail projection picks it up.
    state
        .view
        .apply(&synvelis_workspace::ScriptEditorViewAction::SelectDiagnostic { index: 0 });
    let spec = script_editor_screen_spec(&state);
    let mut labels = Vec::new();
    find_labels(&spec.root, &mut labels);
    assert!(
        labels
            .iter()
            .any(|t| t.contains("unknown-api") && t.contains("synvelisNope")),
        "diagnostic detail shows selected row: {labels:?}"
    );
}

#[test]
fn mount_script_editor_view_succeeds_for_both_states() {
    let host = mount_script_editor_view(&epochs(), instance(), &ScriptEditorScreenState::default())
        .expect("empty spec mounts");
    drop(host);
    let host = mount_script_editor_view(&epochs(), instance(), &populated_state())
        .expect("populated spec mounts");
    drop(host);
}
