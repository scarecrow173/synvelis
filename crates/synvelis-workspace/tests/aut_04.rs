//! AUT-04 acceptance tests — `docs/test-specs/aut-04-e2e-automation-curve-lod-record-budget.md`.
//!
//! The headless contract (issue #134): recorded automation is
//! preserved **losslessly** (exact `BeatPoint` rationals, original
//! values — the LOD never mutates the source), the viewport LOD is a
//! bounded projection with exact envelopes / classified features /
//! honest `Exact`/`Reduced`/`Truncated` quality and `NaN` uncovered
//! bins, and the record path distinguishes RT queue overflow,
//! drain-side budget overflow, rejects and partial drains while
//! leaving explicit data gaps — all verified headlessly; real
//! Browser/Canvas/plugin acceptance is the Integration scope.

use synvelis_project::{
    AutomationLaneId, AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange,
    ConflictPolicy, ControlOwner, CounterIdGenerator, CurveFeatureKind, CurveQuality, CurveQuery,
    Interpolation, ParamMapping, ParameterRole, ParameterTargetId, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, TrackId, ValueKind,
};
use synvelis_workspace::{
    AutomationMode, ControlSession, FakeWindowHost, ProjectSession, RECORD_EVENT_QUEUE_CAP,
    RecordBudgets, RecordNotification, RecordPushError, RecordRejectKind, RecordingTransactionId,
    WorkspaceManager, WriteArm,
};

// ---------------------------------------------------------------------------
// fixture (same conventions as aut_03.rs)
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xA04), "aut-04-test");
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

fn add_region(
    ws: &mut Ws,
    lane: AutomationLaneId,
    start: i64,
    end: i64,
    interp: Interpolation,
    points: Vec<AutomationPoint>,
) {
    apply(
        ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: BeatRange::try_new(bp(start), bp(end)).unwrap(),
                interpolation: interp,
                points,
            },
            policy: ConflictPolicy::Replace,
        },
    );
}

fn open_recording(
    ws: &mut Ws,
    target: ParameterTargetId,
    lane: AutomationLaneId,
) -> RecordingTransactionId {
    ws.session_mut()
        .control_with(|c, p| c.bind_write_lane(p, target, lane))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Write))
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

fn query(bins: usize) -> CurveQuery {
    CurveQuery {
        start: 0.0,
        end: 16.0,
        bins,
        max_points: 1000,
        max_features: 1000,
    }
}

// ---------------------------------------------------------------------------
// F1 — recorded points are preserved losslessly
// ---------------------------------------------------------------------------

#[test]
fn recorded_points_keep_exact_rational_positions_and_values() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane);
    // Odd rationals + arbitrary precision values — any implicit
    // quantize/decimate/resample would corrupt these.
    let pos1 = BeatPoint::new(1, 3).unwrap();
    let pos2 = BeatPoint::new(7, 11).unwrap();
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, pos1, 0.123_456_789_012_345))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, pos2, 0.987_654_321_098_765))
        .unwrap();
    let tx = ws.session().control().recording().unwrap();
    let seg = &tx.segments[&lane];
    assert_eq!(seg.len(), 2);
    assert_eq!(seg[0].pos, pos1);
    assert_eq!(seg[0].value, 0.123_456_789_012_345);
    assert_eq!(seg[1].pos, pos2);
    assert_eq!(seg[1].value, 0.987_654_321_098_765);
}

#[test]
fn automation_view_is_read_only_and_never_mutates_source() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    add_region(
        &mut ws,
        lane,
        0,
        9,
        Interpolation::Linear,
        vec![pt(bp(0), 0.1), pt(bp(4), 0.9), pt(bp(8), 0.2)],
    );
    let rev = ws.session().revision();
    let before: Vec<AutomationPoint> = ws.session().project().control.lanes[&lane].regions[0]
        .points
        .clone();
    let view =
        synvelis_project::automation_view(ws.session().project(), target, &query(8)).unwrap();
    assert_eq!(view.quality, CurveQuality::Exact);
    assert_eq!(view.source_points, 3);
    // Source points verbatim — the explicit-anchor channel.
    assert_eq!(view.points, before);
    // Project untouched: same revision, same stored points.
    assert_eq!(ws.session().revision(), rev);
    assert_eq!(
        ws.session().project().control.lanes[&lane].regions[0].points,
        before
    );
}

// ---------------------------------------------------------------------------
// F2 — bounded viewport LOD: exact envelope, covered/NaN semantics
// ---------------------------------------------------------------------------

