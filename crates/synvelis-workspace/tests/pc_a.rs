//! PC-A acceptance tests — `docs/test-specs/pc-a-e2e-parameter-control.md`.
//!
//! The agreed parameter-control contracts (issue #40 / issue #6),
//! verified headlessly: deterministic `CounterIdGenerator` ids,
//! `FakeWindowHost`, and fixture `TargetCapability` contracts — no GUI,
//! no real plugin.

use ruviform_media_core::{
    ClockAnchor, ClockDomainId, EngineFrame, InputStamp, MediaTime, MediaTimestamp, Ppqn, Tick,
    TickPosition, TimeBase, TimestampProvenance,
};
use ruviform_plugin_core::{ParamFlags, ParamWriteMeans, ParamWriteRejection, check_param_write};
use synvelis_project::{
    AutomationLaneId, AutomationPoint, AutomationRegion, BeatPoint, BeatRange, ClipExtent, ClipId,
    ClipPlacement, ClipPlayback, ConflictPolicy, ControlGraph, ControlScope, CounterIdGenerator,
    Error, Interpolation, LfoShape, MusicalDuration, MusicalTick, ParamMapping, ParameterTargetId,
    PlaybackMapping, Polarity, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision,
    ReferenceSource, SourceFrame, SourceKind, SourceRange, TargetCapability, TimeAnchor, TrackId,
    Transform, ValueKind, evaluate_control,
};
use synvelis_workspace::{
    AutomationMode, ControlSession, FakeWindowHost, GestureEnd, Lowered, ObservationState,
    ProjectSession, SealReason, WorkspaceManager, WriteArm,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xFCA), "pc-a-test");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    WorkspaceManager::new(FakeWindowHost::default(), ProjectSession::new(controller))
}

fn apply(ws: &mut Ws, op: ProjectOp) -> ProjectRevision {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev).expect("fixture op")
}

fn try_apply(ws: &mut Ws, op: ProjectOp) -> Result<ProjectRevision, Error> {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev)
}

fn bp(num: i64) -> BeatPoint {
    BeatPoint::new(num, 1).unwrap()
}

fn frac(num: i64, den: u32) -> BeatPoint {
    BeatPoint::new(num, den).unwrap()
}

fn pt(pos: BeatPoint, value: f64) -> AutomationPoint {
    AutomationPoint { pos, value }
}

fn span(start: BeatPoint, end: BeatPoint) -> BeatRange {
    BeatRange::try_new(start, end).unwrap()
}

fn region(start: BeatPoint, end: BeatPoint, points: Vec<AutomationPoint>) -> AutomationRegion {
    AutomationRegion {
        range: span(start, end),
        interpolation: Interpolation::Step,
        points,
    }
}

fn caps(flags: ParamFlags, min: f64, max: f64) -> TargetCapability {
    TargetCapability {
        param_id: 7,
        flags_bits: flags.bits(),
        min_value: min,
        max_value: max,
        default_value: min,
    }
}

