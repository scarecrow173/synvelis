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
use ruviform_media_core::{InBlockTransportSupport, ScheduleBudget};
use ruviform_plugin_host::OutOfProcessSpec;
use ruviform_ui::BackendReport;
use serde_json::{Value, json};
use synvelis_app::composition::compose_routing_engine_with_out_of_process_and_boundaries;
use synvelis_app::engine::TransportApplicationCommand;
use synvelis_app::{ProductOutOfProcessPlugin, SynvelisApp};
use synvelis_player::{BoundaryDeclaration, TimingPrecision};
use synvelis_project::{BeatPoint, PpqnConversionPolicy, ProcessorId, ProjectOp};

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

fn independent_click_frame(delta_ticks: i64) -> usize {
    // At 93.75 BPM one host project tick is exactly 64 frames and one
    // 256-frame block is exactly four host ticks, so the PPQN projection
    // never accumulates fractional-tick rounding across a block. The
    // adapter reprojects host PPQN 480 to fixture-visible PPQN 960 while
    // preserving the same musical instant and frame delta.
    let frame = delta_ticks * 64 + i64::try_from(LOOKAHEAD_BLOCKS * BLOCK_FRAMES).unwrap();
    usize::try_from(frame).unwrap()
}

fn click_law_json(delta_ticks: i64, expected: usize) -> Value {
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
        "denominator": TEMPO_BPM_MILLI * ADAPTER_PPQN,
        "bpm_milli": TEMPO_BPM_MILLI,
        "lookahead_frames": LOOKAHEAD_BLOCKS * BLOCK_FRAMES,
        "expected_click_frame": expected,
        "fixed_960_fallback": false,
        "projection_note": "960 is the VST3/CLAP adapter projection observed by the fixture; the host plan carries project PPQN 480",
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
}

fn positive_cases(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
) -> Vec<Value> {
    let mut positives = Vec::new();
    for (index, delta_ticks) in [3_i64, 2, 1].into_iter().enumerate() {
        let quarter = 8 + i64::try_from(index).unwrap() * 2;
        let expected = independent_click_frame(delta_ticks);
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
            "independent_calculation": click_law_json(delta_ticks, expected),
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
    app: &SynvelisApp,
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
    MusicalRun {
        generation,
        tempo_revision,
        musical_surface,
        musical_refresh_errors,
        positives,
        stopped_pass,
        paused_pass,
        loop_observed,
    }
}

fn exercise_format(probe: &FixtureProbe, runner_env: &str, runner_stem: &str) {
    let instrument = descriptor_named(probe, TEST_INSTRUMENT);
    let runner = runner_program(runner_env, runner_stem);
    let runner_path = PathBuf::from(&runner);
    let staged_fixture = stage_fixture_sibling(probe, &runner_path);
    let (runner_size, runner_sha256) = file_facts(&runner_path);
    let (staged_size, staged_sha256) = file_facts(&staged_fixture);
    assert_eq!(staged_sha256, probe.sha256);

    let root = scratch(&format!("{}-production-musical", probe.format));
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
    let run = execute_musical_run(&app, &engine, processor);

    println!(
        "tim03-l1-production-musical-transport {}",
        json!({
            "schema": "tim03-l1-production-musical-transport/v1",
            "format": probe.format,
            "test": format!("tim03_l1_production_musical_transport::{}_production_musical_transport", probe.format),
            "environment": {"os": env::consts::OS, "arch": env::consts::ARCH},
            "artifacts": {
                "fixture": {"path": probe.path, "sha256": probe.sha256},
                "staged_fixture": {"path": staged_fixture, "size": staged_size, "sha256": staged_sha256},
                "runner": {"path": runner_path, "size": runner_size, "sha256": runner_sha256, "resolution_env": runner_env},
                "ruviform_resolved_revision": ruviform_revision(),
            },
            "descriptor": {"id": instrument.id, "name": instrument.name, "vendor": instrument.vendor, "version": instrument.version},
            "production_wiring": {
                "seam": "compose_routing_engine_with_out_of_process_and_boundaries",
                "generation": run.generation,
                "direct_endpoint_transport_info_supplied": false,
                "boundary_declaration": "FrameAccurateEvents/InBlockTransportSupport::Unknown",
            },
            "project": {
                "ppqn": PROJECT_PPQN,
                "adapter_projection_ppqn": ADAPTER_PPQN,
                "tempo_revision": run.tempo_revision,
                "tempo_events": [
                    {"tick": 0, "bpm_milli": 120_000},
                    {"tick": 1, "bpm_milli": TEMPO_BPM_MILLI},
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
                "dynamic tempo edit matrix and full frame/boundary matrix",
                "fault/state/crash isolation, GUI/EXT-L3, macOS/Linux runtime, 3OS CI, AudioLens cross-consumer",
            ],
        })
    );
}

#[test]
fn vst3_production_musical_transport() {
    let (probe, _library) = load_vst3();
    exercise_format(&probe, VST3_RUNNER_ENV, VST3_RUNNER_STEM);
}

#[test]
fn clap_production_musical_transport() {
    let (probe, _library) = load_clap();
    exercise_format(&probe, CLAP_RUNNER_ENV, CLAP_RUNNER_STEM);
}
