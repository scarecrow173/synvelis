//! TIM-03 direct L1 `test_instrument` musical transport oracle
//! (synvelis#127, slice `tim03-instrument-transport-oracle-s1`).
//!
//! This test deliberately exercises the direct L1 endpoint API with caller
//! supplied `TransportInfo`. It does not claim the production composition
//! → `PluginNode` → plugin end-to-end musical transport path: the resolved
//! ruviform source currently passes no tempo map/PPQN through the production
//! graph node handoff, tracked upstream as ruviform issue 1040.

mod common;

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::thread;
use std::time::Duration;

use common::tim03_l1::{
    CLAP_RUNNER_ENV, CLAP_RUNNER_STEM, TEST_INSTRUMENT, VST3_RUNNER_ENV, VST3_RUNNER_STEM,
    descriptor_named, file_facts, load_clap, load_vst3, runner_program, ruviform_revision,
    stage_fixture_sibling,
};
use ruviform_audio_core::{PlanarBufferView, PlanarBufferViewMut, SampleRate};
use ruviform_media_core::{
    MediaDuration, MediaRange, MediaTime, MusicalTimeInfo, TimeBase, TransportStatus,
};
use ruviform_plugin_core::{
    EventList, HostInput, HostOutput, PluginDescriptor, ProcessConfig, ProcessResult, TransportInfo,
};
use ruviform_plugin_host::{
    EndpointBounds, HostCommand, HostError, HostReply, OutOfProcessSpec, PluginEndpoint,
    PluginHandle, PluginHost,
};
use serde_json::{Value, json};

const FRAMES: usize = 512;
const FRAMES_U32: u32 = 512;
const SAMPLE_RATE: u32 = 48_000;
const L1_LOOKAHEAD: u64 = 2;
const PHASE_CALLS: u32 = 16;
const CLICK_AMPLITUDE: f32 = 0.5;
const PPQN: u16 = 960;
const RUVIFORM_ISSUE: &str = "https://github.com/scarecrow173/ruviform/issues/1040";
const UPSTREAM_FILE: &str = "crates/ruviform-audio-plugin-bridge/src/node.rs";
const UPSTREAM_LINE: u32 = 657;

fn process_config() -> ProcessConfig {
    ProcessConfig::new(SampleRate::new(SAMPLE_RATE).unwrap(), 1, FRAMES_U32).unwrap()
}

fn execute_ok(host: &mut PluginHost, command: HostCommand, step: &str) {
    match host.execute(command) {
        Ok(HostReply::Ok) => {}
        outcome => panic!("{step} did not complete: {outcome:?}"),
    }
}

fn instantiate(host: &mut PluginHost, plugin_id: &str) -> PluginHandle {
    match host.execute(HostCommand::Instantiate {
        plugin_id: plugin_id.to_owned(),
    }) {
        Ok(HostReply::Instantiated { handle }) => handle,
        outcome => panic!("instantiate failed for {plugin_id}: {outcome:?}"),
    }
}

fn drive_to_processing(host: &mut PluginHost, handle: PluginHandle) {
    execute_ok(host, HostCommand::Initialize { handle }, "initialize");
    execute_ok(
        host,
        HostCommand::Activate {
            handle,
            config: process_config(),
        },
        "activate",
    );
    execute_ok(
        host,
        HostCommand::StartProcessing { handle },
        "start processing",
    );
}

fn time_base() -> TimeBase {
    TimeBase::sample_rate(SAMPLE_RATE).unwrap()
}

fn media_frames_for_ticks(position_ticks: i64, bpm_milli: u32, ppqn: u16) -> i64 {
    let numerator = i128::from(position_ticks) * i128::from(SAMPLE_RATE) * 60_000;
    let denominator = i128::from(bpm_milli) * i128::from(ppqn);
    assert_eq!(numerator % denominator, 0, "test positions must be exact");
    i64::try_from(numerator / denominator).expect("media position fits i64")
}