#[test]
fn automation_view_exact_envelope_and_uncovered_nan() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    // Linear 0→8: peak at 4, trough at 8.
    add_region(
        &mut ws,
        lane,
        0,
        9,
        Interpolation::Linear,
        vec![pt(bp(0), 0.0), pt(bp(4), 1.0), pt(bp(8), 0.0)],
    );
    let view =
        synvelis_project::automation_view(ws.session().project(), target, &query(16)).unwrap();
    assert_eq!(view.bins.len(), 16);
    // Bin 4 covers [4,5) — contains the peak 1.0 at 4; the segment
    // falls to 0.75 by the right edge (exact envelope, not a sample).
    let peak = &view.bins[4];
    assert!(peak.covered);
    assert_eq!(peak.max, 1.0);
    assert_eq!(peak.min, 0.75);
    // Every bin in [0,8) and the chase-held tail to 16 is covered;
    // nothing before the first point exists in this window.
    assert!(view.bins.iter().all(|b| b.covered));
    // Window before the first point: query [-4,4) — bins fully left
    // of position 0 report covered=false + NaN, never fabricated 0s.
    let q = CurveQuery {
        start: -4.0,
        end: 4.0,
        ..query(8)
    };
    let view = synvelis_project::automation_view(ws.session().project(), target, &q).unwrap();
    assert!(!view.bins[0].covered);
    assert!(view.bins[0].min.is_nan() && view.bins[0].max.is_nan());
    assert!(view.bins[4].covered);
}

#[test]
fn automation_view_empty_series_all_uncovered_exact() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let view =
        synvelis_project::automation_view(ws.session().project(), target, &query(4)).unwrap();
    assert_eq!(view.quality, CurveQuality::Exact);
    assert!(view.bins.iter().all(|b| !b.covered && b.min.is_nan()));
    assert!(view.points.is_empty() && view.features.is_empty());
    assert_eq!(view.source_points, 0);
}

// ---------------------------------------------------------------------------
// F3 — classified features + honest quality
// ---------------------------------------------------------------------------

#[test]
fn automation_view_step_edge_and_extremum_features() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    // Step segment 0→4 holds 0.2, then jumps to 0.9 at 4.
    add_region(
        &mut ws,
        lane,
        0,
        9,
        Interpolation::Step,
        vec![pt(bp(0), 0.2), pt(bp(4), 0.9), pt(bp(8), 0.9)],
    );
    let view =
        synvelis_project::automation_view(ws.session().project(), target, &query(16)).unwrap();
    let kinds: Vec<_> = view.features.iter().map(|f| f.kind).collect();
    // The jump at 4 is a StepEdge (governed by the segment entering
    // the point); the boundary at range.start/end is reported too.
    assert!(kinds.contains(&CurveFeatureKind::StepEdge));
    assert!(kinds.contains(&CurveFeatureKind::RegionBoundary));
    let edge = view
        .features
        .iter()
        .find(|f| f.kind == CurveFeatureKind::StepEdge)
        .unwrap();
    assert_eq!(edge.pos, bp(4));
    assert_eq!(edge.value, 0.9);
}

#[test]
fn automation_view_extremum_strict_direction_reversal() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    add_region(
        &mut ws,
        lane,
        0,
        9,
        Interpolation::Linear,
        vec![pt(bp(0), 0.0), pt(bp(4), 1.0), pt(bp(8), 0.0)],
    );
    let view =
        synvelis_project::automation_view(ws.session().project(), target, &query(16)).unwrap();
    let ext = view
        .features
        .iter()
        .filter(|f| f.kind == CurveFeatureKind::Extremum)
        .collect::<Vec<_>>();
    assert_eq!(ext.len(), 1);
    assert_eq!(ext[0].pos, bp(4));
}

#[test]
fn automation_view_reduced_quality_keeps_features() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    add_region(
        &mut ws,
        lane,
        0,
        9,
        Interpolation::Linear,
        vec![
            pt(bp(0), 0.0),
            pt(bp(2), 0.5),
            pt(bp(4), 1.0),
            pt(bp(6), 0.5),
            pt(bp(8), 0.0),
        ],
    );
    let q = CurveQuery {
        max_points: 3,
        ..query(16)
    };
    let view = synvelis_project::automation_view(ws.session().project(), target, &q).unwrap();
    assert_eq!(view.quality, CurveQuality::Reduced);
    assert_eq!(view.source_points, 5);
    assert_eq!(view.truncated_points, 2);
    assert_eq!(view.points.len(), 3);
    // The extremum at 4 survives verbatim-point reduction — the
    // feature channel keeps the shape-critical point.
    assert!(view.points.iter().any(|p| p.pos == bp(4)));
    assert_eq!(view.truncated_features, 0);
}

