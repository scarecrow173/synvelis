//! TIM-03 L1 transport application acceptance slice (synvelis#127).
//!
//! Drives the production composition seam with the ruviform#630
//! `test_gain` VST3/CLAP fixtures as typed L1 out-of-process routes.
//! The scoped oracle is host-side transport application: project beats
//! convert once through a non-default tempo map, transport commands stage
//! through the process-boundary session, automation uses the same
//! beat/media instant, the control ledger reconstructs the converted
//! frame, loop-folded spans are observed, and PCM shows the automation
//! gain law. `test_gain` ignores transport, so this is not a plugin-side
//! transport-consumption oracle.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use ruviform_media_core::{
    DeliveryPolicy, InBlockTransportSupport, ScheduleBudget, TransportCommand, TransportSpan,
    TransportStatus,
};
use ruviform_plugin_host::OutOfProcessSpec;
use ruviform_ui::BackendReport;
use serde_json::{Value, json};
use synvelis_app::composition::compose_routing_engine_with_out_of_process_and_boundaries;
use synvelis_app::engine::{
    TransportApplicationCommand, TransportApplicationError, TransportApplicationReport,
    TransportConversionRole,
};
use synvelis_app::{ProductOutOfProcessPlugin, SynvelisApp};
use synvelis_player::routing_control::BoundaryStageError;
use synvelis_player::{BoundaryDeclaration, TimingPrecision};
use synvelis_project::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, ConflictPolicy,
    ControlOwner, Interpolation, LaneId, ParamMapping, ParameterRole, ParameterTargetId,
    ProcessorId, ProjectOp, SourceId, TargetCapability, ValueKind,
};

use common::tim03_l1::*;

const TEMPO_BPM_MILLI: u32 = 125_000;
const LOOP_START_FRAME: i64 = 385;
const LOOP_END_FRAME: i64 = 641;
const SEEK_FRAME: i64 = 513;
const AUTOMATION_FRAME: i64 = 545;

#[derive(Clone, Copy)]
struct AutomationCase {
    normalized: f32,
    expected_gain: f32,
}

// Independent from the host conversion: test_gain's plain range is 0..=2.
const BELOW_UNITY: AutomationCase = AutomationCase {
    normalized: 0.375,
    expected_gain: 0.75,
};
const ABOVE_UNITY: AutomationCase = AutomationCase {
    normalized: 0.625,
    expected_gain: 1.25,
};

fn beat(num: i64, den: u32) -> BeatPoint {
    BeatPoint::new(num, den).unwrap()
}

fn loop_start_beat() -> BeatPoint {
    beat(1, 60)
}

fn loop_end_beat() -> BeatPoint {
    beat(1, 36)
}

fn seek_beat() -> BeatPoint {
    beat(1, 45)
}

fn automation_beat() -> BeatPoint {
    beat(17, 720)
}

fn add_transport_target(
    controller: &mut synvelis_project::ProjectController,
    processor: ProcessorId,
    normalized: f64,
) -> ParameterTargetId {
    apply(
        controller,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Processor(processor),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(TargetCapability {
                param_id: 0,
                flags_bits: ruviform_plugin_core::ParamFlags::AUTOMATABLE.bits(),
                min_value: 0.0,
                max_value: 2.0,
                default_value: 1.0,
            }),
            base: 1.0,
        },
    );
    let target = controller
        .live()
        .control
        .targets
        .values()
        .find(|target| target.owner == ControlOwner::Processor(processor))
        .unwrap()
        .id;
    apply(controller, ProjectOp::AddAutomationLane { target });
    let lane = *controller
        .live()
        .control
        .lanes
        .values()
        .find(|lane| lane.target == target)
        .map(|lane| &lane.id)
        .unwrap();
    apply(
        controller,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: BeatRange::try_new(beat(0, 1), beat(2, 1)).unwrap(),
                interpolation: Interpolation::Linear,
                points: vec![
                    AutomationPoint {
                        pos: beat(0, 1),
                        value: 0.25,
                    },
                    AutomationPoint {
                        pos: automation_beat(),
                        value: normalized,
                    },
                    AutomationPoint {
                        pos: beat(1, 1),
                        value: 0.5,
                    },
                ],
            },
            policy: ConflictPolicy::Replace,
        },
    );
    target
}

