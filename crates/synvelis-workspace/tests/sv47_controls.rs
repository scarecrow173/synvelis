//! SV-47 acceptance tests — Ruviform controls/binding contract
//! adopted into Synvelis Mixer / Transport / parameter surfaces.
//!
//! The headless contract (`ui/core` gesture model + `ui/binding`
//! command lifecycle) is verified end to end against the real session
//! semantics: intents dispatch onto `ProjectSession`, receipts report
//! the command result, and only landed projections are truth —
//! mirroring the GW-09 fixtures approach. No GUI, no real plugin.

use ruviform_plugin_core::ParamFlags;
use synvelis_project::{
    AutomationLaneId, AutomationTimeScope, BeatPoint, ControlOwner, CounterIdGenerator,
    ParamMapping, ParameterRole, ParameterTargetId, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, TargetCapability, ValueKind, is_issuable_raw,
};
use synvelis_workspace::{
    AutomationMode, CommandIntent, CommandPhase, ControlsUi, DispatchContext, GestureCancelReason,
    ObservationState, PlaybackStatus, ProjectSession, ReceiptStatus, WriteArm,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

fn fresh() -> (ProjectSession, ControlsUi) {
    let project = Project::new(ProjectId::new(0x547), "sv47-test");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    (ProjectSession::new(controller), ControlsUi::new())
}

fn apply(s: &mut ProjectSession, op: ProjectOp) -> ProjectRevision {
    let rev = s.revision();
    s.apply(op, rev).expect("fixture op")
}

fn bp(num: i64) -> BeatPoint {
    BeatPoint::new(num, 1).unwrap()
}

fn caps(flags: ParamFlags) -> TargetCapability {
    TargetCapability {
        param_id: 7,
        flags_bits: flags.bits(),
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.0,
    }
}

fn add_target(s: &mut ProjectSession, base: f64) -> ParameterTargetId {
    apply(
        s,
        ProjectOp::AddParameterTarget {
            name: "p".into(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base,
        },
    );
    // The bootstrap master-fader target is well-known (never issued) —
    // pick the newest *issued* target.
    *s.project()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .unwrap()
}

fn add_readonly_target(s: &mut ProjectSession) -> ParameterTargetId {
    apply(
        s,
        ProjectOp::AddParameterTarget {
            name: "ro".into(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(caps(ParamFlags::AUTOMATABLE.union(ParamFlags::READONLY))),
            base: 0.5,
        },
    );
    *s.project()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .unwrap()
}

fn add_lane(s: &mut ProjectSession, target: ParameterTargetId) -> AutomationLaneId {
    apply(s, ProjectOp::AddAutomationLane { target });
    *s.project().control.lanes.keys().last().unwrap()
}

fn tag(id: ParameterTargetId) -> String {
    id.to_tagged()
}

fn v(plain: f64) -> synvelis_workspace::ControlValue {
    synvelis_workspace::ControlValue {
        plain,
        normalized: plain,
    }
}

fn begin(target: &str, gid: &str) -> CommandIntent {
    CommandIntent::Begin {
        target: target.into(),
        control_id: "fader".into(),
        sequence: 1,
        gesture_id: gid.into(),
        origin_view: None,
        atomic: None,
        value: v(0.5),
    }
}

fn preview(target: &str, gid: &str, value: f64) -> CommandIntent {
    CommandIntent::Preview {
        target: target.into(),
        control_id: "fader".into(),
        sequence: 2,
        gesture_id: gid.into(),
        origin_view: None,
        atomic: None,
        value: v(value),
        coalescible: true,
    }
}

fn commit(target: &str, gid: &str, value: f64) -> CommandIntent {
    CommandIntent::Commit {
        target: target.into(),
        control_id: "fader".into(),
        sequence: 3,
        gesture_id: gid.into(),
        origin_view: None,
        atomic: None,
        value: v(value),
    }
}

fn cancel(target: &str, gid: &str) -> CommandIntent {
    CommandIntent::Cancel {
        target: target.into(),
        control_id: "fader".into(),
        sequence: 4,
        gesture_id: gid.into(),
        origin_view: None,
        atomic: None,
        reason: GestureCancelReason::Escape,
    }
}

fn act(target: &str, action: &str) -> CommandIntent {
    CommandIntent::Action {
        target: target.into(),
        control_id: "transport-bar".into(),
        sequence: 1,
        origin_view: None,
        atomic: None,
        action: action.into(),
        source: None,
    }
}

/// Mints a dispatch context stamped with the binding's live
/// generation — like the upstream binding, which always stamps its
/// own current epoch. A first dispatch (no binding yet) stamps the
/// producing view's generation.
fn ctx(ui: &ControlsUi, view: &str, target: &str, cmd: &str) -> DispatchContext {
    let generation = ui
        .binding(view, target)
        .map_or_else(|| ui.view_state(view).map_or(0, |v| v.1), |b| b.generation);
    DispatchContext {
        command_id: cmd.into(),
        view_id: Some(view.into()),
        window_id: None,
        generation,
        issued_at: 0,
    }
}

fn wire_status(ui: &ControlsUi) -> Vec<ReceiptStatus> {
    ui.wire().iter().map(|d| d.receipt.status).collect()
}

/// Drains the wire, delivering each receipt to its recorded
/// (view, target) binding, and returns the last one — inspect the
/// wire then deliver it.
fn last_receipt(ui: &mut ControlsUi) -> synvelis_workspace::CommandReceipt {
    let drained = ui.drain_wire();
    let last = drained.last().expect("a receipt").receipt.clone();
    for d in drained {
        ui.deliver(&d.view, &d.target, d.receipt);
    }
    last
}

// ---------------------------------------------------------------------------
// AC1 — gesture → explicit command → receipt → all-window truth
// ---------------------------------------------------------------------------

#[test]
fn drag_commits_base_edit_and_all_windows_observe() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.attach_view(&mut s, "inspector", "w2");
    ui.subscribe(&s, "mixer", &t).unwrap();
    ui.subscribe(&s, "inspector", &t).unwrap();

    // Subscribe pushed the current truth to both views.
    for view in ["mixer", "inspector"] {
        let row = ui.strip_row(view, &t).unwrap();
        assert_eq!(row.phase, CommandPhase::Observed);
        assert_eq!(
            row.observed.unwrap().breakdown.evaluation.host_normalized,
            0.5
        );
    }

    // A fader drag in the mixer window: begin → preview → commit.
    // Off mode: the commit is a base edit — one undoable apply.
    ui.set_mode(&mut s, p, AutomationMode::Off).unwrap();
    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    assert_eq!(wire_status(&ui), [ReceiptStatus::Accepted]);
    ui.pump();
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.phase, CommandPhase::Preview);

    ui.dispatch(&mut s, preview(&t, "g1", 0.8), ctx(&ui, "mixer", &t, "c2"));
    // Previews mint no receipts — the droppable channel.
    assert!(ui.wire().is_empty());
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.preview, Some(0.8));

    ui.dispatch(&mut s, commit(&t, "g1", 0.9), ctx(&ui, "mixer", &t, "c3"));
    let r = last_receipt(&mut ui);
    // Off-mode commit applies a base edit — one undoable mutation.
    assert_eq!(r.status, ReceiptStatus::Applied);
    assert!(r.revision.is_some());
    ui.pump();

    // Both windows re-projected the same authoritative truth.
    for view in ["mixer", "inspector"] {
        let row = ui.strip_row(view, &t).unwrap();
        assert_eq!(row.phase, CommandPhase::Observed);
        assert_eq!(
            row.observed.unwrap().breakdown.evaluation.host_normalized,
            0.9
        );
    }
    assert_eq!(s.project().control.targets.get(&p).unwrap().base, 0.9);

    // The commit is exactly one undo step.
    ui.undo(&mut s).unwrap();
    assert_eq!(s.project().control.targets.get(&p).unwrap().base, 0.5);
}

