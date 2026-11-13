//! UI-C5 product wire-surface tests (synvelis#102) — headless.
//!
//! Exercises `syn.control.relation`, `syn.control.edge`,
//! `syn.recording.status` and `syn.recording.action` through the same
//! `SynvelisApp` op methods the `AppService` dispatch calls — the
//! identical path `UiRuntime::dispatch_op` reaches. These are
//! headless/OS-native checks only: they make no real-window, GUI,
//! plugin-window, high-DPI, keyboard/IME, or accessibility claims
//! (UI-C5-05 keeps those lanes Deferred/NotRun honestly).
//!
//! Fixture convention matches `sf05_signal_flow.rs`: project truth is
//! seeded through `ProjectController` commands; the ops under test
//! are the only writers exercised afterwards.

use ruviform_ui::BackendReport;
use ruviform_ui::ipc::ErrorBody;
use serde_json::{Value, json};

use synvelis_app::{SynvelisApp, main_view_id, main_view_instance};
use synvelis_project::{
    AutomationLaneId, AutomationTimeScope, BeatPoint, Command, ControlOwner,
    ModulationAssignmentId, ModulationSourceId, ParamMapping, ParameterRole, ParameterTargetId,
    Polarity, ProjectController, ProjectOp, ProjectRevision, SourceKind, TrackId, Transform,
    ValueKind, is_issuable_raw,
};
use synvelis_workspace::{AutomationMode, WriteArm};

fn app() -> SynvelisApp {
    SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()))
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap()
}