fn build_transport_project(
    app: &mut SynvelisApp,
    root: &std::path::Path,
    descriptor: &str,
    normalized: f64,
) -> (ProcessorId, LaneId, ParameterTargetId, SourceId) {
    let controller = app.workspace_mut().session_mut().controller_mut();
    apply(
        controller,
        ProjectOp::AddTempoEvent {
            tick: 1,
            bpm_milli: TEMPO_BPM_MILLI,
        },
    );
    let (_, strip, lane) = add_track_strip(controller);
    let processor = insert_gain_processor(controller, strip, descriptor);
    let target = add_transport_target(controller, processor, normalized);
    let imported = import_constant_stereo(controller, root);
    let source = imported.commit.source_id;
    add_clip(controller, lane, source, 0, FIRST_CLIP_END, 0);
    (processor, lane, target, source)
}

fn conversion_frame(report: &TransportApplicationReport, role: TransportConversionRole) -> i64 {
    report
        .conversions
        .iter()
        .find(|conversion| conversion.role == role)
        .unwrap_or_else(|| panic!("missing {role:?} conversion"))
        .media
        .value()
}

fn command_frames(command: &TransportCommand) -> Value {
    match *command {
        TransportCommand::Play => json!({"kind":"Play"}),
        TransportCommand::Pause => json!({"kind":"Pause"}),
        TransportCommand::Stop => json!({"kind":"Stop"}),
        TransportCommand::Seek(position) => {
            json!({"kind":"Seek","position_frame":position.value()})
        }
        TransportCommand::SetLoop(range) => json!({
            "kind":"SetLoop",
            "start_frame":range.start().value(),
            "end_frame":range.end().value(),
        }),
        TransportCommand::ClearLoop => json!({"kind":"ClearLoop"}),
        _ => json!({"kind":"unknown"}),
    }
}

fn observed_span(
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
) -> TransportSpan {
    engine
        .with_control_dispatch(|dispatch| dispatch.last_observed_span(processor).copied())
        .flatten()
        .expect("observed transport span")
}

fn block_matches_gain(left: &[f32], right: &[f32], gain: f32) -> bool {
    left.iter().zip(right).all(|(left, right)| {
        (left - INPUT * gain).abs() <= TOLERANCE && (right + INPUT * gain).abs() <= TOLERANCE
    })
}

fn pcm_gain_blocks(report: &synvelis_player::driver::PlayReport, gain: f32) -> Vec<usize> {
    report.pcm[0]
        .as_chunks::<BLOCK_FRAMES>()
        .0
        .iter()
        .zip(report.pcm[1].as_chunks::<BLOCK_FRAMES>().0)
        .enumerate()
        .filter_map(|(index, (left, right))| block_matches_gain(left, right, gain).then_some(index))
        .collect()
}

fn class_blocks(classes: &[Value], class: &str) -> Vec<usize> {
    classes
        .iter()
        .filter(|entry| entry["class"] == class)
        .filter_map(|entry| {
            entry["block"]
                .as_u64()
                .and_then(|block| usize::try_from(block).ok())
        })
        .collect()
}

fn file_facts(path: &std::path::Path) -> (u64, String) {
    (
        fs::metadata(path).unwrap().len(),
        ruviform_plugin_dock_test::sha256_hex(path).unwrap(),
    )
}

fn ruviform_revision() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("Cargo.lock");
    let text = fs::read_to_string(&root).expect("workspace Cargo.lock");
    text.lines()
        .find_map(|line| {
            line.strip_prefix("source = \"git+https://github.com/scarecrow173/ruviform.git#")
                .and_then(|rest| rest.strip_suffix('"'))
                .map(str::to_owned)
        })
        .expect("ruviform git revision in Cargo.lock")
}

