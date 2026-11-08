//! AUT-01 acceptance tests — `docs/test-specs/aut-01-e2e-touch-return-writer-state.md`
//! (issue #131).
//!
//! The agreed mode-boundary + Touch-return contracts, verified
//! headlessly on the session clock: Read/Touch/Latch/Write writer
//! states, the 150 ms Smoothstep return in the pre-modulation
//! reference domain, explicit closure on every transport/focus/plugin
//! boundary, and the `GestureFixation` captured at begin. No GUI, no
//! real plugin — this is the pure state-transition/clock proof of
//! concept.

use ruviform_media_core::{MediaDuration, MediaTime, TimeBase};
use synvelis_project::{
    AutomationLaneId, AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange,
    ConflictPolicy, ControlOwner, CounterIdGenerator, Interpolation, ParamMapping, ParameterRole,
    ParameterTargetId, ProcessorBinding, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, ReferenceSource, TrackId, ValueKind, is_issuable_raw,
};
use synvelis_workspace::{
    AutomationMode, FakeWindowHost, GestureEnd, ProjectSession, WorkspaceManager, WriteArm,
    WriterCloseReason, WriterState,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xAD1), "aut-01-test");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    WorkspaceManager::new(FakeWindowHost::default(), ProjectSession::new(controller))
}

fn apply(ws: &mut Ws, op: ProjectOp) -> ProjectRevision {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev).expect("fixture op")
}

/// The session clock runs on `COMMON_90K`: 1 ms = 90 ticks.
fn clock_ms(ms: i64) -> MediaTime {
    MediaTime::new(ms * 90, TimeBase::COMMON_90K)
}

fn bp(num: i64) -> BeatPoint {
    BeatPoint::new(num, 1).unwrap()
}

fn pt(pos: BeatPoint, value: f64) -> AutomationPoint {
    AutomationPoint { pos, value }
}

fn region(
    start: BeatPoint,
    end: BeatPoint,
    interpolation: Interpolation,
    points: Vec<AutomationPoint>,
) -> AutomationRegion {
    AutomationRegion {
        range: BeatRange::try_new(start, end).unwrap(),
        interpolation,
        points,
    }
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

/// Full record gates: armed + record-enabled + rolling + open
/// transaction fixing `target`.
fn open_recording(ws: &mut Ws, target: ParameterTargetId, lane: AutomationLaneId) {
    ws.session_mut()
        .control_with(|c, p| c.bind_write_lane(p, target, lane))
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
        .unwrap();
}

fn evaluate(ws: &Ws, target: ParameterTargetId, pos: BeatPoint) -> f64 {
    ws.session()
        .control()
        .evaluate(ws.session().project(), target, pos)
        .unwrap()
        .reference
}

fn writer_state(ws: &Ws, target: ParameterTargetId) -> WriterState {
    ws.session().control().writer_state(target)
}

fn lane_points(ws: &Ws, lane: AutomationLaneId) -> Vec<AutomationPoint> {
    ws.session().project().control.lanes[&lane]
        .regions
        .iter()
        .flat_map(|r| r.points.iter().copied())
        .collect()
}

// ---------------------------------------------------------------------------
// AUT-01-01 Read: override during the gesture only, never writes
// ---------------------------------------------------------------------------

#[test]
fn aut_01_read_override_lives_only_during_gesture() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(0), bp(8), Interpolation::Step, vec![pt(bp(0), 0.5)]),
            policy: ConflictPolicy::Reject,
        },
    );
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Read))
        .unwrap();
    let rev = ws.session().revision();

    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    assert_eq!(writer_state(&ws, target), WriterState::Gesture);
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.9)
        .unwrap();
    assert_eq!(
        ws.session()
            .control()
            .evaluate(ws.session().project(), target, bp(1))
            .unwrap()
            .reference_source,
        ReferenceSource::Manual
    );

    // Release: Read returns to the recorded series immediately — no
    // return interpolation, no write, no boundary closure entry.
    let end = ws.session_mut().control_gesture_end(target).unwrap();
    assert_eq!(end, GestureEnd::Returned);
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    assert_eq!(
        ws.session()
            .control()
            .evaluate(ws.session().project(), target, bp(1))
            .unwrap()
            .reference_source,
        ReferenceSource::Automation
    );
    assert!((evaluate(&ws, target, bp(1)) - 0.5).abs() < f64::EPSILON);
    assert!(
        ws.session().control().writer_closes().is_empty(),
        "a normal Read release is not a boundary closure"
    );
    assert_eq!(ws.session().revision(), rev, "Read never mutates");
    assert_eq!(lane_points(&ws, lane).len(), 1, "no write landed");
}

