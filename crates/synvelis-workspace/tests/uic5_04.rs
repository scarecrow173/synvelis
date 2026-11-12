//! UI-C5-04 acceptance fixtures — Synvelis #102 slice 4
//! (`docs/test-specs/pc-c5-04-recording-status.md`).
//!
//! Covers the recording-status read model end-to-end through real
//! session ops: the five display states (Conflict / Prepare /
//! Pending / Applied / Unknown), typed provenance
//! (`SealReason`/`RecordCommitError`/`RecordNotification`/
//! `RecordDisposition`/`AppliedRecord`/`RecordHealthSummary`/revision
//! drift), the per-state legal actions (Inspect / Retry /
//! `ExplicitRebase` / Abort), the caller-held prepared payload's
//! verified verdicts, and the bounded notification tail — all
//! headless with deterministic `CounterIdGenerator` ids.
//!
//! Deterministic: `CounterIdGenerator` ids and `FakeWindowHost`.

use synvelis_project::{
    AutomationLaneId, AutomationTimeScope, BeatPoint, ControlOwner, CounterIdGenerator,
    ParamMapping, ParameterRole, ParameterTargetId, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, TrackId, ValueKind, is_issuable_raw,
};
use synvelis_workspace::{
    AutomationMode, ControlSession, FakeWindowHost, PlaybackStatus, PreparedRecordingCommit,
    PreparedVerdict, ProjectSession, RecordBudgets, RecordCommitError, RecordDisposition,
    RecordGap, RecordNotification, RecordRejectKind, RecordingAction, RecordingDisplayState,
    RecordingProvenance, RecordingStatusOptions, RecordingStatusView, RecordingTransactionId,
    RecordingTxnState, SealReason, WorkspaceManager, WriteArm, recording_status,
    recording_status_with,
};

// ---------------------------------------------------------------------------
// fixture (same conventions as pc_c1.rs / uic5.rs)
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xC504), "uic5-04-test");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    WorkspaceManager::new(FakeWindowHost::default(), ProjectSession::new(controller))
}

fn apply(ws: &mut Ws, op: ProjectOp) -> ProjectRevision {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev).expect("fixture op")
}

fn bp(num: i64) -> BeatPoint {
    BeatPoint::new(num, 1).unwrap()
}

fn add_track_target(ws: &mut Ws, name: &str, base: f64) -> (TrackId, ParameterTargetId) {
    apply(ws, ProjectOp::AddTrack { name: "t".into() });
    let track = *ws.session().project().tracks.keys().last().unwrap();
    apply(
        ws,
        ProjectOp::AddParameterTarget {
            name: name.into(),
            owner: ControlOwner::Track(track),
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base,
        },
    );
    let target = *ws
        .session()
        .project()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .unwrap();
    (track, target)
}

fn add_automation_lane(ws: &mut Ws, target: ParameterTargetId) -> AutomationLaneId {
    apply(ws, ProjectOp::AddAutomationLane { target });
    *ws.session().project().control.lanes.keys().last().unwrap()
}

/// Arms `target` in `Touch` and opens a recording transaction on a
/// rolling transport; returns the transaction id.
fn open_recording(
    ws: &mut Ws,
    target: ParameterTargetId,
    lane: AutomationLaneId,
) -> RecordingTransactionId {
    ws.session_mut()
        .control_with(|c, p| c.bind_write_lane(p, target, lane))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_arm(p, target, WriteArm::Armed))
        .unwrap();
    ws.session_mut().control_mut().set_record_enabled(true);
    ws.session_mut()
        .control_with(ControlSession::transport_play);
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.record_start(p, rev))
        .unwrap()
}

fn write_point(ws: &mut Ws, target: ParameterTargetId, pos: BeatPoint, value: f64) {
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, pos, value))
        .unwrap();
}

fn prepare(ws: &mut Ws) -> PreparedRecordingCommit {
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.prepare_record_commit(p, rev))
        .unwrap()
}

/// Whether any explanation row carries provenance matching `pred`.
fn has_provenance(view: &RecordingStatusView, pred: impl Fn(&RecordingProvenance) -> bool) -> bool {
    view.explanations.iter().any(|e| pred(&e.provenance))
}

// ---------------------------------------------------------------------------
// Unknown — no evidence
// ---------------------------------------------------------------------------

