//! RCD-03 monitor/punch Fake Session fixture（synvelis#117 slice）。
//!
//! `DeviceRig`（fake device → `InputSession` → `StreamWriter` →
//! `StagingArea`）と `MonitorProbe`（`GateScheduler`/`evaluate_gate`
//! を共有する monitor route 観測）を同一 block 駆動で合成し、
//! monitor/punch の route・fault・recovery・session 分離・commit
//! 意味論を session/staging/commit 層まで検証する。
//!
//! これは fake port/headless fixture 契約の検証であり、実 Device・
//! 実 GUI・実 OS での monitor 受入ではない（NotRun — 実機 gate は
//! 後続 slice）。monitor/arm は `InputTapSpec.binding` の metadata
//! であり capture path に入らない構造を、session 全体を通した
//! 観測可能な振る舞いとして pin する。

// 各 test binary が common の subset だけを使うため module 単位で許容
// （device_recording.rs と同じ運用）。
#[allow(dead_code)]
mod common;

use common::*;
use ruviform_audio_core::device::DeviceError;
use ruviform_media_core::ClockDomainId;
use synvelis_project::{CaptureStream, CaptureStreamId, MusicalTick, Project, RecordingSessionId};
use synvelis_recording::MonitorRoute::{Input, Muted, Playback};
use synvelis_recording::{
    ArmState, BoundedVec, ChannelMap, CommitPayload, FAULT_INPUT_DISCONTINUITY, FinalizedOutcome,
    GateSpec, InputStreamSpec, MonitorProbe, MonitorRoute, MonitorSegment, MonitorState,
    PreparedRecordingPlan, StopReason, TrackMonitorContext, TransportPhase, build_payload,
};

// ---------- fixture 定数 ----------

/// punch [12,28) tick = session frame [300,700)、stop 40 tick =
/// `session_end` 1000 frame（25 f/tick）。
const STOP_TICK: i64 = 40;
const GATE_OPEN: (i64, i64) = (300, 700);
const GATE_SPAN_FRAMES: i64 = GATE_OPEN.1 - GATE_OPEN.0;
const GATE_SPAN_FRAMES_U64: u64 = 400;
/// `session_end` を割り切らず punch 境界を block 途中に跨がせる
/// block サイズ。
const BLOCK: u32 = 384;

fn session_id(raw: u128) -> RecordingSessionId {
    RecordingSessionId::new(raw)
}

fn punch_gate() -> GateSpec {
    GateSpec::Punch {
        punch_in: MusicalTick::new(12),
        punch_out: MusicalTick::new(28),
    }
}

/// capture 側 rig・project・session id と、monitor 観測側の
/// `PreparedRecordingPlan`（同一 spec の別 instance）を構築する。
/// `PreparedRecordingPlan` は非 `Clone` で `MonitorProbe<'a>` が
/// `&plan` を借用するため、capture 側（`DeviceRig` が所有）と
/// monitor 側に同一 spec から `preflight` を 2 回呼ぶ。実系でも
/// monitor session は録音側と同一の prepared plan を参照する
/// 契約であり、fixture では同一内容の別 instance で表現する。
fn open_session(
    name: &str,
    sid: u128,
    monitor: MonitorState,
) -> (
    DeviceRig,
    Project,
    RecordingSessionId,
    PreparedRecordingPlan,
) {
    let root = tmp_dir(name);
    let (mut p, lanes, epoch) = project_with_lanes(1);
    let gate = punch_gate();
    let plan = plan_gated(&p, &lanes, &[1], epoch, gate, STOP_TICK);
    let probe_plan = plan_gated(&p, &lanes, &[1], epoch, gate, STOP_TICK);
    let device = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK).with_seed(SEED);
    let spec = InputStreamSpec {
        config: input_config(1),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(&plan, 0, ChannelMap::mono(0), ArmState::On, monitor)],
    };
    let sid = session_id(sid);
    // commit 層は staging の capture_stream_id が project の
    // `recording.streams` に登録済みであることを要求する（本番
    // では `RegisterStream` op が採番する）。staging 側は同一
    // ID を使うよう明示指定する。
    let stream_id = CaptureStreamId::new(500);
    p.recording.streams.insert(
        stream_id,
        CaptureStream {
            id: stream_id,
            session_id: sid,
            lane_id: lanes[0],
            clock_epoch_id: epoch,
        },
    );
    let rig = DeviceRig::open(&root, sid, plan, &[(&device, &spec)], Some(&[stream_id]));
    (rig, p, sid, probe_plan)
}