// ---------------------------------------------------------------------------
// AUT-01-02 Touch: gesture-interval write, then a 150 ms Smoothstep return
// ---------------------------------------------------------------------------

#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenario
fn aut_01_touch_writes_gesture_then_returns_smoothstep() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(0), bp(8), Interpolation::Step, vec![pt(bp(0), 0.0)]),
            policy: ConflictPolicy::Reject,
        },
    );
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
        .unwrap();
    open_recording(&mut ws, target, lane);

    // Touch: only the gesture interval writes — one manual point.
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 1.0)
        .unwrap();
    let rev_before_end = ws.session().revision();
    let end = ws.session_mut().control_gesture_end(target).unwrap();
    assert_eq!(end, GestureEnd::Written { points: 1 });
    assert_eq!(
        ws.session().control().recording().unwrap().segments[&lane]
            .iter()
            .map(|p| (p.pos, p.value))
            .collect::<Vec<_>>(),
        vec![(bp(1), 1.0)],
        "only the gesture interval enters the transaction"
    );

    // Release starts the runtime Touch return — never a written point.
    assert_eq!(
        writer_state(&ws, target),
        WriterState::Returning {
            release_value: 1.0,
            progress: 0.0,
            instant: false,
        }
    );
    let e = ws
        .session()
        .control()
        .evaluate(ws.session().project(), target, bp(1))
        .unwrap();
    assert_eq!(e.reference_source, ReferenceSource::Return);
    assert!((e.reference - 1.0).abs() < f64::EPSILON, "x=0 → release");

    // 75 ms in: x = 0.5, w = 3x²-2x³ = 0.5 → midpoint to automation 0.
    ws.session_mut()
        .control_mut()
        .clock_advance(clock_ms(75))
        .unwrap();
    let e = ws
        .session()
        .control()
        .evaluate(ws.session().project(), target, bp(1))
        .unwrap();
    assert_eq!(e.reference_source, ReferenceSource::Return);
    assert!((e.reference - 0.5).abs() < 1e-9, "w(0.5)=0.5 blend");

    // 150 ms: return complete — swept; evaluation is automation again.
    ws.session_mut()
        .control_mut()
        .clock_advance(clock_ms(150))
        .unwrap();
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    let e = ws
        .session()
        .control()
        .evaluate(ws.session().project(), target, bp(1))
        .unwrap();
    assert_eq!(e.reference_source, ReferenceSource::Automation);
    assert!((e.reference - 0.0).abs() < f64::EPSILON);

    // The whole return was runtime interpolation: no project mutation,
    // no extra recorded points, no closure entry (a return completing
    // is not a boundary event).
    assert_eq!(ws.session().revision(), rev_before_end);
    assert!(ws.session().control().writer_closes().is_empty());
    ws.session_mut().control_record_abort();
}

