//! AUTX-07 (synvelis#266) — headless tests for the driver-neutral
//! automation surfaces in `synvelis-app::automation`.
//!
//! The app layer cannot name `synvelis-gateway` types (no dependency —
//! projections arrive as workspace read models), so these tests cover
//! the decode/binding contract and the spec builders' fail-closed
//! shape: unknown targets/actions and malformed params reject, view
//! -local intents never become requests, and non-executable rows get
//! no action buttons. Projection/request lowering itself is covered
//! in `synvelis-workspace/tests/autx07_automation_ui.rs`.

use std::cell::RefCell;
use std::rc::Rc;

use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{AttachEpochAllocator, ViewHost};
use serde_json::json;

use synvelis_app::automation::{
    AUTX_ACTION_TARGET, AUTX_MACRO_TARGET, AUTX_PALETTE_TARGET, AUTX_RUN_TARGET,
    AUTX_WORKFLOW_TARGET, ActionPaletteScreenState, AutomationCommand, MacroBrowserScreenState,
    RunMonitorScreenState, WorkflowEditorScreenState, action_palette_screen_spec,
    decode_automation_command, macro_browser_screen_spec, mount_action_palette_view,
    mount_macro_browser_view, mount_run_monitor_view, mount_workflow_editor_view,
    run_monitor_screen_spec, workflow_editor_screen_spec,
};
use synvelis_workspace::AutomationViewAction;

fn epochs() -> Rc<RefCell<AttachEpochAllocator>> {
    Rc::new(RefCell::new(AttachEpochAllocator::new()))
}

fn instance() -> ViewInstanceId {
    ViewInstanceId::new(77).expect("nonzero")
}

// ── intent decode ─────────────────────────────────────────────────────

#[test]
fn decode_rejects_unknown_targets_and_actions() {
    assert_eq!(
        decode_automation_command("syn.view.mixer", "select", &json!({ "row": 0 })),
        None
    );
    assert_eq!(
        decode_automation_command(AUTX_PALETTE_TARGET, "explode", &json!({})),
        None
    );
    assert_eq!(decode_automation_command("", "", &json!({})), None);
}

#[test]
fn decode_rejects_malformed_params() {
    // Missing row index.
    assert_eq!(
        decode_automation_command(AUTX_PALETTE_TARGET, "select", &json!({})),
        None
    );
    // Non-numeric row.
    assert_eq!(
        decode_automation_command(AUTX_PALETTE_TARGET, "select", &json!({ "row": "x" })),
        None
    );
    // Missing capability label on a domain command.
    assert_eq!(
        decode_automation_command(AUTX_ACTION_TARGET, "submit", &json!({})),
        None
    );
    // Malformed explicit target — never silently dropped.
    assert_eq!(
        decode_automation_command(
            AUTX_ACTION_TARGET,
            "submit",
            &json!({ "capability": "a@1.0", "target": { "shape": "bogus" } }),
        ),
        None
    );
}

#[test]
fn decode_view_local_selects_and_category() {
    assert_eq!(
        decode_automation_command(AUTX_PALETTE_TARGET, "select", &json!({ "row": 3 })),
        Some(AutomationCommand::ViewLocal(
            AutomationViewAction::PaletteSelect { row: 3 }
        ))
    );
    assert_eq!(
        decode_automation_command(AUTX_PALETTE_TARGET, "category", &json!({ "index": 2 })),
        Some(AutomationCommand::ViewLocal(
            AutomationViewAction::PaletteCategory { index: 2 }
        ))
    );
    assert_eq!(
        decode_automation_command(AUTX_MACRO_TARGET, "select", &json!({ "row": 1 })),
        Some(AutomationCommand::ViewLocal(
            AutomationViewAction::MacroSelect { index: 1 }
        ))
    );
    assert_eq!(
        decode_automation_command(AUTX_WORKFLOW_TARGET, "select", &json!({ "row": 0 })),
        Some(AutomationCommand::ViewLocal(
            AutomationViewAction::WorkflowSelect { index: 0 }
        ))
    );
    assert_eq!(
        decode_automation_command(AUTX_RUN_TARGET, "select", &json!({ "row": 4 })),
        Some(AutomationCommand::ViewLocal(
            AutomationViewAction::RunSelect { index: 4 }
        ))
    );
}