fn add_track_target(ws: &mut Ws, name: &str, base: f64) -> (TrackId, ParameterTargetId) {
    apply(ws, ProjectOp::AddTrack { name: "t".into() });
    let track = *ws.session().project().tracks.keys().last().unwrap();
    apply(
        ws,
        ProjectOp::AddParameterTarget {
            name: name.into(),
            owner: ControlScope::Track(track),
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

fn add_target_with(
    ws: &mut Ws,
    kind: ValueKind,
    mapping: ParamMapping,
    capability: Option<TargetCapability>,
    base: f64,
) -> ParameterTargetId {
    apply(
        ws,
        ProjectOp::AddParameterTarget {
            name: "p".into(),
            owner: ControlScope::Project,
            kind,
            mapping,
            capability,
            base,
        },
    );
    *ws.session()
        .project()
        .control
        .targets
        .keys()
        .last()
        .unwrap()
}

fn add_automation_lane(ws: &mut Ws, target: ParameterTargetId) -> AutomationLaneId {
    apply(ws, ProjectOp::AddAutomationLane { target });
    *ws.session().project().control.lanes.keys().last().unwrap()
}

fn add_macro_source(ws: &mut Ws, value: f64) -> synvelis_project::ModulationSourceId {
    apply(
        ws,
        ProjectOp::AddModulationSource {
            owner: ControlScope::Project,
            kind: SourceKind::Macro { value },
        },
    );
    *ws.session()
        .project()
        .control
        .sources
        .keys()
        .last()
        .unwrap()
}

fn seeded_clip(ws: &mut Ws, track: TrackId, start_tick: i64, end_tick: i64) -> ClipId {
    apply(
        ws,
        ProjectOp::RegisterAsset {
            digest: "aa".into(),
        },
    );
    let asset = *ws.session().project().assets.keys().last().unwrap();
    apply(
        ws,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "aa".into(),
        },
    );
    let source = *ws.session().project().sources.keys().last().unwrap();
    apply(ws, ProjectOp::AddLane { track_id: track });
    let lane = *ws.session().project().lanes.keys().last().unwrap();
    apply(
        ws,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(start_tick)),
                extent: ClipExtent::Musical(MusicalDuration::new(end_tick - start_tick).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(1_000_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    *ws.session().project().clips.keys().last().unwrap()
}

fn musical(tick: i64) -> TimeAnchor {
    TimeAnchor::Musical(MusicalTick::new(tick))
}

fn media_stamp(ticks: i64, provenance: TimestampProvenance) -> MediaTimestamp {
    let time = MediaTime::new(ticks, TimeBase::COMMON_90K);
    match provenance {
        TimestampProvenance::Carried => MediaTimestamp::carried(time),
        TimestampProvenance::Derived => MediaTimestamp::derived(time),
        _ => MediaTimestamp::estimated(time),
    }
}

// ---------------------------------------------------------------------------
// PC-A-01 single source + three projections
// ---------------------------------------------------------------------------

#[test]
fn pc_a_01_single_source_three_projections() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(0), bp(4), vec![pt(bp(1), 0.8)]),
            policy: ConflictPolicy::Reject,
        },
    );
    let source = add_macro_source(&mut ws, 0.5);
    apply(
        &mut ws,
        ProjectOp::AddModulationAssignment {
            source,
            target,
            depth: 0.2,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    let assign = *ws
        .session()
        .project()
        .control
        .assignments
        .keys()
        .last()
        .unwrap();

    let project = ws.session().project();
    // Lane projection == the persisted series (borrowed, not copied).
    let lanes = ControlSession::lane_view(project, target);
    assert_eq!(lanes.len(), 1);
    assert_eq!(lanes[0].1.points, vec![pt(bp(1), 0.8)]);
    // Signal Flow projection == the persisted assignment.
    let flow = ControlSession::signal_flow_view(project, target);
    assert_eq!(flow.len(), 1);
    assert_eq!(flow[0].id, assign);
    assert!((flow[0].depth - 0.2).abs() < f64::EPSILON);
    assert!(flow[0].enabled);
    // Inspector breakdown == the same evaluation the projections read.
    let ins = ws
        .session()
        .control()
        .inspector_breakdown(project, target, bp(1))
        .unwrap();
    assert_eq!(ins.evaluation.automation, Some(0.8));
    assert_eq!(ins.evaluation.contributions.len(), 1);
    assert_eq!(ins.evaluation.contributions[0].assignment, assign);
    assert_eq!(ins.mode, AutomationMode::Read);
    assert_eq!(ins.arm, WriteArm::Disarmed);
}

// ---------------------------------------------------------------------------
// PC-A-02 scope separation + clip-local follows the clip
// ---------------------------------------------------------------------------

#[test]
fn pc_a_02_scope_separation_clip_follows_move() {
    let mut ws = fresh_ws();
    let (track, track_target) = add_track_target(&mut ws, "track-vol", 0.3);
    let track_lane = add_automation_lane(&mut ws, track_target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane: track_lane,
            region: region(bp(0), bp(8), vec![pt(bp(0), 0.5)]),
            policy: ConflictPolicy::Reject,
        },
    );
    let clip = seeded_clip(&mut ws, track, 960, 1920); // beats [1, 2)
    // Clip-local envelope target — coordinates are clip-relative beats.
    apply(
        &mut ws,
        ProjectOp::AddParameterTarget {
            name: "clip-env".into(),
            owner: ControlScope::Clip(clip),
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.1,
        },
    );
    let clip_target = *ws
        .session()
        .project()
        .control
        .targets
        .keys()
        .last()
        .unwrap();
    let clip_lane = add_automation_lane(&mut ws, clip_target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane: clip_lane,
            region: region(bp(0), bp(1), vec![pt(bp(0), 0.9)]),
            policy: ConflictPolicy::Reject,
        },
    );

    let project = ws.session().project();
    // Inside the clip [1,2): the clip envelope resolves local 0.5 →
    // chase holds 0.9; the track envelope keeps its own 0.5 — the clip
    // envelope does NOT override track automation.
    let e = evaluate_control(project, clip_target, frac(3, 2), None).unwrap();
    assert_eq!(e.automation, Some(0.9));
    assert_eq!(e.position, frac(1, 2).to_beat());
    let e = evaluate_control(project, track_target, frac(3, 2), None).unwrap();
    assert_eq!(e.automation, Some(0.5));
    // Outside the clip range: ScopeUnavailable, not a silent fallback.
    assert!(matches!(
        evaluate_control(project, clip_target, bp(5), None),
        Err(Error::ScopeUnavailable { .. })
    ));
    // Note scope is honestly rejected — no note entity model exists.
    assert!(matches!(
        try_apply(
            &mut ws,
            ProjectOp::AddParameterTarget {
                name: "note-env".into(),
                owner: ControlScope::Note(synvelis_project::NoteId::new(1)),
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base: 0.5,
            }
        ),
        Err(Error::Validation(_))
    ));
    // Moving the clip shifts where the envelope lands — the points
    // themselves are not rewritten.
    apply(
        &mut ws,
        ProjectOp::MoveClip {
            clip_id: clip,
            anchor: musical(1920), // moves to beats [2, 3) (extent preserved)
        },
    );
    let project = ws.session().project();
    assert!(matches!(
        evaluate_control(project, clip_target, frac(3, 2), None),
        Err(Error::ScopeUnavailable { .. })
    ));
    let e = evaluate_control(project, clip_target, frac(5, 2), None).unwrap();
    assert_eq!(e.automation, Some(0.9));
}