/// `DeviceRig` の block 駆動に追従する monitor route 観測。
/// `MonitorProbe` が借用する plan は capture 側と同一 spec の
/// 別 instance（`open_session` が返す `probe_plan`）。
struct SessionProbe<'a> {
    inner: MonitorProbe<'a>,
    out: BoundedVec<MonitorSegment>,
    segments: Vec<MonitorSegment>,
}

impl<'a> SessionProbe<'a> {
    fn new(plan: &'a PreparedRecordingPlan, ctx: TrackMonitorContext) -> Self {
        Self {
            inner: MonitorProbe::new(plan, ctx),
            out: BoundedVec::with_capacity(64),
            segments: Vec::new(),
        }
    }

    /// `rig.drive` と同じ block 刻みで feed する。
    fn feed_block(&mut self, frames: u32, phase: TransportPhase) {
        self.inner.feed(frames, phase, &mut self.out);
        assert!(!self.inner.faulted(), "monitor probe faulted");
        self.segments.extend_from_slice(self.out.as_slice());
    }

    fn finished(&self) -> bool {
        self.inner.finished()
    }
}

/// `frames` を `block` 刻みで deliver→pump し、各 block で
/// `probe.feed_block` を呼ぶ（`rig.drive` + monitor 観測の合成）。
/// `rig.drive` と同一の block 刻み・frame 数で feed するため、
/// probe の session 位置は capture と 1:1 に保たれる。
fn drive_with_probe(
    rig: &mut DeviceRig,
    probe: &mut SessionProbe<'_>,
    frames: u32,
    block: u32,
    phase: TransportPhase,
) {
    let mut remaining = frames;
    while remaining > 0 {
        let n = block.min(remaining);
        rig.deliver_all(n);
        rig.pump_all();
        probe.feed_block(n, phase);
        remaining -= n;
    }
    assert!(probe.finished(), "probe did not reach session_end");
}

/// `pred` を満たす segment の session 区間を merge した一覧。
fn merged_regions(
    segments: &[MonitorSegment],
    pred: impl Fn(&MonitorSegment) -> bool,
) -> Vec<(i64, i64)> {
    let mut out: Vec<(i64, i64)> = Vec::new();
    for s in segments.iter().filter(|s| pred(s)) {
        let start = s.session.start().value();
        let end = s.session.end().value();
        match out.last_mut() {
            Some(last) if last.1 == start => last.1 = end,
            _ => out.push((start, end)),
        }
    }
    out
}

/// route ごとの連続区間（隣接する同 route segment を merge）。
fn route_regions(segments: &[MonitorSegment]) -> Vec<(i64, i64, MonitorRoute)> {
    let mut out: Vec<(i64, i64, MonitorRoute)> = Vec::new();
    for s in segments {
        let start = s.session.start().value();
        let end = s.session.end().value();
        match out.last_mut() {
            Some(last) if last.2 == s.route && last.1 == start => last.1 = end,
            _ => out.push((start, end, s.route)),
        }
    }
    out
}

/// segment 列が `[0, session_end)` を隙間・重複なく tile する
/// ことを検証する（probe segment の連続性 pin）。
fn assert_segment_tiling(segments: &[MonitorSegment], session_end: i64) {
    assert!(!segments.is_empty(), "no monitor segments");
    assert_eq!(
        segments[0].session.start().value(),
        0,
        "first segment start"
    );
    let mut cur = 0i64;
    for s in segments {
        assert_eq!(
            s.session.start().value(),
            cur,
            "segment gap/overlap before {cur}: {s:?}"
        );
        cur = s.session.end().value();
    }
    assert_eq!(cur, session_end, "segments must reach session_end");
}

