//! SV185 (issue #185): the bootstrap master-strip fader target is a
//! first-class `syn.control.*` citizen — subscribe resolves the
//! well-known tagged id, the projection carries the authoritative
//! evaluation, and `Begin → Preview → Commit/Cancel` writes
//! `target.base` through `SetParameterBase` like any parameter target.

use synvelis_project::{
    CounterIdGenerator, MASTER_FADER_RAW, ParameterTargetId, Project, ProjectController, ProjectId,
    is_issuable_raw,
};
use synvelis_workspace::{
    AutomationMode, CommandIntent, CommandPhase, CommandReceipt, ControlsUi, DispatchContext,
    GestureCancelReason, ProjectSession, ReceiptStatus,
};

fn fresh() -> (ProjectSession, ControlsUi) {
    let project = Project::new(ProjectId::new(0x185), "sv185-test");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    (ProjectSession::new(controller), ControlsUi::new())
}

/// The wire target string the app's Mixer Fader binds — the tagged id
/// of the bootstrap `ParameterRole::Fader` target.
fn fader_tag() -> String {
    ParameterTargetId::new(MASTER_FADER_RAW).to_tagged()
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
        control_id: target.into(),
        sequence: 1,
        gesture_id: gid.into(),
        origin_view: None,
        atomic: None,
        value: v(1.0),
    }
}

fn preview(target: &str, gid: &str, value: f64) -> CommandIntent {
    CommandIntent::Preview {
        target: target.into(),
        control_id: target.into(),
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
        control_id: target.into(),
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
        control_id: target.into(),
        sequence: 4,
        gesture_id: gid.into(),
        origin_view: None,
        atomic: None,
        reason: GestureCancelReason::Escape,
    }
}

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

fn last_receipt(ui: &mut ControlsUi) -> CommandReceipt {
    let drained = ui.drain_wire();
    let last = drained.last().expect("a receipt").receipt.clone();
    for d in drained {
        ui.deliver(&d.view, &d.target, d.receipt);
    }
    last
}

#[test]
fn bootstrap_fader_projects_authoritative_unity() {
    let (mut s, mut ui) = fresh();
    let t = fader_tag();
    ui.attach_view(&mut s, "mixer", "w1");
    // The well-known target resolves through the same tagged-id
    // parser every parameter target uses — subscribe cannot reject it.
    ui.subscribe(&s, "mixer", &t).unwrap();

    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.phase, CommandPhase::Observed);
    assert_eq!(
        row.observed.unwrap().breakdown.evaluation.host_normalized,
        1.0
    );
    assert_eq!(
        s.project()
            .control
            .targets
            .keys()
            .filter(|id| is_issuable_raw(id.raw()))
            .count(),
        0,
        "the bootstrap target is the only target and is never issued"
    );
}

#[test]
fn fader_gesture_commits_base_edit_and_cancel_restores() {
    let (mut s, mut ui) = fresh();
    let t = fader_tag();
    let id = ParameterTargetId::new(MASTER_FADER_RAW);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();
    // Off mode: the commit is a base edit — one undoable apply.
    ui.set_mode(&mut s, id, AutomationMode::Off).unwrap();

    // begin → preview → commit: one undoable `SetParameterBase`.
    ui.dispatch(&mut s, begin(&t, "g1"), ctx(&ui, "mixer", &t, "c1"));
    ui.dispatch(&mut s, preview(&t, "g1", 0.4), ctx(&ui, "mixer", &t, "c2"));
    assert_eq!(ui.strip_row("mixer", &t).unwrap().preview, Some(0.4));
    ui.dispatch(&mut s, commit(&t, "g1", 0.4), ctx(&ui, "mixer", &t, "c3"));
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Applied);
    ui.pump();

    assert_eq!(s.project().control.targets[&id].base, 0.4);
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.phase, CommandPhase::Observed);
    assert_eq!(
        row.observed.unwrap().breakdown.evaluation.host_normalized,
        0.4
    );

    // Undo restores the authoritative base through the same op.
    ui.undo(&mut s).unwrap();
    assert_eq!(s.project().control.targets[&id].base, 1.0);

    // A cancelled gesture never writes — preview drops, base stays.
    ui.dispatch(&mut s, begin(&t, "g2"), ctx(&ui, "mixer", &t, "c4"));
    ui.dispatch(&mut s, preview(&t, "g2", 0.1), ctx(&ui, "mixer", &t, "c5"));
    ui.dispatch(&mut s, cancel(&t, "g2"), ctx(&ui, "mixer", &t, "c6"));
    let _ = last_receipt(&mut ui);
    ui.pump();
    assert_eq!(s.project().control.targets[&id].base, 1.0);
    let row = ui.strip_row("mixer", &t).unwrap();
    assert_eq!(row.phase, CommandPhase::Observed);
    assert_eq!(
        row.observed.unwrap().breakdown.evaluation.host_normalized,
        1.0
    );
}

#[test]
fn fader_commit_without_gesture_and_stale_generation_write_nothing() {
    let (mut s, mut ui) = fresh();
    let t = fader_tag();
    let id = ParameterTargetId::new(MASTER_FADER_RAW);
    ui.attach_view(&mut s, "mixer", "w1");
    ui.subscribe(&s, "mixer", &t).unwrap();

    // A commit with no open gesture is a domain Validation error —
    // the receipt is rejected and the base never moves.
    ui.dispatch(
        &mut s,
        commit(&t, "orphan", 0.2),
        ctx(&ui, "mixer", &t, "c1"),
    );
    let r = last_receipt(&mut ui);
    assert_eq!(r.status, ReceiptStatus::Rejected);
    assert_eq!(s.project().control.targets[&id].base, 1.0);

    // A ctx stamped below the binding's live generation is dropped
    // before it can reach the domain (RF387 epoch guard).
    let stale = DispatchContext {
        command_id: "c2".into(),
        view_id: Some("mixer".into()),
        window_id: None,
        generation: 0,
        issued_at: 0,
    };
    ui.dispatch(&mut s, begin(&t, "g1"), stale);
    assert_eq!(ui.diagnostics().stale_intents, 1);
    assert_eq!(s.project().control.targets[&id].base, 1.0);
}
