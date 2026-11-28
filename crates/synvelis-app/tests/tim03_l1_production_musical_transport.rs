//! TIM-03 production real-ABI musical transport oracle.
//!
//! Unlike the direct endpoint oracle, this test drives the production
//! composition seam and never supplies `TransportInfo` itself. The only
//! musical input is the project-owned tempo map/PPQN carried by production
//! `TransportPlan`s.

mod common;

use std::env;
use std::path::PathBuf;

use common::tim03_l1::*;
use ruviform_media_core::{
    AppliedReceipt, ApplyStatus, InBlockTransportSupport, ScheduleBudget, TransportSpan,
    TransportStatus,
};
use ruviform_plugin_core::EventKind;
use ruviform_plugin_host::OutOfProcessSpec;
use ruviform_ui::BackendReport;
use serde_json::{Value, json};
use synvelis_app::composition::compose_routing_engine_with_out_of_process_and_boundaries;
use synvelis_app::engine::{TransportApplicationCommand, TransportApplicationReport};
use synvelis_app::{ProductOutOfProcessPlugin, SessionRoutingEngine, SynvelisApp};
use synvelis_player::{BoundaryDeclaration, TimingPrecision, compile_project_musical_transport};
use synvelis_project::tempo::TempoEventDto;
use synvelis_project::{BeatPoint, PpqnConversionPolicy, ProcessorId, Project, ProjectOp};

const PROJECT_PPQN: u32 = 480;
const ADAPTER_PPQN: u32 = 960;
const TEMPO_BPM_MILLI: u32 = 93_750;
const CLICK: f32 = 0.5;
const LOOKAHEAD_BLOCKS: usize = 2;
const RUN_BLOCKS: usize = 5;

fn beat_ticks(ticks: i64) -> BeatPoint {
    BeatPoint::new(ticks, PROJECT_PPQN).unwrap()
}

fn boundary_declaration() -> BoundaryDeclaration {
    BoundaryDeclaration {
        precision: TimingPrecision::FrameAccurateEvents,
        in_block: InBlockTransportSupport::Unknown,
        consent: None,
        budget: ScheduleBudget::new(64, 256, 16, 4096),
    }
}

fn build_project(app: &mut SynvelisApp, root: &std::path::Path, descriptor: &str) -> ProcessorId {
    let controller = app.workspace_mut().session_mut().controller_mut();
    apply(
        controller,
        ProjectOp::SetProjectPpqn {
            old_ppqn: 960,
            new_ppqn: PROJECT_PPQN,
            policy: PpqnConversionPolicy::Exact,
        },
    );
    apply(
        controller,
        ProjectOp::AddTempoEvent {
            tick: 1,
            bpm_milli: TEMPO_BPM_MILLI,
        },
    );
    let (_, strip, lane) = add_track_strip(controller);
    let processor = insert_processor(controller, strip, TEST_INSTRUMENT, descriptor);
    let imported = import_constant_stereo(controller, root);
    add_clip(
        controller,
        lane,
        imported.commit.source_id,
        0,
        FIRST_CLIP_END,
        0,
    );
    processor
}

fn apply_transport(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
    commands: &[TransportApplicationCommand],
) -> synvelis_app::engine::TransportApplicationReport {
    engine
        .control_apply_transport(
            app.workspace().session().project(),
            processor,
            commands,
            None,
        )
        .expect("production transport application")
}

fn click_positions(report: &synvelis_player::driver::PlayReport) -> Vec<usize> {
    let frames = usize::try_from(report.frames).unwrap();
    assert_eq!(report.pcm.len(), 2);
    assert!(report.pcm.iter().all(|channel| channel.len() == frames));
    let mut positions = Vec::new();
    for (index, (left, right)) in report.pcm[0].iter().zip(&report.pcm[1]).enumerate() {
        assert_eq!(
            left.to_bits(),
            right.to_bits(),
            "stereo output diverged at frame {index}"
        );
        if left.to_bits() == CLICK.to_bits() {
            positions.push(index);
        } else {
            assert_eq!(*left, 0.0, "non-click frame {index} must be silent");
        }
    }
    positions
}

fn silent_after_lookahead(report: &synvelis_player::driver::PlayReport) -> bool {
    report.pcm[0]
        .iter()
        .skip(LOOKAHEAD_BLOCKS * BLOCK_FRAMES)
        .all(|sample| *sample == 0.0)
}

fn play_seeked_click(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
    quarter: i64,
    delta_ticks: i64,
) -> (Vec<usize>, synvelis_app::engine::TransportApplicationReport) {
    let position_ticks = quarter * i64::from(PROJECT_PPQN) - delta_ticks;
    let report = apply_transport(
        app,
        engine,
        processor,
        &[
            TransportApplicationCommand::Seek(beat_ticks(position_ticks)),
            TransportApplicationCommand::Play,
        ],
    );
    let mut pcm = vec![Vec::new(), Vec::new()];
    let mut crc = 0_u64;
    for _ in 0..RUN_BLOCKS {
        let one = run_paced(engine, 1);
        for (out, input) in pcm.iter_mut().zip(one.pcm) {
            out.extend_from_slice(&input);
        }
        crc = crc.wrapping_mul(31).wrapping_add(one.crc);
    }
    let run = synvelis_player::driver::PlayReport {
        pcm,
        crc,
        frames: (RUN_BLOCKS * BLOCK_FRAMES) as u64,
    };
    (click_positions(&run), report)
}