/// probe 判定用の track monitor 文脈（Arm On 固定）。
fn probe_ctx(monitor: MonitorState, ready: bool) -> TrackMonitorContext {
    TrackMonitorContext {
        arm: ArmState::On,
        monitor,
        input_output_ready: ready,
        ..TrackMonitorContext::default()
    }
}

/// `span_obs` の 1 要素（session/source/project 範囲 + pass）。
type SpanObs = (i64, i64, u32, i64, i64, i64);
/// commit payload の monitor 非依存構造（segment 範囲・pass・
/// take 参照）。
type CommitShape = (Vec<(i64, i64, u32, i64, i64)>, Vec<u32>, Vec<(u32, usize)>);

/// `build_payload` の commit 構造を monitor 非依存な形で抽出する
/// （ID を除く session/project 範囲・pass 構成・take 参照）。
fn commit_shape(payload: &CommitPayload) -> CommitShape {
    let mut segs = Vec::new();
    let mut takes = Vec::new();
    for s in &payload.streams {
        for seg in &s.segments {
            segs.push((
                seg.session_start,
                seg.session_end,
                seg.pass_index,
                seg.project_in,
                seg.project_out,
            ));
        }
        for t in &s.takes {
            takes.push((t.pass_index, t.segment_ids.len()));
        }
    }
    let passes: Vec<u32> = payload.passes.iter().map(|p| p.index).collect();
    (segs, passes, takes)
}

/// commit payload の segment session 範囲が `regions`（昇順）を
/// 隙間・重複・はみ出しなく cover することを検証する。
fn assert_commit_coverage(payload: &CommitPayload, regions: &[(i64, i64)]) {
    for stream in &payload.streams {
        let mut merged: Vec<(i64, i64)> = Vec::new();
        let mut segs: Vec<(i64, i64)> = stream
            .segments
            .iter()
            .map(|s| (s.session_start, s.session_end))
            .collect();
        segs.sort_unstable();
        for (a, b) in segs {
            match merged.last_mut() {
                Some(last) if last.1 == a => last.1 = b,
                _ => merged.push((a, b)),
            }
        }
        assert_eq!(
            merged.as_slice(),
            regions,
            "stream {} commit segment coverage",
            stream.stream_index
        );
    }
}

// ---------- T1: route/capture coherence + raw/commit 独立性 ----------

/// T1 の 1 case の観測結果。
struct CaseOutcome {
    /// 収集した monitor segment。
    segments: Vec<MonitorSegment>,
    /// staging media の実 bytes。
    media: Vec<u8>,
    /// journal span の `span_obs` 観測。
    span_obs: Vec<SpanObs>,
    /// commit payload の monitor 非依存構造。
    shape: CommitShape,
}

/// clean punch session を 1 本構築・駆動・finish し、route
/// segment・capture・commit の観測を返す。capture 側の被覆・
/// PCM 検証はここで行う（case 非依存の不変条件）。
fn run_clean_case(
    name: &str,
    sid: u128,
    monitor: MonitorState,
    ctx: TrackMonitorContext,
) -> CaseOutcome {
    let (mut rig, p, session, probe_plan) = open_session(name, sid, monitor);
    let frames = u32::try_from(rig.plan.session_end.value()).expect("session_end fits u32");
    let mut probe = SessionProbe::new(&probe_plan, ctx);
    drive_with_probe(&mut rig, &mut probe, frames, BLOCK, TransportPhase::Playing);
    assert!(!rig.session.fault().is_faulted(), "{name}");
    let report = rig.finish();
    assert_eq!(report.refs.len(), 1, "{name}");
    let r = report.refs[0].1.as_ref().expect("finalize");
    assert_eq!(r.outcome, FinalizedOutcome::Complete, "{name}");
    assert_eq!(r.source_frames, GATE_SPAN_FRAMES_U64, "{name}");

    // capture 側: journal span が gate-open [300,700) のみを
    // cover し、PCM が MockPcm と一致する。
    let map = ChannelMap::mono(0);
    let cap = capture_of(&rig.area, 0);
    let spans = all_journal_spans(&cap);
    assert_session_coverage(&spans, &[GATE_OPEN]);
    assert_source_contiguous(&spans, GATE_SPAN_FRAMES);
    verify_capture_pcm(&cap, map.channels(), SEED);

    // commit payload: monitor 非依存で gate-open 区間を反映。
    let (payload, recoverable) =
        build_payload(&p, &rig.area, session, &mut || 0).expect("build_payload");
    assert_eq!(recoverable.len(), 0, "{name} recoverable");
    assert_eq!(payload.streams.len(), 1, "{name} streams");
    assert_commit_coverage(&payload, &[GATE_OPEN]);

    CaseOutcome {
        segments: probe.segments,
        media: cap.media,
        span_obs: span_obs(&spans),
        shape: commit_shape(&payload),
    }
}