fn boundary_declaration() -> BoundaryDeclaration {
    BoundaryDeclaration {
        precision: TimingPrecision::FrameAccurateEvents,
        in_block: InBlockTransportSupport::Unknown,
        consent: None,
        budget: ScheduleBudget::new(64, 256, 16, 4096),
    }
}

fn append_play_report(
    acc: &mut synvelis_player::driver::PlayReport,
    report: synvelis_player::driver::PlayReport,
) {
    for (out, input) in acc.pcm.iter_mut().zip(report.pcm) {
        out.extend_from_slice(&input);
    }
    acc.crc = acc.crc.wrapping_mul(31).wrapping_add(report.crc);
    acc.frames += report.frames;
}

fn conversion_json(report: &TransportApplicationReport) -> Vec<Value> {
    report
        .conversions
        .iter()
        .map(|conversion| {
            json!({
                "role": format!("{:?}", conversion.role),
                "beat": format!("{}/{}", conversion.beat.num(), conversion.beat.den()),
                "media_frame": conversion.media.value(),
            })
        })
        .collect()
}

struct TransportRunEvidence {
    first: TransportApplicationReport,
    second: TransportApplicationReport,
    span: TransportSpan,
    cycles: Vec<synvelis_player::DispatchCycle>,
}

struct TransportRunOutcome {
    evidence: TransportRunEvidence,
    first_run: synvelis_player::driver::PlayReport,
    second_run: synvelis_player::driver::PlayReport,
}

struct ControlPcmEvidence {
    call_start: u64,
    frame_offset: u32,
    position: i64,
    transport_anchor: i64,
    expected_physical_frame: i64,
    pcm_classes: Vec<Value>,
    automation_blocks: Vec<usize>,
    unity_blocks: Vec<usize>,
    wrong_quarter_gain_blocks: Vec<usize>,
    normalized_as_plain_blocks: Vec<usize>,
    silence_blocks: Vec<usize>,
    transition_blocks: Vec<usize>,
}

fn apply_initial_transport(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
) -> TransportApplicationReport {
    let first = {
        let project = app.workspace().session().project();
        engine
            .control_apply_transport(
                project,
                processor,
                &[
                    TransportApplicationCommand::SetLoop {
                        start: loop_start_beat(),
                        end: loop_end_beat(),
                    },
                    TransportApplicationCommand::Seek(seek_beat()),
                    TransportApplicationCommand::Play,
                ],
                Some(automation_beat()),
            )
            .expect("initial transport application")
    };
    assert_eq!(first.call_start.frame(), BLOCK_FRAMES as u64 * 2);
    assert_eq!(
        conversion_frame(&first, TransportConversionRole::LoopStart),
        LOOP_START_FRAME
    );
    assert_eq!(
        conversion_frame(&first, TransportConversionRole::LoopEnd),
        LOOP_END_FRAME
    );
    assert_eq!(
        conversion_frame(&first, TransportConversionRole::Seek),
        SEEK_FRAME
    );
    assert_eq!(
        conversion_frame(&first, TransportConversionRole::Automation),
        AUTOMATION_FRAME
    );
    assert_eq!(first.plan.policy, DeliveryPolicy::Exact);
    assert_eq!(first.loop_wraps, Vec::new());
    let automation_tick = first.automation.as_ref().expect("automation tick evidence");
    assert_eq!(automation_tick.evaluated, 1);
    assert_eq!(automation_tick.events, 1);
    assert_eq!(automation_tick.dispatch_errors, 0);
    first
}

