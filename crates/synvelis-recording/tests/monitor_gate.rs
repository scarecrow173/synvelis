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
    let (p, lanes, epoch) = project_with_lanes(1);
    let req = RecordingRequest {
        targets: vec![synvelis_recording::CaptureTarget {
            lane_id: lanes[0],
            channels: 2,
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
    let device = FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK);
    let spec = synvelis_recording::InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            plan,
            0,
            ChannelMap::stereo(0, 1),
            ArmState::On,
            monitor,
        )],
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
