//! AUT-03 acceptance tests — `docs/test-specs/aut-03-e2e-loop-write-one-pass.md`.
//!
//! The loop-recording contract (issue #133 / AUT-F01 deferred to
//! issue #6): a write pass lives for exactly one loop pass; the wrap
//! seals the open transaction at the exact half-open loop end,
//! subsequent passes never auto-write without an explicit rearm
//! (`record_start`), every boundary flushes delivered gesture points
//! and marks held-write coverage, and the pass/target/revision/scope/
//! epoch fixation survives mode/follow/lane changes — verified
//! headlessly with deterministic `CounterIdGenerator` ids and
//! `FakeWindowHost`; no GUI, no real plugin, no multi-pass take
//! (those live in AUT-F01's oracle, issue #6).

use ruviform_media_core::{
    ClockAnchor, ClockDomainId, ClockEpochId, EngineFrame, TimeBase, TimestampProvenance,
};
use synvelis_project::{
    AutomationLaneId, AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange,
    ConflictPolicy, ControlOwner, CounterIdGenerator, InMemoryStore, Interpolation, ParamMapping,
    ParameterRole, ParameterTargetId, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, TrackId, ValueKind,
};
use synvelis_workspace::{
    AutomationMode, FakeWindowHost, LoopPassId, PlaybackStatus, PreparedRecordingCommit,
    ProjectSession, RecordCommitError, RecordDisposition, RecordingTransactionId,
    RecordingTxnState, SealReason, WorkspaceManager, WriteArm,
};

// ---------------------------------------------------------------------------
// fixture (same conventions as pc_c1.rs)
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xA03), "aut-03-test");
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

fn pt(pos: BeatPoint, value: f64) -> AutomationPoint {
    AutomationPoint { pos, value }
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
        .last()
        .unwrap();
    (track, target)
}

fn add_automation_lane(ws: &mut Ws, target: ParameterTargetId) -> AutomationLaneId {
    apply(ws, ProjectOp::AddAutomationLane { target });
    *ws.session().project().control.lanes.keys().last().unwrap()
}

/// Arms `target` in `mode` on a rolling transport and opens a
/// recording transaction; returns the transaction id.
fn open_recording(
    ws: &mut Ws,
    target: ParameterTargetId,
    lane: AutomationLaneId,
    mode: AutomationMode,
) -> RecordingTransactionId {
    ws.session_mut()
        .control_with(|c, p| c.bind_write_lane(p, target, lane))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, mode))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_arm(p, target, WriteArm::Armed))
        .unwrap();
    ws.session_mut().control_mut().set_record_enabled(true);
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_play);
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.record_start(p, rev))
        .unwrap()
}

fn sealed(ws: &Ws) -> Option<SealReason> {
    ws.session().control().recording().and_then(|tx| tx.sealed)
}

/// Total recorded points across the open transaction's segments.
fn recorded_points(ws: &Ws) -> usize {
    ws.session()
        .control()
        .recording()
        .map_or(0, |tx| tx.segments.values().map(Vec::len).sum())
}

fn write(ws: &mut Ws, target: ParameterTargetId, at: i64, value: f64) {
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(at), value))
        .unwrap();
}

fn advance(ws: &mut Ws, to: i64) -> synvelis_workspace::TransportEffect {
    ws.session_mut()
        .control_with(|c, p| c.transport_advance(p, bp(to)))
        .unwrap()
}

/// Every region point stored on `lane` across all regions.
fn lane_points(ws: &Ws, lane: AutomationLaneId) -> Vec<AutomationPoint> {
    let mut pts: Vec<_> = ws.session().project().control.lanes[&lane]
        .regions
        .iter()
        .flat_map(|r| r.points.iter().copied())
        .collect();
    pts.sort_by_key(|p| p.pos);
    pts
}

// ---------------------------------------------------------------------------
// F1: one loop pass — wrap closes the write range at the loop end
// ---------------------------------------------------------------------------