fn flush_negative(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
    command: TransportApplicationCommand,
) -> bool {
    run_paced(engine, LOOKAHEAD_BLOCKS);
    apply_transport(app, engine, processor, &[command]);
    silent_after_lookahead(&run_paced(engine, RUN_BLOCKS))
}

fn play_loop_clicks(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
    quarter: i64,
) -> Vec<usize> {
    let ppqn = i64::from(PROJECT_PPQN);
    let start = beat_ticks(quarter * ppqn - 8);
    let end = beat_ticks(quarter * ppqn + 4);
    apply_transport(
        app,
        engine,
        processor,
        &[
            TransportApplicationCommand::SetLoop { start, end },
            TransportApplicationCommand::Seek(start),
            TransportApplicationCommand::Play,
        ],
    );
    let run = run_paced(engine, 10);
    click_positions(&run)
}

fn independent_click_frame(delta_ticks: i64, bpm_milli: u32) -> usize {
    // The adapter reprojects host PPQN 480 to fixture-visible PPQN 960
    // while preserving the same musical instant. Compute the frame delta
    // in that projected tick domain, then add the two-block L1 lookahead
    // excluded by the freshness rule.
    let projected_delta_ticks = delta_ticks * 2;
    let numerator = projected_delta_ticks * 48_000 * 60_000;
    let denominator = i64::from(bpm_milli) * i64::from(ADAPTER_PPQN);
    let frame = (numerator + denominator - 1) / denominator
        + i64::try_from(LOOKAHEAD_BLOCKS * BLOCK_FRAMES).unwrap();
    usize::try_from(frame).unwrap()
}

fn click_law_json(delta_ticks: i64, bpm_milli: u32, expected: usize) -> Value {
    let projected_delta_ticks = delta_ticks * 2;
    let boundary = projected_delta_ticks;
    json!({
        "law": "ceil((boundary_tick - position_ticks) * sample_rate * 60000 / (bpm_milli * adapter_ppqn)) + lookahead_frames",
        "host_project_ppqn": PROJECT_PPQN,
        "adapter_projection_ppqn": ADAPTER_PPQN,
        "host_delta_ticks": delta_ticks,
        "projected_position_ticks_mod_boundary": -projected_delta_ticks,
        "boundary_tick": boundary,
        "projected_delta_ticks": projected_delta_ticks,
        "numerator": projected_delta_ticks * 48_000 * 60_000,
        "denominator": bpm_milli * ADAPTER_PPQN,
        "bpm_milli": bpm_milli,
        "lookahead_frames": LOOKAHEAD_BLOCKS * BLOCK_FRAMES,
        "expected_click_frame": expected,
        "fixed_960_fallback": false,
        "projection_note": "960 is the VST3/CLAP adapter projection observed by the fixture; the host plan carries project PPQN 480",
    })
}

fn measure_positive(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
    case: &str,
    quarter: i64,
    delta_ticks: i64,
    bpm_milli: u32,
) -> Value {
    let expected = independent_click_frame(delta_ticks, bpm_milli);
    let (observed, transport_report) =
        play_seeked_click(app, engine, processor, quarter, delta_ticks);
    assert_eq!(
        observed,
        vec![expected],
        "{case}: expected one fresh click at {expected}, got {observed:?}"
    );
    json!({
        "case": case,
        "position_jump": true,
        "host_position_ticks": quarter * i64::from(PROJECT_PPQN) - delta_ticks,
        "independent_calculation": click_law_json(delta_ticks, bpm_milli, expected),
        "observed_click_frames": observed,
        "transport_call_start": transport_report.call_start.frame(),
        "amplitude": CLICK,
        "stereo_identical": true,
        "non_click_frames_silent": true,
        "verdict": "pass",
    })
}

