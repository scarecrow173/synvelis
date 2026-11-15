//! RCD-01 `rf#628-fixture` consumer oracle slice (synvelis#115):
//! `ruviform-audio-test` の deterministic scripted backend
//! （`ScriptedBackend`/`PcmScenario`/`StreamFault`/`oracle`）を本番
//! `InputSession`/`InputAdapter` → `StreamWriter` → staging media +
//! journal → `commit_session` → save → reload 経路へ通し、独立
//! `pcm_reference_block` oracle で sample/byte fidelity・fault・
//! verified prefix を実測する consumer-side oracle。
//!
//! これは **deterministic-backend headless lane のみ**の証拠であり、
//! 実 OS audio device・WASAPI callback・GUI・physical hardware の
//! 受入を一切含意しない（`ruviform-audio-test` lib.rs の test-only
//! boundary と同じ語彙）。`scarecrow173/ruviform#629` 仮想 endpoint
//! EXT-L3・physical・GUI lane は `evidence/sv115/` の case matrix で
//! NotRun/NotRequired のまま維持する。
//!
//! 期待値は `ruviform_audio_test::oracle::pcm_reference_block` —
//! fixture 生成側（`PcmFixture::fill_channel`）を呼ばない独立再実装
//! — で構成し、`pcm_assert_eq`（`Exact`/`AbsEpsilon`）で照合する。
//! `Sine`/`Sweep` は transcendental のため `AbsEpsilon(1e-5)`
//! （ruviform 側 `tests/pipeline_input.rs` の `SINE_EPS` と同値）、
//! それ以外の pattern は `Exact`（bit 一致）。

#[allow(dead_code)]
mod common;

use std::path::PathBuf;
use std::sync::Arc;

use common::{
    RealDeviceRig, StreamCapture, all_journal_spans, assert_prefix_contiguous,
    assert_session_coverage, assert_source_contiguous, capture_of, layout_for, plan_for,
    span_frame_total, tap, tmp_dir,
};
use ruviform_audio_core::device::{SupportedBufferSize, SupportedInputFormat};
use ruviform_audio_core::{AudioFormat, SampleFormat, SampleRate};
use ruviform_audio_test::backend::{
    DeviceSpec, InputSpec, PumpReport, ScriptedBackend, ScriptedDeviceHandle, StreamFault,
    StreamScript,
};
use ruviform_audio_test::clock::{
    JitterSpec, VirtualClock, VirtualClockConfig, VirtualClockDriver,
};
use ruviform_audio_test::oracle::{PcmCompare, pcm_assert_eq, pcm_reference_block};
use ruviform_audio_test::pcm::{PcmFixture, PcmPattern, PcmScenario, PcmSegment};
use ruviform_media_core::ClockDomainId;
use sha2::Digest as _;
use synvelis_project::{
    CaptureStreamId, ClockEpochId, Command, CounterIdGenerator, DirectoryBundleStore, LaneId,
    Project, ProjectController, ProjectId, ProjectOp, RecordingSessionId, SessionState, SourceId,
};
use synvelis_recording::{
    ArmState, ChannelMap, CommitOutcome, CommitReport, FAULT_EPOCH_DISCONTINUITY,
    FAULT_INPUT_DISCONTINUITY, FinalizedOutcome, FinalizedSourceRef, FinishReport, InputBindError,
    InputSession, InputStreamSpec, MonitorState, RecordingCommitManifest, RecordingOp, StagingArea,
    StreamEntry, SynvelisOp, commit_session, crc32, load_finalized, read_commit_manifest,
};

// ---------- deterministic fixture helpers ----------

const STOP_TICK: i64 = 200;
const BLOCK: u32 = 500;
const CALLBACK_QUEUE: usize = 16;
/// scenario の被覆終端。`session_frames`（tick 200 @ 48 kHz で約 5000
/// frame）を十分に超える固定値 — scenario は frame を事前生成しない
/// ため余剰被覆にコストはなく、delivery は `Frames` fault で
/// `session_end` に正確に揃える。
const SCENARIO_END: u64 = 16_384;
/// sine/sweep の oracle 許容誤差（ruviform `SINE_EPS` と同値）。
const SINE_EPS: f32 = 1e-5;

/// 48 kHz / 指定 epoch / drift・jitter 無しの deterministic clock。
fn test_clock(epoch: u64) -> Arc<VirtualClock> {
    Arc::new(
        VirtualClock::new(VirtualClockConfig {
            sample_rate: 48_000,
            epoch,
            offset_ticks: 0,
            drift_ppm: 0,
            jitter: JitterSpec::DISABLED,
        })
        .expect("virtual clock"),
    )
}

/// `channels` ch・48 kHz の F32 fixture。
fn fixture(channels: u16, pattern: PcmPattern) -> PcmFixture {
    PcmFixture::new(SampleRate::new(48_000).expect("rate"), channels, pattern).expect("fixture")
}

/// 単一 segment の連続 scenario（`[0, SCENARIO_END)`）。
fn contiguous_scenario(channels: u16, pattern: PcmPattern) -> PcmScenario {
    PcmScenario::new(vec![PcmSegment {
        frame_start: 0,
        frame_len: SCENARIO_END,
        fixture: fixture(channels, pattern),
    }])
    .expect("scenario")
}

/// `channels` ch device の `InputSpec`（stereo/mono/multichannel 共通）。
fn input_spec_for(
    channels: u32,
    scenario: PcmScenario,
    default_frames: u32,
    faults: Vec<StreamFault>,
) -> InputSpec {
    let mut stream = StreamScript::new(default_frames);
    stream.faults = faults;
    InputSpec {
        default_format: AudioFormat {
            sample_rate: SampleRate::new(48_000).expect("rate"),
            layout: layout_for(channels),
            sample_format: SampleFormat::F32,
        },
        supported_formats: vec![SupportedInputFormat {
            channels,
            min_sample_rate: 44_100,
            max_sample_rate: 96_000,
            buffer_size: SupportedBufferSize::Unknown,
            sample_format: SampleFormat::F32,
        }],
        pcm: scenario,
        stream,
        capture_reporting: true,
    }
}