/// The wrap lands the pass boundary exactly at the loop end: points
/// recorded before it stay, the transaction seals `LoopWrap`, the
/// playback pass increments while the transport keeps rolling, and a
/// mid-loop start still closes at the *end*, not the start position.
#[test]
fn aut_03_wrap_seals_at_exact_loop_end_transport_continues() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    // The pass begins mid-loop: the write range still closes at the
    // loop end, not at some start-anchored duration.
    advance(&mut ws, 1);
    let id = open_recording(&mut ws, target, lane, AutomationMode::Touch);
    assert_eq!(ws.session().control().playback().pass, LoopPassId(1));
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.pass, LoopPassId(1));
    assert_eq!(tx.started_at, bp(1));
    assert!(tx.sealed.is_none());

    write(&mut ws, target, 1, 0.6);
    write(&mut ws, target, 2, 0.7);
    write(&mut ws, target, 3, 0.9);
    assert_eq!(recorded_points(&ws), 3);

    // Crossing the end wraps and seals — `end` itself belongs to the
    // next pass and is never attributed to this transaction.
    let fx = advance(&mut ws, 5);
    assert!(fx.wrapped && fx.sealed);
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.sealed, Some(SealReason::LoopWrap));
    // The transaction keeps the pass it was fixed on — pass counting
    // belongs to the transport, not the frozen recording context.
    assert_eq!(tx.pass, LoopPassId(1));
    let seg = tx.segments[&lane].clone();
    assert!(seg.iter().all(|p| p.pos < bp(4)));

    // Playback continues; the transport is now on pass 2.
    let playback = ws.session().control().playback();
    assert_eq!(playback.status, PlaybackStatus::Playing);
    assert_eq!(playback.pass, LoopPassId(2));
    assert_eq!(playback.position, bp(0));

    // The sealed pass commits — one Replace region, `was_sealed`
    // reported honestly.
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert_eq!(outcome.id, id);
    assert!(outcome.applied && outcome.was_sealed);
    let regions = &ws.session().project().control.lanes[&lane].regions;
    assert_eq!(
        regions[0].points,
        vec![pt(bp(1), 0.6), pt(bp(2), 0.7), pt(bp(3), 0.9)]
    );
}

/// A position at or beyond the loop end can never enter the pass —
/// not through `record_write`, and not through a flushed gesture
/// batch (the clip drops it at the boundary).
#[test]
fn aut_03_loop_end_positions_never_attributed() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    open_recording(&mut ws, target, lane, AutomationMode::Touch);

    // `end` and beyond are the next pass's domain.
    for at in [4, 5, 40] {
        assert!(
            ws.session_mut()
                .control_with(|c, p| c.record_write(p, target, bp(at), 0.5))
                .is_err(),
            "position {at} must be rejected"
        );
    }
    assert_eq!(recorded_points(&ws), 0);

    // A gesture that delivers a point beyond the end contributes only
    // its in-range points when the wrap flushes it.
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(1), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(2), 0.5)
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(6), 0.5)
        .unwrap();
    let fx = advance(&mut ws, 5);
    assert!(fx.wrapped && fx.sealed);
    let seg = &ws.session().control().recording().unwrap().segments[&lane];
    assert_eq!(*seg, vec![pt(bp(2), 0.5)]);
}

// ---------------------------------------------------------------------------
// F2: subsequent passes never auto-write — explicit rearm only
// ---------------------------------------------------------------------------

/// After the seal the write gate stays closed on every later pass;
/// only an explicit `record_start` (the user's rearm) opens a new
/// transaction — fixed on the pass that is actually rolling then.
#[test]
fn aut_03_later_passes_write_nothing_until_explicit_rearm() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);

    advance(&mut ws, 5); // wrap → pass 2, sealed
    assert!(sealed(&ws).is_some());
    // The gate is closed: direct writes and held writers alike.
    assert!(
        ws.session_mut()
            .control_with(|c, p| c.record_write(p, target, bp(1), 0.9))
            .is_err()
    );
    advance(&mut ws, 2);
    advance(&mut ws, 6); // wrap → pass 3, first seal still wins
    assert_eq!(sealed(&ws), Some(SealReason::LoopWrap));
    assert_eq!(ws.session().control().playback().pass, LoopPassId(3));
    // Nothing new landed on the dead pass's columns.
    assert_eq!(recorded_points(&ws), 1);
    ws.session_mut().control_record_abort();

    // Still rolling — an explicit rearm fixes the *current* pass, it
    // never silently reopens the old one.
    let rev = ws.session().revision();
    let id = ws
        .session_mut()
        .control_with(|c, p| c.record_start(p, rev))
        .unwrap();
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.pass, LoopPassId(3));
    assert!(tx.sealed.is_none());
    write(&mut ws, target, 2, 0.8);
    assert_eq!(recorded_points(&ws), 1);
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert_eq!(outcome.id, id);
    assert!(outcome.applied && !outcome.was_sealed);
    assert_eq!(
        ws.session().project().control.lanes[&lane].regions[0].points,
        vec![pt(bp(2), 0.8)]
    );
}