fn control_plan_evidence(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
) -> Value {
    let project = app.workspace().session().project();
    let source = compile_project_musical_transport(project).expect("valid test project source");
    let surface = engine.control_report().expect("control surface evidence");
    let facts = engine
        .with_control_dispatch(|dispatch| {
            let current = dispatch.last_plan_musical_is_current(processor);
            dispatch.last_plan(processor).map(|plan| {
                (
                    plan.tempo_revision.get(),
                    plan.musical_ppqn
                        .map(ruviform_media_core::Ppqn::ticks_per_quarter),
                    plan.events.len(),
                    plan.events
                        .iter()
                        .filter(|event| matches!(event.payload, EventKind::Midi(_)))
                        .count(),
                    plan.events
                        .iter()
                        .filter(|event| matches!(event.payload, EventKind::ParamValue(_)))
                        .count(),
                    plan.events
                        .iter()
                        .filter(|event| event.transport_generation.is_some())
                        .count(),
                    current,
                )
            })
        })
        .flatten()
        .expect("instrument control plan evidence");
    let (
        plan_tempo_revision,
        plan_ppqn,
        event_count,
        midi_events,
        param_events,
        generation_bound_events,
        current,
    ) = facts;
    assert_eq!(plan_tempo_revision, project.tempo_map.revision);
    assert_eq!(plan_ppqn, Some(source.ppqn().ticks_per_quarter()));
    assert_eq!(current, Some(true), "stale live plan treated as current");
    assert!(surface.musical_refresh_errors.is_empty());
    json!({
        "surface": format!("{:?}", surface.musical_transport),
        "refresh_errors": format!("{:?}", surface.musical_refresh_errors),
        "project_tempo_revision": project.tempo_map.revision,
        "plan_tempo_revision": plan_tempo_revision,
        "project_ppqn": PROJECT_PPQN,
        "plan_ppqn": plan_ppqn,
        "plan_carries_current_project_source": current,
        "event_counts": {
            "total": event_count,
            "staged_midi": midi_events,
            "param_events": param_events,
            "transport_generation_bound": generation_bound_events,
        },
    })
}

fn apply_tempo_edit(
    app: &mut SynvelisApp,
    tick: i64,
    bpm_milli: u32,
) -> synvelis_project::ProjectRevision {
    let revision = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(ProjectOp::AddTempoEvent { tick, bpm_milli }, revision)
        .expect("committed tempo edit")
}

fn dynamic_edit_case(
    app: &mut SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
    case: &str,
    edit_tick: i64,
    edit_bpm: u32,
    measure_quarter: i64,
) -> Value {
    let before_revision = app.workspace().session().project().tempo_map.revision;
    let _ = engine.control_drain_observations();
    let resulting_revision = apply_tempo_edit(app, edit_tick, edit_bpm);
    let project = app.workspace().session().project();
    assert_eq!(project.tempo_map.revision, before_revision + 1);
    let control_evidence = control_plan_evidence(app, engine, processor);
    // Separate the edit/install window from the measured transport
    // window: the refresh must already be RT-applied before Seek+Play,
    // and the two L1 lookahead blocks cannot contaminate the oracle.
    run_paced(engine, LOOKAHEAD_BLOCKS);
    let rt_install_receipts: Vec<Value> =
        observed_receipts(engine).iter().map(receipt_json).collect();
    let positive = measure_positive(app, engine, processor, case, measure_quarter, 2, edit_bpm);
    json!({
        "edit": {
            "op": "ProjectOp::AddTempoEvent",
            "tick": edit_tick,
            "bpm_milli": edit_bpm,
            "resulting_project_revision": resulting_revision.0,
            "resulting_tempo_revision": project.tempo_map.revision,
            "ppqn_unchanged": project.ppqn.0 == PROJECT_PPQN,
        },
        "control_evidence_before_measured_transport": control_evidence,
        "rt_install_receipts_before_measured_transport": rt_install_receipts,
        "positive": positive,
    })
}

struct MusicalRun {
    generation: u64,
    tempo_revision: u64,
    musical_surface: String,
    musical_refresh_errors: String,
    positives: Vec<Value>,
    stopped_pass: bool,
    paused_pass: bool,
    loop_observed: Vec<usize>,
    dynamic_before_measured: Value,
    dynamic_while_playing: Value,
    final_tempo_revision: u64,
}

fn positive_cases(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
) -> Vec<Value> {
    let mut positives = Vec::new();
    for (index, delta_ticks) in [3_i64, 2, 1].into_iter().enumerate() {
        let quarter = 8 + i64::try_from(index).unwrap() * 2;
        let expected = independent_click_frame(delta_ticks, TEMPO_BPM_MILLI);
        let (observed, transport_report) =
            play_seeked_click(app, engine, processor, quarter, delta_ticks);
        assert_eq!(
            observed,
            vec![expected],
            "delta {delta_ticks}: expected one fresh click, got {observed:?}"
        );
        positives.push(json!({
            "case": format!("seek_jump_delta_{delta_ticks}_ticks"),
            "position_jump": true,
            "host_position_ticks": quarter * i64::from(PROJECT_PPQN) - delta_ticks,
            "independent_calculation": click_law_json(delta_ticks, TEMPO_BPM_MILLI, expected),
            "observed_click_frames": observed,
            "transport_call_start": transport_report.call_start.frame(),
            "amplitude": CLICK,
            "stereo_identical": true,
            "non_click_frames_silent": true,
            "verdict": "pass",
        }));
    }
    positives
}