// ---------------------------------------------------------------------------
// PC-A-03 conflict detection — Reject reports, Replace explicitly overwrites
// ---------------------------------------------------------------------------

#[test]
fn pc_a_03_conflict_reject_and_replace() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane_a = add_automation_lane(&mut ws, target);
    let lane_b = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane: lane_a,
            region: region(bp(0), bp(4), vec![pt(bp(0), 0.5)]),
            policy: ConflictPolicy::Reject,
        },
    );
    // An overlapping write under Reject → AutomationConflict; project
    // unchanged.
    let rev = ws.session().revision();
    let err = try_apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane: lane_b,
            region: region(bp(2), bp(6), vec![pt(bp(2), 0.9)]),
            policy: ConflictPolicy::Reject,
        },
    );
    assert!(matches!(err, Err(Error::AutomationConflict { .. })));
    assert_eq!(ws.session().revision(), rev);
    assert_eq!(
        ws.session().project().control.lanes[&lane_b].regions,
        Vec::new()
    );
    // Explicit Replace: lane_a's overlapping span is overwritten and
    // the merged series has exactly one source per time.
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane: lane_b,
            region: region(bp(2), bp(6), vec![pt(bp(2), 0.9)]),
            policy: ConflictPolicy::Replace,
        },
    );
    let project = ws.session().project();
    let e = evaluate_control(project, target, bp(1), None).unwrap();
    assert_eq!(e.automation, Some(0.5));
    let e = evaluate_control(project, target, bp(3), None).unwrap();
    assert_eq!(e.automation, Some(0.9));
    assert_eq!(synvelis_project::detect_conflicts(project), Vec::new());
    // A persisted conflict survives nothing: load-time validation
    // reports the same condition via validate_references.
    let mut tampered = project.clone();
    tampered
        .control
        .lanes
        .values_mut()
        .next()
        .unwrap()
        .regions
        .push(region(bp(1), bp(3), vec![pt(bp(1), 0.7)]));
    assert!(matches!(
        tampered.validate_references(),
        Err(Error::AutomationConflict { .. })
    ));
}

// ---------------------------------------------------------------------------
// PC-A-04 breakdown + observation states
// ---------------------------------------------------------------------------

