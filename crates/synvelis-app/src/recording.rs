//! Read-only projection of the persisted `Project.recording` registry
//! plus the recorder session owner — the `syn.recording.info` data
//! surface and the `syn.recorder.*` lifecycle backing state
//! (synvelis#115 RCD-01 headless slice; recorder wire synvelis#114
//! SV-GUI-05 Option B).
//!
//! [`RecorderOwner`] is the app-layer session owner for device
//! recording: `syn.recorder.start` runs `preflight` →
//! `InputSession::open`/`add` (injected `&dyn Device` +
//! `InputStreamSpec`) → `StagingArea`/`StreamWriter` → stream start;
//! `pump`/`stop`/`commit`/`discard` drive the non-RT lifecycle and
//! `commit` finishes through `commit_session`
//! (`SynvelisOp::Recording(RecordingOp::CommitRecording)`). Persisted
//! recording entities stay authoritative in `Project.recording`;
//! the owner holds only session-scoped runtime state and is reset
//! when the project session is recreated. Device enumeration/selection
//! is a later slice — callers inject `Device`s via
//! [`RecorderOwner::set_devices`].

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use serde_json::{Value, json};

use ruviform_audio_core::device::{BufferSize, Device, DeviceError, InputStreamConfig};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate, SpeakerMask};
use synvelis_project::{
    CaptureStreamId, ClockEpochId, MusicalTick, Project, RecordingSessionId, SessionState,
    TakeState, TempoMapRef,
};
use synvelis_recording::{
    ArmState, BufferBudget, CaptureTarget, ChannelMap, ChunkSink, CommitError, CommitOutcome,
    CommitReport, FaultLatch, GateSpec, InputBindError, InputBinding, InputSession,
    InputStreamSpec, InputTapSpec, MonitorState, PreparedRecordingPlan, RecordingOp,
    RecordingRequest, StagingArea, StagingError, StagingSessionConfig, StagingStreamConfig,
    StreamWriter, SynvelisOp, TapWriterEnds, WriterError, commit_session, finish_all, preflight,
    stream_config,
};
use synvelis_workspace::ProjectSession;

/// `syn.recorder.start` の既定バッファ予算
/// （`synvelis-recording` fixture / `syn.monitoring.session` と同値）。
const DEFAULT_BUDGET: BufferBudget = BufferBudget {
    chunks: 16,
    chunk_frames: 1024,
    spans_per_chunk: 8,
    filled_capacity: 16,
    lookahead_spans: 16,
};

/// `syn.recorder.start` の既定 `max_block_frames`。
const DEFAULT_MAX_BLOCK_FRAMES: u32 = 1024;

/// tap ごとの callback→pump work queue 容量
/// （`synvelis-recording` oracle fixture と同値）。
const CALLBACK_QUEUE: usize = 16;

/// `targets[].channels` の wire 上限 — `build_tap_resources` が
/// `max_block_frames × channel_map.len()` の callback queue と
/// `chunk_frames × channels` の PCM pool を eager 確保するため、
/// 上限なしでは巨大確保で process が abort し得る。実用上の
/// 最大 channel 数（7.1.4=12・immersive ≤128）を大きく上回る
/// 256 を上限とする。
const MAX_TARGET_CHANNELS: u32 = 256;

fn session_state(state: SessionState) -> &'static str {
    match state {
        SessionState::Created => "created",
        SessionState::Open => "open",
        SessionState::Closed => "closed",
        SessionState::Finalized => "finalized",
    }
}

fn take_state(state: TakeState) -> &'static str {
    match state {
        TakeState::Raw => "raw",
        TakeState::Verified => "verified",
    }
}

/// `syn.recording.info` payload — persisted recording entities plus
/// recorder status. Read-only; never mutates the session.
#[must_use]
pub fn recording_info(session: &ProjectSession, recorder: &RecorderOwner) -> Value {
    let project = session.project();
    let reg = &project.recording;
    let sessions: Vec<Value> = reg
        .sessions
        .values()
        .map(|s| {
            json!({
                "id": s.id.to_tagged(),
                "name": s.name,
                "state": session_state(s.state),
            })
        })
        .collect();
    let passes: Vec<Value> = reg
        .passes
        .values()
        .map(|p| {
            json!({
                "id": p.id.to_tagged(),
                "session_id": p.session_id.to_tagged(),
                "index": p.index,
            })
        })
        .collect();
    let streams: Vec<Value> = reg
        .streams
        .values()
        .map(|s| {
            json!({
                "id": s.id.to_tagged(),
                "session_id": s.session_id.to_tagged(),
                "lane_id": s.lane_id.to_tagged(),
                "clock_epoch_id": s.clock_epoch_id.to_tagged(),
            })
        })
        .collect();
    let segments: Vec<Value> = reg
        .segments
        .values()
        .map(|g| {
            json!({
                "id": g.id.to_tagged(),
                "stream_id": g.stream_id.to_tagged(),
                "pass_id": g.pass_id.to_tagged(),
                "source_id": g.source_id.to_tagged(),
                "session_start": g.session_range.start().value(),
                "session_end": g.session_range.end().value(),
                "source_start": g.source_range.start().value(),
                "source_end": g.source_range.end().value(),
            })
        })
        .collect();
    let takes: Vec<Value> = reg
        .takes
        .values()
        .map(|t| {
            json!({
                "id": t.id.to_tagged(),
                "session_id": t.session_id.to_tagged(),
                "pass_id": t.pass_id.to_tagged(),
                "stream_id": t.stream_id.to_tagged(),
                "lane_id": t.lane_id.to_tagged(),
                "source_id": t.source_id.to_tagged(),
                "state": take_state(t.state),
                "segment_count": t.segment_ids.len(),
            })
        })
        .collect();
    json!({
        "sessions": sessions,
        "passes": passes,
        "streams": streams,
        "segments": segments,
        "takes": takes,
        "counts": {
            "sessions": reg.sessions.len(),
            "passes": reg.passes.len(),
            "streams": reg.streams.len(),
            "segments": reg.segments.len(),
            "takes": reg.takes.len(),
        },
        "recorder": {
            "status": recorder.status(),
            "active": recorder.active(),
            "transaction_open": session.control().recording().is_some(),
        },
    })
}

