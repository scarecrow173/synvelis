//! RCD-03 monitor/punch/latency slice の fixture 検証
//! （synvelis#117、crate 内完結 slice）。
//!
//! `decide` の route 判定行列、`MonitorProbe` の gate 連動 segment
//! 分割（block 途中境界・Loop pass）、および Monitor Off/Auto/On で
//! Raw File Byte が一致することを fake port fixture で検証する。
//!
//! これは fake port/fixture 契約の検証であり、実 Device・実 GUI・
//! 実 OS での monitor 受入ではない（NotRun — 実機 gate は後続
//! slice）。

// 各 test binary が common の subset だけを使うため module 単位で許容
// （input_adapter.rs と同じ運用）。
#[allow(dead_code)]
mod common;

use common::*;
use ruviform_media_core::ClockDomainId;
use synvelis_project::{MusicalTick, SessionFrame, TempoEventDto, TempoMapRef};
use synvelis_recording::{
    ArmState, BoundedVec, CaptureBlockHeader, CaptureSpan, ChannelMap, ChunkValidity,
    HardwareDirectMonitor, HardwareDirectState, MonitorProbe, MonitorRoute, MonitorRoute::Input,
    MonitorRoute::Muted, MonitorRoute::Playback, MonitorSegment, MonitorState, RecordingRequest,
    TrackMonitorContext, TransportPhase, decide, preflight,
};

// ---------- helpers ----------

/// 既定の track monitor 文脈（全て無効・Hardware Direct Unknown）。
fn ctx() -> TrackMonitorContext {
    TrackMonitorContext::default()
}

/// `(start, end, gate_open, route)` の 4 つ組へ潰した segment 一覧。
type SegmentView = Vec<(i64, i64, bool, MonitorRoute)>;

fn view(segments: &[MonitorSegment]) -> SegmentView {
    segments
        .iter()
        .map(|s| {
            (
                s.session.start().value(),
                s.session.end().value(),
                s.gate_open,
                s.route,
            )
        })
        .collect()
}

/// `frames` を `block` 刻みで `probe.feed` し、全 segment を連結する。
fn probe_segments(
    plan: &synvelis_recording::PreparedRecordingPlan,
    ctx: &TrackMonitorContext,
    frames: u32,
    block: u32,
    phase: TransportPhase,
) -> SegmentView {
    let mut probe = MonitorProbe::new(plan, ctx.clone());
    let mut out = BoundedVec::with_capacity(64);
    let mut all = Vec::new();
    let mut remaining = frames;
    while remaining > 0 {
        let n = block.min(remaining);
        probe.feed(n, phase, &mut out);
        assert!(!probe.faulted(), "probe faulted");
        all.extend_from_slice(out.as_slice());
        remaining -= n;
    }
    assert!(probe.finished(), "probe did not reach session_end");
    view(&all)
}

/// 1 lane project + 指定 `GateSpec` の plan（tick→frame は 25 f/tick）。
fn plan_with_gate(
    gate: synvelis_recording::GateSpec,
    stop_tick: i64,
    max_block_frames: u32,
) -> (
    synvelis_recording::PreparedRecordingPlan,
    synvelis_project::ClockEpochId,
) {
    plan_with_gate_map(gate, stop_tick, max_block_frames, 2)
}

/// `plan_with_gate` の channel 数指定版（ChannelMap は capture 側で指定）。
fn plan_with_gate_map(
    gate: synvelis_recording::GateSpec,
    stop_tick: i64,
    max_block_frames: u32,
    channels: u32,
) -> (
    synvelis_recording::PreparedRecordingPlan,
    synvelis_project::ClockEpochId,
) {
    let (p, lanes, epoch) = project_with_lanes(1);
    let req = RecordingRequest {
        targets: vec![synvelis_recording::CaptureTarget {
            lane_id: lanes[0],
            channels,
        }],
        clock_epoch: epoch,
        tempo_map: TempoMapRef { revision: 0 },
        transport_start: MusicalTick::ZERO,
        stop: MusicalTick::new(stop_tick),
        gate,
        budget: budget(),
        max_block_frames,
    };
    let plan = preflight(&p, req, RATE).expect("preflight must pass");
    (plan, epoch)
}

/// `(header, spans, pcm bits, validity)` を chunk 順に平坦化した
/// capture 結果。Monitor 状態間の byte/span 一致比較に使う。
#[derive(PartialEq, Debug)]
struct Captured {
    headers: Vec<CaptureBlockHeader>,
    spans: Vec<CaptureSpan>,
    pcm: Vec<u32>,
    validity: Vec<ChunkValidity>,
}