#[test]
fn second_window_begin_is_rejected_and_previews_never_misroute() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.attach_view(&mut s, "remote", "w2");
    ui.subscribe(&s, "mixer", &t).unwrap();
    ui.subscribe(&s, "remote", &t).unwrap();

    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.pump();
    // A second window grabbing the same target is rejected, never
    // silently merged into the live gesture.
    ui.dispatch(&mut s, begin(&t, "g2"), ctx(&ui, "remote", &t, "r1"));
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Rejected);

    // The rejected gesture's previews are dropped — they never reach
    // the live session gesture.
    ui.dispatch(&mut s, preview(&t, "g2", 0.1), ctx(&ui, "remote", &t, "r2"));
    ui.dispatch(&mut s, preview(&t, "g1", 0.7), ctx(&ui, "mixer", &t, "c2"));
    assert_eq!(ui.strip_row("mixer", &t).unwrap().preview, Some(0.7));
    assert!(ui.strip_row("remote", &t).unwrap().preview.is_none());
    assert_eq!(ui.diagnostics().dropped_previews, 1);

    // The foreign gesture's commit is rejected — it cannot close the
    // mixer's gesture.
    ui.dispatch(&mut s, commit(&t, "g2", 0.1), ctx(&ui, "remote", &t, "r3"));
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Rejected);
    assert_eq!(
        ui.strip_row("mixer", &t).unwrap().phase,
        CommandPhase::Preview
    );
}