fn exercise_transport_application(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
) -> TransportRunOutcome {
    let first = apply_initial_transport(app, engine, processor);
    let first_run = run_paced(engine, 1);
    assert_eq!(first_run.frames, BLOCK_FRAMES as u64);
    let mut cycles = engine.control_drain_observations();

    let second = {
        let project = app.workspace().session().project();
        engine
            .control_apply_transport(
                project,
                processor,
                &[TransportApplicationCommand::Play],
                None,
            )
            .expect("loop-wrap oracle transport application")
    };
    assert_eq!(second.call_start.frame(), 768);
    assert_eq!(
        second
            .loop_wraps
            .iter()
            .map(|point| point.offset)
            .collect::<Vec<_>>(),
        vec![128],
        "playing position 513 inside loop [385,641) wraps 128 frames into window [768,1024)"
    );
    assert_eq!(second.plan.policy, DeliveryPolicy::Exact);
    assert_eq!(second.plan.boundaries, vec![128]);

    let second_run = run_paced(engine, 1);
    assert_eq!(second_run.frames, BLOCK_FRAMES as u64);
    cycles.extend(engine.control_drain_observations());
    let span = observed_span(engine, processor);
    assert_eq!(span.start.frame(), 768);
    assert_eq!(span.ctx.status, TransportStatus::Playing);
    assert_eq!(span.ctx.position_ticks, 513);
    assert_eq!(
        span.ctx.loop_ticks,
        Some((LOOP_START_FRAME, LOOP_END_FRAME))
    );
    assert!(
        span.ctx.occurrence.get() >= 1,
        "folded span must carry loop occurrence evidence: {:?}",
        span.ctx
    );
    assert!(
        span.ctx.transport_generation.get() >= 1,
        "transport generation must advance after staged seek/play application: {:?}",
        span.ctx
    );
    TransportRunOutcome {
        evidence: TransportRunEvidence {
            first,
            second,
            span,
            cycles,
        },
        first_run,
        second_run,
    }
}

fn collect_control_pcm_evidence(
    engine: &synvelis_app::SessionRoutingEngine,
    target: ParameterTargetId,
    run: &mut TransportRunEvidence,
    activation_report: synvelis_player::driver::PlayReport,
    first_run: synvelis_player::driver::PlayReport,
    second_run: synvelis_player::driver::PlayReport,
    case: AutomationCase,
) -> ControlPcmEvidence {
    let late_run = run_paced(engine, 8);
    run.cycles.extend(engine.control_drain_observations());
    let (call_start, frame_offset, position) =
        find_dispatched(&run.cycles, target, f64::from(case.normalized));
    assert_eq!(position, AUTOMATION_FRAME);
    let transport_anchor = i64::try_from(run.first.call_start.frame()).unwrap() - SEEK_FRAME;
    let expected_physical_frame = AUTOMATION_FRAME + transport_anchor;
    assert_eq!(transport_anchor, -1);
    assert_eq!(frame_offset, block_offset(expected_physical_frame));
    assert_eq!(frame_offset, 32);
    assert_eq!(
        call_start + u64::from(frame_offset),
        u64::try_from(expected_physical_frame).unwrap(),
        "tempo-converted media frame does not reconstruct from the control ledger projection"
    );

    let mut pcm_report = activation_report;
    append_play_report(&mut pcm_report, first_run);
    append_play_report(&mut pcm_report, second_run);
    append_play_report(&mut pcm_report, late_run);
    assert_eq!(pcm_report.frames, BLOCK_FRAMES as u64 * 12);
    let mut pcm_classes = Vec::new();
    classify_blocks(0, &pcm_report.pcm[0], &pcm_report.pcm[1], &mut pcm_classes);
    let automation_blocks = pcm_gain_blocks(&pcm_report, case.expected_gain);
    assert!(
        automation_blocks.contains(&4) && automation_blocks.iter().any(|block| *block >= 5),
        "PCM oracle did not observe fresh post-transition automation-gain blocks (gain {}): {pcm_classes:?}",
        case.expected_gain
    );
    let unity_blocks = pcm_gain_blocks(&pcm_report, 1.0);
    let wrong_quarter_gain_blocks = pcm_gain_blocks(&pcm_report, 0.25);
    assert_eq!(
        wrong_quarter_gain_blocks,
        Vec::<usize>::new(),
        "normalized automation value leaked as a plain gain"
    );
    let normalized_as_plain_blocks = pcm_gain_blocks(&pcm_report, case.normalized);
    assert!(
        normalized_as_plain_blocks.is_empty(),
        "normalized automation keyframe leaked as a plain gain: {normalized_as_plain_blocks:?}"
    );
    let automation_class = json!(format!("input*{}", case.expected_gain));
    for &block in &automation_blocks {
        pcm_classes[block]["class"] = automation_class.clone();
    }
    let silence_blocks = class_blocks(&pcm_classes, "silence");
    let transition_blocks = class_blocks(&pcm_classes, "other");
    ControlPcmEvidence {
        call_start,
        frame_offset,
        position,
        transport_anchor,
        expected_physical_frame,
        pcm_classes,
        automation_blocks,
        unity_blocks,
        wrong_quarter_gain_blocks,
        normalized_as_plain_blocks,
        silence_blocks,
        transition_blocks,
    }
}