/// An idle session maps to `Unknown` with no actions and gate
/// provenance — "recording exists" is never implied.
#[test]
fn uic5_04_idle_session_is_unknown() {
    let ws = fresh_ws();
    let view = recording_status(ws.session());
    assert_eq!(view.state, RecordingDisplayState::Unknown);
    assert!(view.transaction.is_none());
    assert!(view.prepared.is_none());
    assert!(view.outcome.is_none());
    assert_eq!(view.actions, [] as [RecordingAction; 0]);
    assert!(!view.record_enabled);
    assert_eq!(view.transport, PlaybackStatus::Stopped);
    assert!(view.is_current(ws.session()));
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::Gates {
            record_enabled: false,
            status: PlaybackStatus::Stopped
        }
    )));
}

// ---------------------------------------------------------------------------
// Pending — open transaction
// ---------------------------------------------------------------------------

/// An open, unsealed, revision-current transaction maps to `Pending`
/// with the verbatim transaction/lane evidence and the
/// Inspect/Retry/Abort action set. Building the view mutates nothing.
#[test]
fn uic5_04_open_transaction_is_pending() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    write_point(&mut ws, target, bp(1), 0.6);
    write_point(&mut ws, target, bp(2), 0.8);

    let rev_before = ws.session().revision();
    let view = recording_status(ws.session());
    assert_eq!(view.state, RecordingDisplayState::Pending);
    assert_eq!(
        view.actions,
        vec![
            RecordingAction::Inspect,
            RecordingAction::Retry,
            RecordingAction::Abort
        ]
    );
    assert!(view.is_current(ws.session()));

    let tx = view.transaction.as_ref().expect("open transaction");
    assert_eq!(tx.id, id);
    assert_eq!(tx.state, RecordingTxnState::Pending);
    assert_eq!(tx.sealed, None);
    assert_eq!(tx.fixed_revision, rev_before);
    assert_eq!(tx.current_revision, rev_before);
    assert!(!tx.revision_stale);
    assert_eq!(tx.fixed_targets, vec![target]);
    assert_eq!(tx.fixed_lanes.get(&target), Some(&lane));
    assert_eq!(tx.recorded_points, 2);
    assert_eq!(tx.lanes.len(), 1);
    let lane_view = &tx.lanes[0];
    assert_eq!(lane_view.lane, lane);
    assert_eq!(lane_view.target, target);
    assert!(lane_view.target_exists && lane_view.lane_exists);
    assert_eq!(lane_view.recorded_points, 2);
    assert_eq!(lane_view.recorded_range, Some((bp(1), bp(2))));
    assert_eq!(lane_view.gaps, [] as [RecordGap; 0]);

    // Typed provenance: the transaction's own fields verbatim.
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::Transaction {
            id: tid,
            state: RecordingTxnState::Pending,
            sealed: None
        } if *tid == id
    )));
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::Gates {
            record_enabled: true,
            status: PlaybackStatus::Playing
        }
    )));

    // Read-side only: revision and the open transaction are untouched.
    assert_eq!(ws.session().revision(), rev_before);
    assert_eq!(ws.session().control().recording().map(|t| t.id), Some(id));
}

// ---------------------------------------------------------------------------
// Conflict — predictive drift, confirmed failure, explicit rebase
// ---------------------------------------------------------------------------

/// A foreign project change seals the write gate and drifts the
/// revision: the view reports `Conflict` **before** any commit
/// attempt (the same evidence `prepare_record_commit` checks), with
/// `RevisionDrift` + `SealReason::ProjectChange` provenance and no
/// Retry. The failed commit keeps `Conflict`; only
/// `approve_rebase` returns the view to `Pending`.
#[test]
fn uic5_04_foreign_change_conflicts_then_explicit_rebase_pending() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    write_point(&mut ws, target, bp(1), 0.5);
    write_point(&mut ws, target, bp(2), 0.8);
    let fixed_rev = ws.session().control().recording().unwrap().fixed_revision;

    // Foreign drift — predictive Conflict, no commit attempted yet.
    let foreign_rev = apply(&mut ws, ProjectOp::AddTrack { name: "x".into() });
    let view = recording_status(ws.session());
    assert_eq!(view.state, RecordingDisplayState::Conflict);
    assert_eq!(
        view.actions,
        vec![
            RecordingAction::Inspect,
            RecordingAction::ExplicitRebase,
            RecordingAction::Abort
        ]
    );
    assert!(!view.actions.contains(&RecordingAction::Retry));
    let tx = view.transaction.as_ref().unwrap();
    assert_eq!(tx.fixed_revision, fixed_rev);
    assert_eq!(tx.current_revision, foreign_rev);
    assert!(tx.revision_stale);
    assert_eq!(tx.sealed, Some(SealReason::ProjectChange));
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::RevisionDrift { fixed, current }
            if *fixed == fixed_rev && *current == foreign_rev
    )));
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::Transaction {
            sealed: Some(SealReason::ProjectChange),
            ..
        }
    )));

    // Confirmed: the commit reports the typed Conflict and the view
    // stays Conflict with the verbatim state now `Conflict`.
    let err = ws.session_mut().control_record_commit().unwrap_err();
    assert!(matches!(err, RecordCommitError::Conflict { .. }));
    let view = recording_status(ws.session());
    assert_eq!(view.state, RecordingDisplayState::Conflict);
    assert_eq!(view.transaction.unwrap().state, RecordingTxnState::Conflict);

    // Inspect stays legal; explicit rebase re-pins and returns
    // Pending — the seal is kept, never reopened.
    ws.session().control_record_rebase_preview().unwrap();
    ws.session_mut().control_record_approve_rebase(id).unwrap();
    let view = recording_status(ws.session());
    assert_eq!(view.state, RecordingDisplayState::Pending);
    let tx = view.transaction.as_ref().unwrap();
    assert_eq!(tx.state, RecordingTxnState::Pending);
    assert_eq!(tx.fixed_revision, foreign_rev);
    assert_eq!(tx.sealed, Some(SealReason::ProjectChange));
    assert_eq!(
        view.actions,
        vec![
            RecordingAction::Inspect,
            RecordingAction::Retry,
            RecordingAction::Abort
        ]
    );
}