#[test]
fn session_monitor_routes_track_gate_and_raw_stays_gated() {
    /// T1 case 定義（route 期待値と probe 文脈）。
    struct Case {
        name: &'static str,
        monitor: MonitorState,
        ctx: TrackMonitorContext,
        expected: Vec<(i64, i64, MonitorRoute)>,
    }

    let mut auto_muted = probe_ctx(MonitorState::Auto, false);
    auto_muted.muted = true;

    let cases = [
        // Off + Arm: gate-open は playback 抑制（Muted）、他は Playback。
        Case {
            name: "off",
            monitor: MonitorState::Off,
            ctx: probe_ctx(MonitorState::Off, false),
            expected: vec![(0, 300, Playback), (300, 700, Muted), (700, 1000, Playback)],
        },
        // Auto + Arm + Playing: gate-open のみ Input、他は Playback。
        Case {
            name: "auto",
            monitor: MonitorState::Auto,
            ctx: probe_ctx(MonitorState::Auto, false),
            expected: vec![(0, 300, Playback), (300, 700, Input), (700, 1000, Playback)],
        },
        // Auto + muted: mute は Playback route のみに作用し、
        // gate-open の Input route は影響を受けない。
        Case {
            name: "auto-muted",
            monitor: MonitorState::Auto,
            ctx: auto_muted,
            expected: vec![(0, 300, Muted), (300, 700, Input), (700, 1000, Muted)],
        },
        // On + ready: arm/gate に依らず全域 Input。
        Case {
            name: "on-ready",
            monitor: MonitorState::On,
            ctx: probe_ctx(MonitorState::On, true),
            expected: vec![(0, 1000, Input)],
        },
        // On + !ready: Playback フォールバック + input_unavailable。
        Case {
            name: "on-not-ready",
            monitor: MonitorState::On,
            ctx: probe_ctx(MonitorState::On, false),
            expected: vec![(0, 1000, Playback)],
        },
    ];

    let mut media_ref: Option<Vec<u8>> = None;
    let mut spans_ref: Option<Vec<SpanObs>> = None;
    let mut shape_ref: Option<CommitShape> = None;

    for (i, case) in cases.iter().enumerate() {
        let outcome = run_clean_case(
            &format!("t1-{}", case.name),
            100 + u128::try_from(i).unwrap_or(0),
            case.monitor,
            case.ctx.clone(),
        );

        // route/gate coherence: probe の gate_open 区間は
        // [300,700) に merge され、route 区間は decide 規則どおり。
        assert_segment_tiling(&outcome.segments, 1000);
        assert_eq!(
            merged_regions(&outcome.segments, |s| s.gate_open),
            vec![GATE_OPEN],
            "case {} gate_open regions",
            case.name
        );
        assert_eq!(
            route_regions(&outcome.segments),
            case.expected,
            "case {} route regions",
            case.name
        );
        let expect_unavailable = case.monitor == MonitorState::On && !case.ctx.input_output_ready;
        if expect_unavailable {
            assert!(
                outcome.segments.iter().all(|s| s.notices.input_unavailable),
                "case {} must report input_unavailable",
                case.name
            );
        }

        // Raw File 独立性: media bytes・journal span が monitor
        // 状態に依らず一致する。
        match &media_ref {
            None => {
                media_ref = Some(outcome.media.clone());
                spans_ref = Some(outcome.span_obs.clone());
            }
            Some(m) => {
                assert_eq!(&outcome.media, m, "case {} media bytes", case.name);
                assert_eq!(
                    &outcome.span_obs,
                    spans_ref.as_ref().expect("spans ref"),
                    "case {} journal spans",
                    case.name
                );
            }
        }

        // commit payload: monitor 非依存で gate-open 区間を反映。
        match &shape_ref {
            None => shape_ref = Some(outcome.shape.clone()),
            Some(reference) => {
                assert_eq!(&outcome.shape, reference, "case {} commit shape", case.name);
            }
        }
    }

    // commit 構造の sanity: 単一 pass・単一 take が gate-open
    // segment を参照する。
    let (_, passes, takes) = shape_ref.expect("at least one case");
    assert_eq!(passes, vec![0], "single recording pass");
    assert_eq!(takes.len(), 1, "single take");
    assert!(takes[0].1 > 0, "take must reference segments");
}

