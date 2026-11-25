//! TIM-01 PPQN widget slice (synvelis#125) — headless fixture for
//! the *real widget intent path*, end to end: Project-tab `Button`
//! press → `ViewHost::invoke` → `rvf.ui.command.v1` `Action` carrying
//! `params {new_ppqn, policy, rounding?}` → `NativeShell` view-state
//! routing → `syn.project.ppqn` → the view-owned `ppqn_*` gesture →
//! `syn.project.*` ops → the command `Result` receipt the pressing
//! Button settles on → spec reprojection of the `ppqn.*` surface.
//!
//! The fixture is `common::mounted` — the same composition root
//! `synvelis-native` uses (app → mounted `ViewHost` → `NativeShell`
//! → driver), `FakeDriver` standing in for the real window. There is
//! no second project model and no test-only op surface.
//!
//! Boundary: headless OS-L1 semantic evidence only. Real OS window/
//! pixel/IME driving (GUI-L2) stays the deferred external gate
//! (`synvelis#114` + `ruviform#631`) — nothing here claims it.

mod common;

use common::mounted::*;

use ruviform_ui::ipc::Intent;
use ruviform_ui::view::{ButtonSpec, ComponentSpec};
use serde_json::{Value, json};

use synvelis_app::{MainScreenState, main_screen_spec};
use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, CounterIdGenerator, DirectoryBundleStore,
    MusicalDuration, MusicalTick, PlaybackMapping, Project, ProjectController, ProjectId,
    ProjectOp, SourceFrame, SourceRange, TimeAnchor,
};
use synvelis_workspace::ProjectSession;

// ---------- helpers ----------

/// Sends a raw `syn.*` op intent on the fixture window — the same
/// typed driver path `svt_product_e2e` uses for ops without a widget.
fn op(f: &mut MountedFixture, id: &str, name: &str, params: Value) {
    action(
        &mut f.shell,
        &mut f.driver,
        f.window,
        Intent::new(id, name, params),
    );
}

/// A forged `rvf.ui.command.v1` `Action` intent — the same envelope
/// `MountedFixture::pump_view_intents` mints, but authored by the
/// test so params/widget actions a spec could never declare still
/// face the strict shell/op decode.
fn forged_action(
    f: &mut MountedFixture,
    id: &str,
    target: &str,
    action_name: &str,
    params: Option<Value>,
) {
    action(
        &mut f.shell,
        &mut f.driver,
        f.window,
        Intent::new(
            id,
            "rvf.ui.command.v1",
            json!({
                "command_id": id,
                "expect_receipt": true,
                "intent": {
                    "kind": "action",
                    "target": target,
                    "control_id": "ppqn.forged",
                    "sequence": 1,
                    "origin_view": f.view_id.as_str(),
                    "action": action_name,
                    "params": params.unwrap_or(Value::Null),
                }
            }),
        ),
    );
}

/// Finds a component by key anywhere in the spec tree — Panels
/// recurse through `children`, Tabs through each tab's `child` (same
/// walk as `tim01_ppqn_ui.rs`).
fn find_component<'a>(spec: &'a ComponentSpec, name: &str) -> Option<&'a ComponentSpec> {
    if spec.key().0 == name {
        return Some(spec);
    }
    match spec {
        ComponentSpec::Panel(p) => p.children.iter().find_map(|c| find_component(c, name)),
        ComponentSpec::Tabs(t) => t
            .tabs
            .iter()
            .find_map(|tab| find_component(&tab.child, name)),
        _ => None,
    }
}

fn label_text(spec: &ComponentSpec, name: &str) -> String {
    match find_component(spec, name) {
        Some(ComponentSpec::Label(l)) => l.text.clone(),
        other => panic!("label `{name}` missing or wrong kind: {other:?}"),
    }
}

fn button_spec<'a>(spec: &'a ComponentSpec, name: &str) -> &'a ButtonSpec {
    match find_component(spec, name) {
        Some(ComponentSpec::Button(b)) => b,
        other => panic!("button `{name}` missing or wrong kind: {other:?}"),
    }
}