fn add_target(c: &mut ProjectController, name: &str) -> ParameterTargetId {
    apply(
        c,
        ProjectOp::AddParameterTarget {
            name: name.into(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    // Newest issuable target — the bootstrap master-strip fader is a
    // well-known (non-issuable) id and never matches the filter.
    *c.live()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .expect("param target")
}

fn add_lane(c: &mut ProjectController, target: ParameterTargetId) -> AutomationLaneId {
    apply(c, ProjectOp::AddAutomationLane { target });
    *c.live()
        .control
        .lanes
        .keys()
        .next_back()
        .expect("automation lane")
}

fn add_source(c: &mut ProjectController, value: f64) -> ModulationSourceId {
    apply(
        c,
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Project,
            kind: SourceKind::Macro { value },
        },
    );
    *c.live()
        .control
        .sources
        .keys()
        .next_back()
        .expect("modulation source")
}

fn add_assignment(
    c: &mut ProjectController,
    source: ModulationSourceId,
    target: ParameterTargetId,
    depth_source: Option<ParameterTargetId>,
    enabled: bool,
) -> ModulationAssignmentId {
    apply(
        c,
        ProjectOp::AddModulationAssignment {
            source,
            target,
            depth: 0.25,
            depth_source,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled,
        },
    );
    *c.live()
        .control
        .assignments
        .keys()
        .next_back()
        .expect("modulation assignment")
}

/// The control neighborhood the relation tests walk:
///
/// ```text
///   lane ──AutomationDrives──▶ p1 ◀──ModulationDrives── a1 ◀── s1
///     ▲                        │ DepthControlledBy        ▲
///     │                        └──────────────▶ a2 ◀─────┘
///     │                          ▲
///     └──── ModulationDrives ────┘──▶ p3   (a2: s2→p3, depth_source=p1)
/// ```
struct ControlFixture {
    p1: ParameterTargetId,
    p3: ParameterTargetId,
    lane: AutomationLaneId,
    s1: ModulationSourceId,
    s2: ModulationSourceId,
    a1: ModulationAssignmentId,
    a2: ModulationAssignmentId,
}

fn seed_control(app: &mut SynvelisApp) -> ControlFixture {
    let c = app.workspace_mut().session_mut().controller_mut();
    let p1 = add_target(c, "drive");
    let p3 = add_target(c, "modulated");
    let lane = add_lane(c, p1);
    let s1 = add_source(c, 0.5);
    let s2 = add_source(c, 0.25);
    let a1 = add_assignment(c, s1, p1, None, true);
    let a2 = add_assignment(c, s2, p3, Some(p1), true);
    ControlFixture {
        p1,
        p3,
        lane,
        s1,
        s2,
        a1,
        a2,
    }
}

fn entity(kind: &str, id: &str) -> Value {
    json!({ "kind": kind, "id": id })
}

fn param_e(id: ParameterTargetId) -> Value {
    entity("parameter", &id.to_tagged())
}

fn lane_e(id: AutomationLaneId) -> Value {
    entity("automation-lane", &id.to_tagged())
}

fn source_e(id: ModulationSourceId) -> Value {
    entity("modulation-source", &id.to_tagged())
}

fn assign_e(id: ModulationAssignmentId) -> Value {
    entity("assignment", &id.to_tagged())
}

fn relation(app: &mut SynvelisApp, params: &Value) -> Value {
    app.op_control_relation(params)
        .expect("syn.control.relation")
}

fn edge(app: &mut SynvelisApp, params: &Value) -> Value {
    app.op_control_edge(params).expect("syn.control.edge")
}

fn undo_depth(app: &SynvelisApp) -> usize {
    app.workspace()
        .session()
        .controller()
        .history_states()
        .count()
}

// ---------------------------------------------------------------------------
// UI-C5-01/03 — relation inspection, Explore history, mode stability
// ---------------------------------------------------------------------------

/// Parameter → Assignment → `ModulationSource` and Parameter →
/// `AutomationLane` exploration through `syn.control.relation`, with
/// `back` returning to the origin anchor — "元画面へ戻る".
#[test]
fn relation_explore_bidirectional_and_back_to_origin() {
    let mut app = app();
    let f = seed_control(&mut app);

    // No focus yet: view reports an empty inspection and no relation.
    let v = relation(&mut app, &json!({ "action": "view" }));
    assert_eq!(v["inspection"]["target"], Value::Null);
    assert_eq!(v["inspection"]["mode"], "follow");
    assert_eq!(v["relation"], Value::Null);

    // Stateless anchor query: relation rows for the lane without
    // moving the (empty) focus.
    let lane_view = relation(
        &mut app,
        &json!({ "action": "view", "anchor": lane_e(f.lane) }),
    );
    assert_eq!(lane_view["inspection"]["target"], Value::Null);
    let lane_rows = lane_view["relation"]["rows"].as_array().unwrap();
    assert_eq!(lane_rows.len(), 1);
    assert_eq!(lane_rows[0]["kind"], "automation-drives");
    assert_eq!(lane_rows[0]["target"]["id"], f.p1.to_tagged());
    assert_eq!(lane_view["relation"]["coverage"], "complete");

    // Inspect the parameter — enters Explore with the param anchor.
    let r = relation(
        &mut app,
        &json!({ "action": "inspect", "entity": param_e(f.p1) }),
    );
    assert_eq!(r["inspection"]["mode"], "explore");
    assert_eq!(r["inspection"]["target"], f.p1.to_tagged());
    assert_eq!(r["inspection"]["can_back"], false);
    let rows = r["relation"]["rows"].as_array().unwrap();
    assert!(
        rows.iter()
            .any(|row| row["kind"] == "automation-drives"
                && row["source"]["kind"] == "automation-lane"),
        "lane -> param edge: {rows:?}"
    );
    assert!(
        rows.iter()
            .any(|row| row["kind"] == "modulation-drives" && row["source"]["kind"] == "assignment"),
        "assignment -> param edge: {rows:?}"
    );
    // Depth-controlled-by: p1 depth-drives a2 (which targets p3).
    assert!(
        rows.iter().any(|row| row["kind"] == "depth-controlled-by"),
        "depth edge: {rows:?}"
    );
    assert_eq!(
        r["relation"]["project_revision"],
        json!(app.workspace().session().revision().0)
    );

    // Descend Parameter → Assignment: the assignment's rows reach the
    // ModulationSource (and onward to the affected Parameter p3 via
    // its own ModulationDrives edge).
    relation(
        &mut app,
        &json!({ "action": "inspect", "entity": assign_e(f.a2) }),
    );
    let a2 = relation(&mut app, &json!({ "action": "view" }));
    assert_eq!(a2["inspection"]["target"], f.a2.to_tagged());
    assert_eq!(a2["inspection"]["can_back"], true);
    let a2_rows = a2["relation"]["rows"].as_array().unwrap();
    assert!(
        a2_rows
            .iter()
            .any(|row| row["source"]["kind"] == "modulation-source"
                && row["source"]["id"] == f.s2.to_tagged()),
        "source -> assignment edge: {a2_rows:?}"
    );
    assert!(
        a2_rows
            .iter()
            .any(|row| row["target"]["kind"] == "parameter"
                && row["target"]["id"] == f.p3.to_tagged()),
        "assignment -> affected parameter edge: {a2_rows:?}"
    );

    // "元画面へ戻る": back restores the parameter anchor.
    relation(&mut app, &json!({ "action": "back" }));
    let back = relation(&mut app, &json!({ "action": "view" }));
    assert_eq!(back["inspection"]["target"], f.p1.to_tagged());
    assert_eq!(back["inspection"]["can_forward"], true);

    // Forward re-descends; the stacks stay consistent.
    relation(&mut app, &json!({ "action": "forward" }));
    let fwd = relation(&mut app, &json!({ "action": "view" }));
    assert_eq!(fwd["inspection"]["target"], f.a2.to_tagged());
}

/// Inspect is observation-only: editing selection, inspection focus,
/// project revision and undo history are all untouched (inspect ≠
/// edit, UI-C5-02).
#[test]
fn relation_inspect_never_touches_edit_or_undo() {
    let mut app = app();
    let f = seed_control(&mut app);
    let inst = main_view_instance();
    let undo_before = undo_depth(&app);
    let rev_before = app.workspace().session().revision();

    // Adopt the view's workspace state, then snapshot the editing
    // surfaces the relation op must not move.
    relation(&mut app, &json!({ "action": "view" }));
    let (sel_before, focus_before) = {
        let s = app.workspace().state(inst).unwrap();
        (s.object_selections.clone(), s.inspection_focus.clone())
    };

    relation(
        &mut app,
        &json!({ "action": "inspect", "entity": param_e(f.p1) }),
    );
    relation(
        &mut app,
        &json!({ "action": "inspect", "entity": assign_e(f.a1) }),
    );
    relation(&mut app, &json!({ "action": "back" }));
    relation(&mut app, &json!({ "action": "set_mode", "mode": "pinned" }));
    relation(
        &mut app,
        &json!({ "action": "set_follow_source", "follow_source": { "view": main_view_id() } }),
    );

    let s = app.workspace().state(inst).unwrap();
    assert_eq!(s.object_selections, sel_before, "editing selection moved");
    assert_eq!(s.inspection_focus, focus_before, "inspection focus moved");
    assert_eq!(app.workspace().session().revision(), rev_before);
    assert_eq!(undo_depth(&app), undo_before, "inspect created undo");

    // Pinned mode + follow-source round-trip on the wire.
    let v = relation(&mut app, &json!({ "action": "view" }));
    assert_eq!(v["inspection"]["mode"], "pinned");
    assert_eq!(
        v["inspection"]["follow_source"]["view"],
        json!(main_view_id())
    );
    relation(
        &mut app,
        &json!({ "action": "set_follow_source", "follow_source": null }),
    );
    let v = relation(&mut app, &json!({ "action": "view" }));
    assert_eq!(v["inspection"]["follow_source"], Value::Null);
}

/// Malformed entities and unknown actions fail closed at the wire
/// boundary — never a guessed anchor.
#[test]
fn relation_rejects_malformed_entity_and_action() {
    let mut app = app();
    seed_control(&mut app);

    assert!(
        app.op_control_relation(&json!({
            "action": "inspect",
            "entity": { "kind": "parameter", "id": "not-a-tag" },
        }))
        .is_err()
    );
    assert!(
        app.op_control_relation(&json!({
            "action": "inspect",
            "entity": { "kind": "bus", "id": "bus-00" },
        }))
        .is_err()
    );
    assert!(
        app.op_control_relation(&json!({ "action": "teleport" }))
            .is_err()
    );
    assert!(
        app.op_control_relation(&json!({
            "action": "set_mode",
            "mode": "sideways",
        }))
        .is_err()
    );
}

// ---------------------------------------------------------------------------
// UI-C5-02 — Control Layer edge gesture: distinct from routing, fixed
// intent, preview/cancel free, commit = exactly one undoable op,
// stale drops instead of retargeting.
// ---------------------------------------------------------------------------

/// begin → preview → commit applies exactly one
/// `AddModulationAssignment` at the fixated revision; preview and
/// state reads never touch the project or undo.
#[test]
fn control_edge_commit_applies_one_assignment() {
    let mut app = app();
    let fx = seed_control(&mut app);
    let rev = app.workspace().session().revision();
    let undo_before = undo_depth(&app);

    // Wrong endpoint families are rejected before any gesture starts.
    assert!(
        app.op_control_edge(&json!({
            "action": "begin",
            "source": param_e(fx.p1),
            "target": param_e(fx.p3),
        }))
        .is_err(),
        "parameter -> parameter is not a control edge"
    );
    assert!(
        app.op_control_edge(&json!({
            "action": "begin",
            "source": source_e(fx.s1),
            "target": source_e(fx.s2),
        }))
        .is_err(),
        "source -> source is not a control edge"
    );

    let begin = edge(
        &mut app,
        &json!({
            "action": "begin",
            "source": source_e(fx.s1),
            "target": param_e(fx.p3),
        }),
    );
    assert_eq!(begin["gesture"]["fixation"]["source"], fx.s1.to_tagged());
    assert_eq!(begin["gesture"]["fixation"]["target"], fx.p3.to_tagged());
    assert_eq!(begin["gesture"]["fixation"]["revision"], json!(rev.0));
    assert_eq!(begin["gesture"]["fixation"]["view"], json!(main_view_id()));

    // A second begin while the gesture runs is refused — one gesture
    // per view, and the running one is never silently replaced.
    assert!(
        app.op_control_edge(&json!({
            "action": "begin",
            "source": source_e(fx.s2),
            "target": param_e(fx.p1),
        }))
        .is_err()
    );

    // Preview mutates only gesture state — revision and undo depth
    // are untouched (no undo boundary can appear).
    edge(
        &mut app,
        &json!({ "action": "preview", "depth": 0.5, "polarity": "unipolar" }),
    );
    assert_eq!(app.workspace().session().revision(), rev);
    assert_eq!(undo_depth(&app), undo_before);

    // The gesture is visible to `state` and *not* to the Audio
    // Routing gesture surface — the two families cannot be confused.
    let st = edge(&mut app, &json!({ "action": "state" }));
    assert_eq!(st["gesture"]["preview"]["depth"], json!(0.5));
    assert_eq!(st["gesture"]["preview"]["polarity"], "unipolar");
    let routing = app
        .op_signal_flow_routing(&json!({ "action": "state" }))
        .expect("syn.signal_flow.routing state");
    assert_eq!(routing["gesture"], Value::Null);

    let commit = edge(&mut app, &json!({ "action": "commit" }));
    assert_eq!(commit["outcome"]["outcome"], "applied");
    assert_eq!(commit["outcome"]["revision"], json!(rev.0 + 1));
    assert_eq!(app.workspace().session().revision().0, rev.0 + 1);
    assert_eq!(undo_depth(&app), undo_before + 1, "commit added one undo");

    // Exactly one assignment landed, carrying the previewed values.
    let landed = app
        .workspace()
        .session()
        .project()
        .control
        .assignments
        .values()
        .filter(|a| a.source == fx.s1 && a.target == fx.p3)
        .count();
    assert_eq!(landed, 1);
    let assignment = app
        .workspace()
        .session()
        .project()
        .control
        .assignments
        .values()
        .find(|a| a.source == fx.s1 && a.target == fx.p3)
        .unwrap();
    assert_eq!(assignment.depth, 0.5);
    assert_eq!(assignment.polarity, Polarity::Unipolar);

    // The gesture is consumed — state reports none.
    let st = edge(&mut app, &json!({ "action": "state" }));
    assert_eq!(st["gesture"], Value::Null);

    // The single commit is undoable — and undoing it removes the
    // assignment (project mutation boundary held).
    let undo = app.op_project_undo().expect("syn.project.undo");
    assert_eq!(undo["undone"], true);
    let remaining = app
        .workspace()
        .session()
        .project()
        .control
        .assignments
        .values()
        .filter(|a| a.source == fx.s1 && a.target == fx.p3)
        .count();
    assert_eq!(remaining, 0);
}

/// Preview → cancel leaves no project change and no undo boundary;
/// the cancel reason is echoed verbatim.
#[test]
fn control_edge_preview_cancel_is_free() {
    let mut app = app();
    let f = seed_control(&mut app);
    let rev = app.workspace().session().revision();
    let undo_before = undo_depth(&app);

    edge(
        &mut app,
        &json!({
            "action": "begin",
            "source": source_e(f.s1),
            "target": param_e(f.p1),
        }),
    );
    edge(&mut app, &json!({ "action": "preview", "depth": 2.0 }));
    let c = edge(&mut app, &json!({ "action": "cancel", "reason": "escape" }));
    assert_eq!(c["outcome"]["outcome"], "cancelled");
    assert_eq!(c["outcome"]["reason"], "escape");

    assert_eq!(app.workspace().session().revision(), rev);
    assert_eq!(undo_depth(&app), undo_before);
    let st = edge(&mut app, &json!({ "action": "state" }));
    assert_eq!(st["gesture"], Value::Null);
    // No assignment was created for s1 → p1 beyond the seeded a1 —
    // count stays at the fixture's one.
    let count = app
        .workspace()
        .session()
        .project()
        .control
        .assignments
        .values()
        .filter(|a| a.source == f.s1 && a.target == f.p1)
        .count();
    assert_eq!(count, 1, "only the seeded a1 exists");

    // A gesture that was never begun cannot commit or cancel.
    assert!(app.op_control_edge(&json!({ "action": "commit" })).is_err());
    assert!(app.op_control_edge(&json!({ "action": "cancel" })).is_err());
}

/// A project revision that moved between begin and commit drops the
/// gesture as `stale` — it never retargets to the new revision.
#[test]
fn control_edge_commit_stale_revision_drops_gesture() {
    let mut app = app();
    let f = seed_control(&mut app);

    edge(
        &mut app,
        &json!({
            "action": "begin",
            "source": source_e(f.s1),
            "target": param_e(f.p3),
        }),
    );
    edge(&mut app, &json!({ "action": "preview", "depth": 0.75 }));

    // Foreign project change mid-gesture — the fixated revision is
    // now stale.
    let rev = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "renamed".into(),
            },
            rev,
        )
        .unwrap();

    let c = edge(&mut app, &json!({ "action": "commit" }));
    assert_eq!(c["outcome"]["outcome"], "stale");

    // The gesture is gone and nothing was applied — no assignment
    // for s1 → p3 exists.
    let st = edge(&mut app, &json!({ "action": "state" }));
    assert_eq!(st["gesture"], Value::Null);
    let count = app
        .workspace()
        .session()
        .project()
        .control
        .assignments
        .values()
        .filter(|a| a.source == f.s1 && a.target == f.p3)
        .count();
    assert_eq!(count, 0);
}