#[test]
fn duplicate_command_id_is_exactly_once() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();

    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    // An IPC retry of the same command must not re-apply.
    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    assert_eq!(ui.diagnostics().duplicate_commands, 1);
    assert_eq!(wire_status(&ui), [ReceiptStatus::Accepted]);
}

#[test]
fn cancel_discards_preview_and_rewrites_nothing() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();

    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.dispatch(&mut s, preview(&t, "g1", 0.9), ctx(&ui, "mixer", &t, "c2"));
    ui.dispatch(&mut s, cancel(&t, "g1"), ctx(&ui, "mixer", &t, "c3"));
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Accepted);
    ui.pump();

    // The uncommitted preview is gone — truth is the recorded base.
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.phase, CommandPhase::Observed);
    assert_eq!(
        row.observed.unwrap().breakdown.evaluation.host_normalized,
        0.5
    );
    assert_eq!(s.project().control.targets.get(&p).unwrap().base, 0.5);
}

// ---------------------------------------------------------------------------
// AC2 — automation modes write through the existing session semantics
// ---------------------------------------------------------------------------

#[test]
fn touch_gesture_writes_to_transaction_and_commits_as_one_undo_step() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.2);
    let t = tag(p);
    let lane = add_lane(&mut s, p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();

    ui.set_mode(&mut s, p, AutomationMode::Touch).unwrap();
    ui.set_arm(&mut s, p, WriteArm::Armed).unwrap();
    ui.bind_write_lane(&mut s, p, lane).unwrap();
    ui.set_record_enabled(&mut s, true);
    ui.dispatch(
        &mut s,
        act("transport", "play"),
        ctx(&ui, "mixer", &t, "t1"),
    );
    ui.pump();
    ui.record_start(&mut s).unwrap();

    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.dispatch(&mut s, preview(&t, "g1", 0.6), ctx(&ui, "mixer", &t, "c2"));
    ui.dispatch(&mut s, commit(&t, "g1", 0.8), ctx(&ui, "mixer", &t, "c3"));
    let r = last_receipt(&mut ui);
    // The gesture interval landed in the open transaction — applied,
    // though the project apply waits for record_commit.
    assert_eq!(r.status, ReceiptStatus::Applied);

    let outcome = ui.record_commit(&mut s).unwrap();
    assert!(outcome.applied);
    let rev_before_undo = s.revision();
    ui.undo(&mut s).unwrap();
    // One undo step removed the whole recorded region.
    assert!(s.revision() > rev_before_undo);
    let lane_ref = &s.project().control.lanes[&lane];
    assert_eq!(lane_ref.regions.len(), 0);
}

#[test]
fn display_only_ops_never_mutate_the_project() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();
    let rev = s.revision();

    ui.set_mode(&mut s, p, AutomationMode::Latch).unwrap();
    ui.set_arm(&mut s, p, WriteArm::Armed).unwrap();
    ui.set_record_enabled(&mut s, true);

    // Session ops advance the projection revision but never the
    // project's — and every bound view sees the new truth.
    assert_eq!(s.revision(), rev);
    let row = ui.strip_row("mixer", &t).unwrap();
    let proj = row.observed.unwrap();
    assert_eq!(proj.breakdown.mode, AutomationMode::Latch);
    assert_eq!(proj.breakdown.arm, WriteArm::Armed);
}

#[test]
fn latch_mode_commit_reports_latched() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.2);
    let t = tag(p);
    let lane = add_lane(&mut s, p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();

    ui.set_mode(&mut s, p, AutomationMode::Latch).unwrap();
    ui.set_arm(&mut s, p, WriteArm::Armed).unwrap();
    ui.bind_write_lane(&mut s, p, lane).unwrap();
    ui.set_record_enabled(&mut s, true);
    ui.dispatch(
        &mut s,
        act("transport", "play"),
        ctx(&ui, "mixer", &t, "t1"),
    );
    ui.pump();
    ui.record_start(&mut s).unwrap();

    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.dispatch(&mut s, commit(&t, "g1", 0.8), ctx(&ui, "mixer", &t, "c2"));
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Applied);
    assert_eq!(r.detail.as_deref(), Some("latched"));
}

// ---------------------------------------------------------------------------
// AC3 — rejection paths are explicit and honest
// ---------------------------------------------------------------------------

#[test]
fn readonly_target_rejects_begin() {
    let (mut s, mut ui) = fresh();
    let p = add_readonly_target(&mut s);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();

    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.pump();

    // A rejected `begin` is not receipt-expected, so it never
    // displaces the phase — the rejection is reported on the binding's
    // last receipt and the observed truth stands (upstream rule).
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(
        row.last_receipt.as_ref().unwrap().status,
        ReceiptStatus::Rejected
    );
    assert_eq!(
        row.last_receipt.as_ref().unwrap().detail.as_deref(),
        Some("param_readonly")
    );
    assert_eq!(row.phase, CommandPhase::Observed);
    assert!(row.observed.unwrap().read_only);
}