/// `monitor` を変えて fake device 駆動し、writer log を回収する。
fn capture_with_monitor(
    plan: &synvelis_recording::PreparedRecordingPlan,
    epoch: synvelis_project::ClockEpochId,
    monitor: MonitorState,
    frames: u32,
    block: u32,
) -> Captured {
    capture_with_monitor_map(
        plan,
        epoch,
        monitor,
        frames,
        block,
        2,
        ChannelMap::stereo(0, 1),
    )
}

/// `capture_with_monitor` の channel 数・ChannelMap 指定版。
fn capture_with_monitor_map(
    plan: &synvelis_recording::PreparedRecordingPlan,
    epoch: synvelis_project::ClockEpochId,
    monitor: MonitorState,
    frames: u32,
    block: u32,
    channels: u32,
    map: ChannelMap,
) -> Captured {
    let device = FakeInputDevice::new(channels, ClockDomainId::SAMPLE_CLOCK);
    let spec = synvelis_recording::InputStreamSpec {
        config: input_config(channels),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(plan, 0, map, ArmState::On, monitor)],
    };
    let fault = synvelis_recording::FaultLatch::new();
    let opened = synvelis_recording::InputAdapter::open(&device, &spec, fault.clone())
        .expect("open must pass");
    let (mut adapter, mut writers) = into_writers(opened, &fault);
    assert_eq!(writers.len(), 1);
    let stream = device.stream(0);
    drive(&stream, &mut adapter, &mut writers, plan, frames, block);
    assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());
    let mut out = Captured {
        headers: Vec::new(),
        spans: Vec::new(),
        pcm: Vec::new(),
        validity: Vec::new(),
    };
    for entry in &writers[0].1.log {
        out.headers.push(entry.header);
        out.spans.extend_from_slice(&entry.spans);
        out.pcm.extend(entry.pcm.iter().map(|s| s.to_bits()));
        out.validity.push(entry.validity);
    }
    out
}

// ---------- decide 行列 ----------

#[test]
fn decide_off_suppresses_playback_only_while_gate_open() {
    let mut c = ctx();
    c.arm = ArmState::On;
    c.monitor = MonitorState::Off;
    // gate open + arm → playback 抑制（Muted）。
    assert_eq!(
        decide(&c, TransportPhase::Playing, true).route,
        MonitorRoute::Muted
    );
    // gate 閉 → 通常 Playback。
    assert_eq!(
        decide(&c, TransportPhase::Playing, false).route,
        MonitorRoute::Playback
    );
    // arm off では gate_open は無視され Playback。
    c.arm = ArmState::Off;
    assert_eq!(
        decide(&c, TransportPhase::Playing, true).route,
        MonitorRoute::Playback
    );
}

#[test]
fn decide_auto_routes_input_when_stopped_or_gate_open() {
    let mut c = ctx();
    c.arm = ArmState::On;
    c.monitor = MonitorState::Auto;
    for phase in [
        TransportPhase::Stopped,
        TransportPhase::PunchOpen,
        TransportPhase::Playing,
    ] {
        let open = decide(&c, phase, true).route;
        assert_eq!(open, MonitorRoute::Input, "phase {phase:?} gate open");
    }
    // gate 閉 + 停止 → Input。
    assert_eq!(
        decide(&c, TransportPhase::Stopped, false).route,
        MonitorRoute::Input
    );
    // gate 閉 + 再生系 → Playback。
    for phase in [
        TransportPhase::CountIn,
        TransportPhase::PreRoll,
        TransportPhase::PunchWait,
        TransportPhase::PostRoll,
        TransportPhase::Playing,
    ] {
        assert_eq!(
            decide(&c, phase, false).route,
            MonitorRoute::Playback,
            "phase {phase:?}"
        );
    }
}

#[test]
fn decide_auto_without_arm_never_routes_input() {
    let mut c = ctx();
    c.arm = ArmState::Off;
    c.monitor = MonitorState::Auto;
    for phase in [TransportPhase::Stopped, TransportPhase::Playing] {
        for gate_open in [false, true] {
            assert_eq!(
                decide(&c, phase, gate_open).route,
                MonitorRoute::Playback,
                "phase {phase:?} gate_open {gate_open}"
            );
        }
    }
}

#[test]
fn decide_on_requires_verified_io_and_ignores_arm_gate() {
    let mut c = ctx();
    c.monitor = MonitorState::On;
    c.arm = ArmState::Off;
    // input/output 未検証 → Playback + input_unavailable。
    let d = decide(&c, TransportPhase::Playing, false);
    assert_eq!(d.route, MonitorRoute::Playback);
    assert!(d.notices.input_unavailable);
    // 検証済み → arm/gate/phase に依らず Input。
    c.input_output_ready = true;
    for phase in [TransportPhase::Stopped, TransportPhase::Playing] {
        for gate_open in [false, true] {
            let d = decide(&c, phase, gate_open);
            assert_eq!(d.route, MonitorRoute::Input, "phase {phase:?}");
            assert!(!d.notices.input_unavailable);
        }
    }
    c.arm = ArmState::On;
    assert_eq!(
        decide(&c, TransportPhase::Playing, true).route,
        MonitorRoute::Input
    );
}

