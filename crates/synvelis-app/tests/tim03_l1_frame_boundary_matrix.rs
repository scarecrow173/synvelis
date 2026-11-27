//! TIM-03 production real-ABI frame/boundary matrix evidence.
//!
//! This test intentionally uses only the production composition and
//! control seams. It never calls the endpoint process API, never supplies
//! caller-owned `TransportInfo`, and never seeds the child runner. Cells
//! the public seam cannot express are reported as unsupported/NotRun
//! rather than filled with fake-plugin evidence.

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
const BASE_BPM_MILLI: u32 = 93_750;

const CLICK: f32 = 0.5;
const LOOKAHEAD_BLOCKS: usize = 2;

fn edited_tempo(block_frames: usize) -> (u32, i64) {
    // Choose an edited tempo whose adapter-projected tick quantum is an
    // integer divisor of the physical block. The fixture rounds the
    // adapter's floating beat position to PPQN 960 at every block start;
    // these geometries keep that rounding exact instead of hiding it.
    match block_frames {
        100 => (120_000, 20),
        _ => (187_500, 32),
    }
}

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
            bpm_milli: BASE_BPM_MILLI,
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
            "stereo divergence at {index}"
        );
        if left.to_bits() == CLICK.to_bits() {
            positions.push(index);
        } else {
            assert_eq!(*left, 0.0, "non-click frame {index} must be silent");
        }
    }
    positions
}

fn run_blocks(
    engine: &synvelis_app::SessionRoutingEngine,
    blocks: usize,
    block_frames: usize,
) -> synvelis_player::driver::PlayReport {
    run_paced_with_block_frames(engine, blocks, block_frames)
}

fn independent_click_frame(delta_ticks: i64, bpm_milli: u32, block_frames: usize) -> usize {
    // Host project PPQN 480 is reprojected by the VST3/CLAP adapters to
    // fixture-visible PPQN 960. 960 here is adapter projection, never a
    // fixed host fallback. L1 freshness adds lookahead*activated max,
    // where activated max_frames is the production engine block geometry.
    let projected_delta_ticks = delta_ticks * 2;
    let numerator = projected_delta_ticks * 48_000 * 60_000;
    let denominator = i64::from(bpm_milli) * i64::from(ADAPTER_PPQN);
    let frame = (numerator + denominator - 1) / denominator
        + i64::try_from(LOOKAHEAD_BLOCKS * block_frames).unwrap();
    usize::try_from(frame).unwrap()
}

fn click_law_json(delta_ticks: i64, bpm_milli: u32, block_frames: usize, expected: usize) -> Value {
    let projected_delta_ticks = delta_ticks * 2;
    json!({
        "law": "ceil((boundary_tick-position_ticks)*sample_rate*60000/(bpm_milli*adapter_ppqn))+lookahead_frames",
        "host_project_ppqn": PROJECT_PPQN,
        "adapter_projection_ppqn": ADAPTER_PPQN,
        "host_delta_ticks": delta_ticks,
        "projected_delta_ticks": projected_delta_ticks,
        "numerator": projected_delta_ticks * 48_000 * 60_000,
        "denominator": bpm_milli * ADAPTER_PPQN,
        "lookahead_blocks": LOOKAHEAD_BLOCKS,
        "activated_max_frames": block_frames,
        "lookahead_frames": LOOKAHEAD_BLOCKS * block_frames,
        "expected_click_frame": expected,
        "fixed_960_fallback": false,
    })
}

#[derive(Clone, Copy)]
struct SeekCase<'a> {
    case: &'a str,
    quarter: i64,
    delta_ticks: i64,
    bpm_milli: u32,
}

fn seek_positive(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
    block_frames: usize,
    case: SeekCase<'_>,
) -> Value {
    let position_ticks = case.quarter * i64::from(PROJECT_PPQN) - case.delta_ticks;
    let transport_report = apply_transport(
        app,
        engine,
        processor,
        &[
            TransportApplicationCommand::Seek(beat_ticks(position_ticks)),
            TransportApplicationCommand::Play,
        ],
    );
    let expected = independent_click_frame(case.delta_ticks, case.bpm_milli, block_frames);
    let blocks = expected / block_frames + 3;
    let run = run_blocks(engine, blocks, block_frames);
    let observed = click_positions(&run);
    assert_eq!(
        observed,
        vec![expected],
        "{}/W{block_frames}: expected one fresh click at {expected}, got {observed:?}",
        case.case
    );
    json!({
        "case": case.case,
        "scenario": "seek-single",
        "public_commands": ["Seek","Play"],
        "host_position_ticks": position_ticks,
        "independent_calculation": click_law_json(case.delta_ticks, case.bpm_milli, block_frames, expected),
        "observed_click_frames": observed,
        "transport_call_start": transport_report.call_start.frame(),
        "amplitude": CLICK,
        "stereo_identical": true,
        "non_click_frames_silent": true,
        "process_length_proof": "PCM indirect evidence only; exact physical process lengths are proven in the headless recording matrix",
        "verdict": "pass",
    })
}