#[test]
fn unknown_targets_and_unsupported_kinds_are_explicit() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");

    // Unknown logical target.
    ui.dispatch(
        &mut s,
        begin("param-deadbeef", "g1"),
        ctx(&ui, "mixer", "param-deadbeef", "c1"),
    );
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Rejected);

    // An action intent on a param target is unsupported.
    ui.dispatch(&mut s, act(&t, "play"), ctx(&ui, "mixer", &t, "c2"));
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Unsupported);

    // A gesture on the transport *action* surface is unsupported —
    // scrub belongs on transport/position.
    ui.dispatch(
        &mut s,
        begin("transport", "g2"),
        ctx(&ui, "mixer", "transport", "c3"),
    );
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Unsupported);

    // Unknown action names are unsupported.
    ui.dispatch(
        &mut s,
        act("transport", "rewind"),
        ctx(&ui, "mixer", &t, "c4"),
    );
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Unsupported);

    // Subscribe on a bogus target errors instead of half-binding.
    assert!(ui.subscribe(&s, "mixer", "bogus").is_err());
}

#[test]
fn out_of_domain_commit_is_rejected_and_gesture_survives() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();

    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    // NaN cannot land as the final value — the gesture stays live.
    ui.dispatch(
        &mut s,
        commit(&t, "g1", f64::NAN),
        ctx(&ui, "mixer", &t, "c2"),
    );
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Rejected);
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.phase, CommandPhase::Preview);
    // An explicit cancel still ends it honestly.
    ui.dispatch(&mut s, cancel(&t, "g1"), ctx(&ui, "mixer", &t, "c3"));
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Accepted);
}

// ---------------------------------------------------------------------------
// AC4 — detach / dock transfer contractual cancel + generation guard
// ---------------------------------------------------------------------------

#[test]
fn dock_move_cancels_gesture_and_stale_generation_cannot_land() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();
    ui.set_mode(&mut s, p, AutomationMode::Off).unwrap();

    // Drag in window 1.
    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.dispatch(&mut s, preview(&t, "g1", 0.9), ctx(&ui, "mixer", &t, "c2"));
    ui.pump();
    assert_eq!(
        ui.strip_row("mixer", &t).unwrap().phase,
        CommandPhase::Preview
    );

    // Dock transfer: the same logical view mounts in window 2 — the
    // in-flight gesture is contractually cancelled under the old
    // epoch, never committed.
    ui.attach_view(&mut s, "mixer", "w2");
    assert_eq!(ui.diagnostics().cancelled_on_detach, 1);

    // The cancelled preview is gone — the re-attached view re-read the
    // truth (base 0.5, not the 0.9 preview).
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.phase, CommandPhase::Observed);
    assert_eq!(
        row.observed.unwrap().breakdown.evaluation.host_normalized,
        0.5
    );
    assert!(row.preview.is_none());
    assert_eq!(s.project().control.targets.get(&p).unwrap().base, 0.5);

    // The pre-detach begin receipt is still on the wire: delivered
    // now, it lands as stale — the dead epoch cannot move the phase.
    ui.pump();
    assert_eq!(ui.diagnostics().stale_receipts, 1);

    // A new drag in the new window works under the new generation.
    ui.dispatch(&mut s, begin(&t, "g3"), ctx(&ui, "mixer", &t, "c5"));
    ui.dispatch(&mut s, commit(&t, "g3", 0.7), ctx(&ui, "mixer", &t, "c6"));
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Applied);
}

#[test]
fn detach_cancels_write_gesture_without_committing() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.2);
    let t = tag(p);
    let lane = add_lane(&mut s, p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();

    ui.set_mode(&mut s, p, AutomationMode::Write).unwrap();
    ui.set_arm(&mut s, p, WriteArm::Armed).unwrap();
    ui.bind_write_lane(&mut s, p, lane).unwrap();
    ui.set_record_enabled(&mut s, true);
    ui.dispatch(
        &mut s,
        act("transport", "play"),
        ctx(&ui, "mixer", &t, "t1"),
    );
    ui.pump();
    ui.record_start(&mut s).unwrap();

    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.dispatch(&mut s, preview(&t, "g1", 0.9), ctx(&ui, "mixer", &t, "c2"));

    // Unmount mid-drag: nothing is written into the transaction.
    ui.detach_view(&mut s, "mixer", &GestureCancelReason::Unmount);
    assert!(s.control().recording().unwrap().segments.is_empty());
    assert_eq!(ui.diagnostics().cancelled_on_detach, 1);

    // The detached binding is stale; intents aimed at it are dropped.
    ui.dispatch(&mut s, preview(&t, "g1", 0.1), ctx(&ui, "mixer", &t, "c3"));
    assert_eq!(ui.diagnostics().detached_intents, 1);
    assert_eq!(
        ui.strip_row("mixer", &t).unwrap().phase,
        CommandPhase::Stale
    );
}