fn execute_musical_run(
    app: &mut SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
) -> MusicalRun {
    let (generation, activation) = activate_generation(engine, "instrument generation activation");
    assert!(activation.pcm.iter().flatten().all(|sample| *sample == 0.0));
    // Activation starts transport at a quarter boundary. Flush the L1
    // lookahead pipeline so that initial click cannot contaminate the
    // fresh positive windows below; those initial blocks are not counted.
    run_paced(engine, 4);

    let surface = engine.control_report().expect("control surface evidence");
    let musical_surface = format!("{:?}", surface.musical_transport);
    assert!(
        musical_surface.contains("Available") && musical_surface.contains("ppqn: 480"),
        "production surface did not compile project musical transport: {musical_surface}"
    );
    let musical_refresh_errors = format!("{:?}", surface.musical_refresh_errors);
    let tempo_revision = app.workspace().session().project().tempo_map.revision;
    let positives = positive_cases(app, engine, processor);

    let stopped_pass = flush_negative(app, engine, processor, TransportApplicationCommand::Stop);
    assert!(
        stopped_pass,
        "stopped negative emitted non-silent fresh output"
    );
    let paused_pass = flush_negative(app, engine, processor, TransportApplicationCommand::Pause);
    assert!(
        paused_pass,
        "paused negative emitted non-silent fresh output"
    );

    run_paced(engine, LOOKAHEAD_BLOCKS);
    let loop_observed = play_loop_clicks(app, engine, processor, 16);
    assert_eq!(
        loop_observed,
        vec![1024_usize, 1792],
        "loop-derived fresh click positions differ"
    );

    // Re-anchor the transport cursor before the first dynamic edit so the
    // edit tick is unambiguously in its future. Stop and flush the L1
    // lookahead: the post-edit measured seek/play below is a fresh window,
    // not residual output from the baseline loop measurement.
    apply_transport(
        app,
        engine,
        processor,
        &[
            TransportApplicationCommand::ClearLoop,
            TransportApplicationCommand::Seek(beat_ticks(8 * i64::from(PROJECT_PPQN) - 3)),
            TransportApplicationCommand::Stop,
        ],
    );
    run_paced(engine, LOOKAHEAD_BLOCKS + 1);
    let dynamic_before_measured = dynamic_edit_case(
        app,
        engine,
        processor,
        "post_edit_before_measured_transport_quarter16_delta2",
        4_800,
        75_000,
        16,
    );

    // The first post-edit measurement leaves transport playing. Commit the
    // second tempo edit in that state; production revision reconciliation
    // must refresh the live plan before the next measured seek/play.
    let dynamic_while_playing = dynamic_edit_case(
        app,
        engine,
        processor,
        "post_edit_while_playing_quarter24_delta2",
        9_600,
        100_000,
        24,
    );
    let final_tempo_revision = app.workspace().session().project().tempo_map.revision;
    MusicalRun {
        generation,
        tempo_revision,
        musical_surface,
        musical_refresh_errors,
        positives,
        stopped_pass,
        paused_pass,
        loop_observed,
        dynamic_before_measured,
        dynamic_while_playing,
        final_tempo_revision,
    }
}

struct InstrumentRun {
    app: SynvelisApp,
    engine: SessionRoutingEngine,
    processor: ProcessorId,
    provenance: Value,
}

fn instrument_run(
    probe: &FixtureProbe,
    runner_env: &str,
    runner_stem: &str,
    case: &str,
) -> InstrumentRun {
    let instrument = descriptor_named(probe, TEST_INSTRUMENT);
    let runner = runner_program(runner_env, runner_stem);
    let runner_path = PathBuf::from(&runner);
    let staged_fixture = stage_fixture_sibling(probe, &runner_path);
    let (runner_size, runner_sha256) = file_facts(&runner_path);
    let (staged_size, staged_sha256) = file_facts(&staged_fixture);
    assert_eq!(staged_sha256, probe.sha256);
    assert_eq!(staged_size, probe.path.metadata().unwrap().len());
    let root = scratch(&format!("{}-{case}", probe.format));
    std::fs::create_dir_all(root.join("media")).unwrap();
    let mut app = SynvelisApp::new(BackendReport::available(
        "gpui",
        "tim03-l1-production-musical-transport".to_owned(),
    ));
    let processor = build_project(&mut app, &root, instrument.id);
    let engine = compose_routing_engine_with_out_of_process_and_boundaries(
        &mut app,
        session_config(root.join("media")),
        Vec::new(),
        vec![ProductOutOfProcessPlugin {
            spec: OutOfProcessSpec::new(instrument, runner),
            params: Vec::new(),
        }],
        vec![(processor, boundary_declaration())],
    )
    .expect("production instrument route registration");
    let revision = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "tim03-l1-production-musical-transport".into(),
            },
            revision,
        )
        .unwrap();
    InstrumentRun {
        app,
        engine,
        processor,
        provenance: json!({
            "artifacts": {
                "fixture": {"path": probe.path, "size": staged_size, "sha256": probe.sha256},
                "staged_fixture": {"path": staged_fixture, "size": staged_size, "sha256": staged_sha256},
                "runner": {"path": runner_path, "size": runner_size, "sha256": runner_sha256, "resolution_env": runner_env},
                "ruviform_resolved_revision": ruviform_revision(),
            },
            "descriptor": {"id": instrument.id, "name": instrument.name, "vendor": instrument.vendor, "version": instrument.version},
        }),
    }
}