/// The return never mutates the project even with no recording —
/// revision and lane regions are untouched by the whole lifecycle.
#[test]
fn aut_01_touch_return_is_nondestructive() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(0), bp(8), Interpolation::Step, vec![pt(bp(0), 0.5)]),
            policy: ConflictPolicy::Reject,
        },
    );
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
        .unwrap();
    let rev = ws.session().revision();
    let regions = ws.session().project().control.lanes[&lane].regions.clone();

    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.9)
        .unwrap();
    // Gates are closed (not recording): the release returns only.
    assert_eq!(
        ws.session_mut().control_gesture_end(target).unwrap(),
        GestureEnd::Returned
    );
    assert!(matches!(
        writer_state(&ws, target),
        WriterState::Returning {
            release_value,
            ..
        } if (release_value - 0.9).abs() < f64::EPSILON
    ));
    ws.session_mut()
        .control_mut()
        .clock_advance(clock_ms(150))
        .unwrap();
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    assert_eq!(ws.session().revision(), rev, "the return wrote nothing");
    assert_eq!(
        ws.session().project().control.lanes[&lane].regions,
        regions,
        "the persisted curve is untouched by the return"
    );
}

// ---------------------------------------------------------------------------
// AUT-01-03 return duration — 0 ms instant, custom, monotonic clock
// ---------------------------------------------------------------------------

/// A 0 ms duration is an explicit user value: `instant` reports the
/// click/zipper boundary and the return completes immediately.
#[test]
fn aut_01_return_zero_duration_completes_instantly() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(0), bp(8), Interpolation::Step, vec![pt(bp(0), 0.5)]),
            policy: ConflictPolicy::Reject,
        },
    );
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .set_return_duration(MediaDuration::new(0, TimeBase::COMMON_90K))
        .unwrap();
    assert_eq!(ws.session().control().return_duration().value(), 0);

    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.9)
        .unwrap();
    ws.session_mut().control_gesture_end(target).unwrap();

    // instant=true must stay observable even though x=1 already.
    assert_eq!(
        writer_state(&ws, target),
        WriterState::Returning {
            release_value: 0.9,
            progress: 1.0,
            instant: true,
        }
    );
    let e = ws
        .session()
        .control()
        .evaluate(ws.session().project(), target, bp(1))
        .unwrap();
    assert!(
        (e.reference - 0.5).abs() < f64::EPSILON,
        "instant return lands on automation"
    );
    ws.session_mut()
        .control_mut()
        .clock_advance(clock_ms(1))
        .unwrap();
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
}

/// A user-set duration drives x = elapsed/duration on the session
/// clock; a non-monotonic or negative move rejects explicitly.
#[test]
fn aut_01_return_custom_duration_and_clock_contract() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(0), bp(8), Interpolation::Step, vec![pt(bp(0), 0.0)]),
            policy: ConflictPolicy::Reject,
        },
    );
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .set_return_duration(MediaDuration::new(300, TimeBase::new(1, 1_000).unwrap()))
        .unwrap();
    // 300 ms at 90 kHz = 27000 ticks.
    assert_eq!(ws.session().control().return_duration().value(), 27_000);
    assert!(
        ws.session_mut()
            .control_mut()
            .set_return_duration(MediaDuration::new(-1, TimeBase::COMMON_90K))
            .is_err(),
        "a negative duration is a defined rejection"
    );

    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 1.0)
        .unwrap();
    ws.session_mut().control_gesture_end(target).unwrap();

    // The session clock is monotonic: a backwards move rejects.
    ws.session_mut()
        .control_mut()
        .clock_advance(clock_ms(150))
        .unwrap();
    assert_eq!(
        writer_state(&ws, target),
        WriterState::Returning {
            release_value: 1.0,
            progress: 0.5,
            instant: false,
        }
    );
    assert!(
        ws.session_mut()
            .control_mut()
            .clock_advance(clock_ms(149))
            .is_err(),
        "the session clock never moves backwards"
    );
    ws.session_mut()
        .control_mut()
        .clock_advance(clock_ms(300))
        .unwrap();
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
}