// ---------------------------------------------------------------------------
// AC5 — transport: applied truth is always distinct from the request
// ---------------------------------------------------------------------------

#[test]
fn transport_actions_report_applied_vs_noop() {
    let (mut s, mut ui) = fresh();
    ui.attach_view(&mut s, "main", "w1");
    ui.attach_view(&mut s, "second", "w2");
    ui.subscribe(&s, "main", "transport").unwrap();
    ui.subscribe(&s, "main", "transport/position").unwrap();
    ui.subscribe(&s, "second", "transport").unwrap();

    // Play: the request lands → applied; both bound views see Playing.
    ui.dispatch(
        &mut s,
        act("transport", "play"),
        ctx(&ui, "main", "transport", "t1"),
    );
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Applied);
    ui.pump();
    for view in ["main", "second"] {
        let row = ui.transport_row(view).unwrap();
        assert_eq!(row.observed.unwrap().status, PlaybackStatus::Playing);
    }

    // Play again: the request completes but reports it changed
    // nothing — `applied` with a `no change` detail, never a hang.
    ui.dispatch(
        &mut s,
        act("transport", "play"),
        ctx(&ui, "main", "transport", "t2"),
    );
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Applied);
    assert_eq!(r.detail.as_deref(), Some("no change"));
    ui.pump();

    // Pause → resume → stop each report the real transition.
    ui.dispatch(
        &mut s,
        act("transport", "pause"),
        ctx(&ui, "main", "transport", "t3"),
    );
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Applied);
    ui.pump();
    assert_eq!(
        ui.transport_row("main").unwrap().observed.unwrap().status,
        PlaybackStatus::Paused
    );
    ui.dispatch(
        &mut s,
        act("transport", "resume"),
        ctx(&ui, "main", "transport", "t4"),
    );
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Applied);
    ui.pump();
    ui.dispatch(
        &mut s,
        act("transport", "stop"),
        ctx(&ui, "main", "transport", "t5"),
    );
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Applied);
    ui.pump();
    assert_eq!(
        ui.transport_row("main").unwrap().observed.unwrap().status,
        PlaybackStatus::Stopped
    );
}

#[test]
fn seek_preview_never_moves_the_playhead() {
    let (mut s, mut ui) = fresh();
    ui.attach_view(&mut s, "main", "w1");
    ui.subscribe(&s, "main", "transport/position").unwrap();
    ui.subscribe(&s, "main", "transport").unwrap();

    // Scrub: preview shows the tentative position; the playhead holds.
    ui.dispatch(
        &mut s,
        begin("transport/position", "s1"),
        ctx(&ui, "main", "transport/position", "c1"),
    );
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Accepted);
    ui.dispatch(
        &mut s,
        CommandIntent::Preview {
            target: "transport/position".into(),
            control_id: "seek".into(),
            sequence: 2,
            gesture_id: "s1".into(),
            origin_view: None,
            atomic: None,
            value: v(4.0),
            coalescible: true,
        },
        ctx(&ui, "main", "transport/position", "c2"),
    );
    let row = ui.transport_row("main").unwrap();
    assert_eq!(row.preview, Some(4.0));
    assert_eq!(s.control().playback().position, BeatPoint::ZERO);

    // Commit applies the final position.
    ui.dispatch(
        &mut s,
        CommandIntent::Commit {
            target: "transport/position".into(),
            control_id: "seek".into(),
            sequence: 3,
            gesture_id: "s1".into(),
            origin_view: None,
            atomic: None,
            value: v(8.0),
        },
        ctx(&ui, "main", "transport/position", "c3"),
    );
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Applied);
    ui.pump();
    // The seekbar value is seconds: 8 s under the default 120 BPM map is beat 16.
    assert_eq!(s.control().playback().position, bp(16));
    let row = ui.transport_row("main").unwrap();
    assert_eq!(row.observed.unwrap().position, bp(16));
    assert!(row.preview.is_none());

    // A cancelled scrub leaves the playhead untouched.
    ui.dispatch(
        &mut s,
        begin("transport/position", "s2"),
        ctx(&ui, "main", "transport/position", "c4"),
    );
    ui.dispatch(
        &mut s,
        CommandIntent::Preview {
            target: "transport/position".into(),
            control_id: "seek".into(),
            sequence: 5,
            gesture_id: "s2".into(),
            origin_view: None,
            atomic: None,
            value: v(2.0),
            coalescible: true,
        },
        ctx(&ui, "main", "transport/position", "c5"),
    );
    ui.dispatch(
        &mut s,
        cancel("transport/position", "s2"),
        ctx(&ui, "main", "transport/position", "c6"),
    );
    ui.pump();
    assert_eq!(s.control().playback().position, bp(16));
}