fn loop_positive(
    app: &SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
    block_frames: usize,
) -> Value {
    let ppqn = i64::from(PROJECT_PPQN);
    let quarter = 16;
    let before_ticks = if block_frames == 100 { 25 } else { 24 };
    // Keep both the first in-loop click and the first post-wrap click
    // exactly on the physical block grid. For W=100 that requires a
    // longer post-click loop tail because frames-per-host-tick (64) and
    // the 100-frame geometry only share a 1600-frame (25 host tick)
    // lattice; this avoids hiding fractional adapter-tick rounding in
    // the positive oracle.
    let after_ticks = if block_frames == 100 { 25 } else { 4 };
    let start = beat_ticks(quarter * ppqn - before_ticks);
    let end = beat_ticks(quarter * ppqn + after_ticks);
    let report = apply_transport(
        app,
        engine,
        processor,
        &[
            TransportApplicationCommand::SetLoop { start, end },
            TransportApplicationCommand::Seek(start),
            TransportApplicationCommand::Play,
        ],
    );
    let first = LOOKAHEAD_BLOCKS * block_frames + usize::try_from(before_ticks * 64).unwrap();
    let period = usize::try_from((before_ticks + after_ticks) * 64).unwrap();
    let second = first + period;
    // Run beyond the first post-wrap click but below the second loop
    // iteration's following click. Two click positions therefore prove
    // that playback crossed the natural loop discontinuity instead of
    // merely observing the first in-loop quarter click.
    let total_blocks = second / block_frames + 2;
    let run = run_blocks(engine, total_blocks, block_frames);
    let expected = vec![first, second];
    let observed = click_positions(&run);
    assert_eq!(
        observed, expected,
        "W{block_frames}: loop-derived click positions differ"
    );
    json!({
        "case": "natural_loop_wrap_simultaneous_head_batch",
        "scenario": "loop-wrap-natural",
        "public_commands": ["SetLoop","Seek","Play"],
        "same_head_batch_order": "SetLoop, Seek, Play as supplied by the public transport application batch",
        "loop_range_host_ticks": {"start": quarter * ppqn - before_ticks, "end": quarter * ppqn + after_ticks},
        "loop_period_frames_at_bpm": (before_ticks + after_ticks) * 64,
        "first_click_after_loop_start_frames": before_ticks * 64,
        "second_click_is_post_wrap": true,
        "block_aligned_positive_oracle": true,
        "lookahead_frames": LOOKAHEAD_BLOCKS * block_frames,
        "expected_click_frames": expected,
        "observed_click_frames": observed,
        "transport_call_start": report.call_start.frame(),
        "process_length_proof": "PCM indirect evidence only; exact segmentation is authoritative only in the headless matrix",
        "verdict": "pass",
    })
}

fn apply_tempo_edit(app: &mut SynvelisApp, tick: i64, bpm_milli: u32) -> u64 {
    let revision = app.workspace().session().revision();
    let resulting = app
        .workspace_mut()
        .session_mut()
        .apply(ProjectOp::AddTempoEvent { tick, bpm_milli }, revision)
        .expect("committed tempo edit");
    resulting.0
}

fn tempo_revision_positive(
    app: &mut SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
    block_frames: usize,
) -> Value {
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
    run_blocks(engine, LOOKAHEAD_BLOCKS + 1, block_frames);
    let before = app.workspace().session().project().tempo_map.revision;
    let (edit_bpm, edit_delta_ticks) = edited_tempo(block_frames);
    let resulting_revision = apply_tempo_edit(app, 4_800, edit_bpm);
    let project = app.workspace().session().project();
    assert_eq!(project.tempo_map.revision, before + 1);
    assert_eq!(project.ppqn.0, PROJECT_PPQN);
    let surface = engine.control_report().expect("control surface evidence");
    assert!(
        surface.musical_refresh_errors.is_empty(),
        "musical refresh errors: {:?}",
        surface.musical_refresh_errors
    );
    run_blocks(engine, LOOKAHEAD_BLOCKS, block_frames);
    let mut positive = seek_positive(
        app,
        engine,
        processor,
        block_frames,
        SeekCase {
            case: "tempo_revision_plan_boundary",
            quarter: 16,
            delta_ticks: edit_delta_ticks,
            bpm_milli: edit_bpm,
        },
    );
    positive["edit"] = json!({
        "op": "ProjectOp::AddTempoEvent",
        "tick": 4_800,
        "bpm_milli": edit_bpm,
        "resulting_project_revision": resulting_revision,
        "resulting_tempo_revision": before + 1,
        "tempo_command_invented": false,
        "meaning": "tempo-map/tempo-revision discontinuity carried by production musical plan refresh",
    });
    positive["control_evidence"] = json!({
        "musical_transport_surface": format!("{:?}", surface.musical_transport),
        "musical_refresh_errors": format!("{:?}", surface.musical_refresh_errors),
    });
    positive
}