#[test]
fn mute_solo_only_affect_playback_route() {
    let mut c = ctx();
    c.arm = ArmState::On;
    c.monitor = MonitorState::Auto;
    // gate open → Input。mute/solo は Input に作用しない。
    c.muted = true;
    assert_eq!(
        decide(&c, TransportPhase::Playing, true).route,
        MonitorRoute::Input
    );
    c.muted = false;
    c.any_solo = true;
    c.soloed = false;
    assert_eq!(
        decide(&c, TransportPhase::Playing, true).route,
        MonitorRoute::Input
    );
    // Playback 側では mute → Muted、solo 対象外 → Muted、soloed → Playback。
    assert_eq!(
        decide(&c, TransportPhase::Playing, false).route,
        MonitorRoute::Muted
    );
    c.any_solo = false;
    c.muted = true;
    assert_eq!(
        decide(&c, TransportPhase::Playing, false).route,
        MonitorRoute::Muted
    );
    c.muted = false;
    c.any_solo = true;
    c.soloed = true;
    assert_eq!(
        decide(&c, TransportPhase::Playing, false).route,
        MonitorRoute::Playback
    );
}

#[test]
fn hardware_direct_notices_are_external_facts() {
    let mut c = ctx();
    c.arm = ArmState::On;
    c.monitor = MonitorState::Auto;
    // 既定 Unknown → 常に hardware_direct_unknown。
    let d = decide(&c, TransportPhase::Playing, false);
    assert!(d.notices.hardware_direct_unknown);
    assert!(!d.notices.double_monitoring);
    // DeviceControlled + 観測 active + Software Input → double_monitoring。
    c.hardware_direct = HardwareDirectMonitor {
        state: HardwareDirectState::DeviceControlled,
        observed_active: Some(true),
    };
    let d = decide(&c, TransportPhase::Stopped, false); // Auto+arm+Stopped → Input
    assert_eq!(d.route, MonitorRoute::Input);
    assert!(d.notices.double_monitoring);
    assert!(!d.notices.hardware_direct_unknown);
    // Playback route では二重監視にならない。
    let d = decide(&c, TransportPhase::Playing, false);
    assert_eq!(d.route, MonitorRoute::Playback);
    assert!(!d.notices.double_monitoring);
    // UserReported でも観測 active + Input なら警告（制御可否と独立）。
    c.hardware_direct.state = HardwareDirectState::UserReported;
    let d = decide(&c, TransportPhase::Stopped, false);
    assert!(d.notices.double_monitoring);
    // observed_active=None → 二重監視なし。
    c.hardware_direct.observed_active = None;
    let d = decide(&c, TransportPhase::Stopped, false);
    assert!(!d.notices.double_monitoring);
    // mute/solo で observed_active は変わらない（外部事実）。
    c.hardware_direct.observed_active = Some(true);
    c.muted = true;
    let d = decide(&c, TransportPhase::Stopped, false);
    assert_eq!(d.route, MonitorRoute::Input);
    assert!(d.notices.double_monitoring);
}

// ---------- MonitorProbe: Punch 途中境界 ----------

#[test]
fn monitor_probe_splits_punch_at_block_interior_boundaries() {
    // punch [12,28) tick = frame [300,700)。block 500 で 2 block に跨る。
    let (plan, _epoch) = plan_with_gate(
        synvelis_recording::GateSpec::Punch {
            punch_in: MusicalTick::new(12),
            punch_out: MusicalTick::new(28),
        },
        200,
        1024,
    );
    let mut c = ctx();
    c.arm = ArmState::On;
    c.monitor = MonitorState::Auto;
    let segs = probe_segments(&plan, &c, 5000, 500, TransportPhase::Playing);
    // segment は feed block 単位で分割される（閉区間は block ごと）。
    let mut expected = vec![
        (0, 300, false, Playback),
        (300, 500, true, Input),
        (500, 700, true, Input),
        (700, 1000, false, Playback),
    ];
    for start in (1000_i64..5000).step_by(500) {
        expected.push((start, start + 500, false, Playback));
    }
    assert_eq!(segs, expected);
}