/// Pass counting is a transport-run property: wraps increment it,
/// pause/resume keeps it, and a new run (any non-`Playing` → `Playing`
/// edge) restarts it at 1 while sealing an open transaction.
#[test]
fn aut_03_pass_lifecycle() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);

    advance(&mut ws, 5); // pass 2
    ws.session_mut().control_mut().transport_pause();
    ws.session_mut().control_mut().transport_resume();
    assert_eq!(ws.session().control().playback().pass, LoopPassId(2));
    advance(&mut ws, 6); // pass 3
    assert_eq!(ws.session().control().playback().pass, LoopPassId(3));
    // First seal wins: a new-run edge on the already-`LoopWrap`
    // transaction keeps its reason, but the pass still restarts.
    ws.session_mut().control_mut().transport_pause();
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_play);
    assert_eq!(sealed(&ws), Some(SealReason::LoopWrap));
    assert_eq!(ws.session().control().playback().pass, LoopPassId(1));
    ws.session_mut().control_record_abort();

    // An *unsealed* transaction hit by a new run reports it
    // honestly: pause → play seals `NewTransportPlay`, pass 1.
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.record_start(p, rev))
        .unwrap();
    ws.session_mut().control_mut().transport_pause();
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_play);
    assert_eq!(sealed(&ws), Some(SealReason::NewTransportPlay));
    assert_eq!(ws.session().control().playback().pass, LoopPassId(1));

    ws.session_mut().control_record_abort();
    // Stop → play is also a fresh run at pass 1.
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_stop);
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_play);
    assert_eq!(ws.session().control().playback().pass, LoopPassId(1));
}

// ---------------------------------------------------------------------------
// F3: fixation — target / lane / revision / scope / pass / epoch
// ---------------------------------------------------------------------------

/// The transaction pins its write lane: rebinding the target while
/// its transaction is open is a defined rejection, never a silent
/// retarget — recorded points can only land on the lane fixed at
/// `record_start`.
#[test]
fn aut_03_rebound_write_lane_cannot_retarget_points() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane_a = add_automation_lane(&mut ws, target);
    let lane_b = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane_a, AutomationMode::Touch);
    assert_eq!(
        ws.session().control().recording().unwrap().fixed_lanes[&target],
        lane_a
    );

    // The open transaction owns its target→lane mapping — a rebind
    // attempt mid-recording is rejected outright.
    let err = ws
        .session_mut()
        .control_with(|c, p| c.bind_write_lane(p, target, lane_b))
        .unwrap_err();
    assert!(err.to_string().contains("recording transaction is open"));

    write(&mut ws, target, 1, 0.5);
    let tx = ws.session().control().recording().unwrap();
    assert!(tx.segments.contains_key(&lane_a));
    assert!(!tx.segments.contains_key(&lane_b));
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied);
    assert_eq!(lane_points(&ws, lane_a), vec![pt(bp(1), 0.5)]);
    assert_eq!(
        ws.session().project().control.lanes[&lane_b].regions,
        vec![]
    );
}

/// Mode/arm/follow changes on a fixed target seal the range instead
/// of redirecting the write; a change on a target the transaction
/// never fixed is not the transaction's boundary. The fixed scope
/// and epoch captured at `record_start` survive all of it.
#[test]
fn aut_03_fixation_survives_mode_and_arm_drift() {
    let mut ws = fresh_ws();
    let (track, target) = add_track_target(&mut ws, "vol", 0.4);
    let (_t2, other) = add_track_target(&mut ws, "pan", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane, AutomationMode::Touch);
    let epoch = ws.session().control().playback().epoch;
    {
        let tx = ws.session().control().recording().unwrap();
        assert_eq!(tx.fixed_targets, vec![target]);
        assert_eq!(tx.fixed_revision, ws.session().revision());
        assert_eq!(tx.epoch, epoch);
        let (owner, scope) = tx.fixed_scopes[&target];
        assert_eq!(owner, ControlOwner::Track(track));
        assert_eq!(scope, AutomationTimeScope::ProjectMusical);
    }

    // A mode flip on a non-fixed target leaves the transaction alone.
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, other, AutomationMode::Read))
        .unwrap();
    assert!(sealed(&ws).is_none());
    write(&mut ws, target, 1, 0.5);

    // Disarming the fixed target seals — the write can never be
    // silently re-enabled into the same transaction.
    ws.session_mut()
        .control_with(|c, p| c.set_arm(p, target, WriteArm::Disarmed))
        .unwrap();
    assert_eq!(sealed(&ws), Some(SealReason::ArmChanged));
    ws.session_mut()
        .control_with(|c, p| c.set_arm(p, target, WriteArm::Armed))
        .unwrap();
    assert!(
        ws.session_mut()
            .control_with(|c, p| c.record_write(p, target, bp(2), 0.9))
            .is_err()
    );

    // The fixations are untouched; the columns still commit.
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.epoch, epoch);
    assert_eq!(recorded_points(&ws), 1);
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert_eq!(outcome.id, id);
    assert!(outcome.applied && outcome.was_sealed);
}