/// The return evaluates the automation series at each position —
/// a linear ramp is followed continuously, not sampled once at
/// release.
#[test]
fn aut_01_return_follows_curve_per_position() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(
                bp(0),
                bp(9),
                Interpolation::Linear,
                vec![pt(bp(0), 0.0), pt(bp(8), 1.0)],
            ),
            policy: ConflictPolicy::Reject,
        },
    );
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
        .unwrap();
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 1.0)
        .unwrap();
    ws.session_mut().control_gesture_end(target).unwrap();

    // x = 0.5 → w = 0.5; v = release + (v_auto(pos) - release)·w with
    // v_auto evaluated per position, not frozen at release.
    ws.session_mut()
        .control_mut()
        .clock_advance(clock_ms(75))
        .unwrap();
    // bp(0): v_auto = 0 → 1 + (0-1)·0.5 = 0.5
    assert!((evaluate(&ws, target, bp(0)) - 0.5).abs() < 1e-9);
    // bp(4): v_auto = 0.5 → 1 + (0.5-1)·0.5 = 0.75
    assert!((evaluate(&ws, target, bp(4)) - 0.75).abs() < 1e-9);
    // bp(8): v_auto = 1.0 → 1 (past region end → last point value)
    assert!((evaluate(&ws, target, bp(8)) - 1.0).abs() < 1e-9);
}

/// Touching the same target mid-return re-arms the gesture from the
/// *blended* reference — no audible jump on re-grab.
#[test]
fn aut_01_retouch_during_return_seeds_blended_reference() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(0), bp(8), Interpolation::Step, vec![pt(bp(0), 0.0)]),
            policy: ConflictPolicy::Reject,
        },
    );
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
        .unwrap();
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 1.0)
        .unwrap();
    ws.session_mut().control_gesture_end(target).unwrap();

    ws.session_mut()
        .control_mut()
        .clock_advance(clock_ms(75))
        .unwrap();
    let rev2 = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(2), rev2))
        .unwrap();
    // The re-touch supersedes the return; the override seeds at the
    // blended value (w=0.5 → 0.5), so re-grabbing never jumps.
    assert_eq!(writer_state(&ws, target), WriterState::Gesture);
    let e = ws
        .session()
        .control()
        .evaluate(ws.session().project(), target, bp(2))
        .unwrap();
    assert_eq!(e.reference_source, ReferenceSource::Manual);
    assert!(
        (e.reference - 0.5).abs() < 1e-9,
        "gesture seeds at the blended value"
    );
    ws.session_mut().control_gesture_end(target).unwrap();
}

// ---------------------------------------------------------------------------
// AUT-01-04 Latch / Write held states and their boundaries
// ---------------------------------------------------------------------------

/// Latch holds the last manual value after release — it lands on every
/// advance until a boundary closes it; a mode change is an explicit
/// closure, never an implicit re-arm.
#[test]
fn aut_01_latch_holds_until_boundary() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Latch))
        .unwrap();
    open_recording(&mut ws, target, lane);

    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(0), 0.7)
        .unwrap();
    let end = ws.session_mut().control_gesture_end(target).unwrap();
    assert_eq!(end, GestureEnd::Latched { value: 0.7 });
    assert_eq!(
        writer_state(&ws, target),
        WriterState::LatchHeld { value: 0.7 }
    );

    // The held value writes at control rate while gates hold.
    ws.session_mut()
        .control_with(|c, p| c.transport_advance(p, bp(1)))
        .unwrap();
    assert_eq!(
        ws.session().control().recording().unwrap().segments[&lane]
            .iter()
            .map(|p| (p.pos, p.value))
            .collect::<Vec<_>>(),
        vec![(bp(0), 0.7), (bp(1), 0.7)]
    );

    // Mode change is an explicit closure — the held write ends, it
    // never silently re-arms.
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Read))
        .unwrap();
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    let closes: Vec<_> = ws
        .session()
        .control()
        .writer_closes()
        .iter()
        .cloned()
        .collect();
    assert_eq!(closes.len(), 1);
    assert_eq!(closes[0].reason, WriterCloseReason::ModeChanged);
    assert_eq!(closes[0].state, WriterState::LatchHeld { value: 0.7 });
    ws.session_mut().control_record_abort();
}