/// input-only `DeviceSpec`（`clock` は backend 共有 clock への override）。
fn device_spec(
    name: &str,
    channels: u32,
    scenario: PcmScenario,
    clock: Option<Arc<VirtualClock>>,
    faults: Vec<StreamFault>,
) -> DeviceSpec {
    DeviceSpec {
        name: name.to_owned(),
        id: Some(format!("{name}-id")),
        clock,
        output: None,
        input: Some(input_spec_for(channels, scenario, BLOCK, faults)),
    }
}

/// `pattern` に対応する oracle 比較 mode（transcendental のみ epsilon）。
fn compare_mode(pattern: &PcmPattern) -> PcmCompare {
    match pattern {
        PcmPattern::Sine { .. } | PcmPattern::Sweep { .. } => PcmCompare::AbsEpsilon(SINE_EPS),
        _ => PcmCompare::Exact,
    }
}

// ---------- e2e rig helpers（`rcd01_oracle.rs` と同構造） ----------

/// 1 scripted device 分の spec（device channel 数・PCM scenario・armed tap 列）。
struct Rf628Input {
    channels: u32,
    scenario: PcmScenario,
    taps: Vec<(u16, ChannelMap)>,
    faults: Vec<StreamFault>,
    clock: Option<Arc<VirtualClock>>,
}

/// record→finish まで完了した scripted session の成果物一式。
struct Rf628Run {
    root: PathBuf,
    controller: ProjectController,
    session_id: RecordingSessionId,
    area: StagingArea,
    fin: FinishReport,
    session_frames: i64,
    /// device ordinal → pump report 列（fixture metadata/case identity 検証用）。
    reports: Vec<Vec<PumpReport>>,
    /// `stream_index` → 出力 channel → 入力 channel 写像。
    maps: Vec<Vec<u32>>,
    /// `stream_index` → 期待 PCM scenario。
    scenarios: Vec<PcmScenario>,
    /// drive 終了時点の session fault latch bit（fault 分類の直接検証用）。
    fault_bits: u32,
}

/// track/lane を 1 対 1 で `n` 個ずつ作る controller（multi-track 検証用）。
fn controller_with_tracks(n: usize) -> (ProjectController, Vec<LaneId>, ClockEpochId) {
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "rf628-oracle"),
        Box::new(CounterIdGenerator::new(1)),
    );
    let mut lanes = Vec::with_capacity(n);
    for i in 0..n {
        c.apply(Command {
            expected_revision: c.live_revision(),
            op: ProjectOp::AddTrack {
                name: format!("rf628-t{i}"),
            },
        })
        .expect("add track");
        let track = *c.live().tracks.keys().last().expect("track");
        c.apply(Command {
            expected_revision: c.live_revision(),
            op: ProjectOp::AddLane { track_id: track },
        })
        .expect("add lane");
        lanes.push(*c.live().lanes.keys().last().expect("lane"));
    }
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::RegisterClockEpoch {
            description: "rf628".into(),
        },
    })
    .expect("register epoch");
    let epoch = *c.live().clock_epochs.keys().next().expect("epoch");
    (c, lanes, epoch)
}

fn rop(c: &mut ProjectController, op: RecordingOp) {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: SynvelisOp::Recording(op),
    })
    .expect("recording op");
}

/// session 作成 → Open → lane 数分の stream 登録。
fn open_session_ids(
    c: &mut ProjectController,
    lanes: &[LaneId],
    epoch: ClockEpochId,
) -> (RecordingSessionId, Vec<CaptureStreamId>) {
    rop(
        c,
        RecordingOp::CreateSession {
            name: "rf628-session".into(),
        },
    );
    let session_id = *c.live().recording.sessions.keys().next().expect("session");
    rop(c, RecordingOp::OpenSession { session_id });
    let mut ids = Vec::with_capacity(lanes.len());
    for &lane in lanes {
        rop(
            c,
            RecordingOp::RegisterStream {
                session_id,
                lane_id: lane,
                clock_epoch_id: epoch,
            },
        );
        ids.push(*c.live().recording.streams.keys().last().expect("stream"));
    }
    (session_id, ids)
}

/// scripted device → `RealDeviceRig`（`&dyn Device` 経路）→ drive →
/// finish までを実行する。`drive` は `(rig, device handles,
/// clock driver, session_frames)` を受けて pump/fault を制御する。
/// 戻り値の `reports` は device ordinal ごとの `PumpReport` 列。
fn rf628_record(
    name: &str,
    clock: &Arc<VirtualClock>,
    inputs: &[Rf628Input],
    drive: impl FnOnce(
        &mut RealDeviceRig,
        &[ScriptedDeviceHandle],
        Option<&mut VirtualClockDriver>,
        i64,
    ) -> Vec<Vec<PumpReport>>,
) -> Rf628Run {
    rf628_record_with_pre_open(name, clock, inputs, drive, |_, _| {})
}