// ---------- syn.recorder.* session owner ----------

/// 1 tap 分の `syn.recorder.start` 宣言（`ui_ops` が wire params から
/// 組み立てる typed 形）。
#[derive(Clone, Debug)]
pub struct RecorderTapParams {
    /// `plan.targets` の添字 — armed tap は参照先 target の
    /// channel 数と `channel_map` 長の一致が必須。
    pub stream_index: u16,
    /// 出力 channel → 入力 channel の写像。
    pub channel_map: Vec<u32>,
    /// Record Arm。`Off` の tap は capture path を持たず staging
    /// stream を生やさない（Monitor On でも raw capture しない —
    /// `synvelis-recording` の binding 規則どおり）。
    pub arm: ArmState,
    /// Software Monitor mode。
    pub monitor: MonitorState,
}

/// 1 入力 stream 分の `syn.recorder.start` 宣言。
#[derive(Clone, Debug)]
pub struct RecorderStreamParams {
    /// 注入 device 列の index（`open`/`add` の束縛先）。
    pub device: usize,
    /// device 入力 stream の channel 数（`ChannelLayout` はここから導出）。
    pub channels: u32,
    /// この入力 stream 上の tap 列。
    pub taps: Vec<RecorderTapParams>,
}

/// `syn.recorder.start` の typed params。
#[derive(Clone, Debug)]
pub struct RecorderStartParams {
    /// session 表示名（`None` → `"recorder"`）。
    pub name: Option<String>,
    /// Transport 開始 tick。
    pub transport_start: MusicalTick,
    /// 停止 tick。
    pub stop: MusicalTick,
    /// session-scope loop 宣言 `[loop_start, loop_end)`
    /// （`syn.monitoring.session` と同じ契約 — gate は plan に凍結）。
    pub loop_marks: Option<(MusicalTick, MusicalTick)>,
    /// Clock Epoch（`None` は project の最小 epoch —
    /// `syn.monitoring.session` と同じ規則）。
    pub clock_epoch: Option<ClockEpochId>,
    /// session sample rate [frame/sec]。
    pub sample_rate: u32,
    /// 1 callback の最大 frame 数（`None` は既定 1024）。
    pub max_block_frames: Option<u32>,
    /// staging root — `StagingArea::create` の root。
    /// session dir は `<root>/<recording-session-tag>/`。
    pub staging_root: PathBuf,
    /// capture targets — index 順が `stream_index` 順。
    pub targets: Vec<CaptureTarget>,
    /// 入力 stream 束縛（先頭は `InputSession::open`、以降は
    /// `add` で same-clock/epoch/stream_index 一意性を検証）。
    pub streams: Vec<RecorderStreamParams>,
}

/// `syn.recorder.start` の結果。
#[derive(Clone, Copy, Debug)]
pub struct RecorderStartOutcome {
    /// project に作成された recording session。
    pub session_id: RecordingSessionId,
}

/// `syn.recorder.stop` の結果。
#[derive(Clone, Copy, Debug)]
pub struct RecorderStopOutcome {
    /// 接続された capture stream 数。
    pub streams: usize,
    /// finalize まで到達した stream 数。
    pub finalized: usize,
}

/// 稼働 recorder session の位相。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RecorderPhase {
    /// stream 稼働中（capture 可能）。
    Recording,
    /// stream 停止・writer finalize 済み。commit 待ち。
    Stopped,
}

/// 稼働中の recorder session — project 側 session/stream ID、
/// preflight 確定 plan、`InputSession`/`StreamWriter`/`StagingArea`
/// の runtime handle を保持する。永続 entity は
/// `Project.recording` が正本であり、ここに複写しない。
struct ActiveRecording {
    /// project の recording session ID。
    session_id: RecordingSessionId,
    /// session 表示名。
    name: String,
    /// session sample rate（staging/finalize の rate 権威）。
    sample_rate: u32,
    /// staging root（status 投影用）。
    staging_root: PathBuf,
    /// 位相。
    phase: RecorderPhase,
    /// `preflight` 確定 plan（gate scheduler 供給元）。
    plan: PreparedRecordingPlan,
    /// same-clock 検証済み入力 session。
    input: InputSession,
    /// `(stream_index, StreamWriter)` を index 昇順で保持。
    writers: Vec<(u16, StreamWriter)>,
    /// staging 領域（manifest/media/journal/finalized）。
    area: StagingArea,
}