fn musical(position_ticks: i64, bpm_milli: u32) -> MusicalTimeInfo {
    MusicalTimeInfo {
        position_ticks,
        bar_start_tick: 0,
        bpm_milli,
        ppqn: PPQN,
        sig_num: 4,
        sig_den: 4,
    }
}

fn transport(
    status: TransportStatus,
    position_ticks: i64,
    bpm_milli: u32,
    with_musical: bool,
    looped: bool,
) -> TransportInfo {
    let base = time_base();
    let media_frames = media_frames_for_ticks(position_ticks, bpm_milli, PPQN);
    let mut info = TransportInfo::new(status, MediaTime::new(media_frames, base));
    if with_musical {
        info = info.with_musical(musical(position_ticks, bpm_milli));
    }
    if looped {
        info = info.with_loop_range(MediaRange::new(
            MediaTime::new(0, base),
            MediaDuration::new(48_000, base),
        ));
    }
    info
}

fn expected_click_frame(position_ticks: i64, bpm_milli: u32, ppqn: u16) -> Option<usize> {
    if bpm_milli == 0 || ppqn == 0 {
        return None;
    }
    let ppqn = i128::from(ppqn);
    let position = i128::from(position_ticks);
    let mut boundary = position.div_euclid(ppqn) * ppqn;
    if boundary < position {
        boundary += ppqn;
    }
    let delta = boundary - position;
    let numerator = delta * i128::from(SAMPLE_RATE) * 60_000;
    let denominator = i128::from(bpm_milli) * ppqn;
    let frame =
        numerator.div_euclid(denominator) + i128::from(numerator.rem_euclid(denominator) != 0);
    if frame < 0 || frame >= i128::try_from(FRAMES).unwrap() {
        return None;
    }
    Some(usize::try_from(frame).expect("click frame fits usize"))
}

#[derive(Clone, Copy)]
struct ClickInput {
    position_ticks: i64,
    bpm_milli: u32,
}

impl ClickInput {
    fn evidence(self, expected_frame: usize) -> Value {
        let ppqn = i128::from(PPQN);
        let position = i128::from(self.position_ticks);
        let mut boundary = position.div_euclid(ppqn) * ppqn;
        if boundary < position {
            boundary += ppqn;
        }
        let delta = boundary - position;
        let numerator = delta * i128::from(SAMPLE_RATE) * 60_000;
        let denominator = i128::from(self.bpm_milli) * ppqn;
        json!({
            "law": "ceil((boundary_tick - position_ticks) * sample_rate * 60000 / (bpm_milli * ppqn))",
            "sample_rate": SAMPLE_RATE,
            "position_ticks": self.position_ticks,
            "boundary_tick": i64::try_from(boundary).expect("boundary fits i64"),
            "delta_ticks": i64::try_from(delta).expect("delta fits i64"),
            "numerator": i64::try_from(numerator).expect("numerator fits i64"),
            "denominator": i64::try_from(denominator).expect("denominator fits i64"),
            "bpm_milli": self.bpm_milli,
            "ppqn": PPQN,
            "expected_click_frame": expected_frame,
        })
    }
}

#[derive(Clone, Copy)]
struct PhaseSpec {
    name: &'static str,
    transport: TransportInfo,
    click: Option<(ClickInput, usize)>,
    negative_control: Option<&'static str>,
    position_jump: bool,
}