/// `rf628_record` に rig open 前 hook を加えた版。`pre_open` は
/// scripted device/spec の構築後・`RealDeviceRig::open`（内部で
/// `InputSession::open`/`add`）の直前に 1 度だけ呼ばれ、open fault
/// の注入→defined rejection→clear→re-open を rig open 前に
/// 仕込める（SV-7 の open-fault recovery 経路用）。hook が
/// rejection を確認した後、rig open 内の `InputSession::open` が
/// recovery 後の re-open を兼ね、その session がそのまま
/// record に使われる。
fn rf628_record_with_pre_open(
    name: &str,
    clock: &Arc<VirtualClock>,
    inputs: &[Rf628Input],
    drive: impl FnOnce(
        &mut RealDeviceRig,
        &[ScriptedDeviceHandle],
        Option<&mut VirtualClockDriver>,
        i64,
    ) -> Vec<Vec<PumpReport>>,
    pre_open: impl FnOnce(&[ScriptedDeviceHandle], &[InputStreamSpec]),
) -> Rf628Run {
    let n_streams = inputs.iter().map(|s| s.taps.len()).sum::<usize>();
    assert!(n_streams > 0, "at least one armed tap required");
    let mut seen = vec![false; n_streams];
    for spec in inputs {
        for &(si, _) in &spec.taps {
            let slot = usize::from(si);
            assert!(slot < n_streams && !seen[slot], "bad stream_index {si}");
            seen[slot] = true;
        }
    }
    assert!(seen.iter().all(|&x| x), "uncovered stream_index");

    let root = tmp_dir(name);
    let (mut c, lanes, epoch) = controller_with_tracks(n_streams);
    let (session_id, stream_ids) = open_session_ids(&mut c, &lanes, epoch);

    let mut target_channels = vec![0_u32; n_streams];
    for spec in inputs {
        for &(si, ref map) in &spec.taps {
            target_channels[usize::from(si)] = u32::try_from(map.len()).expect("channel map len");
        }
    }
    let plan = plan_for(c.live(), &lanes, &target_channels, epoch, STOP_TICK);
    let session_frames = plan.session_end.value();

    let mut backend = ScriptedBackend::new(Arc::clone(clock));
    let mut specs: Vec<InputStreamSpec> = Vec::with_capacity(inputs.len());
    let mut devices: Vec<ScriptedDeviceHandle> = Vec::with_capacity(inputs.len());
    for spec in inputs {
        specs.push(InputStreamSpec {
            config: common::input_config(spec.channels),
            clock_epoch: epoch,
            max_block_frames: plan.max_block_frames,
            callback_queue: CALLBACK_QUEUE,
            taps: spec
                .taps
                .iter()
                .map(|&(si, ref map)| tap(&plan, si, map.clone(), ArmState::On, MonitorState::Off))
                .collect(),
        });
        devices.push(
            backend
                .add_device(device_spec(
                    "rf628",
                    spec.channels,
                    spec.scenario.clone(),
                    spec.clock.clone(),
                    spec.faults.clone(),
                ))
                .expect("add scripted device"),
        );
    }
    pre_open(&devices, &specs);
    let dyn_inputs: Vec<(&dyn ruviform_audio_core::device::Device, &InputStreamSpec)> = devices
        .iter()
        .zip(specs.iter())
        .map(|(d, s)| (d as &dyn ruviform_audio_core::device::Device, s))
        .collect();
    let mut rig = RealDeviceRig::open(
        &root,
        session_id,
        plan,
        common::RATE,
        &dyn_inputs,
        Some(stream_ids.as_slice()),
    );

    rig.start_all().expect("start_all");
    let mut driver = VirtualClockDriver::new(Arc::clone(clock));
    let reports = drive(&mut rig, &devices, Some(&mut driver), session_frames);
    let fault_bits = rig.session.fault().bits();
    let fin = rig.finish();

    // stream_index → channel map / scenario を解決する。
    let mut maps: Vec<Vec<u32>> = vec![Vec::new(); n_streams];
    let mut scenarios: Vec<Option<PcmScenario>> = vec![None; n_streams];
    for spec in inputs {
        for &(si, ref map) in &spec.taps {
            let slot = usize::from(si);
            maps[slot] = map.channels().to_vec();
            scenarios[slot] = Some(spec.scenario.clone());
        }
    }
    Rf628Run {
        root,
        controller: c,
        session_id,
        area: rig.area.clone(),
        fin,
        session_frames,
        reports,
        maps,
        scenarios: scenarios
            .into_iter()
            .map(|o| o.expect("scenario"))
            .collect(),
        fault_bits,
    }
}

/// `session_frames` を `BLOCK` 刻みで全 device に正確供給する正常系
/// drive。末尾が `BLOCK` 未満なら `StreamFault::Frames` でその
/// callback の frame 数を残りに一致させ、scenario 被覆と実配送範囲を
/// 一致させる。各 pump 後に clock driver を `report.frames` 進め、
/// `rig.pump_all` で RT→non-RT を drain する。
fn drive_session(
    rig: &mut RealDeviceRig,
    handles: &[ScriptedDeviceHandle],
    mut driver: Option<&mut VirtualClockDriver>,
    session_frames: i64,
) -> Vec<Vec<PumpReport>> {
    let total = u64::try_from(session_frames).expect("session frames");
    let mut reports: Vec<Vec<PumpReport>> = vec![Vec::new(); handles.len()];
    let mut delivered = 0_u64;
    let mut callback_index = 0_u64;
    while delivered < total {
        let remaining = total - delivered;
        let n = u64::from(BLOCK).min(remaining);
        if n < u64::from(BLOCK) {
            for h in handles {
                h.inject_input_fault(StreamFault::Frames {
                    at_callback: callback_index,
                    frames: u32::try_from(n).expect("tail frames"),
                });
            }
        }
        for (i, h) in handles.iter().enumerate() {
            let report = h.pump_input_next().expect("pump_input_next");
            reports[i].push(report);
        }
        // session 内の全 device は同一 clock domain を共有するため、
        // 共有 `VirtualClock` は callback period あたり 1 回だけ進める
        // （device 数分進めると cadence が水増しされる）。
        if let Some(d) = driver.as_mut() {
            d.advance_frames(n);
        }
        rig.pump_all();
        delivered += n;
        callback_index += 1;
    }
    reports
}