// ---------------------------------------------------------------------------
// F4: boundary flush — gestures land, held writers cover the tail
// ---------------------------------------------------------------------------

/// An in-flight gesture on a fixed target is flushed into the
/// transaction at the boundary — its delivered points are recorded
/// (they are the pass, not a discardable preview) and the gesture is
/// closed as a runtime boundary.
#[test]
fn aut_03_wrap_flushes_in_flight_gesture() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.4)
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(2), 0.8)
        .unwrap();

    let fx = advance(&mut ws, 5);
    assert!(fx.wrapped && fx.sealed);
    // The gesture's delivered points flushed into the segment…
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.segments[&lane], vec![pt(bp(1), 0.4), pt(bp(2), 0.8)]);
    // …and the gesture itself is gone — a straddling write is
    // structurally impossible.
    assert!(
        ws.session_mut()
            .control_mut()
            .gesture_update(target, bp(3), 0.9)
            .is_err()
    );
}

/// A held `Write`-mode value covers the interval continuously: the
/// replace range's exclusive end is the *loop end*, not a
/// `last point + ε` — the tail a held write covered is cut
/// atomically with it.
#[test]
fn aut_03_coverage_end_extends_replace_range_to_loop_end() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    advance(&mut ws, 1);
    open_recording(&mut ws, target, lane, AutomationMode::Write);
    write(&mut ws, target, 2, 0.7);

    let fx = advance(&mut ws, 5); // wrap seals at the loop end
    assert!(fx.wrapped && fx.sealed);
    let tx = ws.session().control().recording().unwrap();
    // The Write-mode held value was active at the boundary → the
    // lane's coverage ends at the loop end.
    assert_eq!(tx.coverage_end[&lane], bp(4));

    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied);
    let regions = &ws.session().project().control.lanes[&lane].regions;
    assert_eq!(regions.len(), 1);
    // The cut covered [2, 4) — the held write's whole tail, not the
    // epsilon after its last explicit point.
    assert_eq!(regions[0].range, BeatRange::try_new(bp(2), bp(4)).unwrap());
    assert_eq!(regions[0].points, vec![pt(bp(2), 0.7)]);
}

// ---------------------------------------------------------------------------
// F5: the boundary table — every close path seals, first seal wins
// ---------------------------------------------------------------------------

/// Stop, seek and project drift each seal at the right reason and
/// keep the recorded columns inspectable/committable.
#[test]
fn aut_03_stop_seek_and_project_change_seal() {
    // Stop.
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_stop);
    assert_eq!(sealed(&ws), Some(SealReason::Stopped));
    assert_eq!(recorded_points(&ws), 1);
    assert!(ws.session_mut().control_record_commit().unwrap().applied);

    // Seek closes at the pre-seek position — the recorded interval
    // must never pretend contiguity across the jump.
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);
    ws.session_mut()
        .control_with(|c, p| c.transport_seek(p, bp(9)));
    assert_eq!(sealed(&ws), Some(SealReason::Seek));
    assert_eq!(ws.session().control().playback().position, bp(9));

    // A foreign apply drifts the revision: sealed `ProjectChange`,
    // commit conflicts, columns retained.
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);
    apply(&mut ws, ProjectOp::AddTrack { name: "x".into() });
    assert_eq!(sealed(&ws), Some(SealReason::ProjectChange));
    assert!(matches!(
        ws.session_mut().control_record_commit(),
        Err(RecordCommitError::Conflict { .. })
    ));
    assert_eq!(recorded_points(&ws), 1);
}

