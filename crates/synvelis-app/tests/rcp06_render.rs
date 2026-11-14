//! RCP-06 (synvelis#295) — headless tests for the driver-neutral
//! render surfaces in `synvelis-app::render`.
//!
//! Same contract as `autx07_automation.rs`: the app layer cannot name
//! `synvelis-render`/`synvelis-gateway` types, so these tests cover
//! the decode/binding contract and the spec builders' fail-closed
//! shape — unknown targets/actions and malformed params reject,
//! view-local intents never become requests, and non-executable
//! action candidates get no buttons. Projection/request lowering
//! itself is covered in `synvelis-workspace/tests/rcp06_render_ui.rs`.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{AttachEpochAllocator, ComponentSpec, ViewHost};
use serde_json::json;

use synvelis_app::render::{
    BranchDiffScreenState, RCP_BRANCH_TARGET, RCP_RENDER_TARGET, RCP_REQUEST_TARGET,
    RenderRequestScreenState, RenderStatusScreenState, apply_render_view_command,
    branch_diff_screen_spec, decode_render_command, mount_branch_diff_view,
    mount_render_request_view, mount_render_status_view, render_request_screen_spec,
    render_status_screen_spec,
};
use synvelis_player::rcp::{ExecutionIntent, PluginVerdict, RcpNodeId};
use synvelis_project::ProjectRevision;
use synvelis_workspace::{
    ACTION_RENDER_REQUEST, BoundaryDiffRow, BoundaryResolutionKind, BoundarySideView,
    BranchDiffView, NodeRenderEvidence, RenderActionCandidate, RenderActionKind, RenderCommand,
    RenderNodePresentation, RenderPolicyView, RenderReason, RenderRequestPreview, RenderScope,
    RenderSelectionView, RenderTrigger, RenderViewAction, RenderViewState, SelectionPins,
    SnapshotPinView, TriggerProvider, TriggerRow, project_render_node,
};

fn epochs() -> Rc<RefCell<AttachEpochAllocator>> {
    Rc::new(RefCell::new(AttachEpochAllocator::new()))
}

fn instance(n: u64) -> ViewInstanceId {
    ViewInstanceId::new(n).expect("nonzero")
}

// ── fixtures (workspace read models only — no authority types) ────────

fn missing_node() -> RenderNodePresentation {
    let verdict = PluginVerdict::Missing;
    project_render_node(
        RcpNodeId(1),
        NodeRenderEvidence {
            verdict: Some(&verdict),
            ..NodeRenderEvidence::default()
        },
        &ExecutionIntent::MainPlayback,
    )
}

fn selection_view() -> RenderSelectionView {
    RenderSelectionView {
        intent: "main-playback",
        verdict: "blocked",
        non_equivalent: false,
        live_regions: 0,
        substitutions: 0,
        skipped: 0,
        unresolved: 1,
        degraded: 0,
        unresolved_reasons: vec![RenderReason::PluginMissing],
        pins: SelectionPins {
            snapshot_id: 1,
            project_revision: ProjectRevision(3),
            structure_digest: 0x11,
            capability_generation: 2,
            render_index_generation: 4,
            selection_digest: 0x22,
        },
    }
}

fn policy_view() -> RenderPolicyView {
    RenderPolicyView {
        triggers: vec![TriggerRow {
            trigger: RenderTrigger::OnCommit,
            label: "on-commit",
            enabled: false,
            unconditional: false,
            provider: Some(TriggerProvider::Unsupported {
                reason: RenderReason::CommitUnsupported,
                detail: "no-vcs".to_string(),
            }),
            blocked_reason: Some(RenderReason::TriggerDisabled),
        }],
        scope: "playback-compatibility",
        completion: "best-effort",
        quality_profile: "q@1 abcdef123456".to_string(),
        resource_budget: "cpu 0ms".to_string(),
        retention: "protect-referenced=true".to_string(),
        provenance: Vec::new(),
        commit: synvelis_workspace::CommitView::Unsupported {
            reason: RenderReason::CommitUnsupported,
            detail: "no-vcs".to_string(),
        },
        resolution_digest: "digest".to_string(),
        recipe_invariant: true,
    }
}

// ── intent decode ─────────────────────────────────────────────────────