fn phase_specs() -> Vec<PhaseSpec> {
    let click_100 = expected_click_frame(956, 120_000, PPQN).expect("click@100 in block");
    let click_500 = expected_click_frame(940, 120_000, PPQN).expect("click@500 in block");
    let click_250 = expected_click_frame(950, 120_000, PPQN).expect("click@250 in block");
    let click_200 = expected_click_frame(950, 150_000, PPQN).expect("click@200 in block");
    assert_eq!(click_100, 100);
    assert_eq!(click_500, 500);
    assert_eq!(click_250, 250);
    assert_eq!(click_200, 200);
    vec![
        PhaseSpec {
            name: "stopped_negative",
            transport: transport(TransportStatus::Stopped, 956, 120_000, true, false),
            click: None,
            negative_control: Some("stopped"),
            position_jump: false,
        },
        PhaseSpec {
            name: "paused_negative",
            transport: transport(TransportStatus::Paused, 956, 120_000, true, false),
            click: None,
            negative_control: Some("paused"),
            position_jump: false,
        },
        PhaseSpec {
            name: "missing_musical_negative",
            transport: transport(TransportStatus::Playing, 956, 120_000, false, false),
            click: None,
            negative_control: Some("missing_musical"),
            position_jump: false,
        },
        PhaseSpec {
            name: "click_120bpm_position956_frame100",
            transport: transport(TransportStatus::Playing, 956, 120_000, true, false),
            click: Some((
                ClickInput {
                    position_ticks: 956,
                    bpm_milli: 120_000,
                },
                click_100,
            )),
            negative_control: None,
            position_jump: false,
        },
        PhaseSpec {
            name: "seek_click_120bpm_position940_frame500",
            transport: transport(TransportStatus::Playing, 940, 120_000, true, false),
            click: Some((
                ClickInput {
                    position_ticks: 940,
                    bpm_milli: 120_000,
                },
                click_500,
            )),
            negative_control: None,
            position_jump: true,
        },
        PhaseSpec {
            name: "loop_click_120bpm_position950_frame250",
            transport: transport(TransportStatus::Playing, 950, 120_000, true, true),
            click: Some((
                ClickInput {
                    position_ticks: 950,
                    bpm_milli: 120_000,
                },
                click_250,
            )),
            negative_control: None,
            position_jump: false,
        },
        PhaseSpec {
            name: "click_150bpm_position950_frame200",
            transport: transport(TransportStatus::Playing, 950, 150_000, true, false),
            click: Some((
                ClickInput {
                    position_ticks: 950,
                    bpm_milli: 150_000,
                },
                click_200,
            )),
            negative_control: None,
            position_jump: false,
        },
    ]
}

#[derive(Clone, Copy, Default)]
struct LegDeltas {
    passthrough: u64,
    dropped: u64,
}

fn endpoint_block_transport(
    host: &PluginHost,
    handle: PluginHandle,
    endpoint: &mut PluginEndpoint,
    out_left: &mut [f32],
    out_right: &mut [f32],
    events: EventList<'_>,
    transport: Option<TransportInfo>,
) -> (Result<ProcessResult, HostError>, LegDeltas) {
    let in_left = [0.0_f32; FRAMES];
    let in_right = [0.0_f32; FRAMES];
    let input = [&in_left[..], &in_right[..]];
    let input_view = PlanarBufferView::new(&input[..]).unwrap();
    let inputs = [HostInput(&input_view)];
    let mut out_list = [&mut *out_left, &mut *out_right];
    let mut output_view = PlanarBufferViewMut::new(&mut out_list).unwrap();
    let mut outputs = [HostOutput(&mut output_view)];
    let counters = |host: &PluginHost| {
        host.audio_counters(handle).map_or((0, 0), |counters| {
            (counters.passthrough_submits, counters.dropped_submits)
        })
    };
    let before = counters(host);
    let result = endpoint.process(&inputs, &mut outputs, events, transport);
    let after = counters(host);
    (
        result,
        LegDeltas {
            passthrough: after.0 - before.0,
            dropped: after.1 - before.1,
        },
    )
}

#[derive(Clone, Default)]
struct CaseObservations {
    matched_sources: Vec<u64>,
    silent_blocks: u32,
    click_blocks: u32,
    observed_click_frames: Vec<usize>,
    observed_amplitudes: Vec<f32>,
    stereo_identical_blocks: u32,
    non_click_silent_blocks: u32,
}

fn ieee_silent(samples: &[f32]) -> bool {
    samples.iter().all(|sample| *sample == 0.0)
}