/// A return in flight is mode-independent — it completes the release;
/// only held Latch/Write state closes on mode change.
#[test]
fn aut_01_mode_change_keeps_return_but_closes_holds() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(0), bp(8), Interpolation::Step, vec![pt(bp(0), 0.0)]),
            policy: ConflictPolicy::Reject,
        },
    );
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
        .unwrap();
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 1.0)
        .unwrap();
    ws.session_mut().control_gesture_end(target).unwrap();
    assert!(matches!(
        writer_state(&ws, target),
        WriterState::Returning { .. }
    ));

    // A mode change mid-return does not abort the return — it only
    // finishes the release; the target keeps blending to automation.
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Read))
        .unwrap();
    assert!(matches!(
        writer_state(&ws, target),
        WriterState::Returning { .. }
    ));
    ws.session_mut()
        .control_mut()
        .clock_advance(clock_ms(150))
        .unwrap();
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
}

/// Write: only explicitly armed targets write — `record_start` seeds
/// the held reference, advances land it, and a non-target stays
/// untouched. Stop closes the held write.
#[test]
fn aut_01_write_seeds_held_reference_and_stops() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let (_track2, other) = add_track_target(&mut ws, "pan", 0.6);
    let lane = add_automation_lane(&mut ws, target);
    let other_lane = add_automation_lane(&mut ws, other);
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Write))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, other, AutomationMode::Write))
        .unwrap();
    // `other` is Write-mode but NOT armed — it must not write.
    ws.session_mut()
        .control_with(|c, p| c.bind_write_lane(p, target, lane))
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
        .unwrap();

    // Write seeds the reference evaluated at the start position (base
    // 0.4 — no automation under the playhead).
    assert_eq!(
        writer_state(&ws, target),
        WriterState::WriteHeld { value: 0.4 }
    );
    assert_eq!(writer_state(&ws, other), WriterState::Idle);

    // Held value lands at each advance; the un-armed target writes
    // nothing anywhere.
    ws.session_mut()
        .control_with(|c, p| c.transport_advance(p, bp(1)))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.transport_advance(p, bp(2)))
        .unwrap();
    assert_eq!(
        ws.session().control().recording().unwrap().segments[&lane]
            .iter()
            .map(|p| (p.pos, p.value))
            .collect::<Vec<_>>(),
        vec![(bp(1), 0.4), (bp(2), 0.4)],
        "the held reference writes continuously at control rate"
    );
    assert_eq!(
        ws.session()
            .control()
            .recording()
            .unwrap()
            .segments
            .get(&other_lane)
            .map_or(0, Vec::len),
        0
    );

    // Stop is an explicit closure — the held write ends and nothing
    // re-arms on the next play.
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_stop);
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    let closes: Vec<_> = ws
        .session()
        .control()
        .writer_closes()
        .iter()
        .cloned()
        .collect();
    assert_eq!(closes.len(), 1);
    assert_eq!(closes[0].reason, WriterCloseReason::TransportStop);
    assert_eq!(closes[0].state, WriterState::WriteHeld { value: 0.4 });
    ws.session_mut().control_record_abort();
}

// ---------------------------------------------------------------------------
// AUT-01-05 explicit closure on every boundary
// ---------------------------------------------------------------------------