#[test]
fn pc_a_04_breakdown_and_observation_states() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(0), bp(4), vec![pt(bp(1), 0.8)]),
            policy: ConflictPolicy::Reject,
        },
    );
    let source = add_macro_source(&mut ws, 0.5);
    apply(
        &mut ws,
        ProjectOp::AddModulationAssignment {
            source,
            target,
            depth: 0.3,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );

    let project = ws.session().project();
    let e = evaluate_control(project, target, bp(2), None).unwrap();
    // Base/Automation/Reference/Contribution/Host-resolved breakdown.
    assert!((e.base - 0.4).abs() < f64::EPSILON);
    assert_eq!(e.automation, Some(0.8));
    assert_eq!(e.reference_source, ReferenceSource::Automation);
    assert!((e.reference - 0.8).abs() < f64::EPSILON);
    assert_eq!(e.contributions.len(), 1);
    let c = &e.contributions[0];
    assert!((c.raw - 0.5).abs() < f64::EPSILON);
    assert!((c.depth - 0.3).abs() < f64::EPSILON);
    assert!((c.value - 0.15).abs() < 1e-12);
    assert!((e.raw_total - 0.95).abs() < 1e-12);
    assert!((e.host_normalized - 0.95).abs() < 1e-12);
    assert!(!e.saturated && !e.quantized);
    // Before the first point: base reference.
    let e = evaluate_control(project, target, bp(0), None).unwrap();
    assert_eq!(e.reference_source, ReferenceSource::Base);
    assert!((e.reference - 0.4).abs() < f64::EPSILON);

    // Observation states: Unavailable → AtPlayhead → Live → Captured →
    // Stale.
    assert_eq!(
        ws.session().control().observation_state(target),
        ObservationState::Unavailable
    );
    ws.session_mut()
        .control_with(|c, p| {
            c.feed_plugin_report(
                p,
                target,
                0.6,
                media_stamp(100, TimestampProvenance::Carried),
            )
        })
        .unwrap();
    assert_eq!(
        ws.session().control().observation_state(target),
        ObservationState::AtPlayhead
    );
    ws.session_mut().control_mut().transport_play();
    assert_eq!(
        ws.session().control().observation_state(target),
        ObservationState::Live
    );
    ws.session_mut().control_mut().transport_stop();
    ws.session_mut()
        .control_with(|c, p| {
            c.feed_probe(
                p,
                target,
                0.7,
                media_stamp(200, TimestampProvenance::Carried),
            )
        })
        .unwrap();
    assert_eq!(
        ws.session().control().observation_state(target),
        ObservationState::Captured
    );
    // An Estimated-provenance stamp is Stale, never trusted.
    let target2 = add_target_with(
        &mut ws,
        ValueKind::Continuous,
        ParamMapping::Linear,
        None,
        0.5,
    );
    ws.session_mut()
        .control_with(|c, p| {
            c.feed_plugin_report(
                p,
                target2,
                0.5,
                media_stamp(50, TimestampProvenance::Estimated),
            )
        })
        .unwrap();
    assert_eq!(
        ws.session().control().observation_state(target2),
        ObservationState::Stale
    );
}

// ---------------------------------------------------------------------------
// PC-A-05 mode state machine + recording transaction
// ---------------------------------------------------------------------------

#[test]
fn pc_a_05_mode_select_never_mutates_and_off_read_behave() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(0), bp(8), vec![pt(bp(0), 0.5)]),
            policy: ConflictPolicy::Reject,
        },
    );

    // Mode selection alone must not advance the revision.
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Off))
        .unwrap();
    assert_eq!(ws.session().revision(), rev);
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Read))
        .unwrap();
    assert_eq!(ws.session().revision(), rev);

    // Off: gesture end edits the persisted base — one undoable edit;
    // automation data untouched.
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Off))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0)))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.25)
        .unwrap();
    let end = ws.session_mut().control_gesture_end(target).unwrap();
    assert!(matches!(end, GestureEnd::BaseEdit { .. }));
    assert_eq!(ws.session().revision().0, rev.0 + 1);
    assert!((ws.session().project().control.targets[&target].base - 0.25).abs() < f64::EPSILON);
    ws.session_mut().undo().unwrap();
    assert!((ws.session().project().control.targets[&target].base - 0.4).abs() < f64::EPSILON);

    // Read: the manual value overrides during the gesture and returns
    // to the recorded series at end — nothing written, no undo entry.
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Read))
        .unwrap();
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0)))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.9)
        .unwrap();
    let e = ws
        .session()
        .control()
        .evaluate(ws.session().project(), target, bp(1))
        .unwrap();
    assert_eq!(e.reference_source, ReferenceSource::Manual);
    assert!((e.reference - 0.9).abs() < f64::EPSILON);
    let end = ws.session_mut().control_gesture_end(target).unwrap();
    assert_eq!(end, GestureEnd::Returned);
    assert_eq!(ws.session().revision(), rev);
    let e = ws
        .session()
        .control()
        .evaluate(ws.session().project(), target, bp(1))
        .unwrap();
    assert_eq!(e.reference_source, ReferenceSource::Automation);
    assert!((e.reference - 0.5).abs() < f64::EPSILON);
}