fn stereo_bit_identical(left: &[f32], right: &[f32]) -> bool {
    left.iter()
        .zip(right)
        .all(|(left, right)| left.to_bits() == right.to_bits())
}

fn observe_matched_block(
    spec: &PhaseSpec,
    observed: &mut CaseObservations,
    out_left: &[f32],
    out_right: &[f32],
    call_u32: u32,
    source: u64,
) {
    observed.matched_sources.push(source);
    if let Some((_, expected_frame)) = spec.click {
        assert_eq!(
            out_left[expected_frame].to_bits(),
            CLICK_AMPLITUDE.to_bits(),
            "call {call_u32} source {source}: click impulse at frame {expected_frame}"
        );
        assert!(
            out_left
                .iter()
                .enumerate()
                .all(|(index, sample)| index == expected_frame || *sample == 0.0),
            "call {call_u32} source {source}: left channel outside click frame {expected_frame} must be silent"
        );
        assert!(
            out_right
                .iter()
                .enumerate()
                .all(|(index, sample)| index == expected_frame || *sample == 0.0),
            "call {call_u32} source {source}: right channel outside click frame {expected_frame} must be silent"
        );
        assert!(
            stereo_bit_identical(out_left, out_right),
            "call {call_u32} source {source}: click block must be stereo identical"
        );
        observed.click_blocks += 1;
        observed.observed_click_frames.push(expected_frame);
        observed.observed_amplitudes.push(out_left[expected_frame]);
        observed.stereo_identical_blocks += 1;
        observed.non_click_silent_blocks += 1;
    } else {
        assert!(
            ieee_silent(out_left) && ieee_silent(out_right),
            "call {call_u32} source {source}: {} must be silent",
            spec.name
        );
        assert!(
            stereo_bit_identical(out_left, out_right),
            "call {call_u32} source {source}: silent block must be stereo identical"
        );
        observed.silent_blocks += 1;
        observed.stereo_identical_blocks += 1;
    }
}

fn validate_case_delivery(phases: &[PhaseSpec], cases: &[CaseObservations]) {
    for (spec, observed) in phases.iter().zip(cases) {
        assert!(
            observed.matched_sources.len() >= 2,
            "{} barely delivered — matched sources {:?}",
            spec.name,
            observed.matched_sources
        );
        if spec.click.is_some() {
            assert_eq!(
                observed.click_blocks,
                u32::try_from(observed.matched_sources.len()).unwrap(),
                "{}: every matched click-phase block must carry its click",
                spec.name
            );
        } else {
            assert_eq!(
                observed.silent_blocks,
                u32::try_from(observed.matched_sources.len()).unwrap(),
                "{}: every matched negative-control block must be silent",
                spec.name
            );
        }
    }
}