fn exercise_format(probe: &FixtureProbe, runner_env: &str, runner_stem: &str) {
    let InstrumentRun {
        mut app,
        engine,
        processor,
        provenance,
    } = instrument_run(probe, runner_env, runner_stem, "production-musical");
    let run = execute_musical_run(&mut app, &engine, processor);

    println!(
        "tim03-l1-production-musical-transport {}",
        json!({
            "schema": "tim03-l1-production-musical-transport/v1",
            "format": probe.format,
            "test": format!("tim03_l1_production_musical_transport::{}_production_musical_transport", probe.format),
            "environment": {"os": env::consts::OS, "arch": env::consts::ARCH},
            "artifacts": provenance["artifacts"],
            "descriptor": provenance["descriptor"],
            "production_wiring": {
                "seam": "compose_routing_engine_with_out_of_process_and_boundaries",
                "generation": run.generation,
                "direct_endpoint_transport_info_supplied": false,
                "boundary_declaration": "FrameAccurateEvents/InBlockTransportSupport::Unknown",
            },
            "project": {
                "ppqn": PROJECT_PPQN,
                "adapter_projection_ppqn": ADAPTER_PPQN,
                "initial_tempo_revision": run.tempo_revision,
                "final_tempo_revision": run.final_tempo_revision,
                "tempo_events": [
                    {"tick": 0, "bpm_milli": 120_000},
                    {"tick": 1, "bpm_milli": TEMPO_BPM_MILLI},
                    {"tick": 4_800, "bpm_milli": 75_000, "edit_case": "before_measured_transport"},
                    {"tick": 9_600, "bpm_milli": 100_000, "edit_case": "while_playing"},
                ],
                "musical_transport_surface": run.musical_surface,
                "musical_refresh_errors": run.musical_refresh_errors,
            },
            "freshness": {
                "lookahead_blocks": LOOKAHEAD_BLOCKS,
                "rule": "only the third block after each exact seek/play is positive fresh evidence; earlier pipeline blocks may belong to the prior source window and are not counted",
                "frames_per_block": BLOCK_FRAMES,
            },
            "positive_cases": run.positives,
            "dynamic_cases": {
                "valid_tempo_edit_before_measured_transport": run.dynamic_before_measured,
                "valid_tempo_edit_while_playing": run.dynamic_while_playing,
            },
            "negative_controls": {
                "stopped": if run.stopped_pass { "pass" } else { "fail" },
                "paused": if run.paused_pass { "pass" } else { "fail" },
                "missing_or_unknown": "covered at player plan level in tim03_musical_transport_plan; production compile failure leaves both musical halves absent",
            },
            "loop_evidence": {
                "status": "pass",
                "range_host_ticks": {"start": 16 * 480 - 8, "end": 16 * 480 + 4},
                "derivation": "12 host ticks = 768 frames = 3 blocks; the quarter boundary is block 2 frame 0 of each iteration and appears after the two-block L1 lookahead",
                "expected_click_frames": [1024, 1792],
                "observed_click_frames": run.loop_observed,
                "full_dynamic_loop_matrix": "NotRun/follow-up",
            },
            "not_run": [
                "full dynamic tempo edit matrix beyond the two representative cases, and full frame/boundary matrix",
                "fault/state/crash isolation, GUI/EXT-L3, macOS/Linux runtime, 3OS CI, AudioLens cross-consumer",
            ],
        })
    );
}

#[test]
#[ignore = "artifact-dependent L1 lane: requires prebuilt test-only runner (run `mise run test-tim03-production-transport`)"]
fn vst3_production_musical_transport() {
    let (probe, _library) = load_vst3();
    exercise_format(&probe, VST3_RUNNER_ENV, VST3_RUNNER_STEM);
}

#[test]
#[ignore = "artifact-dependent L1 lane: requires prebuilt test-only runner (run `mise run test-tim03-production-transport`)"]
fn clap_production_musical_transport() {
    let (probe, _library) = load_clap();
    exercise_format(&probe, CLAP_RUNNER_ENV, CLAP_RUNNER_STEM);
}

const HISTORY_EDIT_TICK: i64 = 9_600;
const HISTORY_ANCHOR_TICK: i64 = 4_820;
const HISTORY_QUARTER: i64 = 24;
const HISTORY_DELTA_TICKS: i64 = 2;
const HISTORY_A: u32 = 75_000;
const HISTORY_B: u32 = 110_000;

#[derive(Clone, Copy, Debug)]
enum HistoryStep {
    Add,
    UndoBeforeMeasurement,
    RedoPlaying,
    UndoPlaying,
    DivergentAddPlaying,
}