/// `CloseSession` → `commit_session` → save → 別 controller で load。
fn rf628_commit_reload(run: &mut Rf628Run) -> (CommitReport, ProjectController) {
    rop(
        &mut run.controller,
        RecordingOp::CloseSession {
            session_id: run.session_id,
        },
    );
    let report = commit_session(&mut run.controller, &run.area).expect("commit_session");
    let mut store = DirectoryBundleStore::create(run.root.join("bundle")).expect("bundle store");
    run.controller.save(&mut store).expect("save");
    let mut c2 = ProjectController::new(
        Project::new(ProjectId::new(0), "rf628-reload"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c2.load(&store).expect("load");
    (report, c2)
}

// ---------- oracle 検証 ----------

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", sha2::Sha256::digest(bytes))
}

/// journal/media の記録済み bytes を scenario の独立 oracle
/// （`pcm_reference_block`）と照合する。span が跨ぐ segment ごとに
/// 期待 run を生成し、`Sine`/`Sweep` は `AbsEpsilon`、他は `Exact`
/// で `pcm_assert_eq` する。各 chunk の `pcm_crc32` も検算する。
/// 戻り値は照合した出力 sample 数。
fn verify_scenario_media(
    cap: &StreamCapture,
    channel_map: &[u32],
    scenario: &PcmScenario,
) -> usize {
    let out_ch = channel_map.len();
    let mut checked = 0usize;
    for chk in &cap.chunks {
        let start = usize::try_from(chk.media_offset).expect("media offset");
        let end = start + usize::try_from(chk.media_len).expect("media len");
        assert_eq!(
            crc32(&cap.media[start..end]),
            chk.pcm_crc32,
            "chunk {} pcm_crc32 mismatch",
            chk.chunk_id
        );
        for sp in &chk.spans {
            let m_off = usize::try_from(sp.media_offset).expect("span offset") / 4;
            let input_base = chk
                .first_input_frame
                .wrapping_add(u64::try_from(sp.session_start - chk.session_start).unwrap_or(0));
            let span_end = input_base + u64::from(sp.frame_count);
            for (j, &src_ch) in channel_map.iter().enumerate() {
                let src_ch_u16 = u16::try_from(src_ch).expect("src channel");
                // span 内を segment 境界で分割し、各 run を独立 oracle で検算。
                let mut frame = input_base;
                while frame < span_end {
                    let seg = scenario
                        .segment_at(frame)
                        .unwrap_or_else(|| panic!("no scenario segment at frame {frame}"));
                    let seg_end = seg.frame_start + seg.frame_len;
                    let run_end = span_end.min(seg_end);
                    let run_len = u32::try_from(run_end - frame).expect("run len");
                    let want = pcm_reference_block(&seg.fixture, frame, run_len, src_ch_u16);
                    // media は出力 interleave — channel j の sample を gather する。
                    let f0 = usize::try_from(frame - input_base).expect("span frame");
                    let mut got = Vec::with_capacity(usize::try_from(run_len).expect("run len"));
                    for f in f0..f0 + usize::try_from(run_len).expect("run len") {
                        let idx = m_off + f * out_ch + j;
                        let bytes = cap.media.get(idx * 4..idx * 4 + 4).expect("pcm bytes");
                        got.push(f32::from_le_bytes(bytes.try_into().expect("f32 le")));
                    }
                    pcm_assert_eq(&got, &want, compare_mode(&seg.fixture.pattern)).unwrap_or_else(
                        |e| {
                            panic!(
                                "scenario pcm mismatch chunk {} frame {frame} ch {src_ch}: {e}",
                                chk.chunk_id
                            )
                        },
                    );
                    checked += got.len();
                    frame = run_end;
                }
            }
        }
    }
    checked
}

/// `stream_index` の finalized ref を読む。
fn finalized_for(run: &Rf628Run, stream_index: u16) -> FinalizedSourceRef {
    load_finalized(&run.area)
        .into_iter()
        .find(|f| f.stream_index == stream_index)
        .and_then(|f| f.finalized)
        .unwrap_or_else(|| panic!("stream {stream_index} finalized ref"))
}

/// commit manifest の `stream_index` エントリを引く。
fn manifest_entry(manifest: &RecordingCommitManifest, stream_index: u16) -> &StreamEntry {
    manifest
        .payload
        .streams
        .iter()
        .find(|e| e.stream_index == stream_index)
        .unwrap_or_else(|| panic!("manifest entry for stream {stream_index}"))
}

/// 1 stream の完全な oracle 監査: journal media の scenario oracle
/// 照合 + session/source coverage + finalized ref の
/// `verified_sha256` 独立再計算 + commit manifest↔project entity の
/// digest 連鎖。`project` は commit 直後または reload 後の controller。
fn assert_scenario_stream(run: &Rf628Run, project: &ProjectController, stream_index: u16) {
    let cap = capture_of(&run.area, stream_index);
    let map = &run.maps[usize::from(stream_index)];
    let scenario = &run.scenarios[usize::from(stream_index)];
    let checked = verify_scenario_media(&cap, map, scenario);
    assert!(checked > 0, "stream {stream_index} verified no samples");

    let spans = all_journal_spans(&cap);
    assert_session_coverage(&spans, &[(0, run.session_frames)]);
    assert_source_contiguous(&spans, run.session_frames);
    assert_eq!(span_frame_total(&spans), run.session_frames);

    let fin = finalized_for(run, stream_index);
    assert_eq!(fin.outcome, FinalizedOutcome::Complete);
    assert_eq!(
        fin.source_frames,
        u64::try_from(run.session_frames).expect("session frames")
    );
    let prefix = &cap.media[..usize::try_from(fin.media_byte_len).expect("media len")];
    let expect_sha = sha256_prefixed(prefix);
    assert_eq!(
        fin.verified_sha256.as_deref(),
        Some(expect_sha.as_str()),
        "stream {stream_index} verified_sha256"
    );

    let manifest = read_commit_manifest(&run.area)
        .expect("manifest read")
        .expect("commit manifest");
    let entry = manifest_entry(&manifest, stream_index);
    assert_eq!(entry.digest, fin.digest, "stream {stream_index} digest");
    assert_eq!(
        entry.verified_sha256.as_deref(),
        fin.verified_sha256.as_deref(),
        "stream {stream_index} verified_sha256 propagation"
    );
    assert_eq!(entry.media_byte_len, fin.media_byte_len);
    let source_id = SourceId::from_tagged(&entry.source_id).expect("source id");
    let source = project
        .live()
        .sources
        .get(&source_id)
        .unwrap_or_else(|| panic!("source for stream {stream_index}"));
    assert_eq!(source.content_digest, entry.digest);
    let asset = project.live().assets.get(&source.asset_id).expect("asset");
    assert_eq!(asset.digest, entry.digest);
    assert_eq!(asset.digest, source.content_digest);
}

/// 全 stream の finalize が `Complete` であることを確認する。
fn assert_finish_complete(run: &Rf628Run) {
    for (si, r) in &run.fin.refs {
        let fin = r
            .as_ref()
            .unwrap_or_else(|e| panic!("stream {si} finalize: {e:?}"));
        assert_eq!(
            fin.outcome,
            FinalizedOutcome::Complete,
            "stream {si} outcome"
        );
    }
}

/// faulted session の commit→reload 検証: `NothingCommitted` +
/// 全 stream recoverable（`capture-fault` 分類・Source 非昇格）+
/// journal verified prefix の scenario oracle bit 一致 +
/// `verified_sha256` pin。`expected_prefix` は journal が保持すべき
/// 先頭 frame 数。
fn assert_faulted_prefix(run: &Rf628Run, c2: &mut ProjectController, expected_prefix: i64) {
    assert!(run.area.capture_fault_marker_exists());
    for si in 0..u16::try_from(run.maps.len()).expect("stream count") {
        let cap = capture_of(&run.area, si);
        let spans = all_journal_spans(&cap);
        assert_prefix_contiguous(&spans);
        assert_eq!(
            span_frame_total(&spans),
            expected_prefix,
            "stream {si} verified prefix length"
        );
        let checked = verify_scenario_media(
            &cap,
            &run.maps[usize::from(si)],
            &run.scenarios[usize::from(si)],
        );
        assert!(checked > 0, "stream {si} prefix verified no samples");
        let fin = finalized_for(run, si);
        assert_eq!(fin.outcome, FinalizedOutcome::Complete);
        // commit 面の artifact である finalized ref 自体も pin する
        // （journal/span 検証だけでは finalize 報告値の誤りを検出できない）。
        assert_eq!(
            fin.source_frames,
            u64::try_from(expected_prefix).expect("prefix frames"),
            "stream {si} finalized source_frames"
        );
        let channels = u64::try_from(run.maps[usize::from(si)].len()).expect("channels");
        assert_eq!(
            fin.media_byte_len,
            u64::try_from(expected_prefix).expect("prefix frames") * channels * 4,
            "stream {si} finalized media_byte_len"
        );
        let prefix = &cap.media[..usize::try_from(fin.media_byte_len).expect("media len")];
        assert_eq!(
            fin.verified_sha256.as_deref(),
            Some(sha256_prefixed(prefix).as_str()),
            "stream {si} prefix verified_sha256"
        );
    }
    assert!(run.controller.live().sources.is_empty());
    assert!(c2.live().sources.is_empty());
    assert_eq!(
        run.controller.live().recording.sessions[&run.session_id].state,
        SessionState::Closed,
        "nothing-committed session stays Closed"
    );
}

// ---------- tests ----------

/// SV-1: stereo PCM E2E — 3 segment scenario（Noise→Impulse→Sine）を
/// scripted backend 経由で記録し、commit→reload→digest 連鎖まで
/// 独立 oracle で照合する。`PumpReport` の metadata（direction/
/// `callback_index`/`position`/`clock_domain`/`gap`/`errors`）も検証する。
#[test]
fn rf628_stereo_scenario_commit_reload() {
    let clock = test_clock(7);
    let scenario = PcmScenario::new(vec![
        PcmSegment {
            frame_start: 0,
            frame_len: 1024,
            fixture: fixture(
                2,
                PcmPattern::Noise {
                    seed: 11,
                    amplitude: 0.8,
                },
            ),
        },
        PcmSegment {
            frame_start: 1024,
            frame_len: 1024,
            fixture: fixture(
                2,
                PcmPattern::Impulse {
                    period_frames: 64,
                    amplitude: 0.5,
                },
            ),
        },
        PcmSegment {
            frame_start: 2048,
            frame_len: SCENARIO_END - 2048,
            fixture: fixture(
                2,
                PcmPattern::Sine {
                    freq_hz: 440.0,
                    amplitude: 0.5,
                },
            ),
        },
    ])
    .expect("scenario");
    let input = Rf628Input {
        channels: 2,
        scenario,
        taps: vec![(0, ChannelMap::stereo(0, 1))],
        faults: Vec::new(),
        clock: None,
    };
    let mut run = rf628_record("sv1-stereo", &clock, &[input], drive_session);
    assert_finish_complete(&run);

    // fixture metadata / case identity: 全 callback が Input direction・
    // 連番 index・同一 clock domain・gap/error 無しで発火したこと。
    let reports = &run.reports[0];
    assert!(!reports.is_empty());
    let mut expected_position = 0_u64;
    for (i, r) in reports.iter().enumerate() {
        assert_eq!(
            r.direction,
            ruviform_audio_test::backend::StreamDirection::Input
        );
        assert_eq!(r.callback_index, i as u64);
        assert_eq!(r.position, expected_position);
        assert_eq!(r.reported_frame_position, Some(expected_position));
        assert_eq!(r.clock_domain, ClockDomainId::new(7));
        assert_eq!(r.gap_frames, 0, "callback {i} unexpected gap");
        assert_eq!(r.errors_fired, 0, "callback {i} unexpected errors");
        assert!(!r.invalidated);
        expected_position += u64::from(r.frames);
    }
    assert_eq!(
        expected_position,
        u64::try_from(run.session_frames).expect("session frames")
    );

    let (report, mut c2) = rf628_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 1);
    assert_eq!(report.recoverable.len(), 0);

    assert_scenario_stream(&run, &run.controller, 0);
    assert_scenario_stream(&run, &c2, 0);
    let recommit = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(recommit.outcome, CommitOutcome::AlreadyApplied));
}