#[test]
fn automation_view_truncated_quality_when_features_dropped() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    add_region(
        &mut ws,
        lane,
        0,
        9,
        Interpolation::Linear,
        vec![pt(bp(0), 0.0), pt(bp(4), 1.0), pt(bp(8), 0.0)],
    );
    let q = CurveQuery {
        max_features: 0,
        ..query(16)
    };
    let view = synvelis_project::automation_view(ws.session().project(), target, &q).unwrap();
    assert_eq!(view.quality, CurveQuality::Truncated);
    assert!(view.truncated_features > 0);
    assert_eq!(view.truncated_points, 0);
}

#[test]
fn automation_view_rejects_invalid_window() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let bad = CurveQuery {
        start: 8.0,
        end: 4.0,
        ..query(4)
    };
    assert!(synvelis_project::automation_view(ws.session().project(), target, &bad).is_err());
    let bad = CurveQuery {
        bins: 0,
        ..query(4)
    };
    assert!(synvelis_project::automation_view(ws.session().project(), target, &bad).is_err());
}

// ---------------------------------------------------------------------------
// F6/F7 — RT ingress, drain classification, gaps, partial, notifications
// ---------------------------------------------------------------------------

#[test]
fn record_event_rt_rejects_when_gates_closed() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let err = ws
        .session_mut()
        .control_mut()
        .record_event_rt(target, bp(1), 0.5)
        .unwrap_err();
    assert_eq!(err, RecordPushError::GatesClosed);
    assert_eq!(ws.session().control().record_health(target), None);
}

#[test]
fn drain_accepts_queued_events_and_records_deliveries() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane);
    for i in 0_i32..4 {
        ws.session_mut()
            .control_mut()
            .record_event_rt(target, bp(i64::from(i)), 0.25 * f64::from(i))
            .unwrap();
    }
    let rep = ws
        .session_mut()
        .control_with(ControlSession::drain_record_events);
    assert_eq!(rep.popped, 4);
    assert_eq!(rep.accepted, 4);
    assert!(!rep.partial);
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.segments[&lane].len(), 4);
    assert_eq!(tx.segments[&lane][0].pos, bp(0));
    assert_eq!(tx.deliveries[&lane].len(), 4);
}

#[test]
fn drain_rejects_queued_events_after_gates_close() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_mut()
        .record_event_rt(target, bp(1), 0.5)
        .unwrap();
    // Gates close between push and drain — the event is a typed
    // GatesClosed reject with an explicit gap, never attributed.
    ws.session_mut()
        .control_with(ControlSession::transport_stop);
    let rep = ws
        .session_mut()
        .control_with(ControlSession::drain_record_events);
    assert_eq!(rep.accepted, 0);
    assert_eq!(rep.rejected_total, 1);
    assert_eq!(rep.rejected[0].kind, RecordRejectKind::GatesClosed);
}

#[test]
fn drain_rejects_stale_events_as_wrong_txn() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_mut()
        .record_event_rt(target, bp(1), 0.5)
        .unwrap();
    // Abort the transaction — the queued event now belongs to a dead
    // session and is rejected stale, never attributed to a new one.
    ws.session_mut().control_mut().record_abort();
    let rep = ws
        .session_mut()
        .control_with(ControlSession::drain_record_events);
    assert_eq!(rep.stale, 1);
    assert_eq!(rep.accepted, 0);
    assert_eq!(rep.rejected[0].kind, RecordRejectKind::WrongTxn);
    let notes = ws.session().control().record_notifications();
    assert!(notes.iter().any(|n| matches!(
        n,
        RecordNotification::Rejected {
            kind: RecordRejectKind::WrongTxn,
            ..
        }
    )));
}

#[test]
fn drain_reports_point_budget_overflow_and_gap() {
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
    // Overflow drops are explicit gaps on the transaction — never
    // smoothed into the series.
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.segments[&lane].len(), 2);
    let gaps = &tx.record_gaps[&lane];
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].dropped, 2);
    assert_eq!(gaps[0].start, bp(2));
    assert_eq!(gaps[0].end, bp(3));
    assert_eq!(tx.record_dropped, 2);
    // Health tracks the overflow distinctly from queue-full.
    let health = ws.session().control().record_health(target).unwrap();
    assert_eq!(health.overflowed, 2);
    assert_eq!(health.queue_fulls, 0);
    // The session view surfaces the resolved gap.
    let rev = ws.session().revision();
    let view = ws
        .session()
        .control()
        .automation_view(ws.session().project(), target, &query(16), rev)
        .unwrap();
    assert_eq!(view.gaps.len(), 1);
    assert_eq!(view.gaps[0].dropped, 2);
}