// ---------- T2: device fault の monitor 非依存性 ----------

#[test]
fn session_device_faults_do_not_depend_on_monitor_state() {
    let map = ChannelMap::mono(0);
    let mut observed: Vec<(u32, Vec<SpanObs>, i64)> = Vec::new();

    for (i, monitor) in [MonitorState::Off, MonitorState::Auto, MonitorState::On]
        .into_iter()
        .enumerate()
    {
        let (mut rig, _p, _sid, _probe_plan) = open_session(
            &format!("t2-{monitor:?}"),
            200 + u128::try_from(i).unwrap_or(0),
            monitor,
        );
        let frames = u32::try_from(rig.plan.session_end.value()).expect("session_end fits u32");
        // gate-open 区間内部（frame 600 = punch [300,700) の中）まで
        // 正常駆動する。
        rig.drive(600, BLOCK);
        assert!(!rig.session.fault().is_faulted(), "case {monitor:?}");
        // frame gap を注入 → fail-stop。
        rig.streams[0].lock().expect("lock").jump_to(600 + 777);
        rig.deliver_all(BLOCK);
        rig.pump_all();
        assert_eq!(
            rig.session.fault().bits() & FAULT_INPUT_DISCONTINUITY,
            FAULT_INPUT_DISCONTINUITY,
            "case {monitor:?}"
        );
        // 以降の入力は fail-stop で捨てられる（連続を捏造しない）。
        rig.drive(frames, BLOCK);
        let report = rig.finish();
        assert_eq!(report.refs.len(), 1, "case {monitor:?}");

        // verified prefix = gate-open ∩ [0,600) = [300,600)。
        assert!(rig.area.capture_fault_marker_exists(), "case {monitor:?}");
        let cap = capture_of(&rig.area, 0);
        assert!(
            matches!(cap.replay.stop, StopReason::CleanEnd | StopReason::TornTail),
            "case {monitor:?} journal must remain intact: {:?}",
            cap.replay.stop
        );
        let spans = all_journal_spans(&cap);
        assert_session_coverage(&spans, &[(GATE_OPEN.0, 600)]);
        assert_source_contiguous(&spans, span_frame_total(&spans));
        verify_capture_pcm(&cap, map.channels(), SEED);
        observed.push((
            rig.session.fault().bits(),
            span_obs(&spans),
            span_frame_total(&spans),
        ));
    }

    assert!(
        observed.windows(2).all(|w| w[0] == w[1]),
        "monitor state must not alter fault behavior: {observed:?}"
    );

    // device loss（disconnect）でも同じ fail-stop を pin する
    // （MonitorState::Auto で代表）。
    let (mut rig, _p, _sid, _probe_plan) = open_session("t2-disconnect", 210, MonitorState::Auto);
    let frames = u32::try_from(rig.plan.session_end.value()).expect("session_end fits u32");
    rig.drive(500, BLOCK);
    assert!(!rig.session.fault().is_faulted());
    rig.streams[0]
        .lock()
        .expect("lock")
        .raise(DeviceError::DeviceNotFound);
    rig.deliver_all(BLOCK);
    rig.pump_all();
    assert_eq!(
        rig.session.fault().bits() & FAULT_INPUT_DISCONTINUITY,
        FAULT_INPUT_DISCONTINUITY
    );
    rig.drive(frames, BLOCK);
    let _report = rig.finish();
    assert!(rig.area.capture_fault_marker_exists());
    let cap = capture_of(&rig.area, 0);
    let spans = all_journal_spans(&cap);
    // verified prefix = gate-open ∩ [0,500) = [300,500)。
    assert_session_coverage(&spans, &[(GATE_OPEN.0, 500)]);
    verify_capture_pcm(&cap, map.channels(), SEED);
}