fn unsupported_matrix(block_frames: usize) -> Vec<Value> {
    let mut rows = Vec::new();
    for boundary in [0_u32, 1, 100, u32::try_from(block_frames).unwrap() - 1] {
        let valid = boundary < u32::try_from(block_frames).unwrap();
        for scenario in [
            "seek-single",
            "tempo-plan-install",
            "param-arbitrary-frame-offset",
        ] {
            let status = if !valid {
                "invalid-out-of-window-honest-negative"
            } else if boundary == 0 && scenario != "param-arbitrary-frame-offset" {
                "pass-head-public-seam"
            } else {
                "unsupported-by-current-production-seam"
            };
            rows.push(json!({
                "window_frames": block_frames,
                "requested_boundary": boundary,
                "scenario": scenario,
                "status": status,
                "fake_evidence_promoted": false,
                "note": "production TransportApplicationCommand is staged at the next public window head; arbitrary interior command installation is not claimed",
            }));
        }
    }
    rows.push(json!({
        "window_frames": block_frames,
        "scenario": "natural-loop-wrap",
        "status": "pass-primary-pcm-oracle",
        "note": "loop discontinuity is derived by transport geometry rather than requested as an arbitrary interior command boundary",
    }));
    rows
}

fn tempo_revision_geometry(
    probe: &FixtureProbe,
    runner: &std::ffi::OsString,
    block_frames: usize,
) -> Value {
    let instrument = descriptor_named(probe, TEST_INSTRUMENT);
    let root = scratch(&format!(
        "{}-frame-matrix-tempo-W{block_frames}",
        probe.format
    ));
    std::fs::create_dir_all(root.join("media")).unwrap();
    let mut app = SynvelisApp::new(BackendReport::available(
        "gpui",
        format!("tim03-l1-frame-matrix-tempo-W{block_frames}"),
    ));
    let processor = build_project(&mut app, &root, instrument.id);
    let engine = compose_routing_engine_with_out_of_process_and_boundaries(
        &mut app,
        session_config_with_block_frames(root.join("media"), block_frames),
        Vec::new(),
        vec![ProductOutOfProcessPlugin {
            spec: OutOfProcessSpec::new(instrument, runner.clone()),
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
                name: format!("tim03-l1-frame-matrix-tempo-W{block_frames}"),
            },
            revision,
        )
        .unwrap();
    let (_generation, activation) =
        activate_generation_paced(&engine, "tempo matrix generation activation");
    assert!(activation.pcm.iter().flatten().all(|sample| *sample == 0.0));
    tempo_revision_positive(&mut app, &engine, processor, block_frames)
}

fn exercise_geometry(
    probe: &FixtureProbe,
    runner: &std::ffi::OsString,
    block_frames: usize,
) -> Value {
    let instrument = descriptor_named(probe, TEST_INSTRUMENT);
    let runner_path = PathBuf::from(runner.clone());
    let staged_fixture = stage_fixture_sibling(probe, &runner_path);
    let (runner_size, runner_sha256) = file_facts(&runner_path);
    let (staged_size, staged_sha256) = file_facts(&staged_fixture);
    assert_eq!(staged_sha256, probe.sha256);

    let root = scratch(&format!("{}-frame-matrix-W{block_frames}", probe.format));
    std::fs::create_dir_all(root.join("media")).unwrap();
    let mut app = SynvelisApp::new(BackendReport::available(
        "gpui",
        format!("tim03-l1-frame-boundary-matrix-W{block_frames}"),
    ));
    let processor = build_project(&mut app, &root, instrument.id);
    let engine = compose_routing_engine_with_out_of_process_and_boundaries(
        &mut app,
        session_config_with_block_frames(root.join("media"), block_frames),
        Vec::new(),
        vec![ProductOutOfProcessPlugin {
            spec: OutOfProcessSpec::new(instrument, runner.clone()),
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
                name: format!("tim03-l1-frame-boundary-matrix-W{block_frames}"),
            },
            revision,
        )
        .unwrap();

    let (generation, activation) =
        activate_generation_paced(&engine, "matrix generation activation");
    assert!(activation.pcm.iter().flatten().all(|sample| *sample == 0.0));
    run_blocks(&engine, 4, block_frames);
    let surface = engine.control_report().expect("control surface evidence");
    let musical_surface = format!("{:?}", surface.musical_transport);
    assert!(
        musical_surface.contains("Available") && musical_surface.contains("ppqn: 480"),
        "production surface did not compile project musical transport: {musical_surface}"
    );

    let seek = seek_positive(
        &app,
        &engine,
        processor,
        block_frames,
        SeekCase {
            case: "seek_derived_positive",
            quarter: 8,
            delta_ticks: if block_frames == 100 { 25 } else { 8 },
            bpm_milli: BASE_BPM_MILLI,
        },
    );
    let loop_case = loop_positive(&app, &engine, processor, block_frames);
    let tempo = tempo_revision_geometry(probe, runner, block_frames);

    json!({
        "window_geometry": {
            "frames": block_frames,
            "lookahead_blocks": LOOKAHEAD_BLOCKS,
            "activated_max_frames": block_frames,
            "l1_latency_frames": LOOKAHEAD_BLOCKS * block_frames,
            "latency_formula": "lookahead_blocks * activated max_frames; not fixed at 512 frames",
        },
        "generation": generation,
        "artifact_facts": {
            "runner": {"path": runner_path, "size": runner_size, "sha256": runner_sha256},
            "staged_fixture": {"path": staged_fixture, "size": staged_size, "sha256": staged_sha256},
        },
        "musical_transport_surface": musical_surface,
        "positive_cases": [seek, loop_case, tempo],
        "matrix_cells": unsupported_matrix(block_frames),
    })
}

fn exercise_format(probe: &FixtureProbe, runner_env: &str, runner_stem: &str) {
    let instrument = descriptor_named(probe, TEST_INSTRUMENT);
    let runner = runner_program(runner_env, runner_stem);
    let geometries: Vec<Value> = [100_usize, 128, 256]
        .into_iter()
        .map(|block| exercise_geometry(probe, &runner, block))
        .collect();
    println!(
        "tim03-l1-frame-boundary-matrix {}",
        json!({
            "schema": "tim03-l1-frame-boundary-matrix/v1",
            "format": probe.format,
            "test": format!("tim03_l1_frame_boundary_matrix::{}_frame_boundary_matrix", probe.format),
            "environment": {"os": env::consts::OS, "arch": env::consts::ARCH},
            "artifacts": {
                "fixture": {"path": probe.path, "sha256": probe.sha256},
                "ruviform_resolved_revision": ruviform_revision(),
            },
            "descriptor": {"id": instrument.id, "name": instrument.name, "vendor": instrument.vendor, "version": instrument.version},
            "production_wiring": {
                "seam": "compose_routing_engine_with_out_of_process_and_boundaries",
                "boundary_declaration": "TimingPrecision::FrameAccurateEvents/InBlockTransportSupport::Unknown/consent:none/budget:(max_spans_per_callback:64,max_events_per_span:256,max_pending_applies:16,max_deferral:4096)",
                "runner": "synvelis-plugin-test-l1-runner",
                "sibling_staging": true,
                "hash_mismatch_fail_closed": true,
                "direct_endpoint_process_api": false,
                "caller_supplied_transport_info": false,
                "test_only_seeding": false,
                "fake_plan_direct_install": false,
                "direct_endpoint_injection": false,
            },
            "project": {
                "host_project_ppqn": PROJECT_PPQN,
                "adapter_projection_ppqn": ADAPTER_PPQN,
                "initial_tempo_bpm_milli": BASE_BPM_MILLI,
                "edited_tempo_bpm_milli": "geometry-dependent exact-projection tempo; see positive_cases",
                "fixed_960_fallback": false,
            },
            "geometries": geometries,
            "param_frame_offset_oracle": {
                "primary_test_instrument": "NotRun: gain-param/frame-offset effect cannot be independently separated from the deterministic voice/click law through this public seam in this slice",
                "secondary_corroboration": "existing tim03_l1_transport_application test_gain lane; not promoted to primary test_instrument matrix pass",
            },
            "process_length_oracle": {
                "production_real_abi": "PCM indirect evidence only",
                "authoritative_exact_sequence": "headless recording-plugin matrix in synvelis-player",
            },
            "not_run": [
                "arbitrary interior seek/tempo-plan/param scheduling unsupported by the current public production seam",
                "full dynamic tempo edit matrix, automation matrix, fault/state/crash isolation, GUI/EXT-L3, macOS/Linux runtime, 3OS CI, AudioLens cross-consumer",
            ],
        })
    );
}

#[test]
fn vst3_frame_boundary_matrix() {
    let (probe, _library) = load_vst3();
    exercise_format(&probe, VST3_RUNNER_ENV, VST3_RUNNER_STEM);
}

#[test]
fn clap_frame_boundary_matrix() {
    let (probe, _library) = load_clap();
    exercise_format(&probe, CLAP_RUNNER_ENV, CLAP_RUNNER_STEM);
}