/// `syn.recorder.*` の失敗分類 —
/// [`RecorderError::into_body`] が wire `ErrorBody` へ写す。
#[derive(Debug)]
pub enum RecorderError {
    /// recorder session が既に稼働中。
    Busy,
    /// 入力 device が注入されていない。
    NoDevice,
    /// `streams[i].device` が注入 device 列の範囲外。
    DeviceIndex {
        /// `streams` の index。
        stream: usize,
        /// 指定された device index。
        device: usize,
    },
    /// spec/params の検証失敗（空 `taps`・`stream_index` 範囲外・
    /// channel 数不一致・armed tap なし等）。
    Spec(String),
    /// `preflight`/project 層の検証失敗。
    Plan(String),
    /// project op の apply 失敗。
    Project(String),
    /// `InputSession`/`InputAdapter` の bind 失敗。
    Bind(InputBindError),
    /// staging 領域の失敗。
    Staging(StagingError),
    /// stream writer の失敗。
    Writer(WriterError),
    /// device open/start/stop の失敗。
    Device(DeviceError),
    /// commit pipeline の失敗。
    Commit(CommitError),
    /// capture-fault marker 等の I/O 失敗。
    Io(String),
    /// op が要求する位相でない。
    Phase(&'static str),
    /// 稼働中の recorder session が無い。
    NoSession,
}

impl fmt::Display for RecorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("recorder session already active"),
            Self::NoDevice => f.write_str("no recorder input device injected"),
            Self::DeviceIndex { stream, device } => {
                write!(f, "streams[{stream}].device {device} out of range")
            }
            Self::Spec(m) => write!(f, "recorder spec: {m}"),
            Self::Plan(m) => write!(f, "recorder plan: {m}"),
            Self::Project(m) => write!(f, "recorder project op: {m}"),
            Self::Bind(e) => write!(f, "recorder input bind: {e}"),
            Self::Staging(e) => write!(f, "recorder staging: {e}"),
            Self::Writer(e) => write!(f, "recorder writer: {e}"),
            Self::Device(e) => write!(f, "recorder device: {e}"),
            Self::Commit(e) => write!(f, "recorder commit: {e}"),
            Self::Io(m) => write!(f, "recorder io: {m}"),
            Self::Phase(m) => write!(f, "recorder phase: {m}"),
            Self::NoSession => f.write_str("no active recorder session"),
        }
    }
}

impl std::error::Error for RecorderError {}

impl RecorderError {
    /// wire `ErrorBody` への写像 — 呼出側が修正できる spec/状態系は
    /// `invalid_params`、device/staging/writer/commit/IO 等の
    /// infrastructure 失敗は `service_error`。
    #[must_use]
    pub fn into_body(self) -> ruviform_ui::ipc::ErrorBody {
        use ruviform_ui::ipc::{ErrorBody, codes};
        let code = match &self {
            Self::Busy
            | Self::NoDevice
            | Self::DeviceIndex { .. }
            | Self::Spec(_)
            | Self::Plan(_)
            | Self::Project(_)
            | Self::Phase(_)
            | Self::NoSession
            | Self::Bind(
                InputBindError::EmptyTaps
                | InputBindError::ZeroCapacity
                | InputBindError::EmptyChannelMap
                | InputBindError::ChannelOutOfRange
                | InputBindError::DuplicateStreamIndex
                | InputBindError::UnsupportedSampleFormat
                | InputBindError::CapacityOverflow
                | InputBindError::FormatMismatch
                | InputBindError::ClockDomainUnknown
                | InputBindError::ClockDomainMismatch
                | InputBindError::ClockEpochMismatch,
            ) => codes::INVALID_PARAMS,
            _ => codes::SERVICE_ERROR,
        };
        ErrorBody::new(code, self.to_string())
    }
}

/// channel 数から `ChannelLayout` を導出する（`synvelis-recording`
/// の test fixture と同じ規則 — Mono/Stereo/Quad/5.1/7.1 は既知
/// layout、それ以外は low-bit speaker mask）。
fn layout_for(channels: u32) -> ChannelLayout {
    match channels {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        4 => ChannelLayout::Quad,
        6 => ChannelLayout::FivePointOne,
        8 => ChannelLayout::SevenPointOne,
        n if n < 32 => ChannelLayout::custom(SpeakerMask::from_bits((1_u32 << n) - 1))
            .unwrap_or(ChannelLayout::Mono),
        _ => ChannelLayout::custom(SpeakerMask::from_bits(u32::MAX)).unwrap_or(ChannelLayout::Mono),
    }
}