fn observe_transport_phases(
    host: &PluginHost,
    handle: PluginHandle,
    endpoint: &mut PluginEndpoint,
    phases: &[PhaseSpec],
) -> (Vec<CaseObservations>, Value) {
    let empty = EventList::empty(FRAMES_U32);
    let mut cases = vec![CaseObservations::default(); phases.len()];
    let mut initial_latency_blocks = Vec::new();
    let mut substituted_pass_through_blocks = Vec::new();
    let mut dropped_submit_calls = Vec::new();
    let phase_calls = u64::from(PHASE_CALLS);
    let phase_len = usize::try_from(PHASE_CALLS).unwrap();
    let total_calls = phases.len() * phase_len;

    for call in 0..total_calls {
        let call_u32 = u32::try_from(call).expect("call index fits u32");
        let phase_index = call / phase_len;
        let transport = phases[phase_index].transport;
        let mut out_left = vec![0.0_f32; FRAMES];
        let mut out_right = vec![0.0_f32; FRAMES];
        thread::sleep(Duration::from_millis(5));
        let (result, deltas) = endpoint_block_transport(
            host,
            handle,
            endpoint,
            &mut out_left,
            &mut out_right,
            empty,
            Some(transport),
        );
        assert_eq!(result, Ok(ProcessResult::CONTINUE), "call {call_u32}");
        if deltas.dropped > 0 {
            dropped_submit_calls.push(json!({
                "call": call_u32,
                "dropped_delta": deltas.dropped,
            }));
        }
        if deltas.passthrough > 0 {
            substituted_pass_through_blocks.push(json!({
                "call": call_u32,
                "passthrough_delta": deltas.passthrough,
                "reason": "substituted/pass-through output is not fresh plugin evidence",
            }));
            continue;
        }
        let Some(source) = u64::try_from(call)
            .expect("call index fits u64")
            .checked_sub(L1_LOOKAHEAD)
        else {
            initial_latency_blocks.push(json!({
                "call": call_u32,
                "left_silent": ieee_silent(&out_left),
                "right_silent": ieee_silent(&out_right),
                "reason": "inside L1 lookahead; no submitted source block exists",
            }));
            continue;
        };
        let source_phase = usize::try_from(source / phase_calls).expect("source phase fits usize");
        let spec = phases[source_phase];
        let observed = &mut cases[source_phase];
        observe_matched_block(&spec, observed, &out_left, &out_right, call_u32, source);
    }

    validate_case_delivery(phases, &cases);

    let freshness = json!({
        "latency_frames": 1024,
        "lookahead_blocks": L1_LOOKAHEAD,
        "frames_per_block": FRAMES,
        "source_attribution": "a matched output at call c is the child rendering of submitted call c - lookahead_blocks; substituted/pass-through output is excluded",
        "initial_latency_blocks": initial_latency_blocks,
        "substituted_pass_through_blocks": substituted_pass_through_blocks,
        "dropped_submit_calls": dropped_submit_calls,
    });
    (cases, freshness)
}

fn transport_json(transport: TransportInfo) -> Value {
    let musical = transport.musical();
    let loop_range = transport.loop_range();
    json!({
        "status": format!("{:?}", transport.status()),
        "media_position_frames": transport.position().value(),
        "musical": musical.map(|musical| json!({
            "position_ticks": musical.position_ticks,
            "bar_start_tick": musical.bar_start_tick,
            "bpm_milli": musical.bpm_milli,
            "ppqn": musical.ppqn,
            "sig_num": musical.sig_num,
            "sig_den": musical.sig_den,
        })),
        "loop_range": loop_range.map(|range| json!({
            "start_frame": range.start().value(),
            "duration_frames": range.duration().value(),
            "end_frame": range.end().value(),
        })),
    })
}