/// SV-2: multichannel（4ch）device を 2 つの mono tap（ch 1 / ch 3）
/// に分岐 — 別 stream・別 staging media に per-channel Noise が
/// bit 単位で分離されることを commit→reload まで検証する。
#[test]
fn rf628_multichannel_two_mono_taps_commit_reload() {
    let clock = test_clock(7);
    let scenario = contiguous_scenario(
        4,
        PcmPattern::Noise {
            seed: 77,
            amplitude: 0.9,
        },
    );
    let input = Rf628Input {
        channels: 4,
        scenario,
        taps: vec![(0, ChannelMap::mono(1)), (1, ChannelMap::mono(3))],
        faults: Vec::new(),
        clock: None,
    };
    let mut run = rf628_record("sv2-multichan", &clock, &[input], drive_session);
    assert_finish_complete(&run);
    assert_ne!(run.area.media_path(0), run.area.media_path(1));

    let (report, mut c2) = rf628_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 2);
    assert_eq!(report.recoverable.len(), 0);

    for si in 0..2_u16 {
        assert_scenario_stream(&run, &run.controller, si);
        assert_scenario_stream(&run, &c2, si);
    }
    // tap 分離: ch1 と ch3 の PCM が bit 単位で異なる（Noise は
    // (seed, channel, frame) hash のため channel ごとに独立）。
    let m0 = read_commit_manifest(&run.area)
        .expect("manifest read")
        .expect("commit manifest");
    assert_ne!(m0.payload.streams[0].digest, m0.payload.streams[1].digest);
    let recommit = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(recommit.outcome, CommitOutcome::AlreadyApplied));
}