/// The two AUT-03 host boundaries: a clock-epoch install that changes
/// the transport generation, and a stale control plan — each seals
/// the range at the current position; neither fabricates a project
/// mutation.
#[test]
fn aut_03_epoch_change_and_graph_stale_seal() {
    // Epoch change: the anchor installs a different clock generation
    // than the transaction fixed.
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);
    let fixed_epoch = ws.session().control().recording().unwrap().epoch;
    let ns = TimeBase::new(1, 1_000_000_000).unwrap();
    ws.session_mut().control_with(|c, p| {
        c.install_clock_anchor(
            p,
            ClockAnchor {
                domain: ClockDomainId::MONOTONIC,
                stamp_ticks: 0,
                stamp_base: ns,
                frame: EngineFrame::new(ClockEpochId::new(0xE0), 0),
                calibration: TimestampProvenance::Carried,
            },
        );
    });
    assert_eq!(sealed(&ws), Some(SealReason::EpochChange));
    assert_eq!(
        ws.session().control().recording().unwrap().epoch,
        fixed_epoch
    );
    assert_ne!(ws.session().control().playback().epoch, fixed_epoch);
    // An anchor on the same epoch is not a boundary.
    ws.session_mut().control_record_abort();
    let epoch = ws.session().control().playback().epoch;
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    ws.session_mut().control_with(|c, p| {
        c.install_clock_anchor(
            p,
            ClockAnchor {
                domain: ClockDomainId::MONOTONIC,
                stamp_ticks: 10,
                stamp_base: ns,
                frame: EngineFrame::new(epoch, 0),
                calibration: TimestampProvenance::Carried,
            },
        );
    });
    assert!(sealed(&ws).is_none());

    // Stale graph: writes through a mismatched mapping are cut off;
    // runtime gesture state stays (it never touched the project).
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(1), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(2), 0.8)
        .unwrap();
    advance(&mut ws, 2); // the boundary closes at this position
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::note_graph_stale);
    assert_eq!(sealed(&ws), Some(SealReason::GraphStale));
    // The in-flight gesture's delivered points flushed at the close;
    // the gesture survives as runtime state.
    assert_eq!(
        ws.session().control().recording().unwrap().segments[&lane],
        vec![pt(bp(1), 0.5), pt(bp(2), 0.8)]
    );
}

/// Changing the loop range mid-recording voids the pass boundary's
/// coordinate basis — the range closes; re-setting the identical
/// range is not a boundary.
#[test]
fn aut_03_loop_change_seals() {
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);

    // Same range → no boundary.
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    assert!(sealed(&ws).is_none());
    // Different range (or clearing) → sealed.
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(8)))))
        .unwrap();
    assert_eq!(sealed(&ws), Some(SealReason::LoopChanged));
    assert_eq!(recorded_points(&ws), 1);
}

/// First seal wins: once the range is closed, later boundaries update
/// neither the reason nor the columns.
#[test]
fn aut_03_first_seal_wins() {
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);
    advance(&mut ws, 5);
    assert_eq!(sealed(&ws), Some(SealReason::LoopWrap));

    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_stop);
    ws.session_mut()
        .control_with(|c, p| c.transport_seek(p, bp(9)));
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::note_graph_stale);
    assert_eq!(sealed(&ws), Some(SealReason::LoopWrap));
}

// ---------------------------------------------------------------------------
// F6: transaction semantics — one undo, retention, explicit rebase,
//     pre-modulation values, Replace collision, distinct receipts
// ---------------------------------------------------------------------------