#[test]
fn decode_rejects_unknown_targets_and_actions() {
    assert_eq!(
        decode_render_command("syn.view.mixer", "select", &json!({ "row": 0 })),
        None
    );
    assert_eq!(
        decode_render_command(RCP_RENDER_TARGET, "explode", &json!({})),
        None
    );
    assert_eq!(decode_render_command("", "", &json!({})), None);
}

#[test]
fn decode_rejects_malformed_params() {
    assert_eq!(
        decode_render_command(RCP_RENDER_TARGET, "select", &json!({})),
        None
    );
    assert_eq!(
        decode_render_command(RCP_RENDER_TARGET, "select", &json!({ "row": "x" })),
        None
    );
    assert_eq!(
        decode_render_command(RCP_RENDER_TARGET, "job-control", &json!({})),
        None
    );
    assert_eq!(
        decode_render_command(RCP_RENDER_TARGET, "job-control", &json!({ "op": "nuke" })),
        None
    );
    assert_eq!(
        decode_render_command(RCP_REQUEST_TARGET, "request", &json!({})),
        None
    );
    assert_eq!(
        decode_render_command(
            RCP_RENDER_TARGET,
            "toggle-trigger",
            &json!({ "trigger": "bogus", "enabled": true }),
        ),
        None
    );
}

#[test]
fn decode_view_local_and_domain_commands() {
    assert_eq!(
        decode_render_command(RCP_RENDER_TARGET, "select", &json!({ "row": 3 })),
        Some(RenderCommand::ViewLocal(RenderViewAction::SelectJob {
            index: 3
        }))
    );
    assert_eq!(
        decode_render_command(RCP_BRANCH_TARGET, "select", &json!({ "row": 1 })),
        Some(RenderCommand::ViewLocal(RenderViewAction::SelectBoundary {
            index: 1
        }))
    );
    assert_eq!(
        decode_render_command(RCP_REQUEST_TARGET, "select", &json!({ "row": 0 })),
        Some(RenderCommand::ViewLocal(RenderViewAction::SelectBoundary {
            index: 0
        }))
    );
    assert_eq!(
        decode_render_command(RCP_RENDER_TARGET, "job-control", &json!({ "op": "cancel" })),
        Some(RenderCommand::JobControl {
            op: synvelis_workspace::JobControlOp::Cancel
        })
    );
    assert_eq!(
        decode_render_command(RCP_RENDER_TARGET, "job-control", &json!({ "op": "retry" })),
        Some(RenderCommand::JobControl {
            op: synvelis_workspace::JobControlOp::Retry
        })
    );
    assert_eq!(
        decode_render_command(
            RCP_RENDER_TARGET,
            "toggle-trigger",
            &json!({ "trigger": "on-save", "enabled": false }),
        ),
        Some(RenderCommand::ViewLocal(RenderViewAction::ToggleTrigger {
            trigger: RenderTrigger::OnSave,
            enabled: false,
        }))
    );
    assert_eq!(
        decode_render_command(
            RCP_RENDER_TARGET,
            "set-completion",
            &json!({ "completion": "require-complete" }),
        ),
        Some(RenderCommand::ViewLocal(RenderViewAction::SetCompletion {
            completion: synvelis_workspace::CompletionRequirement::RequireComplete,
        }))
    );
    assert_eq!(
        decode_render_command(
            RCP_RENDER_TARGET,
            "set-scope",
            &json!({ "scope": "mix-snapshot" }),
        ),
        Some(RenderCommand::ViewLocal(RenderViewAction::SetScope {
            scope: RenderScope::MixSnapshot,
        }))
    );
    assert_eq!(
        decode_render_command(RCP_RENDER_TARGET, "clear-draft", &json!({})),
        Some(RenderCommand::ViewLocal(RenderViewAction::ClearDraft))
    );
    assert_eq!(
        decode_render_command(
            RCP_REQUEST_TARGET,
            "request",
            &json!({ "action": ACTION_RENDER_REQUEST }),
        ),
        Some(RenderCommand::Request {
            action: ACTION_RENDER_REQUEST.to_string(),
        })
    );
}