fn inactive_generation_evidence(
    app: &mut SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
    lane: LaneId,
    source: SourceId,
    generation: u64,
) -> Value {
    let negative = {
        let revision = app.workspace().session().revision();
        app.workspace_mut()
            .session_mut()
            .apply(
                ProjectOp::AddClip {
                    lane_id: lane,
                    source_id: source,
                    placement: media_placement(FIRST_CLIP_END, FIRST_CLIP_END + 256),
                    playback: natural_playback(0, 256),
                },
                revision,
            )
            .unwrap();
        assert_eq!(engine.control_generation(), Some(generation + 1));
        let project = app.workspace().session().project();
        engine
            .control_apply_transport(
                project,
                processor,
                &[TransportApplicationCommand::Play],
                None,
            )
            .expect_err("published but inactive generation must not stage transport")
    };
    match negative {
        TransportApplicationError::Boundary(BoundaryStageError::InactiveGeneration {
            surface,
            active,
        }) => json!({
            "kind": "InactiveGeneration",
            "surface": surface,
            "active": active,
        }),
        other => panic!("unexpected negative evidence: {other}"),
    }
}

fn loop_wraps_json(report: &TransportApplicationReport) -> Vec<Value> {
    report
        .loop_wraps
        .iter()
        .map(|point| json!({"offset":point.offset,"kind":format!("{:?}",point.kind)}))
        .collect()
}

fn transport_reports_json(run: &TransportRunEvidence) -> Value {
    json!({
        "first": {
            "call_start": run.first.call_start.frame(),
            "window_frames": run.first.window_frames,
            "conversions": conversion_json(&run.first),
            "loop_wraps": loop_wraps_json(&run.first),
            "commands": run.first.commands.iter().map(command_frames).collect::<Vec<_>>(),
            "policy": format!("{:?}", run.first.plan.policy),
            "precision": format!("{:?}", run.first.plan.precision),
            "boundaries": run.first.plan.boundaries,
        },
        "second": {
            "call_start": run.second.call_start.frame(),
            "loop_wraps": loop_wraps_json(&run.second),
            "commands": run.second.commands.iter().map(command_frames).collect::<Vec<_>>(),
            "policy": format!("{:?}", run.second.plan.policy),
            "boundaries": run.second.plan.boundaries,
        },
    })
}

fn control_oracle_json(
    run: &TransportRunEvidence,
    control: &ControlPcmEvidence,
    case: AutomationCase,
) -> Value {
    let automation_tick = run
        .first
        .automation
        .as_ref()
        .expect("automation tick evidence");
    json!({
        "automation_value": case.normalized,
        "position_frame": control.position,
        "transport_anchor": control.transport_anchor,
        "expected_physical_frame": control.expected_physical_frame,
        "call_start": control.call_start,
        "frame_offset": control.frame_offset,
        "stage": "Dispatched",
        "tick": {
            "evaluated": automation_tick.evaluated,
            "events": automation_tick.events,
            "dispatch_errors": automation_tick.dispatch_errors,
            "gated": automation_tick.gated,
            "stale": automation_tick.stale,
        },
    })
}