// ---------------------------------------------------------------------------
// Prepare — caller-held payload awaiting apply
// ---------------------------------------------------------------------------

/// A caller-held `PreparedRecordingCommit` that verifies `Live`
/// maps the view to `Prepare` — awaiting-apply, not a persisted
/// transaction state. `control_record_finish` then applies it and
/// the view moves to `Applied`.
#[test]
fn uic5_04_live_prepared_is_prepare_then_finish_applies() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    write_point(&mut ws, target, bp(1), 0.6);

    // Without the caller-held payload the same session is Pending —
    // the session cannot observe a payload it does not hold.
    assert_eq!(
        recording_status(ws.session()).state,
        RecordingDisplayState::Pending
    );

    let prepared = prepare(&mut ws);
    assert_eq!(prepared.id, id);
    let view = recording_status_with(
        ws.session(),
        Some(&prepared),
        RecordingStatusOptions::default(),
    );
    assert_eq!(view.state, RecordingDisplayState::Prepare);
    assert_eq!(
        view.actions,
        vec![
            RecordingAction::Inspect,
            RecordingAction::Retry,
            RecordingAction::Abort
        ]
    );
    let pv = view.prepared.as_ref().expect("prepared view");
    assert_eq!(pv.id, id);
    assert_eq!(pv.fingerprint, prepared.fingerprint);
    assert!(pv.has_write);
    assert_eq!(pv.verdict, PreparedVerdict::Live);
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::Prepared {
            verdict: PreparedVerdict::Live,
            ..
        }
    )));

    // Finish consumes it — the view then reports Applied.
    ws.session_mut().control_record_finish(&prepared).unwrap();
    let view = recording_status(ws.session());
    assert_eq!(view.state, RecordingDisplayState::Applied);
    assert!(view.transaction.is_none());
}

// ---------------------------------------------------------------------------
// Prepared verdicts — stale/foreign/unknown/duplicate payloads
// ---------------------------------------------------------------------------