/// SV-3: same-clock 2 device（backend 共有 `VirtualClock`・別 Noise
/// seed）が `InputSession::add` の same-clock 検証を通り、別 stream に
/// 別 PCM が記録される。
#[test]
fn rf628_two_devices_same_clock_commit_reload() {
    let clock = test_clock(7);
    let mk = |seed: u64| Rf628Input {
        channels: 1,
        scenario: contiguous_scenario(
            1,
            PcmPattern::Noise {
                seed,
                amplitude: 0.9,
            },
        ),
        taps: vec![(0, ChannelMap::mono(0))],
        faults: Vec::new(),
        clock: None,
    };
    let (mut a, mut b) = (mk(11), mk(22));
    // stream_index を device ごとに分ける。
    b.taps = vec![(1, ChannelMap::mono(0))];
    a.taps = vec![(0, ChannelMap::mono(0))];
    let mut run = rf628_record("sv3-two-devices", &clock, &[a, b], drive_session);
    assert_finish_complete(&run);

    let (report, mut c2) = rf628_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 2);
    assert_eq!(report.recoverable.len(), 0);

    for si in 0..2_u16 {
        assert_scenario_stream(&run, &run.controller, si);
        assert_scenario_stream(&run, &c2, si);
    }
    let manifest = read_commit_manifest(&run.area)
        .expect("manifest read")
        .expect("commit manifest");
    assert_ne!(
        manifest.payload.streams[0].digest,
        manifest.payload.streams[1].digest
    );
    let recommit = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(recommit.outcome, CommitOutcome::AlreadyApplied));
}

/// SV-3 補遺: 別 `VirtualClock`（別 epoch → 別 `ClockDomainId`）の
/// device を `InputSession::add` すると `ClockDomainMismatch` で
/// defined rejection されること（same-clock 契約の負側）。
#[test]
fn rf628_clock_domain_mismatch_rejected() {
    let clock_a = test_clock(7);
    let clock_b = test_clock(8);
    let backend_clock = Arc::clone(&clock_a);
    let mut backend = ScriptedBackend::new(backend_clock);
    let scenario = || {
        contiguous_scenario(
            1,
            PcmPattern::Noise {
                seed: 1,
                amplitude: 0.5,
            },
        )
    };
    let dev_a = backend
        .add_device(device_spec("a", 1, scenario(), None, Vec::new()))
        .expect("device a");
    let dev_b = backend
        .add_device(device_spec("b", 1, scenario(), Some(clock_b), Vec::new()))
        .expect("device b");

    let (p, lanes, epoch) = controller_with_tracks(2);
    let plan = plan_for(p.live(), &lanes, &[1, 1], epoch, STOP_TICK);
    let spec_a = InputStreamSpec {
        config: common::input_config(1),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: CALLBACK_QUEUE,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::mono(0),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    let spec_b = InputStreamSpec {
        config: common::input_config(1),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: CALLBACK_QUEUE,
        taps: vec![tap(
            &plan,
            1,
            ChannelMap::mono(0),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    let (mut session, _ends) = InputSession::open(&dev_a, &spec_a).expect("session open");
    let Err(err) = session.add(&dev_b, &spec_b) else {
        panic!("mismatched clock domain must be rejected")
    };
    assert!(
        matches!(err, InputBindError::ClockDomainMismatch),
        "expected ClockDomainMismatch, got {err:?}"
    );
}

/// SV-4: `PcmScenario` の segment 間 gap（delivered as silence +
/// `Xrun`）で入力不連続 → fail-stop → journal verified prefix が
/// scenario oracle と bit 一致 + commit 層 recoverable 分類。
/// gap は callback 境界に揃え（`A_end = 4 * BLOCK`）、prefix =
/// `A_end` frame を `PumpReport.gap_frames`/`errors_fired` から
/// 独立に構成する。
#[test]
fn rf628_pcm_gap_verified_prefix() {
    let clock = test_clock(7);
    let gap_start = 4 * u64::from(BLOCK); // 2000 — callback 境界揃え
    let gap_end = gap_start + u64::from(BLOCK); // 500 frame の gap
    let scenario = PcmScenario::new(vec![
        PcmSegment {
            frame_start: 0,
            frame_len: gap_start,
            fixture: fixture(
                1,
                PcmPattern::Noise {
                    seed: 5,
                    amplitude: 0.9,
                },
            ),
        },
        PcmSegment {
            frame_start: gap_end,
            frame_len: SCENARIO_END - gap_end,
            fixture: fixture(
                1,
                PcmPattern::Impulse {
                    period_frames: 32,
                    amplitude: 0.4,
                },
            ),
        },
    ])
    .expect("scenario");
    let input = Rf628Input {
        channels: 1,
        scenario,
        taps: vec![(0, ChannelMap::mono(0))],
        faults: Vec::new(),
        clock: None,
    };
    let mut run = rf628_record("sv4-gap", &clock, &[input], drive_session);

    // fixture metadata: gap を含む callback が Xrun を報告し、
    // 以後の callback も gap_frames/errors_fired を観測する
    // （fail-stop は consumer 側 latch が担う）。
    let reports = &run.reports[0];
    let gap_cb = reports
        .iter()
        .position(|r| r.gap_frames > 0)
        .expect("a callback must report gap frames");
    assert_eq!(gap_cb, 4, "gap callback index");
    assert_eq!(reports[gap_cb].gap_frames, u64::from(BLOCK));
    assert!(reports[gap_cb].errors_fired >= 1, "Xrun must fire");
    assert!(run.session_frames > i64::try_from(gap_end).unwrap_or(i64::MAX));
    // Xrun → FAULT_INPUT_DISCONTINUITY に写像されていること。
    assert_ne!(
        run.fault_bits & FAULT_INPUT_DISCONTINUITY,
        0,
        "fault bits {:#06x} must include FAULT_INPUT_DISCONTINUITY",
        run.fault_bits
    );

    let (report, mut c2) = rf628_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::NothingCommitted));
    assert_eq!(report.sources_committed, 0);
    assert_eq!(report.recoverable.len(), 1);
    assert!(
        report.recoverable[0].reason.starts_with("capture-fault"),
        "recoverable reason: {}",
        report.recoverable[0].reason
    );

    assert_faulted_prefix(&run, &mut c2, i64::try_from(gap_start).expect("gap start"));
    let recommit = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(recommit.outcome, CommitOutcome::NothingCommitted));
}

/// SV-5a: `StreamFault::FramePosition`（position jump）で engine 側
/// の discontinuity 検出を起こし、verified prefix が jump 直前で
/// 打ち止めになることを検証する。
#[test]
fn rf628_frame_position_fault_verified_prefix() {
    let clock = test_clock(7);
    let jump_at = 6_u64;
    let jumped_position = jump_at * u64::from(BLOCK) + 300; // 3300
    let input = Rf628Input {
        channels: 1,
        scenario: contiguous_scenario(
            1,
            PcmPattern::Noise {
                seed: 9,
                amplitude: 0.9,
            },
        ),
        taps: vec![(0, ChannelMap::mono(0))],
        faults: vec![StreamFault::FramePosition {
            at_callback: jump_at,
            position: jumped_position,
        }],
        clock: None,
    };
    let mut run = rf628_record("sv5-framepos", &clock, &[input], drive_session);

    // jump が fixture report に反映されていること（position=3300）。
    let reports = &run.reports[0];
    assert_eq!(
        reports[usize::try_from(jump_at).expect("idx")].position,
        jumped_position
    );
    // engine 側 discontinuity 検出 → FAULT_INPUT_DISCONTINUITY。
    assert_ne!(
        run.fault_bits & FAULT_INPUT_DISCONTINUITY,
        0,
        "fault bits {:#06x} must include FAULT_INPUT_DISCONTINUITY",
        run.fault_bits
    );

    let (report, mut c2) = rf628_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::NothingCommitted));
    assert_eq!(report.recoverable.len(), 1);
    assert!(report.recoverable[0].reason.starts_with("capture-fault"));

    // prefix = jump callback 直前までの 6*500 = 3000 frame。
    assert_faulted_prefix(
        &run,
        &mut c2,
        i64::try_from(jump_at * u64::from(BLOCK)).expect("prefix"),
    );
}