#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenario
fn pc_a_05_recording_gates_undo_seal_fixation() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
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

    // Without transport rolling the Touch gesture writes nothing.
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0)))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.9)
        .unwrap();
    let end = ws.session_mut().control_gesture_end(target).unwrap();
    assert_eq!(end, GestureEnd::Returned);
    assert_eq!(
        ws.session().project().control.lanes[&lane].regions,
        Vec::new()
    );

    // Rolling + record_start fixes target set and revision.
    ws.session_mut().control_mut().transport_play();
    let rev = ws.session().revision();
    ws.session_mut().control_mut().record_start(rev).unwrap();
    assert_eq!(
        ws.session().control().recording().unwrap().fixed_targets,
        vec![target]
    );
    // Touch gesture writes the gesture interval (manual values only).
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0)))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.6)
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(2), 0.7)
        .unwrap();
    let end = ws.session_mut().control_gesture_end(target).unwrap();
    assert_eq!(end, GestureEnd::Written { points: 2 });

    // Recording stores pre-modulation manual values: add an LFO +
    // assignment mid-session; the recorded point is the manual value,
    // not the host-resolved (modulated) one — no double modulation.
    apply(
        &mut ws,
        ProjectOp::AddModulationSource {
            owner: ControlScope::Project,
            kind: SourceKind::Lfo {
                rate: bp(1),
                shape: LfoShape::Sine,
                bipolar: true,
            },
        },
    );
    let source = *ws
        .session()
        .project()
        .control
        .sources
        .keys()
        .last()
        .unwrap();
    apply(
        &mut ws,
        ProjectOp::AddModulationAssignment {
            source,
            target,
            depth: 0.4,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    ws.session_mut()
        .control_mut()
        .record_write(target, bp(3), 0.9)
        .unwrap();
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied && !outcome.was_sealed);
    let lane_regions = &ws.session().project().control.lanes[&lane].regions;
    assert_eq!(lane_regions.len(), 1);
    let points = &lane_regions[0].points;
    assert!(points.contains(&pt(bp(1), 0.6)));
    assert!(points.contains(&pt(bp(2), 0.7)));
    assert!(points.contains(&pt(bp(3), 0.9))); // manual, not host-resolved
    // One session = one undo: a single undo removes the whole interval.
    ws.session_mut().undo().unwrap();
    assert_eq!(
        ws.session().project().control.lanes[&lane].regions,
        Vec::new()
    );
    ws.session_mut().redo().unwrap();

    // Loop wrap seals the transaction — the recorded interval is never
    // silently overwritten.
    ws.session_mut()
        .control_mut()
        .set_loop(Some((bp(0), bp(4))))
        .unwrap();
    let rev = ws.session().revision();
    ws.session_mut().control_mut().record_start(rev).unwrap();
    ws.session_mut()
        .control_mut()
        .record_write(target, bp(1), 0.5)
        .unwrap();
    let effect = ws
        .session_mut()
        .control_with(|c, p| c.transport_advance(p, bp(5)))
        .unwrap();
    assert!(effect.wrapped && effect.sealed);
    assert_eq!(
        ws.session().control().recording().unwrap().sealed,
        Some(SealReason::LoopWrap)
    );
    assert!(
        ws.session_mut()
            .control_mut()
            .record_write(target, bp(0), 0.1)
            .is_err()
    );
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied && outcome.was_sealed);

    // Fixation: arming a second target after record_start must not
    // retarget the open transaction.
    let (_t2, target2) = add_track_target(&mut ws, "vol2", 0.5);
    let lane2 = add_automation_lane(&mut ws, target2);
    ws.session_mut()
        .control_with(|c, p| c.bind_write_lane(p, target2, lane2))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target2, AutomationMode::Write))
        .unwrap();
    let rev = ws.session().revision();
    ws.session_mut().control_mut().record_start(rev).unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_arm(p, target2, WriteArm::Armed))
        .unwrap();
    assert!(
        ws.session_mut()
            .control_mut()
            .record_write(target2, bp(0), 0.8)
            .is_err()
    );
    assert_eq!(
        ws.session().control().recording().unwrap().fixed_targets,
        vec![target]
    );
    ws.session_mut().control_record_abort();
}

// ---------------------------------------------------------------------------
// PC-A-06 control graph — zero-delay cycle rejected, order deterministic
// ---------------------------------------------------------------------------