/// One session/pass is exactly one project undo step — even when the
/// pass wrote into several lanes.
#[test]
fn aut_03_multi_lane_pass_is_one_undo() {
    let mut ws = fresh_ws();
    let (_t1, target_a) = add_track_target(&mut ws, "vol", 0.4);
    let (_t2, target_b) = add_track_target(&mut ws, "pan", 0.5);
    let lane_a = add_automation_lane(&mut ws, target_a);
    let lane_b = add_automation_lane(&mut ws, target_b);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    // Arm both targets before `record_start` — one transaction fixes
    // the whole armed write-mode set.
    for (target, lane) in [(target_a, lane_a), (target_b, lane_b)] {
        ws.session_mut()
            .control_with(|c, p| c.bind_write_lane(p, target, lane))
            .unwrap();
        ws.session_mut()
            .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
            .unwrap();
        ws.session_mut()
            .control_with(|c, p| c.set_arm(p, target, WriteArm::Armed))
            .unwrap();
    }
    ws.session_mut().control_mut().set_record_enabled(true);
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_play);
    let rev = ws.session().revision();
    let id = ws
        .session_mut()
        .control_with(|c, p| c.record_start(p, rev))
        .unwrap();
    assert_eq!(
        ws.session()
            .control()
            .recording()
            .unwrap()
            .fixed_targets
            .len(),
        2
    );

    write(&mut ws, target_a, 1, 0.6);
    write(&mut ws, target_b, 2, 0.8);
    advance(&mut ws, 5); // wrap seals both columns at once
    assert_eq!(sealed(&ws), Some(SealReason::LoopWrap));

    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert_eq!(outcome.id, id);
    assert!(outcome.applied);
    assert_eq!(lane_points(&ws, lane_a), vec![pt(bp(1), 0.6)]);
    assert_eq!(lane_points(&ws, lane_b), vec![pt(bp(2), 0.8)]);

    // One undo removes the whole pass; one redo restores it.
    assert!(ws.session_mut().undo().unwrap());
    assert_eq!(lane_points(&ws, lane_a), vec![]);
    assert_eq!(lane_points(&ws, lane_b), vec![]);
    assert!(ws.session_mut().redo().unwrap());
    assert_eq!(lane_points(&ws, lane_a), vec![pt(bp(1), 0.6)]);
    assert_eq!(lane_points(&ws, lane_b), vec![pt(bp(2), 0.8)]);
}

/// A failed commit keeps every recorded point: a revision conflict
/// marks the transaction `Conflict` without consuming it, and only an
/// explicit `approve_rebase` — never a silent adopt-latest — lets the
/// columns land.
#[test]
fn aut_03_conflict_retains_points_until_explicit_rebase() {
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    let id = open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);
    write(&mut ws, target, 2, 0.7);
    advance(&mut ws, 5); // LoopWrap
    let fixed = ws.session().control().recording().unwrap().fixed_revision;

    // A foreign edit drifts the revision *after* the pass closed —
    // the commit still conflicts (first seal wins: LoopWrap).
    let foreign = apply(&mut ws, ProjectOp::AddTrack { name: "x".into() });
    let err = ws.session_mut().control_record_commit().unwrap_err();
    match err {
        RecordCommitError::Conflict {
            fixed: f,
            current,
            affected_targets,
        } => {
            assert_eq!((f, current), (fixed, foreign));
            assert_eq!(affected_targets, vec![target]);
        }
        other => panic!("expected Conflict, got {other:?}"),
    }
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.state, RecordingTxnState::Conflict);
    assert_eq!(tx.sealed, Some(SealReason::LoopWrap)); // first seal
    assert_eq!(recorded_points(&ws), 2); // nothing lost
    assert_eq!(lane_points(&ws, lane), vec![]); // nothing landed

    // Explicit rebase → the same columns commit; one undo undoes it.
    ws.session_mut().control_record_approve_rebase(id).unwrap();
    assert_eq!(
        ws.session().control().recording().unwrap().state,
        RecordingTxnState::Pending
    );
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied && outcome.was_sealed);
    assert_eq!(
        ws.session().project().control.lanes[&lane].regions[0].points,
        vec![pt(bp(1), 0.5), pt(bp(2), 0.7)]
    );
    assert!(ws.session_mut().undo().unwrap());
    assert_eq!(lane_points(&ws, lane), vec![]);
}