// ---------------------------------------------------------------------------
// receipt lifecycle — ordering, duplicates, stale generations, timeout
// ---------------------------------------------------------------------------

#[test]
fn receipt_ordering_timeout_and_stale_guards() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();
    ui.set_mode(&mut s, p, AutomationMode::Off).unwrap();

    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.dispatch(&mut s, commit(&t, "g1", 0.8), ctx(&ui, "mixer", &t, "c2"));

    // Hold the receipts: the commit's phase is Pending until delivered.
    let wire = ui.drain_wire();
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.phase, CommandPhase::Pending);

    // Delivering the same receipt twice: the second is absorbed.
    let commit_receipt = wire
        .iter()
        .find(|d| d.receipt.command_id == "c2")
        .unwrap()
        .receipt
        .clone();
    ui.deliver("mixer", &t, commit_receipt.clone());
    ui.deliver("mixer", &t, commit_receipt);
    assert_eq!(ui.diagnostics().out_of_order_receipts, 1);

    // A receipt no tracked command owns is stale.
    ui.deliver(
        "mixer",
        &t,
        synvelis_workspace::CommandReceipt {
            command_id: "ghost".into(),
            status: ReceiptStatus::Applied,
            revision: None,
            detail: None,
        },
    );
    assert_eq!(ui.diagnostics().stale_receipts, 1);
    // The commit's own projection landed during dispatch — truth ahead
    // of the receipt, so the terminal receipt settles the phase to
    // `observed`, not `applied` (the projection wins).
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.phase, CommandPhase::Observed);

    // A new commit whose receipt never arrives times out — but a late
    // `applied` still moves the phase, like upstream.
    ui.dispatch(&mut s, begin(&t, "g2"), ctx(&ui, "mixer", &t, "c3"));
    ui.dispatch(&mut s, commit(&t, "g2", 0.3), ctx(&ui, "mixer", &t, "c4"));
    let held = ui.drain_wire();
    ui.set_clock(10_000);
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.phase, CommandPhase::Timeout);
    assert!(row.timed_out);
    let late = held
        .iter()
        .find(|d| d.receipt.command_id == "c4")
        .unwrap()
        .receipt
        .clone();
    ui.deliver("mixer", &t, late);
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.phase, CommandPhase::Observed);
}

#[test]
fn close_view_drops_everything() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();
    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));

    ui.close_view(&mut s, "mixer");
    assert!(ui.binding("mixer", &t).is_none());
    assert_eq!(ui.diagnostics().cancelled_on_detach, 1);
    // The closed view leaves a tombstone — late intents are dropped
    // as detached and can never resurrect a binding or mint receipts.
    assert_eq!(ui.view_state("mixer").map(|v| v.2), Some(false));
    ui.drain_wire(); // discard the tracked detach-cancel receipt

    ui.dispatch(&mut s, commit(&t, "g1", 0.9), ctx(&ui, "mixer", &t, "c2"));
    assert_eq!(ui.diagnostics().detached_intents, 1);
    assert!(ui.wire().is_empty());
    assert_eq!(s.project().control.targets.get(&p).unwrap().base, 0.5);
}

#[test]
fn observation_feeds_update_the_projection() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.attach_view(&mut s, "remote", "w2");
    ui.subscribe(&s, "mixer", &t).unwrap();
    ui.subscribe(&s, "remote", &t).unwrap();

    ui.feed_plugin_report(
        &mut s,
        p,
        0.6,
        ruviform_media_core::MediaTimestamp::carried(ruviform_media_core::MediaTime::new(
            0,
            ruviform_media_core::TimeBase::COMMON_90K,
        )),
    )
    .unwrap();
    // Both bound views got the new observation truth.
    for view in ["mixer", "remote"] {
        let row = ui.strip_row(view, &t).unwrap();
        assert_eq!(
            row.observed.unwrap().breakdown.observation,
            ObservationState::AtPlayhead
        );
    }
}

// ---------------------------------------------------------------------------
// review-fix regression — receipt routing, gesture ownership, lifecycle
// ---------------------------------------------------------------------------