/// Every boundary event explicitly closes gesture/writer state and is
/// recorded in the closure log: stop, seek, loop wrap, focus loss.
#[test]
#[allow(clippy::too_many_lines)] // sequential boundary matrix
fn aut_01_boundaries_close_runtime_state() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(0), bp(8), Interpolation::Step, vec![pt(bp(0), 0.5)]),
            policy: ConflictPolicy::Reject,
        },
    );
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
        .unwrap();

    // --- TransportStop: gesture in flight AND a return on another
    // target both close. -----------------------------------------
    let (_t2, other) = add_track_target(&mut ws, "pan", 0.6);
    let other_lane = add_automation_lane(&mut ws, other);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane: other_lane,
            region: region(bp(0), bp(8), Interpolation::Step, vec![pt(bp(0), 0.0)]),
            policy: ConflictPolicy::Reject,
        },
    );
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, other, AutomationMode::Touch))
        .unwrap();
    let rev = ws.session().revision();
    // other: complete a gesture → Returning.
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, other, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(other, bp(1), 0.9)
        .unwrap();
    ws.session_mut().control_gesture_end(other).unwrap();
    // target: gesture in flight.
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.8)
        .unwrap();

    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_stop);
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    assert_eq!(writer_state(&ws, other), WriterState::Idle);
    let closes: Vec<_> = ws
        .session()
        .control()
        .writer_closes()
        .iter()
        .cloned()
        .collect();
    assert!(
        closes
            .iter()
            .all(|c| c.reason == WriterCloseReason::TransportStop),
        "every runtime state closes with the boundary reason"
    );
    assert_eq!(closes.len(), 2, "the gesture and the return both closed");

    // --- TransportSeek ------------------------------------------------
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.transport_seek(p, bp(4)));
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    assert_eq!(
        ws.session()
            .control()
            .writer_closes()
            .back()
            .unwrap()
            .reason,
        WriterCloseReason::TransportSeek
    );

    // --- LoopWrap -----------------------------------------------------
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_loop(p, Some((bp(0), bp(4)))))
        .unwrap();
    let effect = ws
        .session_mut()
        .control_with(|c, p| c.transport_advance(p, bp(5)))
        .unwrap();
    assert!(effect.wrapped);
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    assert_eq!(
        ws.session()
            .control()
            .writer_closes()
            .back()
            .unwrap()
            .reason,
        WriterCloseReason::LoopWrap
    );

    // --- FocusLost ----------------------------------------------------
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut().control_mut().note_focus_lost();
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    assert_eq!(
        ws.session()
            .control()
            .writer_closes()
            .back()
            .unwrap()
            .reason,
        WriterCloseReason::FocusLost
    );
    // A focus loss mid-gesture never wrote the preview.
    assert_eq!(ws.session().project().control.lanes[&lane].regions.len(), 1);
}

/// Pause keeps held state (same playback); a fresh play press closes
/// it — Latch/Write never silently re-arm across runs.
#[test]
fn aut_01_pause_resume_vs_new_play_boundary() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Latch))
        .unwrap();
    open_recording(&mut ws, target, lane);

    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(0), 0.7)
        .unwrap();
    ws.session_mut().control_gesture_end(target).unwrap();
    assert!(matches!(
        writer_state(&ws, target),
        WriterState::LatchHeld { .. }
    ));

    // Pause mutes the write but keeps the held state; resume continues
    // the same playback — no closure.
    ws.session_mut().control_mut().transport_pause();
    assert!(matches!(
        writer_state(&ws, target),
        WriterState::LatchHeld { .. }
    ));
    ws.session_mut().control_mut().transport_resume();
    assert!(matches!(
        writer_state(&ws, target),
        WriterState::LatchHeld { .. }
    ));
    assert!(ws.session().control().writer_closes().is_empty());

    // A fresh `transport_play` is a *new* run — the held write closes
    // explicitly and never re-arms.
    ws.session_mut().control_mut().transport_pause();
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_play);
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    assert_eq!(
        ws.session()
            .control()
            .writer_closes()
            .back()
            .unwrap()
            .reason,
        WriterCloseReason::NewTransportPlay
    );
    ws.session_mut().control_record_abort();
}