#[test]
fn pc_a_06_cycle_rejected_and_order_deterministic() {
    let mut ws = fresh_ws();
    let (_t, a) = add_track_target(&mut ws, "a", 0.5);
    let (_t2, b) = add_track_target(&mut ws, "b", 0.5);
    let s1 = add_macro_source(&mut ws, 0.5);
    let s2 = add_macro_source(&mut ws, 0.3);
    // a drives b's depth; b drives a's depth → zero-delay cycle.
    apply(
        &mut ws,
        ProjectOp::AddModulationAssignment {
            source: s1,
            target: b,
            depth: 0.2,
            depth_source: Some(a),
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    apply(
        &mut ws,
        ProjectOp::AddModulationAssignment {
            source: s2,
            target: a,
            depth: 0.2,
            depth_source: Some(b),
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    assert!(matches!(
        ControlGraph::compile(ws.session().project()),
        Err(Error::ControlCycle { .. })
    ));
    // Evaluation itself also refuses to recurse.
    assert!(matches!(
        evaluate_control(ws.session().project(), a, bp(0), None),
        Err(Error::ControlCycle { .. })
    ));
    // Disabled assignments contribute no edges — disable one and the
    // graph compiles.
    let assign = *ws
        .session()
        .project()
        .control
        .assignments
        .keys()
        .last()
        .unwrap();
    apply(
        &mut ws,
        ProjectOp::SetModulationAssignment {
            id: assign,
            depth: None,
            enabled: Some(false),
        },
    );
    let plan = ControlGraph::compile(ws.session().project()).unwrap();
    assert!(plan.order.contains(&a) && plan.order.contains(&b));

    // Contributions are evaluated in assignment-id order — the fixed
    // order making the sum deterministic.
    let mut ws2 = fresh_ws();
    let (_t3, target) = add_track_target(&mut ws2, "v", 0.5);
    let mut ids = Vec::new();
    for v in [0.2, 0.1] {
        let s = add_macro_source(&mut ws2, v);
        apply(
            &mut ws2,
            ProjectOp::AddModulationAssignment {
                source: s,
                target,
                depth: 0.1,
                depth_source: None,
                polarity: Polarity::Bipolar,
                transform: Transform::Identity,
                enabled: true,
            },
        );
        ids.push(
            *ws2.session()
                .project()
                .control
                .assignments
                .keys()
                .last()
                .unwrap(),
        );
    }
    let e = evaluate_control(ws2.session().project(), target, bp(0), None).unwrap();
    assert!((e.raw_total - (0.5 + 0.02 + 0.01)).abs() < 1e-12);
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(
        e.contributions
            .iter()
            .map(|c| c.assignment)
            .collect::<Vec<_>>(),
        sorted
    );
}

// ---------------------------------------------------------------------------
// PC-A-07 capability honesty
// ---------------------------------------------------------------------------

#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenario
fn pc_a_07_capability_honesty() {
    let mut ws = fresh_ws();
    // READONLY target: every write path rejects — base edit, automation
    // write, gesture, event lowering.
    let ro = add_target_with(
        &mut ws,
        ValueKind::Continuous,
        ParamMapping::Linear,
        Some(caps(
            ParamFlags::AUTOMATABLE.union(ParamFlags::READONLY),
            0.0,
            1.0,
        )),
        0.5,
    );
    assert!(matches!(
        try_apply(
            &mut ws,
            ProjectOp::SetParameterBase {
                target: ro,
                value: 0.1
            }
        ),
        Err(Error::WriteRejected { .. })
    ));
    let ro_lane = add_automation_lane(&mut ws, ro);
    assert!(matches!(
        try_apply(
            &mut ws,
            ProjectOp::WriteAutomationRegion {
                lane: ro_lane,
                region: region(bp(0), bp(2), vec![pt(bp(0), 0.5)]),
                policy: ConflictPolicy::Reject,
            }
        ),
        Err(Error::WriteRejected { .. })
    ));
    assert!(matches!(
        ws.session_mut()
            .control_with(|c, p| c.gesture_begin(p, ro, bp(0))),
        Err(Error::WriteRejected { .. })
    ));
    assert!(matches!(
        ws.session().control().lower(
            ws.session().project(),
            ro,
            bp(0),
            ParamWriteMeans::TimedEvent
        ),
        Err(Error::WriteRejected { .. })
    ));

    // Non-AUTOMATABLE: TimedEvent → explicit degrade to BlockBoundary;
    // Immediate → UnsupportedMeans. The upstream contract reports the
    // typed rejection unchanged.
    let cap_none = caps(ParamFlags::NONE, 0.0, 1.0);
    assert_eq!(
        check_param_write(
            &cap_none.param_info("p").unwrap(),
            ParamWriteMeans::TimedEvent
        ),
        Err(ParamWriteRejection::NotAutomatable)
    );
    let plain = add_target_with(
        &mut ws,
        ValueKind::Continuous,
        ParamMapping::Linear,
        Some(cap_none),
        0.5,
    );
    let lowered = ws
        .session()
        .control()
        .lower(
            ws.session().project(),
            plain,
            bp(0),
            ParamWriteMeans::TimedEvent,
        )
        .unwrap();
    assert!(matches!(
        lowered,
        Lowered::Degraded {
            requested: ParamWriteMeans::TimedEvent,
            ..
        }
    ));
    let exact = ws
        .session()
        .control()
        .lower(
            ws.session().project(),
            plain,
            bp(0),
            ParamWriteMeans::BlockBoundary,
        )
        .unwrap();
    assert!(matches!(exact, Lowered::Exact(_)));
    assert!(matches!(
        ws.session().control().lower(
            ws.session().project(),
            plain,
            bp(0),
            ParamWriteMeans::Immediate
        ),
        Err(Error::WriteRejected { .. })
    ));

    // Discrete kinds quantize the final value; modulation is rejected.
    let bool_t = add_target_with(&mut ws, ValueKind::Boolean, ParamMapping::Linear, None, 0.7);
    let e = evaluate_control(ws.session().project(), bool_t, bp(0), None).unwrap();
    assert!(e.quantized && (e.host_normalized - 1.0).abs() < f64::EPSILON);
    let s = add_macro_source(&mut ws, 0.5);
    assert!(matches!(
        try_apply(
            &mut ws,
            ProjectOp::AddModulationAssignment {
                source: s,
                target: bool_t,
                depth: 0.1,
                depth_source: None,
                polarity: Polarity::Bipolar,
                transform: Transform::Identity,
                enabled: true,
            }
        ),
        Err(Error::Validation(_))
    ));
    // Stepped: 4 steps → legal values {0, 1/3, 2/3, 1}.
    let step_t = add_target_with(
        &mut ws,
        ValueKind::Stepped { steps: 4 },
        ParamMapping::Linear,
        None,
        0.4,
    );
    let e = evaluate_control(ws.session().project(), step_t, bp(0), None).unwrap();
    assert!((e.host_normalized - 1.0 / 3.0).abs() < 1e-9);
    let choice_t = add_target_with(
        &mut ws,
        ValueKind::Choice { options: 3 },
        ParamMapping::Linear,
        None,
        0.9,
    );
    let e = evaluate_control(ws.session().project(), choice_t, bp(0), None).unwrap();
    assert!((e.host_normalized - 1.0).abs() < f64::EPSILON);

    // Non-linear mapping: equal normalized depth produces a
    // position-dependent plain increment, never a fixed plain delta.
    let exp_t = add_target_with(
        &mut ws,
        ValueKind::Continuous,
        ParamMapping::Exponential,
        Some(caps(ParamFlags::AUTOMATABLE, 20.0, 20_000.0)),
        0.5,
    );
    apply(
        &mut ws,
        ProjectOp::AddModulationAssignment {
            source: s,
            target: exp_t,
            depth: 0.2,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    let project = ws.session().project();
    // Macro 0.5 * depth 0.2 = +0.1 normalized at every position; the
    // plain increment must differ with the reference position.
    let e_low = evaluate_control(project, exp_t, bp(0), Some(0.3)).unwrap();
    let e_high = evaluate_control(project, exp_t, bp(0), Some(0.8)).unwrap();
    let plain_delta_low =
        e_low.host_plain - ParamMapping::Exponential.denormalize(e_low.reference, 20.0, 20_000.0);
    let plain_delta_high =
        e_high.host_plain - ParamMapping::Exponential.denormalize(e_high.reference, 20.0, 20_000.0);
    assert!(plain_delta_low.abs() > f64::EPSILON);
    assert!((plain_delta_low - plain_delta_high).abs() > 1.0);
    // Exponential with min <= 0 is rejected at target creation.
    assert!(matches!(
        try_apply(
            &mut ws,
            ProjectOp::AddParameterTarget {
                name: "bad".into(),
                owner: ControlScope::Project,
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Exponential,
                capability: Some(caps(ParamFlags::AUTOMATABLE, -1.0, 1.0)),
                base: 0.5,
            }
        ),
        Err(Error::Validation(_))
    ));
}

// ---------------------------------------------------------------------------
// PC-A-08 time consistency
// ---------------------------------------------------------------------------

#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenario
fn pc_a_08_time_consistency() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    // Ramp 0→1 over beats 0..4; the second point needs a wider
    // half-open range to be legal.
    let mut ramp = region(bp(0), bp(8), vec![pt(bp(0), 0.0), pt(bp(4), 1.0)]);
    ramp.interpolation = Interpolation::Linear;
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: ramp,
            policy: ConflictPolicy::Reject,
        },
    );
    // A beat-stored point at 1/7 stays exactly 1/7 — no tick-grid
    // quantization of the canonical position.
    let odd = frac(1, 7);
    assert_eq!(odd.num(), 1);
    assert_eq!(odd.den(), 7);

    // Evaluations before a tempo edit.
    let before = evaluate_control(ws.session().project(), target, bp(2), None).unwrap();
    // Tempo event added at tick 960 (beat 1) — points keep their beat
    // positions, so beat-space evaluation is identical after the edit.
    apply(
        &mut ws,
        ProjectOp::AddTempoEvent {
            tick: 960,
            bpm_milli: 60_000,
        },
    );
    let after = evaluate_control(ws.session().project(), target, bp(2), None).unwrap();
    assert_eq!(before.automation, after.automation);
    // Tick↔beat: tick 960 at PPQN 960 == beat 1 exactly.
    let tick_beat =
        BeatPoint::from_beat(TickPosition::new(Tick::new(960), Ppqn::new(960).unwrap()).to_beat());
    assert_eq!(tick_beat, bp(1));

    // Offline render == playback on the same grid: stepping the
    // transport and evaluating each position produces the same values
    // as evaluating the same positions directly.
    ws.session_mut().control_mut().transport_play();
    let mut stepped = Vec::new();
    for i in 0..4 {
        let pos = frac(i, 2);
        ws.session_mut()
            .control_with(|c, p| c.transport_advance(p, pos))
            .unwrap();
        stepped.push(
            ws.session()
                .control()
                .evaluate(ws.session().project(), target, pos)
                .unwrap()
                .host_normalized,
        );
    }
    ws.session_mut().control_mut().transport_stop();
    for (i, v) in stepped.iter().enumerate() {
        let direct = evaluate_control(
            ws.session().project(),
            target,
            frac(i64::try_from(i).unwrap(), 2),
            None,
        )
        .unwrap()
        .host_normalized;
        assert_eq!(*v, direct);
    }

    // Loop wrap keeps the evaluation consistent at the loop start.
    ws.session_mut()
        .control_mut()
        .set_loop(Some((bp(0), bp(4))))
        .unwrap();
    let eff = ws
        .session_mut()
        .control_with(|c, p| c.transport_advance(p, bp(6)))
        .unwrap();
    assert!(eff.wrapped);
    assert_eq!(ws.session().control().playback().position, bp(0));

    // stamp→frame: a MONOTONIC-domain anchor on the engine clock; a
    // stamp 1 s after the anchor maps through `map_stamp_to_frame` to
    // 90_000 ticks at COMMON_90K → beat 3/2 (tempo is 60 BPM past
    // tick 960: beat 1 at 0.5 s, beat 2 at 1.5 s).
    let ns = TimeBase::new(1, 1_000_000_000).unwrap();
    let epoch = ws.session().control().playback().epoch;
    ws.session_mut()
        .control_mut()
        .install_clock_anchor(ClockAnchor {
            domain: ClockDomainId::MONOTONIC,
            stamp_ticks: 0,
            stamp_base: ns,
            frame: EngineFrame::new(epoch, 0),
            calibration: TimestampProvenance::Carried,
        });
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0)))
        .unwrap();
    let stamp = InputStamp {
        ticks: 1_000_000_000, // 1 s after the anchor
        base: ns,
        domain: ClockDomainId::MONOTONIC,
        provenance: TimestampProvenance::Carried,
    };
    ws.session_mut()
        .control_with(|c, p| c.stamp_gesture(p, target, &stamp, 0.9))
        .unwrap();
    let e = ws
        .session()
        .control()
        .evaluate(ws.session().project(), target, frac(3, 2))
        .unwrap();
    assert_eq!(e.reference_source, ReferenceSource::Manual);
    assert!((e.reference - 0.9).abs() < f64::EPSILON);
    // A foreign-domain stamp is a defined rejection, never a drop.
    let bad = InputStamp {
        domain: ClockDomainId::UNKNOWN,
        ..stamp
    };
    assert!(
        ws.session_mut()
            .control_with(|c, p| c.stamp_gesture(p, target, &bad, 0.9))
            .is_err()
    );
}