impl HistoryStep {
    fn operation(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::UndoBeforeMeasurement | Self::UndoPlaying => "undo",
            Self::RedoPlaying => "redo",
            Self::DivergentAddPlaying => "divergent-add",
        }
    }

    fn playing(self) -> bool {
        matches!(
            self,
            Self::RedoPlaying | Self::UndoPlaying | Self::DivergentAddPlaying
        )
    }

    fn measured(self) -> bool {
        matches!(
            self,
            Self::UndoBeforeMeasurement | Self::RedoPlaying | Self::DivergentAddPlaying
        )
    }

    fn bpm(self) -> u32 {
        match self {
            Self::Add | Self::RedoPlaying => HISTORY_A,
            Self::UndoBeforeMeasurement | Self::UndoPlaying => TEMPO_BPM_MILLI,
            Self::DivergentAddPlaying => HISTORY_B,
        }
    }

    fn apply(self, app: &mut SynvelisApp) {
        match self {
            Self::Add | Self::DivergentAddPlaying => {
                apply_tempo_edit(app, HISTORY_EDIT_TICK, self.bpm());
            }
            Self::UndoBeforeMeasurement | Self::UndoPlaying => {
                assert!(app.workspace_mut().session_mut().undo().unwrap());
            }
            Self::RedoPlaying => assert!(app.workspace_mut().session_mut().redo().unwrap()),
        }
    }
}

fn observed_transport(run: &InstrumentRun) -> TransportSpan {
    run.engine
        .with_control_dispatch(|dispatch| dispatch.last_observed_span(run.processor).copied())
        .flatten()
        .expect("observed node transport span")
}

fn history_position(span: TransportSpan, project: &Project, playing: bool) -> Value {
    assert_eq!(
        span.ctx.status,
        if playing {
            TransportStatus::Playing
        } else {
            TransportStatus::Paused
        }
    );
    assert_eq!(span.ctx.loop_range(), None);
    let end_position = span.ctx.position_at_offset(span.frames());
    let map = project.tempo_map.compile().unwrap();
    let affected_position = map
        .beat_to_media(beat_ticks(HISTORY_EDIT_TICK).to_beat(), end_position.base())
        .unwrap();
    assert!(
        end_position.value() < affected_position.value(),
        "retroactive edit at {end_position:?}, affected position {affected_position:?}"
    );
    let beat = map.media_to_beat(end_position).unwrap();
    json!({
        "classification": if playing { "while_playing" } else { "before_measured_transport" },
        "status": format!("{:?}", span.ctx.status),
        "host_position_beats": [beat.numerator(), i64::from(beat.denominator())],
        "media_frame_at_span_end": end_position.value(),
        "node_frame_at_span_end": span.end.frame(),
        "affected_edit_tick": HISTORY_EDIT_TICK,
        "affected_edit_media_frame": affected_position.value(),
        "transport_generation": span.ctx.transport_generation.get(),
        "retroactive": false,
    })
}

fn observed_receipts(engine: &SessionRoutingEngine) -> Vec<AppliedReceipt> {
    engine
        .control_drain_observations()
        .into_iter()
        .flat_map(|cycle| cycle.receipts)
        .collect()
}

fn receipt_json(receipt: &AppliedReceipt) -> Value {
    json!({
        "command_id": format!("{:?}", receipt.id),
        "status": format!("{:?}", receipt.status),
        "requested_frame": receipt.requested_frame.frame(),
        "applied_frame": receipt.applied_frame.map(ruviform_media_core::EngineFrame::frame),
        "transport_generation": receipt.transport_generation.get(),
        "plan_generation": receipt.plan_generation.get(),
        "clock_epoch": receipt.clock_epoch.get(),
        "degradation": receipt.degradation.map(|d| format!("{d:?}")),
        "reason": receipt.reason.map(|r| format!("{r:?}")),
    })
}

fn assert_exact_apply(receipt: &AppliedReceipt) {
    assert_eq!(receipt.status, ApplyStatus::Applied);
    assert_eq!(receipt.applied_frame, Some(receipt.requested_frame));
    assert_eq!(receipt.reason, None);
    assert_eq!(receipt.degradation, None);
}

fn anchor_history(run: &InstrumentRun, playing: bool) -> TransportApplicationReport {
    let InstrumentRun {
        app,
        engine,
        processor,
        ..
    } = run;
    let seek = TransportApplicationCommand::Seek(beat_ticks(HISTORY_ANCHOR_TICK));
    let commands = if playing {
        assert_eq!(observed_transport(run).ctx.status, TransportStatus::Playing);
        vec![seek]
    } else {
        vec![seek, TransportApplicationCommand::Pause]
    };
    let _ = observed_receipts(engine);
    let report = apply_transport(app, engine, *processor, &commands);
    run_paced(engine, LOOKAHEAD_BLOCKS + 1);
    let receipts = observed_receipts(engine);
    let applied: Vec<_> = receipts
        .iter()
        .filter(|r| r.status == ApplyStatus::Applied)
        .collect();
    assert_eq!(
        applied.len(),
        commands.len(),
        "anchor commands did not apply: {receipts:?}"
    );
    for receipt in applied {
        assert_exact_apply(receipt);
        assert_eq!(receipt.requested_frame, report.call_start);
    }
    let span = observed_transport(run);
    let expected = report.conversions[0].media.value()
        + if playing {
            i64::try_from(span.end.frame() - report.call_start.frame()).unwrap()
        } else {
            0
        };
    assert_eq!(span.ctx.position_at_offset(span.frames()).value(), expected);
    report
}