#[test]
fn monitor_probe_off_mutes_gate_open_playback() {
    let (plan, _epoch) = plan_with_gate(
        synvelis_recording::GateSpec::Punch {
            punch_in: MusicalTick::new(12),
            punch_out: MusicalTick::new(28),
        },
        200,
        1024,
    );
    let mut c = ctx();
    c.arm = ArmState::On;
    c.monitor = MonitorState::Off;
    let segs = probe_segments(&plan, &c, 5000, 500, TransportPhase::Playing);
    // gate open 区間は Muted（playback 抑制）、閉区間は Playback。
    // segment は feed block 単位で分割される。
    let mut expected = vec![
        (0, 300, false, Playback),
        (300, 500, true, Muted),
        (500, 700, true, Muted),
        (700, 1000, false, Playback),
    ];
    for start in (1000_i64..5000).step_by(500) {
        expected.push((start, start + 500, false, Playback));
    }
    assert_eq!(segs, expected);
}

#[test]
fn monitor_probe_stopped_auto_is_input_even_with_gate_closed() {
    // Normal gate open=0（全区間 open）だが、Stopped 相 + gate 閉の
    // 区間を作るため Punch で gate 外を使う。
    let (plan, _epoch) = plan_with_gate(
        synvelis_recording::GateSpec::Punch {
            punch_in: MusicalTick::new(12),
            punch_out: MusicalTick::new(28),
        },
        200,
        1024,
    );
    let mut c = ctx();
    c.arm = ArmState::On;
    c.monitor = MonitorState::Auto;
    // Stopped 相: gate 閉 → Input（停止中モニタ）、gate open → Input。
    let segs = probe_segments(&plan, &c, 5000, 500, TransportPhase::Stopped);
    assert!(
        segs.iter().all(|(_, _, _, r)| *r == Input),
        "stopped+auto+arm must be Input everywhere: {segs:?}"
    );
}

#[test]
fn monitor_probe_punch_in_loop_repeats_gate_open_per_pass() {
    // loop [0,40)tick=1000f、punch [10,30)tick=frame[250,750)。
    // stop 100tick=2500f → pass0 全、pass1 全、pass2 は [2000,2500)。
    let (plan, _epoch) = plan_with_gate(
        synvelis_recording::GateSpec::PunchInLoop {
            loop_start: MusicalTick::ZERO,
            loop_end: MusicalTick::new(40),
            punch_in: MusicalTick::new(10),
            punch_out: MusicalTick::new(30),
        },
        100,
        1024,
    );
    let mut c = ctx();
    c.arm = ArmState::On;
    c.monitor = MonitorState::Auto;
    // block 1000 は pass 境界 (1000f) と整列し、open 区間が
    // block 内で分割されない。
    let segs = probe_segments(&plan, &c, 2500, 1000, TransportPhase::Playing);
    // 期待 open 区間: pass0 [250,750)、pass1 [1250,1750)、
    // pass2 [2250,2500)（最終 pass 途中停止でクランプ）。
    let open: Vec<(i64, i64)> = segs
        .iter()
        .filter(|(_, _, g, _)| *g)
        .map(|(a, b, _, _)| (*a, *b))
        .collect();
    assert_eq!(open, vec![(250, 750), (1250, 1750), (2250, 2500)]);
    // 全 segment は [0,2500) を隙間・重複なく連続タイルする。
    let mut pos = 0;
    for (a, b, _, _) in &segs {
        assert_eq!(*a, pos, "gap/overlap before segment {a}..{b}");
        pos = *b;
    }
    assert_eq!(pos, 2500);
    // open → Input、closed → Playback（Auto+arm+Playing）。
    for (_, _, g, r) in &segs {
        assert_eq!(*r, if *g { Input } else { Playback });
    }
}

// ---------- Raw File Byte 独立 ----------

#[test]
fn raw_capture_bytes_are_identical_across_monitor_states() {
    // Normal gate（全区間 open）で Off/Auto/On を比較。
    let (plan, epoch) = plan_with_gate(
        synvelis_recording::GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        200,
        1024,
    );
    let frames = u32::try_from(plan.session_end.value()).expect("session_end fits u32");
    let off = capture_with_monitor(&plan, epoch, MonitorState::Off, frames, 500);
    let auto = capture_with_monitor(&plan, epoch, MonitorState::Auto, frames, 500);
    let on = capture_with_monitor(&plan, epoch, MonitorState::On, frames, 500);
    // 全 5000 frame が保存されることを被覆量で確認してから比較する。
    let covered: u32 = off.spans.iter().map(|s| s.frame_count).sum();
    assert_eq!(covered, 5000);
    assert_eq!(off, auto, "Auto changed raw capture");
    assert_eq!(off, on, "On changed raw capture");
}