fn case_json(spec: &PhaseSpec, observed: &CaseObservations) -> Value {
    let click_evidence = spec.click.map(|(input, expected)| {
        json!({
            "independent_calculation": input.evidence(expected),
            "observed_click_frames": observed.observed_click_frames,
            "observed_amplitudes": observed.observed_amplitudes,
        })
    });
    json!({
        "case": spec.name,
        "input_transport": transport_json(spec.transport),
        "negative_control": spec.negative_control,
        "position_jump_case": spec.position_jump,
        "expected_click_frame": spec.click.map(|(_, frame)| frame),
        "click_evidence": click_evidence,
        "matched_source_calls": observed.matched_sources,
        "click_blocks": observed.click_blocks,
        "silent_blocks": observed.silent_blocks,
        "stereo_identical_blocks": observed.stereo_identical_blocks,
        "non_click_silent_blocks": observed.non_click_silent_blocks,
        "verdict": "pass",
    })
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn command_text(program: &str, args: &[&str]) -> String {
    let output = ProcessCommand::new(program)
        .args(args)
        .current_dir(workspace_root())
        .output()
        .unwrap_or_else(|error| panic!("failed to run {program}: {error}"));
    assert!(
        output.status.success(),
        "{program} {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("command output must be utf-8")
        .trim()
        .to_owned()
}

fn git_fact(args: &[&str]) -> String {
    command_text("git", args)
}

fn rustc_version() -> String {
    command_text("rustc", &["--version"])
}

fn artifact_json(
    probe_format: &str,
    fixture_path: &Path,
    fixture_sha256: &str,
    runner_env: &str,
    runner_path: &Path,
    staged_fixture: &Path,
) -> Value {
    let (fixture_size, _) = file_facts(fixture_path);
    let (runner_size, runner_sha256) = file_facts(runner_path);
    let (staged_size, staged_sha256) = file_facts(staged_fixture);
    json!({
        "fixture": {
            "path": fixture_path,
            "size": fixture_size,
            "sha256": fixture_sha256,
        },
        "runner": {
            "path": runner_path,
            "size": runner_size,
            "sha256": runner_sha256,
            "resolution_env": runner_env,
        },
        "staged_fixture": {
            "path": staged_fixture,
            "size": staged_size,
            "sha256": staged_sha256,
        },
        "ruviform_resolved_revision": ruviform_revision(),
        "format": probe_format,
    })
}

fn upstream_gap_json() -> Value {
    json!({
        "confirmed": true,
        "ruviform_issue_url": RUVIFORM_ISSUE,
        "file": UPSTREAM_FILE,
        "line": UPSTREAM_LINE,
        "resolved_source": "call_transport_info(&span.ctx, rel, None, None)",
        "impact": "PluginNode does not pass a tempo map or PPQN to call_transport_info, so production graph end-to-end musical transport does not reach the plugin",
        "production_end_to_end_musical_transport": "NotRun/Blocked upstream",
        "synvelis_permanent_workaround": false,
    })
}

#[derive(Clone, Copy)]
struct EvidenceInput<'a> {
    probe: &'a common::tim03_l1::FixtureProbe,
    instrument: &'a PluginDescriptor,
    runner_env: &'a str,
    runner_path: &'a Path,
    staged_fixture: &'a Path,
    scan_visible: bool,
    freshness: &'a Value,
    phases: &'a [PhaseSpec],
    cases: &'a [CaseObservations],
}

fn evidence_json(input: EvidenceInput<'_>) -> Value {
    let probe = input.probe;
    let instrument = input.instrument;
    let runner_env = input.runner_env;
    let runner_path = input.runner_path;
    let staged_fixture = input.staged_fixture;
    let scan_visible = input.scan_visible;
    let freshness = input.freshness;
    let phases = input.phases;
    let cases = input.cases;
    let case_jsons: Vec<Value> = phases
        .iter()
        .zip(cases)
        .map(|(spec, observed)| case_json(spec, observed))
        .collect();
    let artifacts = artifact_json(
        probe.format,
        &probe.path,
        &probe.sha256,
        runner_env,
        runner_path,
        staged_fixture,
    );
    json!({
        "schema": "tim03-l1-instrument-transport-oracle/v1",
        "format": probe.format,
        "test": format!("tim03_l1_instrument_transport::{}_instrument_transport_clicks", probe.format),
        "target": {
            "branch": git_fact(&["branch", "--show-current"]),
            "head": git_fact(&["rev-parse", "HEAD"]),
        },
        "environment": {
            "os": env::consts::OS,
            "arch": env::consts::ARCH,
            "toolchain": rustc_version(),
        },
        "artifacts": artifacts,
        "descriptor": {
            "id": instrument.id,
            "name": instrument.name,
            "vendor": instrument.vendor,
            "version": instrument.version,
            "features": instrument.features,
            "scan_visible": scan_visible,
            "descriptor_count_in_fixture_scan": probe.descriptors.len(),
        },
        "lifecycle": {
            "scan": "pass",
            "instantiate": "pass",
            "initialize": "pass",
            "activate": "pass",
            "start_processing": "pass",
            "endpoint_prepare": "pass",
            "endpoint_drop": "pass",
            "stop_processing": "pass",
            "deactivate": "pass",
            "destroy": "pass",
        },
        "latency": freshness,
        "cases": case_jsons,
        "negative_controls": {
            "stopped": "pass",
            "paused": "pass",
            "missing_musical": "pass",
            "meaning": "missing musical proves the fixture does not invent musical timing and the audio oracle cannot false-positive when the upstream handoff is absent; it does not pass the production graph path",
        },
        "upstream_graph_handoff_gap": upstream_gap_json(),
        "production_end_to_end_musical_transport": "NotRun/Blocked upstream",
        "not_run": [
            "production composition → PluginNode → plugin end-to-end musical transport (blocked by upstream handoff gap)",
            "dynamic tempo edit matrix, full frame/boundary matrix, automation parameter matrix expansion",
            "test_fault/test_stateful, crash/hang/timeout isolation, State/Unload, RT/Crash final acceptance",
            "GUI/Native Dock/EXT-L3, provisioned endpoints, macOS/Linux runtime, 3OS CI, AudioLens cross-consumer",
        ],
        "commands": [
            "cargo build --locked -p synvelis-plugin-test-l1-runner",
            "cargo test --locked -p synvelis-app --test tim03_l1_instrument_transport -- --test-threads=1 --nocapture",
        ],
    })
}

fn exercise_format(probe: &common::tim03_l1::FixtureProbe, runner_env: &str, runner_stem: &str) {
    assert!(probe.path.is_file());
    let instrument = descriptor_named(probe, TEST_INSTRUMENT);
    let runner = runner_program(runner_env, runner_stem);
    let runner_path = PathBuf::from(&runner);
    let staged_fixture = stage_fixture_sibling(probe, &runner_path);
    assert_eq!(
        file_facts(&staged_fixture).1,
        probe.sha256,
        "staged fixture hash must match the resolved fixture"
    );

    let mut host = PluginHost::new();
    host.register_out_of_process(OutOfProcessSpec::new(instrument, runner))
        .expect("register instrument L1 route");
    let scanned = match host.execute(HostCommand::Scan).expect("scan") {
        HostReply::Scanned(descriptors) => descriptors,
        reply => panic!("scan returned unexpected reply: {reply:?}"),
    };
    let scan_visible = scanned
        .iter()
        .any(|descriptor| descriptor.id == instrument.id);
    assert!(scan_visible, "scan does not expose test_instrument");
    let handle = instantiate(&mut host, instrument.id);
    drive_to_processing(&mut host, handle);
    assert_eq!(host.latency_frames(handle), Some(1024));
    let mut endpoint = host
        .prepare_endpoint(
            handle,
            EndpointBounds {
                max_frames: FRAMES_U32,
                channels: 2,
            },
        )
        .expect("instrument L1 endpoint attach must succeed");
    assert_eq!(endpoint.latency_frames(), 1024);

    let phases = phase_specs();
    let (cases, freshness) = observe_transport_phases(&host, handle, &mut endpoint, &phases);

    drop(endpoint);
    execute_ok(
        &mut host,
        HostCommand::StopProcessing { handle },
        "stop processing",
    );
    execute_ok(&mut host, HostCommand::Deactivate { handle }, "deactivate");
    execute_ok(&mut host, HostCommand::Destroy { handle }, "destroy");
    assert_eq!(host.lifecycle_state(handle), None);

    println!(
        "tim03-l1-instrument-transport {}",
        evidence_json(EvidenceInput {
            probe,
            instrument: &instrument,
            runner_env,
            runner_path: &runner_path,
            staged_fixture: &staged_fixture,
            scan_visible,
            freshness: &freshness,
            phases: &phases,
            cases: &cases,
        })
    );
}

#[test]
fn clap_instrument_transport_clicks() {
    let (probe, _library) = load_clap();
    exercise_format(&probe, CLAP_RUNNER_ENV, CLAP_RUNNER_STEM);
}

#[test]
fn vst3_instrument_transport_clicks() {
    let (probe, _library) = load_vst3();
    exercise_format(&probe, VST3_RUNNER_ENV, VST3_RUNNER_STEM);
}