/// The committed series holds the manual (pre-modulation) values
/// verbatim, the commit never implies a save, and a real save/reload
/// round-trips the landed take — dispatch, observation and project
/// saved stay distinct receipts.
#[test]
fn aut_03_raw_values_saved_separately_survive_reload() {
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    let id = open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.25);
    write(&mut ws, target, 3, 0.75);
    advance(&mut ws, 5);

    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied);
    // Stored values are the raw manual inputs — never host-resolved.
    assert_eq!(
        ws.session().project().control.lanes[&lane].regions[0].points,
        vec![pt(bp(1), 0.25), pt(bp(3), 0.75)]
    );
    // The commit receipt is registered; the save receipt is not —
    // applying ≠ persisting.
    assert!(ws.session().control().applied_record(id).is_some());
    assert!(ws.session().controller().saved().is_none());

    // A real save + reload reproduces the landed take exactly.
    let mut store = InMemoryStore::new();
    ws.session_mut().controller_mut().save(&mut store).unwrap();
    let mut c2 = ProjectController::new(
        Project::new(ProjectId::new(0xA03), "aut-03-test"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c2.load(&store).unwrap();
    assert_eq!(
        c2.live().control.lanes[&lane].regions,
        ws.session().project().control.lanes[&lane].regions
    );
}

/// Region collisions go through AUT-02's explicit `Replace`: the
/// recorded span overwrites, the outside curve survives untouched.
#[test]
fn aut_03_replace_collision_preserves_outside_curve() {
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    // An existing region the pass will collide with.
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: BeatRange::try_new(bp(0), bp(10)).unwrap(),
                interpolation: Interpolation::Step,
                points: vec![pt(bp(1), 0.3), pt(bp(4), 0.7), pt(bp(8), 0.9)],
            },
            policy: ConflictPolicy::Replace,
        },
    );
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    advance(&mut ws, 1);
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 2, 0.5);
    advance(&mut ws, 5); // coverage: in-flight? Touch + no gesture →
    // range is [2, 2+ε); no held writer.
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied);

    let pts = lane_points(&ws, lane);
    // Outside the recorded span the old curve survives.
    for keep in [pt(bp(1), 0.3), pt(bp(4), 0.7), pt(bp(8), 0.9)] {
        assert!(pts.contains(&keep), "kept point {keep:?} lost: {pts:?}");
    }
    // The pass's own point landed.
    assert!(pts.contains(&pt(bp(2), 0.5)));
}

// ---------------------------------------------------------------------------
// F7: edge cases
// ---------------------------------------------------------------------------

/// A wrap on an empty transaction still seals honestly — the commit
/// applies nothing and reports `was_sealed`.
#[test]
fn aut_03_empty_pass_seals_and_commits_nothing() {
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    let id = open_recording(&mut ws, target, lane, AutomationMode::Touch);
    advance(&mut ws, 5);
    assert_eq!(sealed(&ws), Some(SealReason::LoopWrap));
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert_eq!(outcome.id, id);
    assert!(!outcome.applied && outcome.was_sealed);
    assert_eq!(
        ws.session().control().last_disposition(),
        Some(RecordDisposition::Committed { id, revision: None })
    );
}

/// A rearm at the same instant as the wrap lands on the fresh pass —
/// pass fixation is per-transaction, never confused by a same-frame
/// open.
#[test]
fn aut_03_rearm_same_frame_as_wrap_fixes_new_pass() {
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);
    advance(&mut ws, 5); // wrap → pass 2, position back at loop start
    ws.session_mut().control_record_abort();

    // Same control-rate frame: the new transaction fixes pass 2 and
    // the wrapped position, not the old pass's context.
    let rev = ws.session().revision();
    let id = ws
        .session_mut()
        .control_with(|c, p| c.record_start(p, rev))
        .unwrap();
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.pass, LoopPassId(2));
    assert_eq!(tx.started_at, bp(0));
    assert_eq!(recorded_points(&ws), 0);
    write(&mut ws, target, 1, 0.9);
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert_eq!(outcome.id, id);
    assert_eq!(
        ws.session().project().control.lanes[&lane].regions[0].points,
        vec![pt(bp(1), 0.9)]
    );
}

/// The prepared commit pins the sealed state and the fixed revision:
/// applying it never reaches for the "current" revision, and a stale
/// prepared payload can never hijack a later transaction (the
/// fingerprint is what the finish consumes).
#[test]
fn aut_03_prepared_commit_pins_fixed_context() {
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    open_recording(&mut ws, target, lane, AutomationMode::Touch);
    write(&mut ws, target, 1, 0.5);
    let fixed = ws.session().control().recording().unwrap().fixed_revision;
    advance(&mut ws, 5);

    let rev = ws.session().revision();
    let prepared: PreparedRecordingCommit = ws
        .session_mut()
        .control_with(|c, p| c.prepare_record_commit(p, rev))
        .unwrap();
    assert_eq!(prepared.expected_revision, fixed);
    assert!(prepared.was_sealed);
    assert!(prepared.op.is_some());
}