fn span_oracle_json(span: &TransportSpan) -> Value {
    json!({
        "start_frame": span.start.frame(),
        "status": format!("{:?}", span.ctx.status),
        "position_ticks": span.ctx.position_ticks,
        "loop_ticks": span.ctx.loop_ticks,
        "loop_occurrence": span.ctx.occurrence.get(),
        "transport_generation": span.ctx.transport_generation.get(),
        "plan_generation": span.ctx.plan_generation.get(),
        "folded_position_expected": 513,
    })
}

fn pcm_oracle_json(control: &ControlPcmEvidence, case: AutomationCase) -> Value {
    json!({
        "input": {"left": INPUT, "right": -INPUT},
        "automation_normalized": case.normalized,
        "automation_plain_gain": case.expected_gain,
        "expected_pcm": {"left": INPUT * case.expected_gain, "right": -INPUT * case.expected_gain},
        "automation_gain_blocks": control.automation_blocks.clone(),
        "matched_blocks": control.automation_blocks.clone(),
        "silence_blocks": control.silence_blocks.clone(),
        "unity_blocks": control.unity_blocks.clone(),
        "transition_or_mixed_blocks": control.transition_blocks.clone(),
        "substituted_pass_through_distinction": "not separately observable through the production PCM surface",
        "block_classes": control.pcm_classes.clone(),
        "wrong_quarter_gain_blocks": control.wrong_quarter_gain_blocks.clone(),
        "normalized_as_plain_blocks": control.normalized_as_plain_blocks.clone(),
        "lookahead_blocks": 2,
        "freshness_rule": "only full stereo blocks matching input times the expected plain gain count; silence, lookahead, unity, and transition blocks do not count",
    })
}

struct ArtifactEvidence<'a> {
    runner_env: &'a str,
    runner_path: &'a Path,
    staged_fixture: &'a Path,
    runner_size: u64,
    runner_sha256: &'a str,
    staged_size: u64,
    staged_sha256: &'a str,
}

fn print_transport_application_evidence(
    probe: &FixtureProbe,
    artifacts: &ArtifactEvidence<'_>,
    generation: u64,
    run: &TransportRunEvidence,
    control: &ControlPcmEvidence,
    negative_json: &Value,
    case: AutomationCase,
) {
    println!(
        "tim03-l1-transport-application {}",
        json!({
            "schema": "tim03-l1-transport-application/v1",
            "format": probe.format,
            "production_wiring": {
                "seam": "compose_routing_engine_with_out_of_process_and_boundaries",
                "route": "L1 OutOfProcessSpec",
                "activation": "Active",
                "descriptor": probe.gain.id,
                "descriptor_count": probe.descriptors.len(),
                "generation": generation,
            },
            "artifacts": {
                "fixture": {"path": probe.path, "sha256": probe.sha256},
                "staged_fixture": {"path": artifacts.staged_fixture, "size": artifacts.staged_size, "sha256": artifacts.staged_sha256},
                "runner": {"path": artifacts.runner_path, "size": artifacts.runner_size, "sha256": artifacts.runner_sha256, "resolution_env": artifacts.runner_env},
                "ruviform_resolved_revision": ruviform_revision(),
            },
            "tempo_map": {
                "initial_bpm_milli": 120_000,
                "added_bpm_milli": TEMPO_BPM_MILLI,
                "added_tick": 1,
                "revision": run.first.tempo_revision,
                "frame_base": "1/48000",
            },
            "transport_application": transport_reports_json(run),
            "control_oracle": control_oracle_json(run, control, case),
            "span_oracle": span_oracle_json(&run.span),
            "pcm_oracle": pcm_oracle_json(control, case),
            "negative_evidence": negative_json,
            "not_run": [
                "test_instrument plugin-side musical transport oracle (test_gain ignores TransportInfo)",
                "dynamic tempo edit matrix, full frame/boundary matrix, fault/state isolation",
                "GUI/Native Dock/EXT-L3, provisioned endpoints, macOS/Linux runtime"
            ],
            "commands": [
                "cargo build --locked -p synvelis-plugin-test-l1-runner",
                "cargo test --locked -p synvelis-app --test tim03_l1_transport_application -- --ignored --test-threads=1 --nocapture"
            ]
        })
    );
}