/// Command ids are unique *per binding*: a receipt minted for one
/// target must never land on a sibling binding of the same view.
#[test]
fn receipts_route_to_their_own_binding() {
    let (mut s, mut ui) = fresh();
    let p1 = add_target(&mut s, 0.5);
    let p2 = add_readonly_target(&mut s);
    let t1 = tag(p1);
    let t2 = tag(p2);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t1).unwrap();
    ui.subscribe(&s, "mixer", &t2).unwrap();
    ui.set_mode(&mut s, p1, AutomationMode::Off).unwrap();

    // Both bindings mint the same command id — legal per contract.
    ui.dispatch(&mut s, begin(&t1, "g1"), ctx(&ui, "mixer", &t1, "c1"));
    ui.dispatch(&mut s, begin(&t2, "g2"), ctx(&ui, "mixer", &t2, "c1"));
    ui.dispatch(&mut s, commit(&t1, "g1", 0.7), ctx(&ui, "mixer", &t1, "c2"));

    // t1's commit is applied; t2's begin was rejected (readonly).
    // Each receipt must land on its own binding — t1's `c1` tracks
    // an accepted begin, t2's `c1` a rejected one; they can never
    // cross.
    ui.pump();
    let r1 = ui.strip_row("mixer", &t1).unwrap();
    let r2 = ui.strip_row("mixer", &t2).unwrap();
    assert_eq!(
        r1.last_receipt.as_ref().unwrap().status,
        ReceiptStatus::Applied
    );
    assert_eq!(
        r2.last_receipt.as_ref().unwrap().status,
        ReceiptStatus::Rejected
    );
}

/// Reusing a live gesture id is a rejection — it can never overwrite
/// the owner record and orphan a session gesture (nor let one view's
/// unmount kill another view's gesture).
#[test]
fn gesture_id_collision_is_rejected_and_owner_survives() {
    let (mut s, mut ui) = fresh();
    let p1 = add_target(&mut s, 0.3);
    let p2 = add_target(&mut s, 0.6);
    let t1 = tag(p1);
    let t2 = tag(p2);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.attach_view(&mut s, "remote", "w2");
    ui.subscribe(&s, "mixer", &t1).unwrap();
    ui.subscribe(&s, "remote", &t2).unwrap();

    ui.dispatch(&mut s, begin(&t1, "g1"), ctx(&ui, "mixer", &t1, "c1"));
    assert_eq!(wire_status(&ui), [ReceiptStatus::Accepted]);
    ui.pump();

    // Same gesture id on another view/target: rejected, owner intact.
    ui.dispatch(&mut s, begin(&t2, "g1"), ctx(&ui, "remote", &t2, "r1"));
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Rejected);

    // Unmounting the rejected view does not touch the live gesture.
    ui.detach_view(&mut s, "remote", &GestureCancelReason::Unmount);
    assert_eq!(ui.diagnostics().cancelled_on_detach, 0);
    assert_eq!(
        ui.strip_row("mixer", &t1).unwrap().phase,
        CommandPhase::Preview
    );

    // The owner can still end its own gesture honestly — and a fresh
    // begin proves the session gesture was really discarded (the
    // session rejects `begin` while one is active).
    ui.dispatch(&mut s, cancel(&t1, "g1"), ctx(&ui, "mixer", &t1, "c2"));
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Accepted);
    ui.dispatch(&mut s, begin(&t1, "g3"), ctx(&ui, "mixer", &t1, "c3"));
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Accepted);
}

/// Intents on unregistered or closed views are dropped as detached —
/// they mint no binding, no gesture, no receipt and no domain state.
#[test]
fn detached_views_cannot_dispatch_or_resurrect() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);

    // A never-attached view cannot dispatch at all.
    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "ghost", &t, "c1"));
    assert_eq!(ui.diagnostics().detached_intents, 1);
    assert!(ui.wire().is_empty());
    assert!(ui.binding("ghost", &t).is_none());

    // A closed view's tombstone rejects a late begin the same way.
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();
    ui.close_view(&mut s, "mixer");
    ui.dispatch(&mut s, begin(&t, "g2"), ctx(&ui, "mixer", &t, "c2"));
    assert_eq!(ui.diagnostics().detached_intents, 2);
    assert!(ui.binding("mixer", &t).is_none());
    // No session gesture could have been minted — a reattached view
    // begins cleanly.
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();
    ui.dispatch(&mut s, begin(&t, "g3"), ctx(&ui, "mixer", &t, "c3"));
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Accepted);
}

/// `record_commit` flips `recording_active` in the transport truth and
/// the engine clock keeps ticking — a held latch value with a closed
/// transaction mutes its write, never the advance.
#[test]
fn record_commit_lands_recording_off_and_clock_keeps_ticking() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.2);
    let t = tag(p);
    let lane = add_lane(&mut s, p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();
    ui.subscribe(&s, "mixer", "transport").unwrap();

    ui.set_mode(&mut s, p, AutomationMode::Latch).unwrap();
    ui.set_arm(&mut s, p, WriteArm::Armed).unwrap();
    ui.bind_write_lane(&mut s, p, lane).unwrap();
    ui.set_record_enabled(&mut s, true);
    ui.dispatch(
        &mut s,
        act("transport", "play"),
        ctx(&ui, "mixer", "transport", "t1"),
    );
    ui.pump();
    ui.record_start(&mut s).unwrap();
    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.dispatch(&mut s, commit(&t, "g1", 0.8), ctx(&ui, "mixer", &t, "c2"));
    ui.pump();

    let outcome = ui.record_commit(&mut s).unwrap();
    assert!(outcome.applied);
    // The consumed transaction is visible as recording_active: false
    // — the projection must land past the revision guard.
    assert!(
        !ui.transport_row("mixer")
            .unwrap()
            .observed
            .unwrap()
            .recording_active
    );

    // The held latch value has no open transaction — the clock keeps
    // ticking instead of erroring forever.
    ui.transport_advance(&mut s, bp(4)).unwrap();
    ui.transport_advance(&mut s, bp(8)).unwrap();
    assert_eq!(s.control().playback().position, bp(8));
}