/// A plugin crash closes runtime state of targets it owns only —
/// other owners' states are untouched.
#[test]
fn aut_01_plugin_crash_closes_only_its_targets() {
    let mut ws = fresh_ws();
    // The master strip exists at bootstrap; insert a processor.
    let strip = *ws.session().project().routing.strips.keys().next().unwrap();
    apply(
        &mut ws,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "plug".into(),
            binding: ProcessorBinding::Unresolved {
                descriptor: "plug".into(),
            },
            sidechain_input: false,
        },
    );
    let proc = *ws
        .session()
        .project()
        .routing
        .processors
        .keys()
        .last()
        .unwrap();
    apply(
        &mut ws,
        ProjectOp::AddParameterTarget {
            name: "plug-gain".into(),
            owner: ControlOwner::Processor(proc),
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let plug_target = *ws
        .session()
        .project()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .unwrap();
    let (_track, track_target) = add_track_target(&mut ws, "vol", 0.4);

    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, plug_target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, track_target, bp(0), rev))
        .unwrap();

    ws.session_mut()
        .control_with(|c, p| c.note_plugin_crash(p, proc));
    assert_eq!(writer_state(&ws, plug_target), WriterState::Idle);
    assert_eq!(
        writer_state(&ws, track_target),
        WriterState::Gesture,
        "another owner's gesture is untouched"
    );
    let closes: Vec<_> = ws
        .session()
        .control()
        .writer_closes()
        .iter()
        .cloned()
        .collect();
    assert_eq!(closes.len(), 1);
    assert_eq!(closes[0].target, plug_target);
    assert_eq!(closes[0].reason, WriterCloseReason::PluginCrash);
}

/// A deleted target's runtime state is explicitly closed — a gesture
/// can never keep writing a target that no longer exists.
#[test]
fn aut_01_target_deleted_closes_state() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.9)
        .unwrap();
    assert_eq!(writer_state(&ws, target), WriterState::Gesture);

    apply(&mut ws, ProjectOp::RemoveParameterTarget { target });
    assert_eq!(writer_state(&ws, target), WriterState::Idle);
    let closes: Vec<_> = ws
        .session()
        .control()
        .writer_closes()
        .iter()
        .cloned()
        .collect();
    assert_eq!(closes.len(), 1);
    assert_eq!(closes[0].reason, WriterCloseReason::TargetDeleted);
    // The gesture is gone — ending it now is a defined rejection, not
    // a silent write to a dead target.
    assert!(
        ws.session_mut().control_gesture_end(target).is_err(),
        "a deleted target's gesture cannot end"
    );
}

// ---------------------------------------------------------------------------
// AUT-01-06 GestureFixation — the captured begin context
// ---------------------------------------------------------------------------

/// Target, revision, arm and clock context are fixed at
/// `gesture_begin` — later Inspector-follow/arm changes cannot
/// retarget the write or reinterpret the captured context.
#[test]
fn aut_01_fixation_captures_begin_context() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0), rev))
        .unwrap();

    let fixation = ws
        .session()
        .control()
        .gesture_fixation(target)
        .expect("gesture in flight");
    assert_eq!(fixation.target, target);
    assert_eq!(fixation.revision, rev);
    assert_eq!(fixation.arm, WriteArm::Disarmed);
    assert_eq!(fixation.epoch, ws.session().control().playback().epoch);
    assert_eq!(fixation.frame_base, TimeBase::COMMON_90K);

    // Arming after begin does not retro-fix the captured arm state —
    // the write keeps the context it fixed.
    ws.session_mut()
        .control_with(|c, p| c.set_arm(p, target, WriteArm::Armed))
        .unwrap();
    assert_eq!(
        ws.session().control().gesture_fixation(target).unwrap().arm,
        WriteArm::Disarmed
    );
    ws.session_mut()
        .control_mut()
        .gesture_cancel(target)
        .unwrap();
    assert!(ws.session().control().gesture_fixation(target).is_none());
    // Cancel is an explicit closure.
    assert_eq!(
        ws.session()
            .control()
            .writer_closes()
            .back()
            .unwrap()
            .reason,
        WriterCloseReason::Cancelled
    );
}