#[test]
fn raw_capture_spans_identical_across_monitor_states_with_punch() {
    // Punch gate で gate 閉区間（PCM 無し）を含めても一致すること。
    let (plan, epoch) = plan_with_gate(
        synvelis_recording::GateSpec::Punch {
            punch_in: MusicalTick::new(12),
            punch_out: MusicalTick::new(28),
        },
        200,
        1024,
    );
    let frames = u32::try_from(plan.session_end.value()).expect("session_end fits u32");
    let off = capture_with_monitor(&plan, epoch, MonitorState::Off, frames, 500);
    let auto = capture_with_monitor(&plan, epoch, MonitorState::Auto, frames, 500);
    let on = capture_with_monitor(&plan, epoch, MonitorState::On, frames, 500);
    // 保存総量 = punch 区間 400 frame のみ。
    let total: u32 = off.spans.iter().map(|s| s.frame_count).sum();
    assert_eq!(total, 400);
    assert_eq!(off, auto);
    assert_eq!(off, on);
}

#[test]
fn arm_off_monitor_auto_creates_no_writer() {
    let (plan, epoch) = plan_with_gate(
        synvelis_recording::GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        200,
        1024,
    );
    let device = FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK);
    let spec = synvelis_recording::InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::stereo(0, 1),
            ArmState::Off,
            MonitorState::Auto,
        )],
    };
    let fault = synvelis_recording::FaultLatch::new();
    let opened = synvelis_recording::InputAdapter::open(&device, &spec, fault.clone())
        .expect("open must pass");
    // Arm Off + Monitor Auto: Writer endpoint は作られない。
    assert!(opened.writer_ends.is_empty());
    let (mut adapter, writers) = into_writers(opened, &fault);
    assert!(writers.is_empty());
    let stream = device.stream(0);
    for _ in 0..4 {
        stream.lock().expect("lock").deliver(500);
        adapter.pump(&plan.scheduler);
    }
    assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());
}

#[test]
fn monitor_does_not_double_correct_placement_across_tempo_boundary() {
    // tick 480 で 120→60 BPM（vs_b の tempo fixture と同型）。
    // session_end = 84000 frame。Monitor Off/Auto で span 列が
    // 完全一致することを確認し、monitor が placement に二重補正を
    // 掛けないことを検証する。
    let (mut p, lanes, epoch) = project_with_lanes(1);
    p.tempo_map.tempo_events.push(TempoEventDto {
        tick: 480,
        bpm_milli: 60_000,
    });
    p.tempo_map.revision = 1;
    let req = RecordingRequest {
        targets: vec![synvelis_recording::CaptureTarget {
            lane_id: lanes[0],
            channels: 2,
        }],
        clock_epoch: epoch,
        tempo_map: TempoMapRef { revision: 1 },
        transport_start: MusicalTick::ZERO,
        stop: MusicalTick::new(1920),
        gate: synvelis_recording::GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        budget: budget(),
        max_block_frames: 1024,
    };
    let plan = preflight(&p, req, RATE).expect("preflight must pass");
    assert_eq!(plan.session_end, SessionFrame::new(84_000).unwrap());
    let frames = u32::try_from(plan.session_end.value()).expect("session_end fits u32");
    // block 997 は tempo 境界 frame 12000 を跨ぐ奇数刻み。
    let off = capture_with_monitor(&plan, epoch, MonitorState::Off, frames, 997);
    let auto = capture_with_monitor(&plan, epoch, MonitorState::Auto, frames, 997);
    // 全 84000 frame が保存されることを被覆量で確認してから比較する。
    let covered: u32 = off.spans.iter().map(|s| s.frame_count).sum();
    assert_eq!(covered, 84_000);
    // tempo 境界で span が分割されていること（gate 権威の写像）。
    assert!(
        off.spans
            .iter()
            .any(|s| s.session_start.value() == 12_000 && s.project_in.value() == 480),
        "expected span starting at tempo boundary"
    );
    assert_eq!(off, auto, "monitor changed placement/pcm across tempo");
}

// ---------- Loop / PunchInLoop gate ----------

/// `segs` が `[0, end)` を隙間・重複・空区間なく tile することを検証。
fn assert_tiled(segs: &SegmentView, end: i64) {
    let mut pos = 0;
    for (a, b, _, _) in segs {
        assert_eq!(*a, pos, "gap/overlap before segment {a}..{b}");
        assert!(*b > *a, "empty segment {a}..{b}");
        pos = *b;
    }
    assert_eq!(pos, end, "segments do not reach session end");
}

/// `segs` の `gate_open` 区間一覧（半開 `[in, out)`）。
fn open_intervals(segs: &SegmentView) -> Vec<(i64, i64)> {
    segs.iter()
        .filter(|(_, _, g, _)| *g)
        .map(|(a, b, _, _)| (*a, *b))
        .collect()
}