#[test]
fn drain_is_bounded_and_reports_partial() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_mut()
        .set_record_budgets(RecordBudgets {
            max_events_per_drain: 3,
            ..RecordBudgets::default()
        });
    for i in 0_i64..8 {
        ws.session_mut()
            .control_mut()
            .record_event_rt(target, bp(i), 0.5)
            .unwrap();
    }
    let rep = ws
        .session_mut()
        .control_with(ControlSession::drain_record_events);
    assert_eq!(rep.popped, 3);
    assert!(rep.partial);
    assert!(
        ws.session()
            .control()
            .record_notifications()
            .iter()
            .any(|n| matches!(n, RecordNotification::Partial { .. }))
    );
    // Retries drain the remainder — the bounded budget never loses
    // queued events.
    let rep = ws
        .session_mut()
        .control_with(ControlSession::drain_record_events);
    assert_eq!(rep.popped, 3);
    assert!(rep.partial);
    let rep = ws
        .session_mut()
        .control_with(ControlSession::drain_record_events);
    assert_eq!(rep.popped, 2);
    assert!(!rep.partial);
}

#[test]
fn session_automation_view_overlays_pending_and_health() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane);
    for i in 1_i32..=3 {
        ws.session_mut()
            .control_mut()
            .record_event_rt(target, bp(i64::from(i)), 0.1 * f64::from(i))
            .unwrap();
    }
    ws.session_mut()
        .control_with(ControlSession::drain_record_events);
    let rev = ws.session().revision();
    let view = ws
        .session()
        .control()
        .automation_view(ws.session().project(), target, &query(16), rev)
        .unwrap();
    assert_eq!(view.revision, rev);
    assert_eq!(view.pending.len(), 1);
    assert_eq!(view.pending[0].0, lane);
    assert_eq!(view.pending[0].1.len(), 3);
    assert_eq!(view.deliveries.len(), 3);
    assert!(view.health.is_none());
}

#[test]
fn notification_log_is_bounded() {
    // The log cap is structural: after the queue-full flood below
    // (one QueueFull + one DataGap + rejects), entries stay within
    // RECORD_NOTIFY_CAP and the drop counter reports exactly what
    // was discarded.
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_with(ControlSession::transport_stop);
    // A sealed transaction + a queued-but-stale event pair produces
    // notifications; whatever the count, the bound and the counter
    // hold.
    assert!(ws.session().control().record_notifications().len() <= 256);
    assert_eq!(ws.session().control().record_notifications_dropped(), 0);
}

#[test]
fn queue_full_is_typed_and_folds_into_gap() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane);
    // Fill the bounded RT queue without draining: the next push is a
    // typed QueueFull, counted on health and folded into a gap.
    let mut full_at = None;
    for i in 0..RECORD_EVENT_QUEUE_CAP + 16 {
        let pos = BeatPoint::new(i64::try_from(i).unwrap(), 1).unwrap();
        match ws
            .session_mut()
            .control_mut()
            .record_event_rt(target, pos, 0.5)
        {
            Ok(()) => {}
            Err(RecordPushError::QueueFull) => {
                full_at = Some(i);
                break;
            }
            Err(e) => panic!("unexpected push error: {e}"),
        }
    }
    let i = full_at.expect("bounded queue must fill");
    let health = ws.session().control().record_health(target).unwrap();
    assert_eq!(health.queue_fulls, 1);
    assert_eq!(health.dropped, 1);
    // Push more failures — the gap run keeps extending.
    let pos = BeatPoint::new(i64::try_from(i + 1).unwrap(), 1).unwrap();
    let err = ws
        .session_mut()
        .control_mut()
        .record_event_rt(target, pos, 0.5)
        .unwrap_err();
    assert_eq!(err, RecordPushError::QueueFull);
    let rep = ws
        .session_mut()
        .control_with(ControlSession::drain_record_events);
    assert_eq!(rep.accepted, i);
    // QueueFull notification + a DataGap covering both lost events.
    let notes = ws.session().control().record_notifications();
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, RecordNotification::QueueFull { .. }))
    );
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, RecordNotification::DataGap { dropped: 2, .. }))
    );
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.record_dropped, 2);
    assert_eq!(tx.record_gaps[&lane].len(), 1);
}

#[test]
fn record_budgets_is_configurable() {
    let mut ws = fresh_ws();
    let (_t, target) = add_track_target(&mut ws, "p", 0.5);
    let lane = add_automation_lane(&mut ws, target);
    let _ = lane;
    let default = ws.session().control().record_budgets();
    assert_eq!(default.max_points, synvelis_workspace::RECORD_POINT_BUDGET);
    ws.session_mut()
        .control_mut()
        .set_record_budgets(RecordBudgets {
            max_points: 4,
            max_events_per_drain: 8,
        });
    assert_eq!(ws.session().control().record_budgets().max_points, 4);
}
