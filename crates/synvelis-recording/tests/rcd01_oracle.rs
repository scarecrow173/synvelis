//! RCD-01 closeout oracle slice (synvelis#115): independent PCM oracle
//! for the synthetic-L1 recording acceptance boundary.
//!
//! 既存 `verify_capture_pcm`/`verify_tap_log`/`verify_log` は期待 PCM を
//! production `MockPcm`（`src/mock.rs`）で再計算する — 入力生成
//! （`FakeStreamShared.pcm`）と期待値検算が同じ関数を共有する
//! 自己参照検証であり、「device→adapter→writer→journal→commit→reload
//! の独立 sample/byte Oracle」を満たさない。本 file はその gap を
//! 閉じる最終 Oracle を確立する:
//!
//! - **入力生成と期待値検算が同じ関数を経由しない構造**。
//!   `OracleInputDevice`/`OracleInputStream`（test-local の
//!   `ruviform_audio_core::device` port 実装）が callback へ供給する
//!   PCM は本 file 内の `oracle_sample` で生成し、さらに
//!   `OracleStreamShared.emitted` が「callback buffer に実際に置かれた
//!   f32 bits」を device input frame 単位で記録する。検証側は
//!   journal/media の記録済み bytes をこの **emission log** と
//!   bitwise 照合する — 生成関数を検算側が再計算する経路は存在しない。
//! - `oracle_sample` は `MockPcm::sample` と構造・定数を共有しない
//!   independent mixer（fmix64 系 avalanche + 別 bit window + 別
//!   f32 射影）。golden const 配列で出力を pin し、generator 自体の
//!   正当性も固定する。
//! - oracle 経路は `MockPcm`・`verify_capture_pcm`・`verify_tap_log`・
//!   `verify_log`・`MockInput`/`block_pcm`・`synvelis_recording::mock::*`
//!   を期待値構成に一切呼ばない（import/call graph で確認可能）。
//!   `common` からは journal/media の read-only 解析 helper
//!   （`capture_of`/`all_journal_spans`/`assert_*`）と rig
//!   （`RealDeviceRig` — `&dyn Device` 経路）・format helper のみを使う。
//!
//! これは fake port 契約 + 実 production `StreamWriter`/staging/
//! commit/reload 経路の synthetic-L1 検証であり、実 Device Pass・
//! GUI-L2・EXT-L3 の受入ではない（`POLICIES.md`）。実機 lane は
//! `real-device` feature の opt-in `tests/real_input.rs`、
//! `scarecrow173/ruviform#628` fixture variant・`scarecrow173/ruviform#629`
//! 仮想 endpoint EXT-L3 は `evidence/sv115/` の case matrix で
//! NotRun/NotRequired として正直に分類する。

#[allow(dead_code)]
mod common;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use common::{
    FRAMES_MAX, RATE, RealDeviceRig, StreamCapture, all_journal_spans, assert_prefix_contiguous,
    assert_session_coverage, assert_source_contiguous, capture_of, input_config, plan_for,
    span_frame_total, tap, tmp_dir,
};
use ruviform_audio_core::device::{
    BufferSize, DEVICE_CLOCK_TIMEBASE, Device, DeviceError, DeviceId, DeviceResult,
    InputCallbackInfo, InputDataCallback, InputErrorCallback, InputStream, InputStreamConfig,
    InputTimestamp, OutputDataCallback, OutputErrorCallback, OutputStream, OutputStreamConfig,
    StreamLatency, SupportedInputFormat, SupportedOutputFormat,
};
use ruviform_audio_core::{AudioBufferView, AudioFormat};
use ruviform_media_core::{ClockDomainId, MediaTime};
use sha2::Digest as _;
use synvelis_project::{
    CaptureStreamId, ClockEpochId, Command, CounterIdGenerator, DirectoryBundleStore, LaneId,
    Project, ProjectController, ProjectId, ProjectOp, RecordingSessionId, SessionState, SourceId,
};
use synvelis_recording::{
    ArmState, ChannelMap, CommitOutcome, CommitReport, FinalizedOutcome, FinalizedSourceRef,
    FinishReport, InputStreamSpec, MonitorState, RecordingCommitManifest, RecordingOp, StagingArea,
    StreamEntry, SynvelisOp, commit_session, crc32, load_finalized, read_commit_manifest,
};

// ---------- independent oracle generator ----------