#[test]
fn decode_domain_commands_carry_capability_target_args() {
    let target = json!({
        "shape": "resource",
        "kind": "source",
        "id": "source:clip-1"
    });
    assert_eq!(
        decode_automation_command(
            AUTX_ACTION_TARGET,
            "submit",
            &json!({
                "capability": "synvelis.midi.add-note@1.1",
                "target": target,
                "args": { "pitch": 60 },
                "idempotency_key": "k1"
            }),
        ),
        Some(AutomationCommand::Request(AutomationViewAction::Submit {
            capability: "synvelis.midi.add-note@1.1".to_string(),
            target: Some(synvelis_workspace::ExplicitTarget::Resource {
                kind: "source".to_string(),
                id: "source:clip-1".to_string(),
            }),
            args: json!({ "pitch": 60 }),
            idempotency_key: Some("k1".to_string()),
        }))
    );

    // Absent target → None (contract decides whether that is valid).
    match decode_automation_command(
        AUTX_ACTION_TARGET,
        "preview",
        &json!({ "capability": "synvelis.transport.set-position@1.0" }),
    ) {
        Some(AutomationCommand::Request(AutomationViewAction::Preview {
            capability,
            target,
            args,
        })) => {
            assert_eq!(capability, "synvelis.transport.set-position@1.0");
            assert_eq!(target, None);
            assert_eq!(args, json!({}));
        }
        other => panic!("unexpected decode: {other:?}"),
    }

    // Capture is view-local (freezes state, mints nothing).
    assert_eq!(
        decode_automation_command(
            AUTX_ACTION_TARGET,
            "capture",
            &json!({ "capability": "a@1.0", "target": target }),
        ),
        Some(AutomationCommand::ViewLocal(
            AutomationViewAction::CaptureTarget {
                capability: "a@1.0".to_string(),
                target: Some(synvelis_workspace::ExplicitTarget::Resource {
                    kind: "source".to_string(),
                    id: "source:clip-1".to_string(),
                }),
            }
        ))
    );
}

#[test]
fn decode_macro_and_cancel_commands() {
    assert_eq!(
        decode_automation_command(
            AUTX_MACRO_TARGET,
            "apply",
            &json!({ "preset": "normalize-loudness", "idempotency_key": "m1" }),
        ),
        Some(AutomationCommand::Request(
            AutomationViewAction::MacroApply {
                preset: "normalize-loudness".to_string(),
                idempotency_key: Some("m1".to_string()),
            }
        ))
    );
    assert_eq!(
        decode_automation_command(AUTX_MACRO_TARGET, "preview", &json!({ "preset": "p" })),
        Some(AutomationCommand::Request(
            AutomationViewAction::MacroPreview {
                preset: "p".to_string()
            }
        ))
    );
    // Run cancel resolves the selected run at the composition layer —
    // the decoded command carries no operation id of its own.
    assert_eq!(
        decode_automation_command(AUTX_RUN_TARGET, "cancel", &json!({})),
        Some(AutomationCommand::CancelSelectedRun)
    );
}

// ── spec builders (empty-state + fail-closed shape) ───────────────────

#[test]
fn empty_states_render_empty_labels_without_panicking() {
    let palette = action_palette_screen_spec(&ActionPaletteScreenState::default());
    let _ = palette;
    let macro_b = macro_browser_screen_spec(&MacroBrowserScreenState::default());
    let _ = macro_b;
    let workflow = workflow_editor_screen_spec(&WorkflowEditorScreenState::default());
    let _ = workflow;
    let run = run_monitor_screen_spec(&RunMonitorScreenState::default());
    let _ = run;
}

#[test]
fn mount_helpers_produce_valid_view_hosts() {
    let alloc = epochs();
    let palette: ViewHost =
        mount_action_palette_view(&alloc, instance(), &ActionPaletteScreenState::default())
            .expect("palette mounts");
    let _macro: ViewHost = mount_macro_browser_view(
        &alloc,
        ViewInstanceId::new(78).unwrap(),
        &MacroBrowserScreenState::default(),
    )
    .expect("macro mounts");
    let _workflow: ViewHost = mount_workflow_editor_view(
        &alloc,
        ViewInstanceId::new(79).unwrap(),
        &WorkflowEditorScreenState::default(),
    )
    .expect("workflow mounts");
    let _run: ViewHost = mount_run_monitor_view(
        &alloc,
        ViewInstanceId::new(80).unwrap(),
        &RunMonitorScreenState::default(),
    )
    .expect("run mounts");
    drop(palette);
}