/// gate 構築 — `syn.monitoring.session` と同じ規則: loop 宣言があれば
/// `Loop`/`PunchInLoop`、無ければ authored punch（`project.monitoring.
/// punch` が正本）→ `Punch`/`Normal`。
///
/// `loop_start < transport_start` は session frame が負になり
/// `GateScheduler` が構成できないため、monitoring と同じく
/// start 時に拒否する（`synvelis-recording` の `preflight` は
/// loop/stop 関係のみ検査し、この制約は持たない）。
fn recorder_gate(
    project: &Project,
    params: &RecorderStartParams,
) -> Result<GateSpec, RecorderError> {
    if let Some((loop_start, _)) = params.loop_marks
        && loop_start < params.transport_start
    {
        return Err(RecorderError::Spec(
            "loop_start must not precede transport_start".to_owned(),
        ));
    }
    let punch = project.monitoring.punch;
    Ok(match params.loop_marks {
        Some((loop_start, loop_end)) => match (punch.punch_in, punch.punch_out) {
            (Some(punch_in), Some(punch_out)) => GateSpec::PunchInLoop {
                loop_start,
                loop_end,
                punch_in,
                punch_out,
            },
            _ => GateSpec::Loop {
                loop_start,
                loop_end,
            },
        },
        None => match (punch.punch_in, punch.punch_out) {
            (Some(punch_in), Some(punch_out)) => GateSpec::Punch {
                punch_in,
                punch_out,
            },
            _ => GateSpec::Normal {
                open: params.transport_start,
            },
        },
    })
}

/// `syn.recorder.start` の spec 検証 — domain rule の複写はせず、
/// wire 契約固有の整合（device index・`stream_index` の target 範囲・
/// armed tap の channel 数一致・armed tap の存在）のみを確認する。
fn validate_stream_params(
    params: &RecorderStartParams,
    device_count: usize,
) -> Result<(), RecorderError> {
    if params.targets.is_empty() {
        return Err(RecorderError::Spec("targets must not be empty".to_owned()));
    }
    for (ti, target) in params.targets.iter().enumerate() {
        if target.channels == 0 || target.channels > MAX_TARGET_CHANNELS {
            return Err(RecorderError::Spec(format!(
                "targets[{ti}].channels must be in 1..={MAX_TARGET_CHANNELS}"
            )));
        }
    }
    if params.streams.is_empty() {
        return Err(RecorderError::Spec("streams must not be empty".to_owned()));
    }
    let mut armed = 0usize;
    for (si, stream) in params.streams.iter().enumerate() {
        if stream.device >= device_count {
            return Err(RecorderError::DeviceIndex {
                stream: si,
                device: stream.device,
            });
        }
        if stream.channels == 0 || stream.channels > 32 {
            return Err(RecorderError::Spec(format!(
                "streams[{si}].channels must be in 1..=32"
            )));
        }
        if stream.taps.is_empty() {
            return Err(RecorderError::Spec(format!(
                "streams[{si}].taps must not be empty"
            )));
        }
        for tap in &stream.taps {
            let idx = usize::from(tap.stream_index);
            let target = params.targets.get(idx).ok_or_else(|| {
                RecorderError::Spec(format!(
                    "stream_index {} has no capture target",
                    tap.stream_index
                ))
            })?;
            if tap.arm == ArmState::On {
                armed += 1;
                let map_channels = u32::try_from(tap.channel_map.len())
                    .map_err(|_| RecorderError::Spec("channel_map too large".to_owned()))?;
                if target.channels != map_channels {
                    return Err(RecorderError::Spec(format!(
                        "stream_index {} channel count mismatch: target {} vs channel_map {}",
                        tap.stream_index, target.channels, map_channels
                    )));
                }
            }
        }
    }
    if armed == 0 {
        return Err(RecorderError::Spec(
            "no armed taps — nothing would be recorded".to_owned(),
        ));
    }
    Ok(())
}

/// `RecorderStreamParams` → `InputStreamSpec`（`session_end`/`budget`/
/// `max_block_frames` は確定 plan 由来）。
fn input_stream_spec(
    stream: &RecorderStreamParams,
    plan: &PreparedRecordingPlan,
    clock_epoch: ClockEpochId,
) -> Result<InputStreamSpec, RecorderError> {
    let rate = SampleRate::new(plan.sample_rate)
        .map_err(|_| RecorderError::Spec("sample_rate out of range".to_owned()))?;
    let format = AudioFormat {
        sample_rate: rate,
        layout: layout_for(stream.channels),
        sample_format: SampleFormat::F32,
    };
    let taps = stream
        .taps
        .iter()
        .map(|t| {
            Ok(InputTapSpec {
                binding: InputBinding {
                    channel_map: ChannelMap::new(t.channel_map.clone())
                        .map_err(RecorderError::Bind)?,
                    arm: t.arm,
                    monitor: t.monitor,
                },
                stream_index: t.stream_index,
                session_end: plan.session_end,
                budget: plan.budget,
            })
        })
        .collect::<Result<Vec<_>, RecorderError>>()?;
    Ok(InputStreamSpec {
        config: InputStreamConfig {
            format,
            buffer_size: BufferSize::Default,
        },
        clock_epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: CALLBACK_QUEUE,
        taps,
    })
}