/// `open_intervals` の隣接区間を連結した半開区間一覧
/// （feed block 単位の分割を吸収する）。
fn merged_open_intervals(segs: &SegmentView) -> Vec<(i64, i64)> {
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (a, b) in open_intervals(segs) {
        if let Some(last) = merged.last_mut()
            && last.1 == a
        {
            last.1 = b;
        } else {
            merged.push((a, b));
        }
    }
    merged
}

/// `pass` の span を session 順に並べ、project が `[want_in,
/// want_out)` を連続・重複なく被覆することを検証する。
/// span 分割粒度（block/chunk 境界）に依らない不変条件 —
/// pass ごとに同じ project 区間が再記録される（Raw Placement
/// 二重補正 0）ことの証拠。
fn assert_pass_project_chain(
    spans: &[synvelis_recording::CaptureSpan],
    pass: u32,
    want_in: i64,
    want_out: i64,
) {
    let mut v: Vec<&synvelis_recording::CaptureSpan> =
        spans.iter().filter(|s| s.pass_index == pass).collect();
    v.sort_by_key(|s| s.session_start.value());
    assert!(!v.is_empty(), "pass {pass} has no spans");
    assert_eq!(v[0].project_in.value(), want_in, "pass {pass} project_in");
    assert_eq!(
        v.last().map(|s| s.project_out.value()),
        Some(want_out),
        "pass {pass} project_out"
    );
    for w in v.windows(2) {
        assert_eq!(
            w[0].project_out.value(),
            w[1].project_in.value(),
            "pass {pass} project chain broken"
        );
        let prev_end = w[0].session_start.value() + i64::from(w[0].frame_count);
        assert_eq!(
            prev_end,
            w[1].session_start.value(),
            "pass {pass} session chain broken"
        );
    }
}

#[test]
fn monitor_probe_loop_gate_open_is_contiguous_from_loop_start() {
    // loop [10,50)tick = frame [250,1250)、pass_len=1000f。
    // stop=110tick=2750f → pass0 [250,1250), pass1 [1250,2250),
    // pass2 [2250,2750)（session_end でクランプ）。Loop gate の
    // capture 区間は loop body 全体なので gate_open は pass を
    // 連続に tile し [250,2750) となる（pass 境界で開閉しない）。
    let (plan, _epoch) = plan_with_gate(
        synvelis_recording::GateSpec::Loop {
            loop_start: MusicalTick::new(10),
            loop_end: MusicalTick::new(50),
        },
        110,
        1024,
    );
    let mut c = ctx();
    c.arm = ArmState::On;
    c.monitor = MonitorState::Auto;
    // block 384 は loop_start 250 と pass 境界 1250/2250 を
    // すべて block 途中に跨ぐ。
    let segs = probe_segments(&plan, &c, 2750, 384, TransportPhase::Playing);
    assert_tiled(&segs, 2750);
    // gate open 区間は [250,2750) に連続（feed block 単位で
    // 分割されるが、開閉の遷移は loop_start のみ）。
    let open = open_intervals(&segs);
    assert_eq!(open.first().map(|(a, _)| *a), Some(250));
    assert_eq!(open.last().map(|(_, b)| *b), Some(2750));
    // 各 open 区間が連続していること（pass 境界で閉区間が入らない）。
    for w in open.windows(2) {
        assert_eq!(w[0].1, w[1].0, "gap inside loop open region");
    }
    // route: closed → Playback、open → Input（Auto+arm+Playing）。
    for (_, _, g, r) in &segs {
        assert_eq!(*r, if *g { Input } else { Playback });
    }
    // loop_start より前 [0,250) は閉区間。
    assert!(
        segs.iter()
            .any(|(a, b, g, r)| { *a == 0 && *b == 250 && !*g && *r == Playback })
    );
}

#[test]
fn monitor_probe_punch_in_loop_block_interior_boundaries() {
    // loop [0,40)tick=1000f、punch [10,30)tick=frame[250,750)。
    // stop=100tick=2500f → open 区間 [250,750),[1250,1750),
    // [2250,2500)（最終 pass 途中停止でクランプ）。block 384 は
    // punch/pass 境界をすべて block 途中に跨ぐ。
    let (plan, _epoch) = plan_with_gate(
        synvelis_recording::GateSpec::PunchInLoop {
            loop_start: MusicalTick::ZERO,
            loop_end: MusicalTick::new(40),
            punch_in: MusicalTick::new(10),
            punch_out: MusicalTick::new(30),
        },
        100,
        1024,
    );
    let mut c = ctx();
    c.arm = ArmState::On;
    c.monitor = MonitorState::Auto;
    let segs = probe_segments(&plan, &c, 2500, 384, TransportPhase::Playing);
    assert_tiled(&segs, 2500);
    // pass ごとの punch 区間が session-frame 精度で再 gate される
    // （block 分割は連結して比較）。
    assert_eq!(
        merged_open_intervals(&segs),
        vec![(250, 750), (1250, 1750), (2250, 2500)]
    );
    for (_, _, g, r) in &segs {
        assert_eq!(*r, if *g { Input } else { Playback });
    }
    // block 384 刻みでは punch 境界が block 内に落ちる segment が
    // 存在すること（block 途中分割の証拠）。
    assert!(
        segs.iter().any(|(a, _, _, _)| *a % 384 != 0 && *a != 0),
        "expected segments starting inside blocks: {segs:?}"
    );
}