/// Pause closes the write gates without killing the clock: a held
/// latch value skips its write while paused and the playhead still
/// advances.
#[test]
fn pause_mutes_held_writes_but_clock_survives() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.2);
    let t = tag(p);
    let lane = add_lane(&mut s, p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();

    ui.set_mode(&mut s, p, AutomationMode::Latch).unwrap();
    ui.set_arm(&mut s, p, WriteArm::Armed).unwrap();
    ui.bind_write_lane(&mut s, p, lane).unwrap();
    ui.set_record_enabled(&mut s, true);
    ui.dispatch(
        &mut s,
        act("transport", "play"),
        ctx(&ui, "mixer", &t, "t1"),
    );
    ui.pump();
    ui.record_start(&mut s).unwrap();
    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.dispatch(&mut s, commit(&t, "g1", 0.8), ctx(&ui, "mixer", &t, "c2"));
    ui.pump();

    ui.dispatch(
        &mut s,
        act("transport", "pause"),
        ctx(&ui, "mixer", &t, "t2"),
    );
    ui.pump();
    // Paused gates are closed — the held write is muted, not an error.
    ui.transport_advance(&mut s, bp(4)).unwrap();
    assert_eq!(s.control().playback().position, bp(4));
}

/// One playhead, one scrub: a second window grabbing
/// `transport/position` while a scrub is live is rejected.
#[test]
fn scrub_is_exclusive_across_windows() {
    let (mut s, mut ui) = fresh();
    ui.attach_view(&mut s, "main", "w1");
    ui.attach_view(&mut s, "second", "w2");
    ui.subscribe(&s, "main", "transport/position").unwrap();
    ui.subscribe(&s, "second", "transport/position").unwrap();

    ui.dispatch(
        &mut s,
        begin("transport/position", "s1"),
        ctx(&ui, "main", "transport/position", "c1"),
    );
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Accepted);

    ui.dispatch(
        &mut s,
        begin("transport/position", "s2"),
        ctx(&ui, "second", "transport/position", "r1"),
    );
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Rejected);
}

/// Detach clears in-flight command state: a remounted binding that
/// re-mints a command id is a fresh command, never a duplicate.
#[test]
fn detach_clears_inflight_state_so_reminted_ids_are_fresh() {
    let (mut s, mut ui) = fresh();
    let p = add_target(&mut s, 0.5);
    let t = tag(p);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();
    ui.set_mode(&mut s, p, AutomationMode::Off).unwrap();

    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.dispatch(&mut s, commit(&t, "g1", 0.4), ctx(&ui, "mixer", &t, "c2"));
    ui.pump();

    ui.detach_view(&mut s, "mixer", &GestureCancelReason::Unmount);
    ui.attach_view(&mut s, "mixer", "w1");

    // The producer restarted its id space — `c1`/`c2` are new
    // commands on the new epoch, not duplicates of the dead ones.
    ui.dispatch(&mut s, begin(&t, "g2"), ctx(&ui, "mixer", &t, "c1"));
    ui.dispatch(&mut s, commit(&t, "g2", 0.9), ctx(&ui, "mixer", &t, "c2"));
    assert_eq!(ui.diagnostics().duplicate_commands, 0);
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Applied);
    assert_eq!(s.project().control.targets.get(&p).unwrap().base, 0.9);
}

/// Actions are meaningless on the scrub surface — `transport/position`
/// rejects them as unsupported, symmetric with gestures on
/// `transport`.
#[test]
fn actions_on_position_target_are_unsupported() {
    let (mut s, mut ui) = fresh();
    ui.attach_view(&mut s, "main", "w1");
    ui.subscribe(&s, "main", "transport/position").unwrap();

    ui.dispatch(
        &mut s,
        act("transport/position", "stop"),
        ctx(&ui, "main", "transport/position", "c1"),
    );
    assert_eq!(last_receipt(&mut ui).status, ReceiptStatus::Unsupported);
    assert_eq!(s.control().playback().status, PlaybackStatus::Stopped);
}