/// armed tap から staging stream 設定を組み立てる
/// （`synvelis-recording` test fixture の `staging_streams` と同じ
/// 規則 — `stream_index` は `plan.targets` の添字、
/// `capture_stream_id` は project 登録済み `CaptureStreamId`）。
fn staging_configs(
    plan: &PreparedRecordingPlan,
    specs: &[InputStreamSpec],
    stream_ids: &[CaptureStreamId],
) -> Result<Vec<StagingStreamConfig>, RecorderError> {
    let mut out = Vec::new();
    for spec in specs {
        for tap in &spec.taps {
            if tap.binding.arm != ArmState::On {
                continue;
            }
            let idx = usize::from(tap.stream_index);
            let target = plan.targets.get(idx).ok_or_else(|| {
                RecorderError::Spec(format!(
                    "stream_index {} has no plan target",
                    tap.stream_index
                ))
            })?;
            let capture_stream_id = stream_ids.get(idx).copied().ok_or_else(|| {
                RecorderError::Spec(format!(
                    "stream_index {} has no registered capture stream",
                    tap.stream_index
                ))
            })?;
            out.push(stream_config(
                tap.stream_index,
                capture_stream_id,
                target.lane_id,
                target.channels,
            ));
        }
    }
    out.sort_by_key(|c| c.stream_index);
    Ok(out)
}

/// `TapWriterEnds` 列を manifest の stream 設定と照合して
/// `StreamWriter` を構築する（`stream_index` 昇順）。
fn build_writers(
    area: &StagingArea,
    ends: Vec<TapWriterEnds>,
    fault: &FaultLatch,
) -> Result<Vec<(u16, StreamWriter)>, RecorderError> {
    let mut writers = Vec::with_capacity(ends.len());
    for w in ends {
        let cfg = area
            .manifest
            .streams
            .iter()
            .find(|m| m.stream_index == w.stream_index)
            .map(|m| StagingStreamConfig {
                stream_index: m.stream_index,
                capture_stream_id: m.capture_stream_id.clone(),
                lane_id: m.lane_id.clone(),
                channels: m.channels,
            })
            .ok_or_else(|| {
                RecorderError::Spec(format!(
                    "no manifest stream for stream_index {}",
                    w.stream_index
                ))
            })?;
        writers.push((
            w.stream_index,
            StreamWriter::create(area, cfg, w.ends, fault.clone())
                .map_err(RecorderError::Writer)?,
        ));
    }
    writers.sort_by_key(|(i, _)| *i);
    Ok(writers)
}

/// project 層へ recording op を適用する（`session.apply` 経由 —
/// engine port・control session の post-commit 通知を通す）。
fn apply_recording(session: &mut ProjectSession, op: RecordingOp) -> Result<(), RecorderError> {
    let revision = session.revision();
    session
        .apply(SynvelisOp::Recording(op), revision)
        .map(|_| ())
        .map_err(|e| RecorderError::Project(e.to_string()))
}

/// apply 直後の project map に新規追加された key を返す
/// （id generator は単調増加のため最新 = 最大 key）。
fn newest_key<K: Ord + Copy, V>(map: &BTreeMap<K, V>) -> Option<K> {
    map.keys().next_back().copied()
}

/// `rig_session` への prepared 入力束 — `syn.recorder.start` が
/// preflight/device open を終えた時点の値をまとめる。
struct RigInputs<'a> {
    /// wire params（staging root/sample rate の参照元）。
    params: &'a RecorderStartParams,
    /// `preflight` 確定 plan。
    plan: &'a PreparedRecordingPlan,
    /// `InputStreamSpec` 列。
    specs: &'a [InputStreamSpec],
    /// 解決済み clock epoch。
    clock_epoch: ClockEpochId,
    /// `CreateSession` で採番した session ID。
    session_id: RecordingSessionId,
    /// `InputSession::open`/`add` が返した writer endpoint 列。
    writer_ends: Vec<TapWriterEnds>,
    /// open 済み `InputSession`（未 start）。
    input: &'a InputSession,
}

/// `syn.recorder.start` の project mutation 以降 —
/// `OpenSession`→`RegisterStream`→`StagingArea`→`StreamWriter`→
/// stream start。失敗時は呼出側が best-effort `CloseSession` で
/// Open session を残さない（`input`/`writer_ends` は drop で
/// stream 停止・endpoint 解放される）。
fn rig_session(
    session: &mut ProjectSession,
    inputs: RigInputs<'_>,
) -> Result<(StagingArea, Vec<(u16, StreamWriter)>), RecorderError> {
    let RigInputs {
        params,
        plan,
        specs,
        clock_epoch,
        session_id,
        writer_ends,
        input,
    } = inputs;
    apply_recording(session, RecordingOp::OpenSession { session_id })?;
    let mut stream_ids = Vec::with_capacity(plan.targets.len());
    for target in &plan.targets {
        apply_recording(
            session,
            RecordingOp::RegisterStream {
                session_id,
                lane_id: target.lane_id,
                clock_epoch_id: clock_epoch,
            },
        )?;
        stream_ids.push(
            newest_key(&session.project().recording.streams)
                .ok_or_else(|| RecorderError::Project("no stream after register".to_owned()))?,
        );
    }
    let streams_cfg = staging_configs(plan, specs, &stream_ids)?;
    let area = StagingArea::create(
        &params.staging_root,
        &StagingSessionConfig {
            session_id,
            sample_rate: params.sample_rate,
            streams: streams_cfg,
        },
    )
    .map_err(RecorderError::Staging)?;
    let writers = build_writers(&area, writer_ends, input.fault())?;
    for adapter in input.adapters() {
        adapter.stream().start().map_err(RecorderError::Device)?;
    }
    Ok((area, writers))
}