fn history_snapshot(run: &InstrumentRun) -> Value {
    let session = run.app.workspace().session();
    json!({
        "project_revision": session.revision().0,
        "tempo_revision": session.project().tempo_map.revision,
        "tempo_content_fingerprint": session.project().tempo_map.tempo_events,
    })
}

fn expected_history_project(baseline: &Project, step: HistoryStep) -> Project {
    let mut expected = baseline.clone();
    if step.bpm() != TEMPO_BPM_MILLI {
        expected.tempo_map.revision += 1;
        expected.tempo_map.tempo_events.push(TempoEventDto {
            tick: HISTORY_EDIT_TICK,
            bpm_milli: step.bpm(),
        });
    }
    expected
}

fn history_plan(run: &InstrumentRun, bpm: u32) -> (u64, Value) {
    let mut evidence = control_plan_evidence(&run.app, &run.engine, run.processor);
    let (generation, bpm_milli) = run
        .engine
        .with_control_dispatch(|dispatch| {
            let plan = dispatch.last_plan(run.processor).unwrap();
            let sample = beat_ticks(HISTORY_EDIT_TICK + 1).to_beat();
            let music = plan
                .tempo_map
                .as_ref()
                .unwrap()
                .time_info(sample, plan.musical_ppqn.unwrap())
                .unwrap();
            assert_eq!(
                music.bpm_milli, bpm,
                "wrong live tempo before applying any new transport command"
            );
            (plan.generation.get(), music.bpm_milli)
        })
        .unwrap();
    evidence["plan_bpm_milli_after_edit_tick"] = json!(bpm_milli);
    (generation, evidence)
}

fn history_install(run: &InstrumentRun, before: TransportSpan, generation: u64) -> Vec<Value> {
    run_paced(&run.engine, LOOKAHEAD_BLOCKS);
    let receipts = observed_receipts(&run.engine);
    let applied: Vec<_> = receipts
        .iter()
        .filter(|r| r.status == ApplyStatus::Applied)
        .collect();
    assert_eq!(
        applied.len(),
        1,
        "expected only the plan install: {receipts:?}"
    );
    assert_exact_apply(applied[0]);
    assert_eq!(applied[0].plan_generation.get(), generation);
    assert_eq!(
        applied[0].transport_generation,
        before.ctx.transport_generation
    );
    let after = observed_transport(run);
    assert_eq!(after.ctx.status, before.ctx.status);
    let elapsed = u32::try_from(after.end.frame() - before.start.frame()).unwrap();
    assert_eq!(
        after.ctx.position_at_offset(after.frames()),
        before.ctx.position_at_offset(elapsed)
    );
    receipts.iter().map(receipt_json).collect()
}

fn run_history_step(
    run: &mut InstrumentRun,
    baseline: &Project,
    step: HistoryStep,
    routing: u64,
) -> Value {
    let anchor = anchor_history(run, step.playing());
    let span = observed_transport(run);
    let session = run.app.workspace().session();
    let position = history_position(span, session.project(), step.playing());
    let project_revision = session.revision().0;
    let graph_build_attempts = session.engine().graph_builds();
    let before = history_snapshot(run);
    let old_plan = history_plan(
        run,
        if matches!(
            step,
            HistoryStep::UndoBeforeMeasurement | HistoryStep::UndoPlaying
        ) {
            HISTORY_A
        } else {
            TEMPO_BPM_MILLI
        },
    )
    .0;

    step.apply(&mut run.app);
    let session = run.app.workspace().session();
    assert_eq!(session.revision().0, project_revision + 1);
    assert_eq!(
        *session.project(),
        expected_history_project(baseline, step),
        "{step:?} changed more than the intended tempo snapshot"
    );
    assert_eq!(session.engine().graph_builds(), graph_build_attempts + 1);
    assert_eq!(session.engine().last_built_revision(), session.revision());
    assert_eq!(run.engine.control_generation(), Some(routing));

    // Transport application also syncs the project; prove notification and
    // RT installation before invoking it, or that retry could mask a defect.
    let (generation, plan) = history_plan(run, step.bpm());
    assert_ne!(generation, old_plan);
    let receipts = history_install(run, span, generation);
    let measurement = step.measured().then(|| {
        assert!(
            HISTORY_QUARTER * i64::from(PROJECT_PPQN) - HISTORY_DELTA_TICKS > HISTORY_EDIT_TICK
        );
        measure_positive(
            &run.app,
            &run.engine,
            run.processor,
            step.operation(),
            HISTORY_QUARTER,
            HISTORY_DELTA_TICKS,
            step.bpm(),
        )
    });
    json!({
        "operation": step.operation(),
        "before": before,
        "after": history_snapshot(run),
        "operation_transport": position,
        "anchor": {"host_ticks": HISTORY_ANCHOR_TICK, "call_start": anchor.call_start.frame(), "commands": format!("{:?}", anchor.commands)},
        "control_plan_before_measurement": plan,
        "rt_install_receipts": receipts,
        "retained_live_plan_generation": generation,
        "collateral_project_content_unchanged": true,
        "graph_notification_revision": project_revision + 1,
        "pcm": measurement,
    })
}