#[test]
fn view_local_command_applies_to_state_only() {
    let mut state = RenderViewState::default();
    apply_render_view_command(
        &mut state,
        &RenderCommand::ViewLocal(RenderViewAction::SelectBoundary { index: 2 }),
    );
    assert_eq!(state.selected_boundary, Some(2));
    // Domain commands are not view-local — applying them is a no-op
    // on view state (the composition layer routes them to authority).
    apply_render_view_command(
        &mut state,
        &RenderCommand::Request {
            action: ACTION_RENDER_REQUEST.to_string(),
        },
    );
    apply_render_view_command(
        &mut state,
        &RenderCommand::JobControl {
            op: synvelis_workspace::JobControlOp::Cancel,
        },
    );
    assert_eq!(state.selected_boundary, Some(2));
    assert_eq!(state.selected_job, None);
}

// ── spec builders ─────────────────────────────────────────────────────

/// Recursively collects every label/button text + table cell text in
/// a spec tree — the headless evidence that axis labels, reason
/// codes, and pins actually reach the driver-neutral contract.
fn collect_text(spec: &ComponentSpec, out: &mut Vec<String>) {
    match spec {
        ComponentSpec::Label(l) => out.push(l.text.clone()),
        ComponentSpec::Button(b) => {
            out.push(format!("button:{}", b.label));
            out.push(b.target.clone());
            out.push(b.action.clone());
        }
        ComponentSpec::Table(t) => {
            for row in &t.rows {
                for cell in row {
                    if let Some(ruviform_ui::view::CellValue::Text(s)) = cell {
                        out.push(s.clone());
                    }
                }
            }
        }
        ComponentSpec::Panel(p) => {
            for child in &p.children {
                collect_text(child, out);
            }
        }
        _ => {}
    }
}

fn spec_text(spec: &ruviform_ui::view::ViewSpec) -> String {
    let mut out = Vec::new();
    collect_text(&spec.root, &mut out);
    out.join("\n")
}

#[test]
fn empty_states_render_empty_labels_without_panicking() {
    let _ = render_status_screen_spec(&RenderStatusScreenState::default());
    let _ = branch_diff_screen_spec(&BranchDiffScreenState::default());
    let _ = render_request_screen_spec(&RenderRequestScreenState::default());
}

#[test]
fn mount_helpers_produce_valid_view_hosts() {
    let alloc = epochs();
    let _status: ViewHost =
        mount_render_status_view(&alloc, instance(91), &RenderStatusScreenState::default())
            .expect("status mounts");
    let _branch: ViewHost =
        mount_branch_diff_view(&alloc, instance(92), &BranchDiffScreenState::default())
            .expect("branch mounts");
    let _request: ViewHost =
        mount_render_request_view(&alloc, instance(93), &RenderRequestScreenState::default())
            .expect("request mounts");
}

#[test]
fn status_spec_surfaces_axes_reasons_pins_and_policy_without_color() {
    let state = RenderStatusScreenState {
        view: RenderViewState::default(),
        selection: None,
        policy: None,
        nodes: vec![missing_node()],
        jobs: Vec::new(),
    };
    let spec = render_status_screen_spec(&state);
    let text = spec_text(&spec);
    // Five independent axes — each carries a stable non-color label.
    for axis in [
        "edit-possible",
        "blocked",
        "render-missing",
        "quality-unavailable",
        "runtime-configured",
    ] {
        assert!(text.contains(axis), "missing axis label {axis}");
    }
    // Shared reason code reaches the driver contract verbatim.
    assert!(text.contains("capability.plugin-missing"));
}

#[test]
fn status_spec_shows_policy_provenance_commit_and_trigger_block() {
    let state = RenderStatusScreenState {
        view: RenderViewState::default(),
        selection: Some(selection_view()),
        policy: Some(policy_view()),
        nodes: Vec::new(),
        jobs: Vec::new(),
    };
    let spec = render_status_screen_spec(&state);
    let text = spec_text(&spec);
    assert!(text.contains("on-commit"));
    assert!(text.contains("policy.commit-unsupported"));
    assert!(text.contains("policy.trigger-disabled"));
    assert!(text.contains("recipe-invariant"));
    // Selection verdict + pin generations visible.
    assert!(text.contains("verdict blocked"));
    assert!(text.contains("rev 3"));
}