/// oracle の 1 sample を `(input_frame, channel, seed)` から決定的に
/// 生成する。`MockPcm::sample`（`src/mock.rs`）とは mixer 構造・
/// 定数・bit window・f32 射影の全てが異なる:
///
/// - `MockPcm`: 3 項 `wrapping_mul` 定数 XOR → `(v^(v>>29))*0xBF58476D1CE4E5B9`
///   → `>>48` → u16 → `/65536`（`[0,1)`）。
/// - 本関数: `(frame, channel, seed)` を xor/rotate/乗算で攪拌 →
///   murmur3 fmix64 系の xor-shift-multiply 2 段 avalanche
///   （`0xFF51AFD7ED558CCD`/`0xC4CEB9FE1A85EC53` — `MockPcm` の
///   `0xBF58476D1CE4E5B9` 系と非共有）→ 上位 24 bit を f64 経由で
///   `[-1,1)` へ射影。
///
/// 値域は `[-1,1)` で `MockPcm` の `[0,1)` とも重ならない。
/// 出力は IEEE-754 binary32 の決定的丸め（platform 非依存）。
#[allow(clippy::cast_possible_truncation)] // f64→f32 射影は意図的（fixture PCM は f32）
fn oracle_sample(input_frame: u64, channel: u32, seed: u32) -> f32 {
    let mut z = input_frame
        ^ u64::from(channel).rotate_left(21)
        ^ u64::from(seed).wrapping_mul(0xD1B5_4A32_D192_ED03);
    z = (z ^ (z >> 33)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    z = (z ^ (z >> 33)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    z ^= z >> 33;
    // 上位 24 bit → [0,1) → [-1,1)。除数は 2^24 で f64 除算は正確。
    let v = f64::from(u32::try_from(z >> 40).expect("24-bit window fits u32")) / 16_777_216.0 - 1.0;
    v as f32
}

/// `oracle_sample` の golden const（`(input_frame, channel, seed,
/// expected f32 bits)`）。generator の drift を pin する補助 anchor —
/// 本体の検証は emission log との bitwise 照合であり、const は
/// 「oracle が意味のある多様な PCM を出す」ことの独立した担保。
const ORACLE_GOLDEN: &[(u64, u32, u32, u32)] = &[
    // (input_frame, channel, seed, expected f32 bits)
    (0, 0, 11, 0xbeb1_2f4c),
    (1, 0, 11, 0xbf2e_9c69),
    (0, 1, 11, 0xbebd_9584),
    (100, 0, 11, 0xbab9_3e00),
    (0, 0, 22, 0xbf7c_bc98),
    (7, 3, 42, 0xbf09_460e),
    (4999, 1, 33, 0xbee8_9372),
];

// ---------- oracle device（test-local `Device`/`InputStream` port） ----------

/// scripted oracle stream の共有状態。`emitted` は「callback buffer
/// に実際に置かれた f32 bits」を device input frame 単位で記録する
/// emission log — 検証側の期待値はこの観測値であり、生成関数の
/// 再計算ではない。`u32::MAX` は未供給 frame（position gap）を表す
/// sentinel で、`oracle_sample` の出力（`[-1,1)`、NaN/Inf を含まない）
/// と衝突しない。
struct OracleStreamShared {
    data_cb: Option<InputDataCallback>,
    domain: ClockDomainId,
    format: AudioFormat,
    /// 次に供給する device input frame 番号。
    position: u64,
    /// `stream.now()` が返す値。
    now_value: i64,
    /// planar PCM slab（channel-major）。
    slab: Vec<f32>,
    /// PCM 生成 seed（device 単位）。
    seed: u32,
    /// `emitted[device_channel][absolute_input_frame]` = f32 bits。
    emitted: Vec<Vec<u32>>,
    started: bool,
}

impl OracleStreamShared {
    /// `frames` 分の oracle PCM を slab に書き、emission log に記録して
    /// から callback へ供給する。`FakeStreamShared::deliver` と同じ
    /// `InputCallbackInfo` 形状（`frame_position` 報告・`capture: None`）。
    fn deliver(&mut self, frames: u32) {
        let frames_usize = usize::try_from(frames).expect("frames");
        assert!(frames_usize > 0 && frames_usize <= FRAMES_MAX);
        let ch = channels_of(self.format);
        let base = self.position;
        let seed = self.seed;
        {
            let slab = &mut self.slab;
            let emitted = &mut self.emitted;
            for c in 0..ch {
                for f in 0..frames_usize {
                    let frame = base.wrapping_add(u64::try_from(f).unwrap_or(u64::MAX));
                    let sample = oracle_sample(frame, u32::try_from(c).unwrap_or(u32::MAX), seed);
                    slab[c * frames_usize + f] = sample;
                    let idx = usize::try_from(frame).expect("frame index");
                    let log = &mut emitted[c];
                    if log.len() <= idx {
                        log.resize(idx + 1, u32::MAX);
                    }
                    log[idx] = sample.to_bits();
                }
            }
        }
        let info = InputCallbackInfo {
            timestamp: InputTimestamp {
                callback: MediaTime::new(0, DEVICE_CLOCK_TIMEBASE),
                capture: None,
            },
            clock_domain: self.domain,
            frame_position: Some(self.position),
        };
        let view = AudioBufferView::try_new(&self.slab[..ch * frames_usize], ch, frames_usize)
            .expect("view");
        if let Some(cb) = &mut self.data_cb {
            cb(view, info);
        }
        self.position = self.position.wrapping_add(u64::from(frames));
    }

    /// 位置を `pos` へ跳ばす（discontinuity 注入用。gap frame は
    /// emission log に `u32::MAX` sentinel のまま残る）。
    fn jump_to(&mut self, pos: u64) {
        self.position = pos;
    }

    /// device が `(channel, absolute_input_frame)` に実際に供給した
    /// f32 bits を返す。未供給なら `u32::MAX` sentinel。
    fn emitted_bits(&self, channel: usize, frame: u64) -> u32 {
        self.emitted
            .get(channel)
            .and_then(|log| {
                usize::try_from(frame)
                    .ok()
                    .and_then(|idx| log.get(idx).copied())
            })
            .unwrap_or(u32::MAX)
    }

    /// `start()` が呼ばれたか（lifecycle 検証用）。
    fn is_started(&self) -> bool {
        self.started
    }
}

/// scripted oracle `InputStream`。
struct OracleInputStream {
    shared: Arc<Mutex<OracleStreamShared>>,
}

impl InputStream for OracleInputStream {
    fn start(&self) -> DeviceResult<()> {
        self.shared.lock().expect("lock").started = true;
        Ok(())
    }

    fn stop(&self) -> DeviceResult<()> {
        self.shared.lock().expect("lock").started = false;
        Ok(())
    }

    fn format(&self) -> AudioFormat {
        self.shared.lock().expect("lock").format
    }

    fn buffer_size(&self) -> DeviceResult<BufferSize> {
        Ok(BufferSize::Default)
    }

    fn now(&self) -> DeviceResult<MediaTime> {
        let value = self.shared.lock().expect("lock").now_value;
        Ok(MediaTime::new(value, DEVICE_CLOCK_TIMEBASE))
    }

    fn clock_domain(&self) -> ClockDomainId {
        self.shared.lock().expect("lock").domain
    }

    fn input_latency(&self) -> DeviceResult<Option<StreamLatency>> {
        Ok(None)
    }
}

/// scripted oracle device の共有状態。
struct OracleDeviceShared {
    format: AudioFormat,
    domain: ClockDomainId,
    seed: u32,
    streams: Vec<Arc<Mutex<OracleStreamShared>>>,
}

/// scripted oracle `Device`（input のみ実装、output は `Unsupported`）。
/// `FakeInputDevice` と構造は同型だが PCM 源は `MockPcm` ではなく
/// `oracle_sample` + emission log 記録。
struct OracleInputDevice {
    shared: Arc<Mutex<OracleDeviceShared>>,
}

impl OracleInputDevice {
    /// `channels` ch・`domain`・`seed` の oracle input device を作る。
    fn new(channels: u32, domain: ClockDomainId, seed: u32) -> Self {
        Self {
            shared: Arc::new(Mutex::new(OracleDeviceShared {
                format: common::input_format(channels),
                domain,
                seed,
                streams: Vec::new(),
            })),
        }
    }

    /// open 済み stream への scripted handle（test 駆動用）。
    /// `open_input_stream` 呼出し順に `streams` へ push される。
    fn stream(&self, index: usize) -> Arc<Mutex<OracleStreamShared>> {
        self.shared.lock().expect("lock").streams[index].clone()
    }
}

impl Device for OracleInputDevice {
    fn name(&self) -> DeviceResult<String> {
        Ok("oracle-input".to_owned())
    }

    fn id(&self) -> DeviceResult<Option<DeviceId>> {
        let seed = self.shared.lock().expect("lock").seed;
        Ok(Some(DeviceId::new(format!("oracle:{seed}"))))
    }

    fn default_output_format(&self) -> DeviceResult<AudioFormat> {
        Err(DeviceError::Unsupported)
    }

    fn supported_output_formats(&self) -> DeviceResult<Vec<SupportedOutputFormat>> {
        Ok(Vec::new())
    }

    fn open_output_stream(
        &self,
        _config: &OutputStreamConfig,
        _data_callback: OutputDataCallback,
        _error_callback: OutputErrorCallback,
    ) -> DeviceResult<Box<dyn OutputStream>> {
        Err(DeviceError::Unsupported)
    }

    fn default_input_format(&self) -> DeviceResult<AudioFormat> {
        Ok(self.shared.lock().expect("lock").format)
    }

    fn supported_input_formats(&self) -> DeviceResult<Vec<SupportedInputFormat>> {
        Ok(Vec::new())
    }

    fn open_input_stream(
        &self,
        config: &InputStreamConfig,
        data_callback: InputDataCallback,
        _error_callback: InputErrorCallback,
    ) -> DeviceResult<Box<dyn InputStream>> {
        let mut g = self.shared.lock().expect("lock");
        let channels = channels_of(config.format);
        let shared = Arc::new(Mutex::new(OracleStreamShared {
            data_cb: Some(data_callback),
            domain: g.domain,
            format: config.format,
            position: 0,
            now_value: 0,
            slab: vec![0.0; channels.saturating_mul(FRAMES_MAX)],
            seed: g.seed,
            emitted: vec![Vec::new(); channels],
            started: false,
        }));
        g.streams.push(Arc::clone(&shared));
        Ok(Box::new(OracleInputStream { shared }))
    }
}

/// `AudioFormat` の channel 数。
fn channels_of(format: AudioFormat) -> usize {
    usize::try_from(format.layout.channels()).unwrap_or(0)
}

// ---------- e2e rig helpers ----------

const STOP_TICK: i64 = 200;
const DRIVE_BLOCK: u32 = 500;
const CALLBACK_QUEUE: usize = 16;

/// 1 oracle device 分の spec（device channel 数・PCM seed・armed tap 列）。
/// `taps` の `stream_index` は `plan.targets` の添字（= lane 順）と一致する。
struct OracleInputSpec {
    device_channels: u32,
    seed: u32,
    taps: Vec<(u16, ChannelMap)>,
}

/// record→finish まで完了した oracle session の成果物一式。
struct OracleRun {
    root: PathBuf,
    controller: ProjectController,
    session_id: RecordingSessionId,
    area: StagingArea,
    fin: FinishReport,
    /// plan の session 末尾 frame（coverage 検証の期待値）。
    session_frames: i64,
    /// `stream_index` ごとの device stream emission log handle。
    emitted: Vec<Arc<Mutex<OracleStreamShared>>>,
    /// `stream_index` ごとの出力 channel → 入力 channel 写像。
    maps: Vec<Vec<u32>>,
}

/// track/lane を 1 対 1 で `n` 個ずつ作る controller（multi-track 検証用）。
fn controller_with_tracks(n: usize) -> (ProjectController, Vec<LaneId>, ClockEpochId) {
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "rcd-01-oracle"),
        Box::new(CounterIdGenerator::new(1)),
    );
    let mut lanes = Vec::with_capacity(n);
    for i in 0..n {
        c.apply(Command {
            expected_revision: c.live_revision(),
            op: ProjectOp::AddTrack {
                name: format!("oracle-t{i}"),
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
            description: "oracle".into(),
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
/// 戻り値の `CaptureStreamId` 列は lane 順 = `stream_index` 順。
fn open_session_ids(
    c: &mut ProjectController,
    lanes: &[LaneId],
    epoch: ClockEpochId,
) -> (RecordingSessionId, Vec<CaptureStreamId>) {
    rop(
        c,
        RecordingOp::CreateSession {
            name: "oracle-session".into(),
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

/// oracle device → `RealDeviceRig`（`&dyn Device` 経路）→ drive →
/// finish までを実行する。`drive` は `(rig, per-device stream handles,
/// session_frames)` を受けて PCM 供給を制御する（fault 注入もここ）。
fn oracle_record(
    name: &str,
    inputs: &[OracleInputSpec],
    drive: impl FnOnce(&mut RealDeviceRig, &[Arc<Mutex<OracleStreamShared>>], i64),
) -> OracleRun {
    let n_streams = inputs.iter().map(|s| s.taps.len()).sum::<usize>();
    assert!(n_streams > 0, "at least one armed tap required");
    // stream_index の被覆を検証（各 target に armed tap が 1 つずつ）。
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

    // plan target の channel 数 = 各 stream_index の channel_map 長。
    let mut target_channels = vec![0_u32; n_streams];
    for spec in inputs {
        for &(si, ref map) in &spec.taps {
            target_channels[usize::from(si)] = u32::try_from(map.len()).expect("channel map len");
        }
    }
    let plan = plan_for(c.live(), &lanes, &target_channels, epoch, STOP_TICK);
    let session_frames = plan.session_end.value();

    let specs: Vec<InputStreamSpec> = inputs
        .iter()
        .map(|spec| InputStreamSpec {
            config: input_config(spec.device_channels),
            clock_epoch: epoch,
            max_block_frames: plan.max_block_frames,
            callback_queue: CALLBACK_QUEUE,
            taps: spec
                .taps
                .iter()
                .map(|&(si, ref map)| tap(&plan, si, map.clone(), ArmState::On, MonitorState::Off))
                .collect(),
        })
        .collect();
    let devices: Vec<OracleInputDevice> = inputs
        .iter()
        .map(|spec| {
            OracleInputDevice::new(spec.device_channels, ClockDomainId::SAMPLE_CLOCK, spec.seed)
        })
        .collect();
    let dyn_inputs: Vec<(&dyn Device, &InputStreamSpec)> = devices
        .iter()
        .zip(specs.iter())
        .map(|(d, s)| (d as &dyn Device, s))
        .collect();
    let mut rig = RealDeviceRig::open(
        &root,
        session_id,
        plan,
        RATE,
        &dyn_inputs,
        Some(stream_ids.as_slice()),
    );

    // 各 device は spec ごとに 1 stream を open する（ordinal 0）。
    let handles: Vec<Arc<Mutex<OracleStreamShared>>> =
        devices.iter().map(|d| d.stream(0)).collect();
    rig.start_all().expect("start_all");
    for h in &handles {
        assert!(h.lock().expect("lock").is_started(), "stream started");
    }
    drive(&mut rig, &handles, session_frames);
    let fin = rig.finish();

    // stream_index → emission log handle / channel map を解決する。
    let mut emitted: Vec<Option<Arc<Mutex<OracleStreamShared>>>> = vec![None; n_streams];
    let mut maps: Vec<Vec<u32>> = vec![Vec::new(); n_streams];
    for (input_idx, spec) in inputs.iter().enumerate() {
        for &(si, ref map) in &spec.taps {
            let slot = usize::from(si);
            emitted[slot] = Some(Arc::clone(&handles[input_idx]));
            maps[slot] = map.channels().to_vec();
        }
    }
    OracleRun {
        root,
        controller: c,
        session_id,
        area: rig.area.clone(),
        fin,
        session_frames,
        emitted: emitted
            .into_iter()
            .map(|o| o.expect("emitted handle"))
            .collect(),
        maps,
    }
}

/// 全 stream に `session_frames` を `DRIVE_BLOCK` 刻みで均等供給する
/// 正常系 drive。
fn drive_session(rig: &mut RealDeviceRig, handles: &[Arc<Mutex<OracleStreamShared>>], total: i64) {
    let mut remaining = u32::try_from(total).expect("session frames");
    while remaining > 0 {
        let n = DRIVE_BLOCK.min(remaining);
        for h in handles {
            h.lock().expect("lock").deliver(n);
        }
        rig.pump_all();
        remaining -= n;
    }
}

/// `CloseSession` → `commit_session` → `DirectoryBundleStore` save →
/// 別 `ProjectController` で load（commit→reload の正規経路）。
/// 戻り値は `(CommitReport, reloaded controller)`。
fn oracle_commit_reload(run: &mut OracleRun) -> (CommitReport, ProjectController) {
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
        Project::new(ProjectId::new(0), "oracle-reload"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c2.load(&store).expect("load");
    (report, c2)
}

// ---------- oracle 検証 ----------

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", sha2::Sha256::digest(bytes))
}

/// journal/media の記録済み bytes を device emission log と bitwise
/// 照合する独立 oracle。`channel_map[j]` = 出力 channel j の入力
/// channel 番号。各 chunk の `pcm_crc32` も検算する。
/// 戻り値は照合した出力 sample 数。
fn verify_oracle_media(
    cap: &StreamCapture,
    channel_map: &[u32],
    emitted: &OracleStreamShared,
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
            for f in 0..sp.frame_count {
                let input_frame = input_base.wrapping_add(u64::from(f));
                for (j, &src_ch) in channel_map.iter().enumerate() {
                    let idx = m_off + usize::try_from(f).expect("frame") * out_ch + j;
                    let bytes = cap.media.get(idx * 4..idx * 4 + 4).expect("pcm bytes");
                    let actual = f32::from_le_bytes(bytes.try_into().expect("f32 le"));
                    let expected = emitted
                        .emitted_bits(usize::try_from(src_ch).expect("src channel"), input_frame);
                    assert_ne!(
                        expected,
                        u32::MAX,
                        "stream input frame {input_frame} ch {src_ch} was never emitted"
                    );
                    assert_eq!(
                        actual.to_bits(),
                        expected,
                        "oracle pcm mismatch chunk {} f {f} out_ch {j} (in_ch {src_ch})",
                        chk.chunk_id
                    );
                    checked += 1;
                }
            }
        }
    }
    checked
}

/// `stream_index` の finalized ref を読む。
fn finalized_for(run: &OracleRun, stream_index: u16) -> FinalizedSourceRef {
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

/// 1 stream の完全な oracle 監査: journal media の emission-log
/// bitwise 照合 + session/source coverage + finalized ref の
/// `verified_sha256` 独立再計算 + commit manifest↔project entity の
/// digest 連鎖。`project` は commit 直後または reload 後の controller。
fn assert_oracle_stream(run: &OracleRun, project: &ProjectController, stream_index: u16) {
    let cap = capture_of(&run.area, stream_index);
    let map = &run.maps[usize::from(stream_index)];
    let handle = Arc::clone(&run.emitted[usize::from(stream_index)]);
    let checked = {
        let g = handle.lock().expect("emitted lock");
        verify_oracle_media(&cap, map, &g)
    };
    assert!(checked > 0, "stream {stream_index} verified no samples");

    // journal 幾何: session 全域を隙間なく cover・source 連続。
    let spans = all_journal_spans(&cap);
    assert_session_coverage(&spans, &[(0, run.session_frames)]);
    assert_source_contiguous(&spans, run.session_frames);
    assert_eq!(span_frame_total(&spans), run.session_frames);

    // finalized ref: Complete・全 source frame・verified prefix の
    // SHA-256 を test 内で独立再計算して pin する。
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

    // commit manifest ↔ project entity の digest 連鎖。
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
fn assert_finish_complete(run: &OracleRun) {
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

// ---------- tests ----------

#[test]
fn oracle_sample_golden_consts() {
    for &(frame, ch, seed, bits) in ORACLE_GOLDEN {
        assert_eq!(
            oracle_sample(frame, ch, seed).to_bits(),
            bits,
            "oracle_sample({frame}, {ch}, {seed})"
        );
    }
    // generator の妥当性: seed/channel/frame で値が変わり、値域は [-1,1)。
    let base = oracle_sample(0, 0, 11);
    assert_ne!(base, oracle_sample(0, 0, 22), "seed sensitivity");
    assert_ne!(base, oracle_sample(0, 1, 11), "channel sensitivity");
    assert_ne!(base, oracle_sample(1, 0, 11), "frame sensitivity");
    for &(frame, ch, seed, _) in ORACLE_GOLDEN {
        let v = oracle_sample(frame, ch, seed);
        assert!((-1.0..1.0).contains(&v), "oracle range: {v}");
    }
}

/// case (i): 1 stereo oracle device の 2 mono tap が別 stream・別
/// staging media・別 track/lane に oracle 値で分離され、
/// commit→save→reload まで fidelity を実測する。
#[test]
fn oracle_stereo_device_two_mono_taps_commit_reload() {
    let spec = OracleInputSpec {
        device_channels: 2,
        seed: 11,
        taps: vec![(0, ChannelMap::mono(0)), (1, ChannelMap::mono(1))],
    };
    let mut run = oracle_record("stereo-2mono", &[spec], drive_session);
    assert_finish_complete(&run);

    // 別 staging media file。
    assert_ne!(run.area.media_path(0), run.area.media_path(1));

    let (report, mut c2) = oracle_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 2);
    assert_eq!(report.recoverable.len(), 0);

    // commit 直後・reload 後の両 controller で oracle fidelity を実測。
    for si in 0..2_u16 {
        assert_oracle_stream(&run, &run.controller, si);
        assert_oracle_stream(&run, &c2, si);
    }
    // multi-track: 2 track/2 lane/2 clip が分離して登録される。
    assert_eq!(run.controller.live().tracks.len(), 2);
    assert_eq!(run.controller.live().lanes.len(), 2);
    assert_eq!(run.controller.live().clips.len(), 2);
    assert_eq!(c2.live().sources.len(), 2);
    assert!(
        c2.live()
            .recording
            .applied_commits
            .contains_key(&run.session_id)
    );

    // reload 後の再 commit は AlreadyApplied（重複登録なし）。
    let report2 = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(report2.outcome, CommitOutcome::AlreadyApplied));
    assert_eq!(c2.live().sources.len(), 2);
}

/// case (ii): same-clock 2 oracle device（別 seed）が `InputSession::add`
/// で束ねられ、別 stream に別 seed の oracle PCM が記録される。
#[test]
fn oracle_two_devices_same_clock_commit_reload() {
    let spec_a = OracleInputSpec {
        device_channels: 1,
        seed: 11,
        taps: vec![(0, ChannelMap::mono(0))],
    };
    let spec_b = OracleInputSpec {
        device_channels: 1,
        seed: 22,
        taps: vec![(1, ChannelMap::mono(0))],
    };
    let mut run = oracle_record("two-devices", &[spec_a, spec_b], drive_session);
    assert_finish_complete(&run);

    let (report, mut c2) = oracle_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 2);
    assert_eq!(report.recoverable.len(), 0);

    for si in 0..2_u16 {
        assert_oracle_stream(&run, &run.controller, si);
        assert_oracle_stream(&run, &c2, si);
    }
    // 別 seed の PCM が実際に分離されていること（digest が異なる）。
    let manifest = read_commit_manifest(&run.area)
        .expect("manifest read")
        .expect("commit manifest");
    assert_ne!(
        manifest.payload.streams[0].digest,
        manifest.payload.streams[1].digest
    );

    let report2 = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(report2.outcome, CommitOutcome::AlreadyApplied));
}

/// stereo identity mapping（2ch → 2ch）の end-to-end fidelity。
#[test]
fn oracle_stereo_identity_commit_reload() {
    let spec = OracleInputSpec {
        device_channels: 2,
        seed: 33,
        taps: vec![(0, ChannelMap::identity(2))],
    };
    let mut run = oracle_record("stereo-id", &[spec], drive_session);
    assert_finish_complete(&run);

    let (report, mut c2) = oracle_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 1);
    assert_eq!(report.recoverable.len(), 0);

    assert_oracle_stream(&run, &run.controller, 0);
    assert_oracle_stream(&run, &c2, 0);
    let report2 = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(report2.outcome, CommitOutcome::AlreadyApplied));
}

/// multichannel（4ch Quad layout identity）の end-to-end fidelity。
#[test]
fn oracle_multichannel_identity_commit_reload() {
    let spec = OracleInputSpec {
        device_channels: 4,
        seed: 44,
        taps: vec![(0, ChannelMap::identity(4))],
    };
    let mut run = oracle_record("quad-id", &[spec], drive_session);
    assert_finish_complete(&run);

    let (report, mut c2) = oracle_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 1);
    assert_eq!(report.recoverable.len(), 0);

    assert_oracle_stream(&run, &run.controller, 0);
    assert_oracle_stream(&run, &c2, 0);
    let report2 = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(report2.outcome, CommitOutcome::AlreadyApplied));
}

/// fault case: input discontinuity（position gap）注入 → fail-stop →
/// journal verified prefix が oracle prefix と bit 一致
/// （`verified_sha256` 再計算照合を含む）かつ、commit 層で全 stream が
/// honest に recoverable 分類される（Source 昇格なし）ことを
/// commit→reload 越しに検証する。
///
/// capture-fault marker は session スコープのため、健全側 stream も
/// commit 対象にならない — これは「完走していない session を complete
/// と偽装しない」正直な分類であり、prefix fidelity は journaled
/// bytes のみで保証される（未検証 frame の連続捏造なし）。
#[test]
fn oracle_input_discontinuity_verified_prefix() {
    let spec_a = OracleInputSpec {
        device_channels: 1,
        seed: 11,
        taps: vec![(0, ChannelMap::mono(0))],
    };
    let spec_b = OracleInputSpec {
        device_channels: 1,
        seed: 22,
        taps: vec![(1, ChannelMap::mono(0))],
    };
    let mut run = oracle_record(
        "discontinuity",
        &[spec_a, spec_b],
        |rig, handles, _total| {
            // 3 block × 500 frame = 1500 frame ずつ正常供給。
            for _ in 0..3 {
                for h in handles {
                    h.lock().expect("lock").deliver(500);
                }
                rig.pump_all();
            }
            // device A の position を 777 frame 跳ばす → gap 注入。
            handles[0].lock().expect("lock").jump_to(1500 + 777);
            for h in handles {
                h.lock().expect("lock").deliver(500);
            }
            rig.pump_all();
            assert!(rig.session.fault().is_faulted(), "discontinuity must latch");
            // fail-stop 後の入力は捨てられる — 供給を続けても prefix は伸びない。
            for _ in 0..4 {
                for h in handles {
                    h.lock().expect("lock").deliver(500);
                }
                rig.pump_all();
            }
        },
    );

    // session スコープの capture-fault marker が durable に残る。
    assert!(run.area.capture_fault_marker_exists());

    let (report, mut c2) = oracle_commit_reload(&mut run);
    // faulted session は commit 対象 stream を持たず、全 stream が
    // recoverable に分類される（Source へ昇格しない）。
    assert!(matches!(report.outcome, CommitOutcome::NothingCommitted));
    assert_eq!(report.sources_committed, 0);
    assert_eq!(report.recoverable.len(), 2);
    for r in &report.recoverable {
        assert!(
            r.reason.starts_with("capture-fault"),
            "recoverable reason: {}",
            r.reason
        );
    }
    assert!(run.controller.live().sources.is_empty());
    assert!(c2.live().sources.is_empty());
    assert_eq!(
        run.controller.live().recording.sessions[&run.session_id].state,
        SessionState::Closed,
        "nothing-committed session stays Closed"
    );

    // journal verified prefix が oracle prefix と bit 一致することを
    // reload 後の disk 状態で再検証する（commit→reload 越し）。
    for si in 0..2_u16 {
        let cap = capture_of(&run.area, si);
        let spans = all_journal_spans(&cap);
        assert_prefix_contiguous(&spans);
        assert_eq!(
            span_frame_total(&spans),
            1500,
            "stream {si} verified prefix ends at discontinuity"
        );
        let handle = Arc::clone(&run.emitted[usize::from(si)]);
        let checked = {
            let g = handle.lock().expect("emitted lock");
            verify_oracle_media(&cap, &run.maps[usize::from(si)], &g)
        };
        assert!(checked > 0, "stream {si} prefix verified no samples");
        // finalized ref: journaled prefix は durable に完結するため
        // writer 視点の outcome は Complete。recoverable 分類は
        // session スコープの capture-fault marker が担う（outcome
        // ではなく marker が commit 対象外を決める — `build_payload`）。
        // verified prefix の SHA-256 を pin する。
        let fin = finalized_for(&run, si);
        assert_eq!(fin.outcome, FinalizedOutcome::Complete);
        let prefix = &cap.media[..usize::try_from(fin.media_byte_len).expect("media len")];
        assert_eq!(
            fin.verified_sha256.as_deref(),
            Some(sha256_prefixed(prefix).as_str()),
            "stream {si} prefix verified_sha256"
        );
    }

    // reload 後の再 commit も同じく honest な NothingCommitted。
    let report2 = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(report2.outcome, CommitOutcome::NothingCommitted));
    assert_eq!(c2.live().sources.len(), 0);
}