fn exercise_history(probe: &FixtureProbe, runner_env: &str, runner_stem: &str) {
    let mut run = instrument_run(probe, runner_env, runner_stem, "tempo-undo-redo");
    let (routing, _) = activate_generation_paced(&run.engine, "undo/redo instrument activation");
    run_paced(&run.engine, LOOKAHEAD_BLOCKS + 2);
    let baseline = run.app.workspace().session().project().clone();
    assert_eq!(baseline.ppqn.0, PROJECT_PPQN);
    assert_eq!(
        baseline.tempo_map.tempo_events,
        [
            TempoEventDto {
                tick: 0,
                bpm_milli: 120_000
            },
            TempoEventDto {
                tick: 1,
                bpm_milli: TEMPO_BPM_MILLI
            },
        ]
    );
    let sequence: Vec<_> = [
        HistoryStep::Add,
        HistoryStep::UndoBeforeMeasurement,
        HistoryStep::RedoPlaying,
        HistoryStep::UndoPlaying,
        HistoryStep::DivergentAddPlaying,
    ]
    .into_iter()
    .map(|step| run_history_step(&mut run, &baseline, step, routing))
    .collect();
    let project_b = run.app.workspace().session().project();
    let a = compile_project_musical_transport(&expected_history_project(
        &baseline,
        HistoryStep::RedoPlaying,
    ))
    .unwrap();
    let b = compile_project_musical_transport(project_b).unwrap();
    assert_eq!(a.tempo_revision(), b.tempo_revision());
    assert!(!a.same_project_basis(&b));
    assert!(!run.app.workspace_mut().session_mut().redo().unwrap());

    println!(
        "tim03-dynamic-tempo-undo-redo {}",
        json!({
            "schema": "tim03-dynamic-tempo-undo-redo/v1",
            "row_family": "production_real_abi_undo_redo_matrix",
            "format": probe.format,
            "test": format!("{}_production_tempo_undo_redo", probe.format),
            "tested_sha": env::var("TIM03_VERIFIED_SHA").ok(),
            "environment": {"os": env::consts::OS, "arch": env::consts::ARCH},
            "artifacts": run.provenance["artifacts"],
            "descriptor": run.provenance["descriptor"],
            "production_wiring": {
                "seam": "compose_routing_engine_with_out_of_process_and_boundaries -> PluginNode -> L1 runner -> test_instrument",
                "generation": routing,
                "boundary_declaration": format!("{:?}", boundary_declaration()),
                "history_path": "ProjectSession",
                "direct_endpoint_process_api": false,
                "caller_supplied_transport_info": false,
                "test_only_seed_or_fake_plan_install": false,
                "direct_endpoint_injection": false,
            },
            "project_ppqn": PROJECT_PPQN,
            "adapter_projection_ppqn": ADAPTER_PPQN,
            "lookahead": {
                "blocks": LOOKAHEAD_BLOCKS,
                "frames_per_block": BLOCK_FRAMES,
                "initial_blocks_used_as_positive": false,
                "rule": "one exact click after lookahead required; silence, pass-through and duplicate clicks cannot satisfy the oracle",
            },
            "same_revision_different_source": {"tempo_revision": a.tempo_revision().get(), "same_project_basis": false},
            "operation_sequence": sequence,
            "not_run": ["full dynamic edit matrix", "fault/state/crash", "GUI/EXT-L3", "macOS/Linux runtime", "3OS CI", "AudioLens cross-consumer"],
            "verdict": "pass",
        })
    );
}

#[test]
#[ignore = "artifact-dependent L1 lane: requires prebuilt test-only runner (run `mise run test-tim03-dynamic-musical-transport`)"]
fn vst3_production_tempo_undo_redo() {
    let (probe, _library) = load_vst3();
    exercise_history(&probe, VST3_RUNNER_ENV, VST3_RUNNER_STEM);
}

#[test]
#[ignore = "artifact-dependent L1 lane: requires prebuilt test-only runner (run `mise run test-tim03-dynamic-musical-transport`)"]
fn clap_production_tempo_undo_redo() {
    let (probe, _library) = load_clap();
    exercise_history(&probe, CLAP_RUNNER_ENV, CLAP_RUNNER_STEM);
}