#[test]
fn branch_diff_spec_shows_pins_boundaries_and_missing_plugins() {
    let pin = SnapshotPinView {
        locator: "\"saved\"".to_string(),
        digest: format!("sha256:{}", "ab".repeat(32)),
        capture_generation: 1,
    };
    let side = |resolved: bool| BoundarySideView {
        boundary: "master".to_string(),
        resolution: if resolved {
            BoundaryResolutionKind::Resolved
        } else {
            BoundaryResolutionKind::Unknown
        },
        recipe_key: resolved.then(|| "sha256:abc123".to_string()),
        renderer: resolved.then(|| "pinned".to_string()),
        reasons: if resolved {
            Vec::new()
        } else {
            vec![RenderReason::ArtifactUnknown]
        },
    };
    let diff = BranchDiffView {
        left_pin: pin.clone(),
        right_pin: pin,
        context: "a vs b".to_string(),
        rows: vec![BoundaryDiffRow {
            index: 0,
            left: side(true),
            right: side(false),
            changed_parameters: Vec::new(),
            missing_plugins: vec!["plug-x".to_string()],
            uncertainty: Vec::new(),
            coverage: "1/2 resolved".to_string(),
            preview_markers: Vec::new(),
        }],
        missing_plugins: vec!["plug-x".to_string()],
        semantic_diffs: Vec::new(),
        preview_markers: Vec::new(),
        warnings: Vec::new(),
    };
    let spec = branch_diff_screen_spec(&BranchDiffScreenState {
        view: RenderViewState::default(),
        diff: Some(diff),
    });
    let text = spec_text(&spec);
    assert!(text.contains("1/2 resolved"));
    assert!(text.contains("plug-x"));
    assert!(text.contains("render.artifact-unknown"));
    assert!(text.contains("sha256:abc123"));
}

#[test]
fn request_spec_gates_buttons_to_executable_candidates_only() {
    let candidate = |id: &'static str, executable: bool| RenderActionCandidate {
        id,
        label: id,
        kind: RenderActionKind::Request,
        executable,
        disabled_reason: (!executable).then_some(RenderReason::GrantRequired),
        requires_grant: id == synvelis_workspace::ACTION_RENDER_PUBLISH,
    };
    let preview = RenderRequestPreview {
        pin: SnapshotPinView {
            locator: "\"saved\"".to_string(),
            digest: format!("sha256:{}", "cd".repeat(32)),
            capture_generation: 1,
        },
        purpose: "selection",
        boundaries: Vec::new(),
        coverage: "0/0 demands resolved".to_string(),
        expected_bytes: None,
        missing_plugins: Vec::new(),
        requires_owner_machine: false,
        invalidation: None,
        actions: vec![
            candidate(ACTION_RENDER_REQUEST, true),
            candidate(synvelis_workspace::ACTION_RENDER_PUBLISH, false),
        ],
    };
    let spec = render_request_screen_spec(&RenderRequestScreenState {
        view: RenderViewState::default(),
        preview: Some(preview),
    });
    let text = spec_text(&spec);
    // The executable request action becomes a button on the shared
    // request target; the grant-gated publish action gets none — the
    // spec itself is fail-closed (its id survives only inside the
    // disabled-reasons text, never as a button).
    assert!(text.contains(RCP_REQUEST_TARGET));
    assert!(text.contains("button:synvelis.render.request"));
    assert!(!text.contains("button:synvelis.render.publish"));
    // Disabled reason stays visible as text (shared reason code).
    assert!(text.contains("policy.grant-required"));
}

#[test]
fn populated_status_state_mounts_without_spec_errors() {
    let state = RenderStatusScreenState {
        view: RenderViewState {
            surface: synvelis_workspace::RenderSurface::Status,
            selected_job: Some(0),
            draft_triggers: Some(BTreeSet::from([RenderTrigger::Manual])),
            ..RenderViewState::default()
        },
        selection: None,
        policy: Some(policy_view()),
        nodes: vec![missing_node()],
        jobs: Vec::new(),
    };
    let alloc = epochs();
    let _host: ViewHost =
        mount_render_status_view(&alloc, instance(94), &state).expect("populated mounts");
    let text = spec_text(&render_status_screen_spec(&state));
    assert!(text.contains("draft policy edits pending"));
}