/// SV-5b: `StreamFault::Error`（`ClockDiscontinuity`）が error
/// callback 経由で `FAULT_EPOCH_DISCONTINUITY` に写像され fail-stop
/// することを検証する（defined rejection が crash/`Failed` 昇格と
/// 混同されない）。
#[test]
fn rf628_clock_discontinuity_error_fault() {
    let clock = test_clock(7);
    let fault_at = 4_u64;
    let input = Rf628Input {
        channels: 1,
        scenario: contiguous_scenario(
            1,
            PcmPattern::Noise {
                seed: 3,
                amplitude: 0.9,
            },
        ),
        taps: vec![(0, ChannelMap::mono(0))],
        faults: vec![StreamFault::Error {
            at_callback: fault_at,
            error: ruviform_audio_core::device::DeviceError::ClockDiscontinuity,
        }],
        clock: None,
    };
    let mut run = rf628_record("sv5-errfault", &clock, &[input], drive_session);

    let reports = &run.reports[0];
    assert!(
        reports[usize::try_from(fault_at).expect("idx")].errors_fired >= 1,
        "scripted error must fire"
    );

    let (report, mut c2) = rf628_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::NothingCommitted));
    assert_eq!(report.recoverable.len(), 1);

    assert_faulted_prefix(
        &run,
        &mut c2,
        i64::try_from(fault_at * u64::from(BLOCK)).expect("prefix"),
    );
    // fault bit が epoch-discontinuity に分類されていることを
    // session fault latch の生値で直接確認する。
    assert_ne!(
        run.fault_bits & FAULT_EPOCH_DISCONTINUITY,
        0,
        "fault bits {:#06x} must include FAULT_EPOCH_DISCONTINUITY",
        run.fault_bits
    );
}

/// SV-5c: `OpenFaults` — persistent open error で `InputSession::open`
/// が defined rejection（`InputBindError::Device`）を返し、clear 後は
/// 同じ device が open できる（transient recovery の正直な分類）。
#[test]
fn rf628_open_fault_rejected_then_recovers() {
    let clock = test_clock(7);
    let mut backend = ScriptedBackend::new(Arc::clone(&clock));
    let scenario = contiguous_scenario(
        1,
        PcmPattern::Noise {
            seed: 1,
            amplitude: 0.5,
        },
    );
    let dev = backend
        .add_device(device_spec("open-fault", 1, scenario, None, Vec::new()))
        .expect("device");

    let (p, lanes, epoch) = controller_with_tracks(1);
    let plan = plan_for(p.live(), &lanes, &[1], epoch, STOP_TICK);
    let spec = InputStreamSpec {
        config: common::input_config(1),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: CALLBACK_QUEUE,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::mono(0),
            ArmState::On,
            MonitorState::Off,
        )],
    };

    dev.set_open_input_error(Some(ruviform_audio_core::device::DeviceError::Backend(
        "permission_denied",
    )));
    let Err(err) = InputSession::open(&dev, &spec) else {
        panic!("open must be rejected")
    };
    assert!(
        matches!(err, InputBindError::Device(_)),
        "expected InputBindError::Device, got {err:?}"
    );

    // transient fault を clear すれば同じ device/spec で open できる
    // （rejection が永続的破損でなく defined な一時失敗であること）。
    dev.set_open_input_error(None);
    let (session, _ends) = InputSession::open(&dev, &spec).expect("open after clear");
    drop(session);
}