/// recorder session owner — `SynvelisApp` が保持し、
/// `project.new`/`open` で session 部分のみ reset される
/// （注入 device は host resource であり project state ではないため
/// 保持する）。
#[derive(Default)]
pub struct RecorderOwner {
    /// 注入済み入力 device（`set_devices` で host が供給）。
    devices: Vec<Box<dyn Device>>,
    /// 稼働中の recorder session。
    active: Option<ActiveRecording>,
}

impl RecorderOwner {
    /// 注入 device 列を置き換える — 実 device 列挙・選択は後続
    /// slice の責務であり、本 seam は `&dyn Device` のみを受け取る。
    pub fn set_devices(&mut self, devices: Vec<Box<dyn Device>>) {
        self.devices = devices;
    }

    /// 注入 device 数（capabilities 投影用）。
    #[must_use]
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// 稼働 session があるか（`syn.window.close_guard` の
    /// `recorder_active` 判定に使う — Recording/Stopped の双方で
    /// 未 commit media を失わないため veto する）。
    #[must_use]
    pub fn active(&self) -> bool {
        self.active.is_some()
    }

    /// 位相文字列 — `"idle"` / `"recording"` / `"stopped"`。
    #[must_use]
    pub fn status(&self) -> &'static str {
        match self.active.as_ref().map(|a| a.phase) {
            None => "idle",
            Some(RecorderPhase::Recording) => "recording",
            Some(RecorderPhase::Stopped) => "stopped",
        }
    }

    /// `syn.recorder.status` の投影。
    #[must_use]
    pub fn status_json(&self) -> Value {
        let (state, session_id, name, sample_rate, staging_root, fault_bits) =
            match self.active.as_ref() {
                None => (
                    "idle",
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                ),
                Some(a) => (
                    self.status(),
                    json!(a.session_id.to_tagged()),
                    json!(a.name),
                    json!(a.sample_rate),
                    json!(a.staging_root.display().to_string()),
                    json!(format!("{:#06x}", a.input.fault().bits())),
                ),
            };
        json!({
            "state": state,
            "active": self.active.is_some(),
            "session_id": session_id,
            "name": name,
            "sample_rate": sample_rate,
            "staging_root": staging_root,
            "devices": self.devices.len(),
            "fault_bits": fault_bits,
        })
    }

    /// `syn.recorder.start` — preflight → `InputSession::open`/`add`
    /// → project session/stream 登録 → `StagingArea`/`StreamWriter`
    /// → stream start までを一括で行う。
    ///
    /// device open は project mutation より先に行うため、device 起因
    /// の失敗は project を汚さない。project mutation 後の失敗
    /// （staging/writer/start）は best-effort で `CloseSession` を
    /// 適用して Open session を残さない。
    ///
    /// # Errors
    /// [`RecorderError`] — spec/preflight/project/bind/staging/
    /// writer/device の各失敗を typed に返す。
    pub fn start(
        &mut self,
        session: &mut ProjectSession,
        params: RecorderStartParams,
    ) -> Result<RecorderStartOutcome, RecorderError> {
        if self.active.is_some() {
            return Err(RecorderError::Busy);
        }
        if self.devices.is_empty() {
            return Err(RecorderError::NoDevice);
        }
        validate_stream_params(&params, self.devices.len())?;
        let project = session.project();
        let clock_epoch = params
            .clock_epoch
            .or_else(|| project.clock_epochs.keys().next().copied())
            .ok_or_else(|| {
                RecorderError::Plan(
                    "project has no clock epoch — register one before starting a recorder session"
                        .to_owned(),
                )
            })?;
        let gate = recorder_gate(project, &params)?;
        let request = RecordingRequest {
            targets: params.targets.clone(),
            clock_epoch,
            tempo_map: TempoMapRef {
                revision: project.tempo_map.revision,
            },
            transport_start: params.transport_start,
            stop: params.stop,
            gate,
            budget: DEFAULT_BUDGET,
            max_block_frames: params.max_block_frames.unwrap_or(DEFAULT_MAX_BLOCK_FRAMES),
        };
        let plan = preflight(project, request, params.sample_rate)
            .map_err(|e| RecorderError::Plan(e.to_string()))?;
        let specs = params
            .streams
            .iter()
            .map(|s| input_stream_spec(s, &plan, clock_epoch))
            .collect::<Result<Vec<_>, _>>()?;

        // --- device open（project mutation より先） ---
        let (mut input, mut writer_ends) =
            InputSession::open(self.devices[params.streams[0].device].as_ref(), &specs[0])
                .map_err(RecorderError::Bind)?;
        for (i, spec) in specs.iter().enumerate().skip(1) {
            let device = params.streams[i].device;
            let ends = input
                .add(self.devices[device].as_ref(), spec)
                .map_err(RecorderError::Bind)?;
            writer_ends.extend(ends);
        }

        // --- project mutation（CreateSession→Open→RegisterStream） ---
        let name = params.name.clone().unwrap_or_else(|| "recorder".to_owned());
        apply_recording(session, RecordingOp::CreateSession { name: name.clone() })?;
        let session_id = newest_key(&session.project().recording.sessions)
            .ok_or_else(|| RecorderError::Project("no session after create".to_owned()))?;
        let rig = rig_session(
            session,
            RigInputs {
                params: &params,
                plan: &plan,
                specs: &specs,
                clock_epoch,
                session_id,
                writer_ends,
                input: &input,
            },
        );
        match rig {
            Ok((area, writers)) => {
                self.active = Some(ActiveRecording {
                    session_id,
                    name,
                    sample_rate: params.sample_rate,
                    staging_root: params.staging_root,
                    phase: RecorderPhase::Recording,
                    plan,
                    input,
                    writers,
                    area,
                });
                Ok(RecorderStartOutcome { session_id })
            }
            Err(e) => {
                // Open session を残さない best-effort cleanup。
                // `input`/`writer_ends` は drop で stream 停止・
                // endpoint 解放される（未 start の stream は副作用なし）。
                let _ = apply_recording(session, RecordingOp::CloseSession { session_id });
                Err(e)
            }
        }
    }

    /// `syn.recorder.pump` — device callback が積んだ work queue を
    /// drain し engine → writer へ流す非 RT pump
    /// （`InputSession::pump_all` + 全 `StreamWriter::pump`）。
    /// 戻り値は fault latch が立っているか。
    ///
    /// # Errors
    /// session 無し・位相が `Recording` でない場合。
    pub fn pump(&mut self) -> Result<bool, RecorderError> {
        let active = self.active.as_mut().ok_or(RecorderError::NoSession)?;
        if active.phase != RecorderPhase::Recording {
            return Err(RecorderError::Phase("pump requires a recording session"));
        }
        active.input.pump_all(&active.plan.scheduler);
        for (_, w) in &mut active.writers {
            w.pump();
        }
        Ok(active.input.fault().is_faulted())
    }

    /// `syn.recorder.stop` — 全 stream を停止し、capture-fault が
    /// あれば marker を記録した上で checkpoint → finalize
    /// （`finish_all`）まで進める。位相は `Stopped` になり、
    /// project session は `Open` のまま commit を待つ。
    ///
    /// # Errors
    /// session 無し・位相が `Recording` でない場合、stop/checkpoint/
    /// finalize の失敗（失敗時は位相を進めない）。
    pub fn stop(&mut self) -> Result<RecorderStopOutcome, RecorderError> {
        let active = self.active.as_mut().ok_or(RecorderError::NoSession)?;
        if active.phase != RecorderPhase::Recording {
            return Err(RecorderError::Phase("stop requires a recording session"));
        }
        let mut first_err = None;
        for adapter in active.input.adapters() {
            if let Err(e) = adapter.stream().stop()
                && first_err.is_none()
            {
                first_err = Some(e);
            }
        }
        if let Some(e) = first_err {
            return Err(RecorderError::Device(e));
        }
        if active.input.fault().is_faulted() {
            let detail = format!("fault bits {:#06x}", active.input.fault().bits());
            active
                .area
                .record_capture_fault(&detail)
                .map_err(|e| RecorderError::Io(e.to_string()))?;
        }
        for (_, w) in &mut active.writers {
            w.checkpoint().map_err(RecorderError::Writer)?;
        }
        let tag = active.area.manifest.session_id.clone();
        let report = finish_all(
            &tag,
            active.sample_rate,
            active.writers.iter_mut().map(|(_, w)| w),
        );
        let mut finalized = 0usize;
        for (_, result) in report.refs {
            match result {
                // `AlreadyFinalized` は finalized.json 公開済み =
                // 停止契約の完了条件を満たす。部分 finalize 失敗からの
                // retry で先行 stream が再 finalize されても成功扱い
                // にしないと stop が永久に wedge する。
                Ok(_) | Err(WriterError::AlreadyFinalized) => finalized += 1,
                Err(e) => return Err(RecorderError::Writer(e)),
            }
        }
        active.phase = RecorderPhase::Stopped;
        Ok(RecorderStopOutcome {
            streams: active.writers.len(),
            finalized,
        })
    }

    /// `syn.recorder.commit` — `CloseSession` 適用後に
    /// `commit_session` で finalized media を project へ commit する
    /// （内部で `CommitRecording` を `ProjectController::apply`）。
    /// commit が project revision を進めた場合は結果の如何に関わらず
    /// `notify_committed_change` で engine/control 通知を replay する
    /// — `commit_session` は `CommitRecording` 適用後に marker 書込み
    /// が失敗すると `Err` を返し得、その場合 retry の `AlreadyApplied`
    /// は revision を進めず再通知できないため。
    ///
    /// outcome が commit 確定（`Applied`/`AlreadyApplied`）なら
    /// session を破棄する。`Failed`/`IntegrityConflict`/
    /// `NothingCommitted` は project session が `Closed` のまま残り
    /// wire に session 除去 op も無いため、`stopped` session を
    /// 保持して retry/discard を可能にする（drop すると孤立する）。
    ///
    /// # Errors
    /// session 無し・位相が `Stopped` でない場合、CloseSession/
    /// commit の失敗（失敗時は session を `Stopped` のまま保持し
    /// retry 可能にする）。
    pub fn commit(&mut self, session: &mut ProjectSession) -> Result<CommitReport, RecorderError> {
        let active = self.active.take().ok_or(RecorderError::NoSession)?;
        if active.phase != RecorderPhase::Stopped {
            self.active = Some(active);
            return Err(RecorderError::Phase(
                "commit requires a stopped recorder session",
            ));
        }
        let session_id = active.session_id;
        // `CloseSession` は `Open` からのみ validate が通る
        // （`Closed`/`Finalized` への再適用は reject）。
        // `commit_session` 失敗後の retry では session が既に
        // `Closed`/`Finalized` のため、適用済みなら skip して
        // `commit_session` の冪等経路（`commit_status` →
        // `AlreadyApplied`/`IntegrityConflict`）に委ねる。
        let state = session
            .project()
            .recording
            .sessions
            .get(&session_id)
            .map(|s| s.state);
        match state {
            Some(SessionState::Open) => {
                if let Err(e) = apply_recording(session, RecordingOp::CloseSession { session_id }) {
                    self.active = Some(active);
                    return Err(e);
                }
            }
            Some(SessionState::Closed | SessionState::Finalized) => {}
            _ => {
                self.active = Some(active);
                return Err(RecorderError::Project(format!(
                    "recording session {session_id} missing or not open"
                )));
            }
        }
        let revision_before = session.revision();
        let result = commit_session(session.controller_mut(), &active.area);
        if session.revision() != revision_before {
            session.notify_committed_change();
        }
        match result {
            Ok(report) => {
                let terminal = matches!(
                    report.outcome,
                    CommitOutcome::Applied { .. } | CommitOutcome::AlreadyApplied
                );
                if !terminal {
                    self.active = Some(active);
                }
                Ok(report)
            }
            Err(e) => {
                self.active = Some(active);
                Err(RecorderError::Commit(e))
            }
        }
    }

    /// `syn.recorder.discard` — 稼働 session を commit せず破棄する。
    /// Recording 中なら stream を停止し、project session には
    /// best-effort で `CloseSession` を適用して Open のまま残さない
    /// （staging file は recovery 対象として disk に残る —
    /// `StagingArea` に削除 API は無い）。戻り値は session が
    /// closed 状態に到達したか（`Open` から `CloseSession` を
    /// 適用した場合と、既に `Closed`/`Finalized` だった場合の
    /// 双方で true）。
    ///
    /// # Errors
    /// session 無しの場合。
    pub fn discard(&mut self, session: &mut ProjectSession) -> Result<bool, RecorderError> {
        let active = self.active.take().ok_or(RecorderError::NoSession)?;
        if active.phase == RecorderPhase::Recording {
            for adapter in active.input.adapters() {
                let _ = adapter.stream().stop();
            }
        }
        // `CloseSession` は `Open` からのみ有効。既に
        // `Closed`/`Finalized`（例: commit 失敗後の discard）
        // なら目標状態を満たすため closed=true として報告する。
        let state = session
            .project()
            .recording
            .sessions
            .get(&active.session_id)
            .map(|s| s.state);
        let closed = match state {
            Some(SessionState::Closed | SessionState::Finalized) => true,
            _ => apply_recording(
                session,
                RecordingOp::CloseSession {
                    session_id: active.session_id,
                },
            )
            .is_ok(),
        };
        Ok(closed)
    }

    /// project session 再生成時の reset — 稼働 session を破棄する
    /// （旧 project の session/stream ID は無効になるため project
    /// op は適用しない）。Recording 中なら stream を停止する。
    /// 注入 device は保持する。
    pub fn reset(&mut self) {
        if let Some(active) = self.active.take()
            && active.phase == RecorderPhase::Recording
        {
            for adapter in active.input.adapters() {
                let _ = adapter.stream().stop();
            }
        }
    }
}

/// commit outcome の wire 文字列。
#[must_use]
pub fn commit_outcome_name(outcome: &CommitOutcome) -> &'static str {
    match outcome {
        CommitOutcome::Applied { .. } => "applied",
        CommitOutcome::AlreadyApplied => "already-applied",
        CommitOutcome::NothingCommitted => "nothing-committed",
        CommitOutcome::IntegrityConflict => "integrity-conflict",
        CommitOutcome::Failed { .. } => "failed",
    }
}

/// take state の wire 文字列（`syn.recorder.mark_take_verified`
/// の結果投影用）。
#[must_use]
pub fn take_state_name(state: TakeState) -> &'static str {
    take_state(state)
}