/// Caller-held payloads are verified, never trusted: extra recorded
/// points break the fingerprint (`IntegrityConflict`), an unknown id
/// is `UnknownTransaction`, and a payload whose transaction already
/// committed is `AlreadyApplied` — each typed, each keeping the
/// display state honest.
#[test]
fn uic5_04_prepared_verdicts_are_typed() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    write_point(&mut ws, target, bp(1), 0.5);
    let prepared = prepare(&mut ws);

    // More recorded points change the payload fingerprint — the old
    // prepared snapshot is an IntegrityConflict, and the display
    // stays Pending (never Prepare on a stale claim).
    write_point(&mut ws, target, bp(2), 0.7);
    let view = recording_status_with(
        ws.session(),
        Some(&prepared),
        RecordingStatusOptions::default(),
    );
    assert_eq!(view.state, RecordingDisplayState::Pending);
    assert_eq!(
        view.prepared.unwrap().verdict,
        PreparedVerdict::Rejected(RecordCommitError::IntegrityConflict { id })
    );

    // An unknown transaction id is rejected verbatim.
    let foreign = PreparedRecordingCommit {
        id: RecordingTransactionId(999),
        fingerprint: 0,
        expected_revision: ws.session().revision(),
        op: None,
        was_sealed: false,
    };
    let view = recording_status_with(
        ws.session(),
        Some(&foreign),
        RecordingStatusOptions::default(),
    );
    assert_eq!(view.state, RecordingDisplayState::Pending);
    assert_eq!(
        view.prepared.unwrap().verdict,
        PreparedVerdict::Rejected(RecordCommitError::UnknownTransaction {
            id: RecordingTransactionId(999)
        })
    );

    // Commit through the fresh prepare path; replaying the payload
    // that actually committed resolves to AlreadyApplied against the
    // registry — typed provenance on the Applied view. (The earlier
    // stale payload stays an IntegrityConflict: same id, different
    // fingerprint.)
    let fresh = prepare(&mut ws);
    ws.session_mut().control_record_finish(&fresh).unwrap();
    let view = recording_status_with(
        ws.session(),
        Some(&fresh),
        RecordingStatusOptions::default(),
    );
    assert_eq!(view.state, RecordingDisplayState::Applied);
    let applied_rev = ws
        .session()
        .control()
        .applied_record(id)
        .map(|r| r.revision);
    assert_eq!(
        view.prepared.unwrap().verdict,
        PreparedVerdict::Rejected(RecordCommitError::AlreadyApplied {
            id,
            revision: applied_rev.unwrap()
        })
    );
    let view = recording_status_with(
        ws.session(),
        Some(&prepared),
        RecordingStatusOptions::default(),
    );
    assert_eq!(view.state, RecordingDisplayState::Applied);
    assert_eq!(
        view.prepared.unwrap().verdict,
        PreparedVerdict::Rejected(RecordCommitError::IntegrityConflict { id })
    );
}

// ---------------------------------------------------------------------------
// Applied — committed disposition + receipt
// ---------------------------------------------------------------------------

/// A successful commit consumes the transaction; the view reports
/// `Applied` with the verbatim `RecordDisposition::Committed`, the
/// applied-registry receipt and no legal actions.
#[test]
fn uic5_04_commit_success_is_applied_with_receipt() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    write_point(&mut ws, target, bp(1), 0.6);
    write_point(&mut ws, target, bp(3), 0.9);

    let outcome = ws.session_mut().control_record_commit().unwrap();
    let committed_rev = outcome.revision.unwrap();
    let view = recording_status(ws.session());
    assert_eq!(view.state, RecordingDisplayState::Applied);
    assert!(view.transaction.is_none());
    assert_eq!(view.actions, [] as [RecordingAction; 0]);
    assert!(view.is_current(ws.session()));

    let out = view.outcome.as_ref().expect("outcome");
    assert_eq!(
        out.disposition,
        RecordDisposition::Committed {
            id,
            revision: Some(committed_rev)
        }
    );
    let receipt = out.receipt.expect("applied receipt");
    assert_eq!(receipt.revision, committed_rev);
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::Disposition(RecordDisposition::Committed { id: did, .. })
            if *did == id
    )));
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::AppliedReceipt { id: rid, receipt: r }
            if *rid == id && r.revision == committed_rev
    )));
    // The committed region landed on the fixed lane.
    assert_eq!(ws.session().project().control.lanes[&lane].regions.len(), 1);
}

/// An empty transaction commits nothing — still `Applied` (the
/// disposition is Committed with `revision: None`) and no receipt
/// exists, reported honestly rather than as Unknown.
#[test]
fn uic5_04_empty_commit_is_applied_without_receipt() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    ws.session_mut().control_record_commit().unwrap();

    let view = recording_status(ws.session());
    assert_eq!(view.state, RecordingDisplayState::Applied);
    let out = view.outcome.unwrap();
    assert_eq!(
        out.disposition,
        RecordDisposition::Committed { id, revision: None }
    );
    assert!(out.receipt.is_none());
    assert_eq!(view.actions, [] as [RecordingAction; 0]);
}

// ---------------------------------------------------------------------------
// Unknown — aborted / no committed disposition
// ---------------------------------------------------------------------------

/// Abort discards the open transaction: the view reports `Unknown`
/// with the verbatim `RecordDisposition::Aborted` provenance and no
/// actions — never a phantom Applied.
#[test]
fn uic5_04_abort_is_unknown_with_aborted_provenance() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    write_point(&mut ws, target, bp(1), 0.5);
    ws.session_mut().control_record_abort();

    let view = recording_status(ws.session());
    assert_eq!(view.state, RecordingDisplayState::Unknown);
    assert!(view.transaction.is_none());
    assert_eq!(view.actions, [] as [RecordingAction; 0]);
    let out = view.outcome.as_ref().expect("outcome");
    assert_eq!(out.disposition, RecordDisposition::Aborted { id });
    assert!(out.receipt.is_none());
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::Disposition(RecordDisposition::Aborted { id: aid })
            if *aid == id
    )));
}