/// The `ppqn.*` surface text exactly as the next `ViewHost::update`
/// would render it — the spec is re-projected from the app's
/// `MainScreenState` cell, which the service tail refreshes after
/// every op.
fn rendered_label(f: &MountedFixture, name: &str) -> String {
    let screen = f.screen.borrow().clone();
    label_text(&main_screen_spec(&screen).root, name)
}

/// Writes a project bundle whose clip is anchored at an *odd*
/// musical tick — the one shape a recommended-value change cannot
/// always re-project exactly (7 ticks at 960 PPQN is 3.5 ticks at
/// 480 PPQN, so `exact` must report inexact sites and `round` must
/// apply). Mirrors `common::mounted::write_clip_bundle` — same
/// `DirectoryBundleStore` + save path the app loads.
fn write_odd_tick_bundle(dir: &std::path::Path) {
    let mut store = DirectoryBundleStore::create(dir).expect("bundle store");
    let mut session = ProjectSession::new(ProjectController::new(
        Project::new(ProjectId::new(0x0DD), "odd-tick"),
        Box::new(CounterIdGenerator::new(1)),
    ));
    apply_op(
        &mut session,
        ProjectOp::RegisterAsset {
            digest: "deadbeef".to_string(),
        },
    );
    let asset_id = *session.project().assets.keys().next().expect("asset");
    apply_op(
        &mut session,
        ProjectOp::RegisterSource {
            asset_id,
            content_digest: "deadbeef".to_string(),
        },
    );
    apply_op(
        &mut session,
        ProjectOp::AddTrack {
            name: "t1".to_string(),
        },
    );
    let track_id = *session.project().tracks.keys().next().expect("track");
    apply_op(&mut session, ProjectOp::AddLane { track_id });
    let lane_id = *session.project().lanes.keys().next().expect("lane");
    let source_id = *session.project().sources.keys().next().expect("source");
    apply_op(
        &mut session,
        ProjectOp::AddClip {
            lane_id,
            source_id,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(7)),
                extent: ClipExtent::Musical(MusicalDuration::new(960).expect("extent")),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(1_000_000).expect("frame"),
                )
                .expect("region"),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    session
        .controller_mut()
        .save(&mut store)
        .expect("save bundle");
}