// ---------------------------------------------------------------------------
// UI-C5-04 — recording status read model + explicit actions
// ---------------------------------------------------------------------------

fn bp(num: i64) -> BeatPoint {
    BeatPoint::new(num, 1).unwrap()
}

/// Track + track-owned target + automation lane — the minimum a
/// write-armed recording transaction fixes.
fn add_track_target(app: &mut SynvelisApp, name: &str) -> (TrackId, ParameterTargetId) {
    let c = app.workspace_mut().session_mut().controller_mut();
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(
        c,
        ProjectOp::AddParameterTarget {
            name: name.into(),
            owner: ControlOwner::Track(track),
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.4,
        },
    );
    let target = *c
        .live()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .unwrap();
    (track, target)
}

/// Arms `target`'s lane and opens a recording transaction on a
/// rolling transport — mirrors the workspace `uic5_04` fixture.
fn open_recording(app: &mut SynvelisApp, target: ParameterTargetId) -> AutomationLaneId {
    let lane = {
        let c = app.workspace_mut().session_mut().controller_mut();
        apply(c, ProjectOp::AddAutomationLane { target });
        *c.live().control.lanes.keys().next_back().unwrap()
    };
    {
        let s = app.workspace_mut().session_mut();
        s.control_with(|c, p| c.bind_write_lane(p, target, lane))
            .unwrap();
        s.control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
            .unwrap();
        s.control_with(|c, p| c.set_arm(p, target, WriteArm::Armed))
            .unwrap();
        s.control_mut().set_record_enabled(true);
        s.control_with(synvelis_workspace::ControlSession::transport_play);
        let rev = s.revision();
        s.control_with(|c, p| c.record_start(p, rev)).unwrap();
    }
    lane
}