// ---------- T3: punch 中 device loss → fail-stop → recovery ----------

#[test]
fn session_punch_device_loss_fail_stop_and_new_session_recovery() {
    let map = ChannelMap::mono(0);

    // Session A: punch gate-open 区間内部（frame 500）で device
    // loss → fail-stop。verified prefix は gate-open ∩ [0,500)。
    let (mut rig_a, p_a, sid_a, _probe_plan_a) = open_session("t3-loss", 300, MonitorState::Auto);
    let frames_a = u32::try_from(rig_a.plan.session_end.value()).expect("session_end fits u32");
    rig_a.drive(500, BLOCK);
    assert!(!rig_a.session.fault().is_faulted());
    rig_a.streams[0]
        .lock()
        .expect("lock")
        .raise(DeviceError::DeviceNotFound);
    rig_a.deliver_all(BLOCK);
    rig_a.pump_all();
    assert_eq!(
        rig_a.session.fault().bits() & FAULT_INPUT_DISCONTINUITY,
        FAULT_INPUT_DISCONTINUITY
    );
    rig_a.drive(frames_a, BLOCK);
    let report_a = rig_a.finish();
    assert_eq!(report_a.refs.len(), 1);
    assert!(rig_a.area.capture_fault_marker_exists());
    let cap_a = capture_of(&rig_a.area, 0);
    let spans_a = all_journal_spans(&cap_a);
    assert_session_coverage(&spans_a, &[(GATE_OPEN.0, 500)]);
    assert_source_contiguous(&spans_a, span_frame_total(&spans_a));
    verify_capture_pcm(&cap_a, map.channels(), SEED);
    let prefix_obs_a = span_obs(&spans_a);

    // commit 層の分類: capture-fault marker により stream は
    // recoverable（capture-fault: 理由）に分離され、Complete
    // media としては commit されない。
    let (payload_a, recoverable_a) =
        build_payload(&p_a, &rig_a.area, sid_a, &mut || 0).expect("build_payload");
    assert_eq!(recoverable_a.len(), 1, "faulted stream must be recoverable");
    assert!(payload_a.streams.is_empty(), "no stream may commit");
    assert!(payload_a.passes.is_empty(), "no pass may commit");
    assert!(
        recoverable_a[0].reason.starts_with("capture-fault:"),
        "reason: {}",
        recoverable_a[0].reason
    );

    // Session B（recovery）: 回復 device 相当の新規
    // `FakeInputDevice` + 新規 staging + 新規 session id で同一
    // 形状の rig を構築し、clean に完走させる。in-session resume
    // は主張しない（fail-stop が製品契約）。
    let (mut rig_b, p_b, sid_b, _probe_plan_b) =
        open_session("t3-recover", 301, MonitorState::Auto);
    let frames_b = u32::try_from(rig_b.plan.session_end.value()).expect("session_end fits u32");
    rig_b.drive(frames_b, BLOCK);
    assert!(!rig_b.session.fault().is_faulted());
    let report_b = rig_b.finish();
    assert_eq!(report_b.refs.len(), 1);
    let r_b = report_b.refs[0].1.as_ref().expect("finalize");
    assert_eq!(r_b.outcome, FinalizedOutcome::Complete);
    assert_eq!(r_b.source_frames, GATE_SPAN_FRAMES_U64);
    let cap_b = capture_of(&rig_b.area, 0);
    let spans_b = all_journal_spans(&cap_b);
    assert_session_coverage(&spans_b, &[GATE_OPEN]);
    assert_source_contiguous(&spans_b, GATE_SPAN_FRAMES);
    verify_capture_pcm(&cap_b, map.channels(), SEED);
    let (payload_b, recoverable_b) =
        build_payload(&p_b, &rig_b.area, sid_b, &mut || 0).expect("build_payload");
    assert_eq!(recoverable_b.len(), 0);
    assert_eq!(payload_b.streams.len(), 1);
    assert_commit_coverage(&payload_b, &[GATE_OPEN]);

    // 旧 session の staging/prefix は recovery で変更されない。
    let cap_a2 = capture_of(&rig_a.area, 0);
    assert_eq!(
        span_obs(&all_journal_spans(&cap_a2)),
        prefix_obs_a,
        "old session verified prefix must be preserved"
    );
    assert!(rig_a.area.capture_fault_marker_exists());
}