fn exercise_format(
    probe: &FixtureProbe,
    runner_env: &str,
    runner_stem: &str,
    case: AutomationCase,
) {
    assert!(probe.path.is_file());
    let runner = runner_program(runner_env, runner_stem);
    let runner_path = PathBuf::from(&runner);
    let staged_fixture = stage_fixture_sibling(probe, &runner_path);
    let (runner_size, runner_sha256) = file_facts(&runner_path);
    let (staged_size, staged_sha256) = file_facts(&staged_fixture);
    assert_eq!(staged_sha256, probe.sha256);

    let root = scratch(probe.format);
    fs::create_dir_all(root.join("media")).unwrap();
    let mut app = SynvelisApp::new(BackendReport::available(
        "gpui",
        "tim03-l1-transport-application".to_owned(),
    ));
    let (processor, lane, target, source) =
        build_transport_project(&mut app, &root, probe.gain.id, f64::from(case.normalized));
    let engine = compose_routing_engine_with_out_of_process_and_boundaries(
        &mut app,
        session_config(root.join("media")),
        Vec::new(),
        vec![ProductOutOfProcessPlugin {
            spec: OutOfProcessSpec::new(probe.gain, runner.clone()),
            params: vec![gain_param()],
        }],
        vec![(processor, boundary_declaration())],
    )
    .expect("production L1 route registration");
    let revision = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "tim03-l1-transport-application".into(),
            },
            revision,
        )
        .unwrap();

    let (generation, activation_report) =
        activate_generation(&engine, "initial generation did not activate");
    assert_eq!(engine.control_generation(), Some(generation));
    assert_eq!(engine.control_frame_base(), Some(frame_base()));
    let TransportRunOutcome {
        mut evidence,
        first_run,
        second_run,
    } = exercise_transport_application(&app, &engine, processor);
    let control = collect_control_pcm_evidence(
        &engine,
        target,
        &mut evidence,
        activation_report,
        first_run,
        second_run,
        case,
    );
    let negative_json =
        inactive_generation_evidence(&mut app, &engine, processor, lane, source, generation);
    let artifacts = ArtifactEvidence {
        runner_env,
        runner_path: &runner_path,
        staged_fixture: &staged_fixture,
        runner_size,
        runner_sha256: &runner_sha256,
        staged_size,
        staged_sha256: &staged_sha256,
    };
    print_transport_application_evidence(
        probe,
        &artifacts,
        generation,
        &evidence,
        &control,
        &negative_json,
        case,
    );
}

#[test]
#[ignore = "artifact-dependent L1 lane: requires prebuilt test-only runner (run `mise run test-tim03-transport`)"]
fn tim03_l1_vst3_transport_application() {
    let (probe, _library) = load_vst3();
    exercise_format(&probe, VST3_RUNNER_ENV, VST3_RUNNER_STEM, BELOW_UNITY);
}

#[test]
#[ignore = "artifact-dependent L1 lane: requires prebuilt test-only runner (run `mise run test-tim03-transport`)"]
fn tim03_l1_clap_transport_application() {
    let (probe, _library) = load_clap();
    exercise_format(&probe, CLAP_RUNNER_ENV, CLAP_RUNNER_STEM, BELOW_UNITY);
}

#[test]
#[ignore = "artifact-dependent L1 lane: requires prebuilt test-only runner (run `mise run test-tim03-transport`)"]
fn tim03_l1_vst3_transport_application_above_unity() {
    let (probe, _library) = load_vst3();
    exercise_format(&probe, VST3_RUNNER_ENV, VST3_RUNNER_STEM, ABOVE_UNITY);
}

#[test]
#[ignore = "artifact-dependent L1 lane: requires prebuilt test-only runner (run `mise run test-tim03-transport`)"]
fn tim03_l1_clap_transport_application_above_unity() {
    let (probe, _library) = load_clap();
    exercise_format(&probe, CLAP_RUNNER_ENV, CLAP_RUNNER_STEM, ABOVE_UNITY);
}