fn write_point(app: &mut SynvelisApp, target: ParameterTargetId, pos: BeatPoint, value: f64) {
    app.workspace_mut()
        .session_mut()
        .control_with(|c, p| c.record_write(p, target, pos, value))
        .unwrap();
}

fn recording_status(app: &SynvelisApp) -> Value {
    app.op_recording_status(&json!({}))
        .expect("syn.recording.status")
}

fn recording_action(app: &mut SynvelisApp, params: &Value) -> Result<Value, ErrorBody> {
    app.op_recording_action(params)
}

/// Idle session → `unknown` with no actions; an open transaction →
/// `pending` with typed provenance and the legal Inspect/Retry/
/// Abort set; abort returns to `unknown` with `Aborted` provenance.
#[test]
fn recording_status_pending_actions_and_abort() {
    let mut app = app();

    let idle = recording_status(&app);
    assert_eq!(idle["state"], "unknown");
    assert_eq!(idle["actions"], json!([]));
    assert_eq!(idle["transaction"], Value::Null);

    let (_track, target) = add_track_target(&mut app, "vol");
    open_recording(&mut app, target);
    write_point(&mut app, target, bp(1), 0.6);
    write_point(&mut app, target, bp(2), 0.8);

    let st = recording_status(&app);
    assert_eq!(st["state"], "pending");
    assert_eq!(st["record_enabled"], true);
    assert_eq!(st["transport"], "playing");
    assert_eq!(
        st["actions"],
        json!(["inspect", "retry", "abort"]),
        "Pending legal actions"
    );
    let tx = &st["transaction"];
    assert!(tx["id"].as_str().unwrap().starts_with("rec-tx-"));
    assert_eq!(tx["recorded_points"], 2);
    assert_eq!(tx["revision_stale"], false);
    // Typed provenance rides every explanation row.
    assert!(
        st["explanations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["provenance"]["kind"].is_string())
    );

    // Inspect reports what an explicit rebase would write — a read,
    // never a merge.
    let insp = recording_action(&mut app, &json!({ "action": "inspect" })).expect("inspect");
    assert_eq!(insp["preview"]["lanes"].as_array().unwrap().len(), 1);
    assert!(insp["preview"]["fixed_revision"].is_u64());

    // Abort discards the transaction — nothing applied, state Unknown
    // with the Aborted disposition surfaced through provenance.
    recording_action(&mut app, &json!({ "action": "abort" })).expect("abort");
    let st = recording_status(&app);
    assert_eq!(st["state"], "unknown");
    assert_eq!(st["transaction"], Value::Null);
    assert_eq!(st["actions"], json!([]));
    assert!(
        st["explanations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["provenance"]["kind"] == "disposition"
                && e["provenance"]["disposition"]["kind"] == "aborted"),
        "aborted provenance: {st}"
    );
}

/// Foreign project drift → `conflict` with no Retry; Inspect +
/// explicit Rebase re-pin the transaction to `pending`; Retry then
/// commits exactly once → `applied` with receipt provenance.
#[test]
fn recording_conflict_rebase_retry_applied() {
    let mut app = app();
    let (_track, target) = add_track_target(&mut app, "vol");
    open_recording(&mut app, target);
    write_point(&mut app, target, bp(1), 0.5);
    write_point(&mut app, target, bp(2), 0.8);

    // Foreign drift seals the transaction at the predictive check —
    // Conflict *before* any commit attempt.
    let rev = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(ProjectOp::AddTrack { name: "x".into() }, rev)
        .unwrap();

    let st = recording_status(&app);
    assert_eq!(st["state"], "conflict");
    assert_eq!(
        st["actions"],
        json!(["inspect", "rebase", "abort"]),
        "Conflict offers no Retry until rebase"
    );
    assert_eq!(st["transaction"]["revision_stale"], true);
    assert_eq!(st["transaction"]["sealed"], "project-change");
    assert!(
        st["explanations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["provenance"]["kind"] == "revision-drift"),
        "revision-drift provenance: {st}"
    );

    // Retry during Conflict is a typed rejection, not a silent write.
    assert!(
        recording_action(&mut app, &json!({ "action": "retry" })).is_err(),
        "conflict retry must fail"
    );

    // Inspect → explicit Rebase → Pending at the new fixed revision.
    recording_action(&mut app, &json!({ "action": "inspect" })).expect("inspect");
    recording_action(&mut app, &json!({ "action": "rebase" })).expect("rebase");
    let st = recording_status(&app);
    assert_eq!(st["state"], "pending");
    assert_eq!(st["transaction"]["sealed"], "project-change");
    assert_eq!(st["transaction"]["revision_stale"], false);

    // Retry now commits — one applied outcome, receipt provenance,
    // and the view settles on `applied` with no legal actions.
    let out =
        recording_action(&mut app, &json!({ "action": "retry" })).expect("retry after rebase");
    assert_eq!(out["outcome"]["applied"], true);
    assert!(out["outcome"]["revision"].is_u64());

    let st = recording_status(&app);
    assert_eq!(st["state"], "applied");
    assert_eq!(st["actions"], json!([]));
    assert_eq!(st["transaction"], Value::Null);
    assert_eq!(
        st["outcome"]["receipt"]["revision"],
        out["outcome"]["revision"]
    );
}

/// `syn.recording.action` rejects unknown actions and an explicit
/// rebase id that does not name the open transaction.
#[test]
fn recording_action_rejects_unknown_and_wrong_id() {
    let mut app = app();
    let (_track, target) = add_track_target(&mut app, "vol");
    open_recording(&mut app, target);

    assert!(
        app.op_recording_action(&json!({ "action": "obliterate" }))
            .is_err()
    );
    // A well-formed but non-open transaction id cannot be rebased.
    assert!(
        recording_action(&mut app, &json!({ "action": "rebase", "id": "rec-tx-999" })).is_err(),
        "unknown transaction id must fail"
    );

    // Abort with an explicit bogus action still leaves the session
    // honest — unknown action changed nothing.
    let st = recording_status(&app);
    assert_eq!(st["state"], "pending");
}