#[test]
fn loop_gate_capture_spans_repeat_project_mapping_per_pass() {
    // Loop gate の capture span は pass ごとに同じ project 区間
    // （loop body の musical 範囲）を指し、session_start のみが
    // pass_len ずつ進む — Raw Placement の二重補正がないことの
    // 証拠。pass_index も 0,1,2 と進む。
    let (plan, epoch) = plan_with_gate(
        synvelis_recording::GateSpec::Loop {
            loop_start: MusicalTick::new(10),
            loop_end: MusicalTick::new(50),
        },
        110,
        1024,
    );
    let frames = u32::try_from(plan.session_end.value()).expect("session_end fits u32");
    let off = capture_with_monitor(&plan, epoch, MonitorState::Off, frames, 500);
    let auto = capture_with_monitor(&plan, epoch, MonitorState::Auto, frames, 500);
    let on = capture_with_monitor(&plan, epoch, MonitorState::On, frames, 500);
    assert_eq!(off, auto, "Auto changed raw capture on Loop gate");
    assert_eq!(off, on, "On changed raw capture on Loop gate");
    // pass ごとに span を集約し、project が loop body
    // [10,50) を連続被覆することを検証する（span 分割粒度に
    // 依らない不変条件）。pass2 は session_end=2750 でクランプ
    // され project [10,30) の部分 pass。
    let pass_ids: std::collections::BTreeSet<u32> =
        off.spans.iter().map(|s| s.pass_index).collect();
    assert_eq!(
        pass_ids,
        [0, 1, 2].into_iter().collect(),
        "expected 3 loop passes"
    );
    assert_pass_project_chain(&off.spans, 0, 10, 50);
    assert_pass_project_chain(&off.spans, 1, 10, 50);
    assert_pass_project_chain(&off.spans, 2, 10, 30);
    // 各 pass の先頭 span session_start は pass_len=1000f 刻み。
    let first_of_pass: Vec<(u32, i64)> = off
        .spans
        .iter()
        .filter(|s| s.project_in.value() == 10)
        .map(|s| (s.pass_index, s.session_start.value()))
        .collect();
    assert_eq!(
        first_of_pass,
        vec![(0, 250), (1, 1250), (2, 2250)],
        "loop pass session starts must advance by pass_len"
    );
}

#[test]
fn punch_in_loop_capture_spans_repeat_project_mapping_per_pass() {
    // PunchInLoop: 各 pass が同じ punch project 区間 [10,30) を
    // 再記録する。pass ごとの (project_in, project_out) 列が
    // 一致し、Raw Placement 二重補正がないことを検証する。
    let (plan, epoch) = plan_with_gate(
        synvelis_recording::GateSpec::PunchInLoop {
            loop_start: MusicalTick::ZERO,
            loop_end: MusicalTick::new(40),
            punch_in: MusicalTick::new(10),
            punch_out: MusicalTick::new(30),
        },
        100,
        1024,
    );
    let frames = u32::try_from(plan.session_end.value()).expect("session_end fits u32");
    let off = capture_with_monitor(&plan, epoch, MonitorState::Off, frames, 500);
    let auto = capture_with_monitor(&plan, epoch, MonitorState::Auto, frames, 500);
    let on = capture_with_monitor(&plan, epoch, MonitorState::On, frames, 500);
    assert_eq!(off, auto, "Auto changed raw capture on PunchInLoop gate");
    assert_eq!(off, on, "On changed raw capture on PunchInLoop gate");
    // 各 pass が同じ punch project 区間 [10,30) を連続被覆する
    // ことを検証する。pass2 は session_end=2500 でクランプされ
    // project [10,20) の部分 pass。
    let pass_ids: std::collections::BTreeSet<u32> =
        off.spans.iter().map(|s| s.pass_index).collect();
    assert_eq!(
        pass_ids,
        [0, 1, 2].into_iter().collect(),
        "expected 3 punch passes"
    );
    assert_pass_project_chain(&off.spans, 0, 10, 30);
    assert_pass_project_chain(&off.spans, 1, 10, 30);
    assert_pass_project_chain(&off.spans, 2, 10, 20);
    // 保存総量 = punch 区間 500f × 2 完全 pass + 最終 pass 250f。
    let total: u32 = off.spans.iter().map(|s| s.frame_count).sum();
    assert_eq!(total, 1250);
}