// ---------------------------------------------------------------------------
// Pending — Rejected stays Pending with verbatim state
// ---------------------------------------------------------------------------

/// A structurally rejected transaction (its lane was deleted
/// mid-recording) keeps `Pending` as the display state — the
/// verbatim `RecordingTxnState::Rejected` rides in the transaction
/// view and provenance, and Retry/Abort stay legal.
#[test]
fn uic5_04_rejected_transaction_stays_pending() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    write_point(&mut ws, target, bp(1), 0.5);
    write_point(&mut ws, target, bp(2), 0.8);

    // Delete the lane mid-recording: drift → Conflict → explicit
    // rebase → structural reject. Columns are retained.
    apply(&mut ws, ProjectOp::RemoveAutomationLane { lane });
    assert!(matches!(
        ws.session_mut().control_record_commit(),
        Err(RecordCommitError::Conflict { .. })
    ));
    ws.session_mut().control_record_approve_rebase(id).unwrap();
    let err = ws.session_mut().control_record_commit().unwrap_err();
    assert!(matches!(err, RecordCommitError::Invalid(_)), "{err:?}");

    let view = recording_status(ws.session());
    assert_eq!(view.state, RecordingDisplayState::Pending);
    assert_eq!(
        view.actions,
        vec![
            RecordingAction::Inspect,
            RecordingAction::Retry,
            RecordingAction::Abort
        ]
    );
    let tx = view.transaction.as_ref().unwrap();
    assert_eq!(tx.state, RecordingTxnState::Rejected);
    assert_eq!(tx.recorded_points, 2);
    // The deleted lane reports its dead state honestly.
    assert_eq!(tx.lanes.len(), 1);
    assert!(tx.lanes[0].target_exists);
    assert!(!tx.lanes[0].lane_exists);
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::Transaction {
            state: RecordingTxnState::Rejected,
            ..
        }
    )));
}

// ---------------------------------------------------------------------------
// notices / health / bounded tail
// ---------------------------------------------------------------------------

/// Drain-side rejects and data gaps surface as bounded
/// `RecordNotification` provenance rows, the target's
/// `RecordHealthSummary` counters ride as `Health` provenance, and
/// `notice_tail` bounds the surfaced log — overflow is counted, never
/// silent.
#[test]
fn uic5_04_notices_health_and_notice_tail() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_mut()
        .set_record_budgets(RecordBudgets {
            max_points: 2,
            ..RecordBudgets::default()
        });
    for i in 0_i64..4 {
        ws.session_mut()
            .control_mut()
            .record_event_rt(target, bp(i), 0.5)
            .unwrap();
    }
    let rep = ws
        .session_mut()
        .control_with(ControlSession::drain_record_events);
    assert_eq!(rep.accepted, 2);
    assert_eq!(rep.overflowed, 2);

    let view = recording_status(ws.session());
    assert_eq!(view.state, RecordingDisplayState::Pending);
    // The notification tail carries the typed entries, newest last.
    assert_eq!(view.notices.len(), 2);
    assert!(matches!(
        view.notices[0],
        RecordNotification::DataGap { dropped: 2, .. }
    ));
    assert!(matches!(
        view.notices[1],
        RecordNotification::Rejected {
            kind: RecordRejectKind::OverBudget,
            count: 2,
            ..
        }
    ));
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::Notification(RecordNotification::DataGap { .. })
    )));
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::Notification(RecordNotification::Rejected {
            kind: RecordRejectKind::OverBudget,
            ..
        })
    )));
    // Health counters for the fixed target, verbatim.
    assert!(has_provenance(&view, |p| matches!(
        p,
        RecordingProvenance::Health(h)
            if h.target == target && h.overflowed == 2 && h.dropped == 2
    )));
    // The gap is explicit transaction evidence on the lane view.
    let tx = view.transaction.as_ref().unwrap();
    assert_eq!(tx.record_dropped, 2);
    assert_eq!(tx.lanes[0].gaps.len(), 1);
    assert_eq!(tx.lanes[0].gaps[0].dropped, 2);
    assert_eq!(tx.lanes[0].delivery_intervals, 2);

    // notice_tail bounds the surfaced log to the newest entries.
    let tail = recording_status_with(
        ws.session(),
        None,
        RecordingStatusOptions { notice_tail: 1 },
    );
    assert_eq!(tail.notices.len(), 1);
    assert!(matches!(
        tail.notices[0],
        RecordNotification::Rejected { .. }
    ));
}