/// Opens the odd-tick fixture bundle through `syn.project.open` —
/// same as `open_clip_project` but anchored at tick 7.
fn open_odd_tick_project(f: &mut MountedFixture, tag: &str) {
    let dir = std::env::temp_dir().join(format!("sv125-ppqn-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    write_odd_tick_bundle(&dir);
    let path = dir.to_string_lossy().to_string();
    op(f, "po", "syn.project.open", json!({ "path": path }));
    assert!(results_for(&f.driver, f.window, "po").last().unwrap().0);
    f.deliver_to_host();
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- spec shape ----------

/// The widget slice mounts the interactive half of the `ppqn.*`
/// surface: one `exact`/`round` Button pair per recommended value
/// carrying `params {new_ppqn, policy, rounding?}` on the `view`
/// target, plus the params-free Confirm/Cancel pair.
#[test]
fn project_tab_mounts_ppqn_widgets() {
    let spec = main_screen_spec(&MainScreenState::default());
    assert!(matches!(
        find_component(&spec.root, "ppqn.candidates"),
        Some(ComponentSpec::Panel(_))
    ));
    assert!(matches!(
        find_component(&spec.root, "ppqn.gesture"),
        Some(ComponentSpec::Panel(_))
    ));
    for ppqn in [480_u32, 960, 1000, 1920] {
        let exact = button_spec(&spec.root, &format!("ppqn.select.{ppqn}.exact"));
        assert_eq!(exact.target, "view");
        assert_eq!(exact.action, "ppqn.select");
        assert_eq!(
            exact.params,
            Some(json!({ "new_ppqn": ppqn, "policy": "exact" }))
        );
        let round = button_spec(&spec.root, &format!("ppqn.select.{ppqn}.round"));
        assert_eq!(
            round.params,
            Some(json!({
                "new_ppqn": ppqn,
                "policy": "round",
                "rounding": "nearest_even",
            }))
        );
    }
    let confirm = button_spec(&spec.root, "ppqn.confirm");
    assert_eq!(confirm.action, "ppqn.confirm");
    assert!(confirm.params.is_none());
    let cancel = button_spec(&spec.root, "ppqn.cancel");
    assert_eq!(cancel.action, "ppqn.cancel");
    assert!(cancel.params.is_none());
}

// ---------- widget → intent → gesture → op → receipt → spec ----------

/// A select Button press mints `syn.project.ppqn` action `select`:
/// the candidate is recorded, the preview op runs in the same
/// receipt, and the re-projected `ppqn.*` surface shows the pending
/// gesture — before any `set_ppqn` mutation.
#[test]
fn widget_select_runs_preview_and_updates_surface() {
    let mut f = fixture();
    f.press("ppqn.select.480.exact");
    assert_eq!(last_result_ok(&f.driver, f.window), Some(true));
    {
        let screen = f.screen.borrow();
        assert_eq!(screen.ppqn.pending_new_ppqn, Some(480));
        assert_eq!(screen.ppqn.pending_policy, Some("exact"));
        let preview = screen.ppqn.preview.as_ref().expect("preview recorded");
        assert!(preview.applies, "empty project converts exactly");
        assert_eq!(preview.old_ppqn, 960);
        assert_eq!(preview.new_ppqn, 480);
    }
    assert_eq!(f.screen.borrow().ppqn.current_ppqn, 960);
    let preview_text = rendered_label(&f, "ppqn.preview");
    assert!(preview_text.contains("960 → 480"), "{preview_text}");
}

/// select → confirm applies the change: `set_ppqn` lands the new
/// PPQN, the pending gesture clears, and the status readout
/// re-projects to the new value.
#[test]
fn widget_confirm_applies_after_select() {
    let mut f = fixture();
    f.press("ppqn.select.480.exact");
    assert_eq!(last_result_ok(&f.driver, f.window), Some(true));
    f.press("ppqn.confirm");
    let (ok, data) = last_result(&f.driver, f.window).expect("confirm result");
    assert!(ok, "confirm failed: {data:?}");
    let data = data.expect("confirm payload");
    assert_eq!(data["confirmed"], 480);
    assert_eq!(data["project"]["ppqn"], 480);
    {
        let screen = f.screen.borrow();
        assert_eq!(screen.ppqn.current_ppqn, 480);
        assert_eq!(screen.ppqn.pending_new_ppqn, None);
    }
    assert!(
        rendered_label(&f, "ppqn.status").contains("PPQN 480"),
        "status did not re-project"
    );
    // The change went through the real op — the project oracle sees it.
    op(&mut f, "pi", "syn.project.info", json!({}));
    let (_, info) = results_for(&f.driver, f.window, "pi")
        .last()
        .unwrap()
        .clone();
    assert_eq!(info.unwrap()["ppqn"], 480);
}

/// select → cancel discards the pending candidate without touching
/// the live PPQN — nothing was ever applied, so cancel is
/// non-destructive by construction.
#[test]
fn widget_cancel_discards_pending() {
    let mut f = fixture();
    f.press("ppqn.select.1000.round");
    assert_eq!(last_result_ok(&f.driver, f.window), Some(true));
    assert_eq!(f.screen.borrow().ppqn.pending_new_ppqn, Some(1000));
    f.press("ppqn.cancel");
    let (ok, data) = last_result(&f.driver, f.window).expect("cancel result");
    assert!(ok, "cancel failed: {data:?}");
    assert_eq!(data.expect("cancel payload")["canceled"], true);
    {
        let screen = f.screen.borrow();
        assert_eq!(screen.ppqn.pending_new_ppqn, None);
        assert_eq!(screen.ppqn.current_ppqn, 960);
    }
}

/// Re-selecting while a candidate is pending replaces it — the new
/// select's preview is the only one confirm can settle.
#[test]
fn widget_reselect_replaces_candidate() {
    let mut f = fixture();
    f.press("ppqn.select.480.exact");
    assert_eq!(last_result_ok(&f.driver, f.window), Some(true));
    f.press("ppqn.select.1920.round");
    assert_eq!(last_result_ok(&f.driver, f.window), Some(true));
    {
        let screen = f.screen.borrow();
        assert_eq!(screen.ppqn.pending_new_ppqn, Some(1920));
        assert_eq!(screen.ppqn.pending_policy, Some("round"));
    }
    // Confirm settles the *latest* candidate, not the first.
    f.press("ppqn.confirm");
    let (ok, data) = last_result(&f.driver, f.window).expect("confirm result");
    assert!(ok, "confirm failed: {data:?}");
    assert_eq!(data.expect("confirm payload")["confirmed"], 1920);
    assert_eq!(f.screen.borrow().ppqn.current_ppqn, 1920);
}

/// Explicit-confirm semantics: Confirm without an applicable preview
/// is an honest rejection — never an implicit commit. Cancel without
/// a pending selection still succeeds honestly (`canceled: false`).
#[test]
fn widget_confirm_and_cancel_without_pending() {
    let mut f = fixture();
    f.press("ppqn.confirm");
    assert_eq!(
        last_result_ok(&f.driver, f.window),
        Some(false),
        "confirm without pending must reject"
    );
    assert_eq!(f.screen.borrow().ppqn.current_ppqn, 960);
    f.press("ppqn.cancel");
    let (ok, data) = last_result(&f.driver, f.window).expect("cancel result");
    assert!(ok, "cancel without pending still answers: {data:?}");
    assert_eq!(data.expect("cancel payload")["canceled"], false);
}

/// A rejected preview (`applies: false`) blocks confirm honestly:
/// select `480 exact` on the odd-tick project reports inexact sites,
/// and Confirm fails rather than mutating. `round` on the same
/// candidate applies and confirms.
#[test]
fn widget_exact_preview_rejection_blocks_confirm() {
    let mut f = fixture();
    open_odd_tick_project(&mut f, "inexact");
    f.push_screen_state();
    f.press("ppqn.select.480.exact");
    assert_eq!(
        last_result_ok(&f.driver, f.window),
        Some(true),
        "preview itself succeeds — the *preview* reports inexact"
    );
    {
        let screen = f.screen.borrow();
        let preview = screen.ppqn.preview.as_ref().expect("preview recorded");
        assert!(!preview.applies, "tick-7 anchor is inexact at 480");
        assert!(preview.inexact_count >= 1);
    }
    f.press("ppqn.confirm");
    assert_eq!(
        last_result_ok(&f.driver, f.window),
        Some(false),
        "confirm over a rejected preview must reject"
    );
    assert_eq!(f.screen.borrow().ppqn.current_ppqn, 960);
    // Same candidate under `round` — inexact sites convert rounded,
    // the preview applies, and confirm lands 480.
    f.press("ppqn.select.480.round");
    assert_eq!(last_result_ok(&f.driver, f.window), Some(true));
    assert!(
        f.screen.borrow().ppqn.preview.as_ref().unwrap().applies,
        "round converts the odd tick"
    );
    f.press("ppqn.confirm");
    assert_eq!(last_result_ok(&f.driver, f.window), Some(true));
    assert_eq!(f.screen.borrow().ppqn.current_ppqn, 480);
}

// ---------- strict params / honest rejection ----------

/// Params a spec could never declare still face the strict decode:
/// forged `ppqn.select` actions with out-of-range, mistyped, or
/// contract-violating params reject on the wire — never silently
/// coerced, never applied.
#[test]
fn widget_forged_select_params_rejected() {
    let mut f = fixture();
    let cases: &[(&str, Value)] = &[
        ("out-of-range", json!({ "new_ppqn": 0, "policy": "exact" })),
        ("above-max", json!({ "new_ppqn": 32768, "policy": "exact" })),
        (
            "unknown-policy",
            json!({ "new_ppqn": 480, "policy": "bogus" }),
        ),
        (
            "exact-with-rounding",
            json!({ "new_ppqn": 480, "policy": "exact", "rounding": "floor" }),
        ),
        (
            "round-missing-rounding",
            json!({ "new_ppqn": 480, "policy": "round" }),
        ),
        (
            "unknown-rounding",
            json!({ "new_ppqn": 480, "policy": "round", "rounding": "bogus" }),
        ),
        (
            "wrong-type",
            json!({ "new_ppqn": "480", "policy": "exact" }),
        ),
        ("missing-ppqn", json!({ "policy": "exact" })),
        ("missing-policy", json!({ "new_ppqn": 480 })),
    ];
    for (i, (name, params)) in cases.iter().enumerate() {
        let id = format!("bad{i}");
        forged_action(&mut f, &id, "view", "ppqn.select", Some(params.clone()));
        let results = results_for(&f.driver, f.window, &id);
        let (ok, _) = results.last().expect("forged select result").clone();
        assert!(!ok, "case {name} was accepted: {params}");
        assert_eq!(
            f.screen.borrow().ppqn.pending_new_ppqn,
            None,
            "case {name} left a pending candidate"
        );
    }
    assert_eq!(f.screen.borrow().ppqn.current_ppqn, 960);
}

/// Unknown `ppqn.*` actions and params on a domain target are
/// rejected at the shell/binding boundary — the op's action set is
/// closed, and `view` remains the only project-adjacent target that
/// may carry params.
#[test]
fn widget_unknown_action_and_target_boundary() {
    let mut f = fixture();
    forged_action(&mut f, "unk", "view", "ppqn.bogus", None);
    assert!(
        !results_for(&f.driver, f.window, "unk").last().unwrap().0,
        "unknown ppqn action accepted"
    );
    // A `project`-target action carrying params is not representable
    // — the binding rejects it before routing.
    forged_action(
        &mut f,
        "dom",
        "project",
        "save",
        Some(json!({ "new_ppqn": 480 })),
    );
    assert!(
        !results_for(&f.driver, f.window, "dom").last().unwrap().0,
        "domain action with params must reject"
    );
    // A non-object params blob cannot even name a candidate.
    forged_action(&mut f, "arr", "view", "ppqn.select", Some(json!([480])));
    assert!(
        !results_for(&f.driver, f.window, "arr").last().unwrap().0,
        "non-object ppqn params must reject"
    );
}

/// The same `syn.project.ppqn` op reachable from a widget is also
/// the typed driver path — arbitrary values outside the recommended
/// list (here 2400) select/preview/confirm identically.
#[test]
fn op_path_selects_arbitrary_value() {
    let mut f = fixture();
    let view_id = f.view_id.clone();
    op(
        &mut f,
        "sel",
        "syn.project.ppqn",
        json!({
            "view_id": view_id,
            "action": "select",
            "new_ppqn": 2400,
            "policy": "exact",
        }),
    );
    let (ok, _) = results_for(&f.driver, f.window, "sel")
        .last()
        .unwrap()
        .clone();
    assert!(ok, "op select failed");
    assert_eq!(f.screen.borrow().ppqn.pending_new_ppqn, Some(2400));
    op(
        &mut f,
        "cfm",
        "syn.project.ppqn",
        json!({ "view_id": view_id, "action": "confirm" }),
    );
    let (ok, data) = results_for(&f.driver, f.window, "cfm")
        .last()
        .unwrap()
        .clone();
    assert!(ok, "op confirm failed: {data:?}");
    assert_eq!(data.unwrap()["confirmed"], 2400);
    assert_eq!(f.screen.borrow().ppqn.current_ppqn, 2400);
}