#[test]
fn punch_in_loop_gate_across_tempo_boundary_keeps_project_mapping() {
    // tempo 120→60bpm at tick 480。loop [400,560)tick・punch
    // [450,530)tick は tempo 境界を跨ぐ。pass ごとの punch
    // project mapping が tempo に依らず同一 tick 区間を指し、
    // Monitor Off/Auto で capture が一致することを検証する
    // （Raw Placement 二重補正 0）。
    let (mut p, lanes, epoch) = project_with_lanes(1);
    p.tempo_map.tempo_events.push(TempoEventDto {
        tick: 480,
        bpm_milli: 60_000,
    });
    p.tempo_map.revision = 1;
    let req = RecordingRequest {
        targets: vec![synvelis_recording::CaptureTarget {
            lane_id: lanes[0],
            channels: 2,
        }],
        clock_epoch: epoch,
        tempo_map: TempoMapRef { revision: 1 },
        transport_start: MusicalTick::ZERO,
        stop: MusicalTick::new(900),
        gate: synvelis_recording::GateSpec::PunchInLoop {
            loop_start: MusicalTick::new(400),
            loop_end: MusicalTick::new(560),
            punch_in: MusicalTick::new(450),
            punch_out: MusicalTick::new(530),
        },
        budget: budget(),
        max_block_frames: 1024,
    };
    let plan = preflight(&p, req, RATE).expect("preflight must pass");
    let frames = u32::try_from(plan.session_end.value()).expect("session_end fits u32");
    let off = capture_with_monitor(&plan, epoch, MonitorState::Off, frames, 997);
    let auto = capture_with_monitor(&plan, epoch, MonitorState::Auto, frames, 997);
    // tempo 境界で span が分割され、punch project 区間が
    // pass ごとに同一 tick 区間 [450,530) を連続被覆することを
    // 検証する（pass_len=6000f・session_end=33000f で 4 pass、
    // punch は全 pass で session_end 内に完結）。
    let pass_ids: std::collections::BTreeSet<u32> =
        off.spans.iter().map(|s| s.pass_index).collect();
    assert_eq!(
        pass_ids.len(),
        4,
        "expected 4 punch passes across tempo boundary: {pass_ids:?}"
    );
    for pass in 0..4 {
        assert_pass_project_chain(&off.spans, pass, 450, 530);
    }
    assert_eq!(off, auto, "monitor changed placement/pcm across tempo+loop");
}

// ---------- Mono/Stereo binding ----------

#[test]
fn mono_binding_capture_and_probe_match_channel_agnostic_contract() {
    // channel 数非依存（spec）: mono binding でも Monitor Off/Auto/On で
    // Raw byte/span が一致し、probe の route 分割は session frame の
    // みで成立する。
    let (plan, epoch) = plan_with_gate_map(
        synvelis_recording::GateSpec::Punch {
            punch_in: MusicalTick::new(12),
            punch_out: MusicalTick::new(28),
        },
        200,
        1024,
        1,
    );
    let frames = u32::try_from(plan.session_end.value()).expect("session_end fits u32");
    let map = ChannelMap::mono(0);
    let off =
        capture_with_monitor_map(&plan, epoch, MonitorState::Off, frames, 500, 1, map.clone());
    let auto = capture_with_monitor_map(
        &plan,
        epoch,
        MonitorState::Auto,
        frames,
        500,
        1,
        map.clone(),
    );
    let on = capture_with_monitor_map(&plan, epoch, MonitorState::On, frames, 500, 1, map);
    // 保存総量 = punch 区間 400 frame のみ（stereo と同一契約）。
    let total: u32 = off.spans.iter().map(|s| s.frame_count).sum();
    assert_eq!(total, 400);
    assert_eq!(off, auto, "Auto changed raw capture on mono binding");
    assert_eq!(off, on, "On changed raw capture on mono binding");
    // probe も同一 plan で gate 連動 segment を返す（Auto+arm+Playing:
    // open → Input、closed → Playback）。
    let mut c = ctx();
    c.arm = ArmState::On;
    c.monitor = MonitorState::Auto;
    let segs = probe_segments(&plan, &c, frames, 500, TransportPhase::Playing);
    let mut pos = 0;
    for (a, b, _, _) in &segs {
        assert_eq!(*a, pos, "gap/overlap before segment {a}..{b}");
        pos = *b;
    }
    assert_eq!(pos, plan.session_end.value());
    for (_, _, g, r) in &segs {
        assert_eq!(*r, if *g { Input } else { Playback });
    }
}