/// mixed session: 2 stream を正常録音・finalize した後、stream 0 の
/// `finalized.json` のみを失わせる（finalize→commit 間の crash 相当）。
/// commit 層は健全 stream 1 を Complete として commit→reload の
/// oracle fidelity を通し、失われた stream 0 は `missing-finalized`
/// で正直に recoverable に留める — 健全/異常の混在で分類が
/// 偽装なく分離されることを検証する。
#[test]
fn oracle_missing_finalized_mixed_session() {
    let spec_a = OracleInputSpec {
        device_channels: 1,
        seed: 11,
        taps: vec![(0, ChannelMap::mono(0))],
    };
    let spec_b = OracleInputSpec {
        device_channels: 1,
        seed: 22,
        taps: vec![(1, ChannelMap::mono(0))],
    };
    let mut run = oracle_record("mixed-finalized", &[spec_a, spec_b], drive_session);
    assert_finish_complete(&run);

    // finalize→commit 間の crash を模して stream 0 の finalized ref を失う。
    std::fs::remove_file(run.area.finalized_path(0)).expect("remove finalized");

    let (report, mut c2) = oracle_commit_reload(&mut run);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 1);
    assert_eq!(report.recoverable.len(), 1);
    assert_eq!(report.recoverable[0].stream_index, 0);
    assert_eq!(report.recoverable[0].reason, "missing-finalized");

    // 健全 stream 1 は commit→reload の oracle fidelity を通る。
    assert_oracle_stream(&run, &run.controller, 1);
    assert_oracle_stream(&run, &c2, 1);
    assert_eq!(c2.live().sources.len(), 1);

    // stream 0 は Source に昇格しない。
    let manifest = read_commit_manifest(&run.area)
        .expect("manifest read")
        .expect("commit manifest");
    assert_eq!(manifest.payload.streams.len(), 1);
    assert_eq!(manifest.payload.streams[0].stream_index, 1);

    // reload 後の再 commit は AlreadyApplied（同一 manifest 再利用）。
    let report2 = commit_session(&mut c2, &run.area).expect("recommit");
    assert!(matches!(report2.outcome, CommitOutcome::AlreadyApplied));
    assert_eq!(c2.live().sources.len(), 1);
}