// ---------- T4: 2 session 並行の fault 分離 ----------

#[test]
fn session_faults_do_not_leak_between_monitoring_sessions() {
    let map = ChannelMap::mono(0);

    // Session A は Monitor On、Session B は Monitor Off を
    // tap binding で宣言する（monitor 状態は capture path の
    // metadata であり、session ごとに独立）。
    let (mut a, _pa, _sid_a, _probe_plan_a) = open_session("t4-a", 400, MonitorState::On);
    let (mut b, pb, sid_b, _probe_plan_b) = open_session("t4-b", 401, MonitorState::Off);
    let frames = u32::try_from(a.plan.session_end.value()).expect("session_end fits u32");

    // block 単位で interleave 駆動し、A の gate-open 内部
    // （frame 500）で frame gap を注入して A のみ fail-stop
    // させる。
    a.drive(BLOCK, BLOCK);
    b.drive(BLOCK, BLOCK);
    a.drive(116, BLOCK);
    b.drive(116, BLOCK);
    assert!(!a.session.fault().is_faulted());
    assert!(!b.session.fault().is_faulted());
    a.streams[0].lock().expect("lock").jump_to(500 + 777);
    a.deliver_all(BLOCK);
    a.pump_all();
    assert_eq!(
        a.session.fault().bits() & FAULT_INPUT_DISCONTINUITY,
        FAULT_INPUT_DISCONTINUITY
    );

    // B は影響を受けず完走する: Complete finalize・gate-open
    // 被覆・PCM verified・commit 成功・capture-fault marker 無し。
    b.drive(frames - 500, BLOCK);
    assert!(!b.session.fault().is_faulted());
    let report_b = b.finish();
    assert_eq!(report_b.refs.len(), 1);
    let r_b = report_b.refs[0].1.as_ref().expect("finalize");
    assert_eq!(r_b.outcome, FinalizedOutcome::Complete);
    assert_eq!(r_b.source_frames, GATE_SPAN_FRAMES_U64);
    assert!(!b.area.capture_fault_marker_exists());
    let cap_b = capture_of(&b.area, 0);
    let spans_b = all_journal_spans(&cap_b);
    assert_session_coverage(&spans_b, &[GATE_OPEN]);
    assert_source_contiguous(&spans_b, GATE_SPAN_FRAMES);
    verify_capture_pcm(&cap_b, map.channels(), SEED);
    let (payload_b, recoverable_b) =
        build_payload(&pb, &b.area, sid_b, &mut || 0).expect("build_payload");
    assert_eq!(recoverable_b.len(), 0);
    assert_eq!(payload_b.streams.len(), 1);
    assert_commit_coverage(&payload_b, &[GATE_OPEN]);

    // A は verified prefix [300,500) を保持して終了する。
    a.drive(frames, BLOCK);
    let _report_a = a.finish();
    assert!(a.area.capture_fault_marker_exists());
    let cap_a = capture_of(&a.area, 0);
    let spans_a = all_journal_spans(&cap_a);
    assert_session_coverage(&spans_a, &[(GATE_OPEN.0, 500)]);
    verify_capture_pcm(&cap_a, map.channels(), SEED);
    // A の fault bits/marker が B の session・area・payload に
    // 現れないことは上記の B 側 assert（marker 無し・Complete・
    // commit 成功）で pin される。`FaultLatch` は session 共有
    // であり session 間分離は構造的契約。
}