/// SV-5d: device loss（hot-unplug 相当の `vanish`）で進行中 stream が
/// `StreamInvalidated` を受け fail-stop し、verified prefix が
/// 直前まで保持されることを検証する。
#[test]
fn rf628_device_vanish_fails_stop() {
    let clock = test_clock(7);
    let input = Rf628Input {
        channels: 1,
        scenario: contiguous_scenario(
            1,
            PcmPattern::Noise {
                seed: 21,
                amplitude: 0.9,
            },
        ),
        taps: vec![(0, ChannelMap::mono(0))],
        faults: Vec::new(),
        clock: None,
    };
    let vanish_at = 3_u64;
    let mut run = rf628_record(
        "sv5-vanish",
        &clock,
        &[input],
        move |rig, handles, mut driver, _total| {
            // 3 callback 分（1500 frame）を正常供給してから device を失う。
            let mut delivered = 0_u64;
            let mut cb = 0_u64;
            let mut reports = vec![Vec::new()];
            while cb < vanish_at {
                let h = &handles[0];
                let report = h.pump_input_next().expect("pump");
                reports[0].push(report);
                if let Some(d) = driver.as_mut() {
                    d.advance_frames(u64::from(report.frames));
                }
                rig.pump_all();
                delivered += u64::from(report.frames);
                cb += 1;
            }
            assert_eq!(delivered, vanish_at * u64::from(BLOCK));
            handles[0].vanish();
            // 失われた device の pump は defined error を返し、error
            // callback 経由で fault latch が立つ。
            let res = handles[0].pump_input_next();
            assert!(res.is_err(), "vanished device pump must fail: {res:?}");
            assert!(
                rig.session.fault().is_faulted(),
                "StreamInvalidated must latch fault"
            );
            // fail-stop 後も残りを供給し続けても prefix は伸びない。
            for _ in 0..3 {
                let _ = handles[0].pump_input_next();
                rig.pump_all();
            }
            reports
        },
    );
    assert_ne!(
        run.fault_bits & FAULT_INPUT_DISCONTINUITY,
        0,
        "fault bits {:#06x} must include FAULT_INPUT_DISCONTINUITY",
        run.fault_bits
    );

    let (report, mut c2) = rf628_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::NothingCommitted));
    assert_eq!(report.recoverable.len(), 1);
    assert_faulted_prefix(
        &run,
        &mut c2,
        i64::try_from(vanish_at * u64::from(BLOCK)).expect("prefix"),
    );
}

/// SV-7: open-fault recovery 後の record/commit/reload — SV-5c
/// （`rf628_open_fault_rejected_then_recovers`）が re-open 成功で
/// 打ち止めにした後段を埋める。persistent open error で
/// `InputSession::open` が `InputBindError::Device` の defined
/// rejection を返し、error clear 後に `RealDeviceRig::open` 内の
/// `InputSession::open`（同じ device/spec の re-open）が成功して
/// record→drive→finish→commit→save→別 `ProjectController`
/// reload まで独立 oracle で照合する。rejection→re-open→record
/// の分類は SV-5c と同じ機構を再利用し、recovery 専用の新経路は
/// 作らない。
#[test]
fn rf628_open_fault_recovery_commit_reload() {
    let clock = test_clock(7);
    let input = Rf628Input {
        channels: 1,
        scenario: contiguous_scenario(
            1,
            PcmPattern::Noise {
                seed: 13,
                amplitude: 0.9,
            },
        ),
        taps: vec![(0, ChannelMap::mono(0))],
        faults: Vec::new(),
        clock: None,
    };
    let mut run = rf628_record_with_pre_open(
        "sv7-open-recovery",
        &clock,
        &[input],
        drive_session,
        |handles, specs| {
            // persistent open error → defined rejection（SV-5c と同じ
            // `InputBindError::Device` 分類）。clear 後の re-open は
            // この後の `RealDeviceRig::open` 内 `InputSession::open`
            // が担い、その session がそのまま record に使われる。
            let dev = &handles[0];
            dev.set_open_input_error(Some(ruviform_audio_core::device::DeviceError::Backend(
                "permission_denied",
            )));
            let Err(err) = InputSession::open(dev, &specs[0]) else {
                panic!("open must be rejected while persistent error is set")
            };
            assert!(
                matches!(err, InputBindError::Device(_)),
                "expected InputBindError::Device, got {err:?}"
            );
            dev.set_open_input_error(None);
        },
    );
    assert_finish_complete(&run);

    let (report, mut c2) = rf628_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 1);
    assert_eq!(report.recoverable.len(), 0);

    assert_scenario_stream(&run, &run.controller, 0);
    assert_scenario_stream(&run, &c2, 0);
    let recommit = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(recommit.outcome, CommitOutcome::AlreadyApplied));
}

/// SV-8: literal stereo device → `ChannelMap::mono(0)`/`mono(1)`
/// の 2 mono tap（別 stream・別 staging media）。SV-2 の
/// 4ch→mono(1)/mono(3) とは別に、stereo pair の両 channel を
/// 個別 tap する構成を rf628 lane でも網羅する
/// （synthetic-L1 lane の `oracle_stereo_device_two_mono_taps`
/// に対応する fixture variant）。
#[test]
fn rf628_stereo_device_two_mono_taps_commit_reload() {
    let clock = test_clock(7);
    let scenario = contiguous_scenario(
        2,
        PcmPattern::Noise {
            seed: 31,
            amplitude: 0.9,
        },
    );
    let input = Rf628Input {
        channels: 2,
        scenario,
        taps: vec![(0, ChannelMap::mono(0)), (1, ChannelMap::mono(1))],
        faults: Vec::new(),
        clock: None,
    };
    let mut run = rf628_record("sv8-stereo-2mono", &clock, &[input], drive_session);
    assert_finish_complete(&run);
    assert_ne!(run.area.media_path(0), run.area.media_path(1));

    let (report, mut c2) = rf628_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 2);
    assert_eq!(report.recoverable.len(), 0);

    for si in 0..2_u16 {
        assert_scenario_stream(&run, &run.controller, si);
        assert_scenario_stream(&run, &c2, si);
    }
    // L/R tap 分離: ch0 と ch1 の PCM が bit 単位で異なる（Noise は
    // (seed, channel, frame) hash のため channel ごとに独立）。
    let m0 = read_commit_manifest(&run.area)
        .expect("manifest read")
        .expect("commit manifest");
    assert_ne!(m0.payload.streams[0].digest, m0.payload.streams[1].digest);
    let recommit = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(recommit.outcome, CommitOutcome::AlreadyApplied));
}
