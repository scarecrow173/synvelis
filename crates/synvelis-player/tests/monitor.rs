//! RCD-03 monitor/latency slice の fixture 検証（synvelis#117、
//! crate 内完結 slice）。
//!
//! `MonitorAdapter` の gain ramp・native placement・route 別 PCM・
//! input missing fail-closed と、`LatencyReport` の品質/Clock
//! Epoch/basis 解決・round-trip 二重加算禁止・low-latency route
//! 状態を Fixed PCM fixture で検証する。
//!
//! これは fixture/headless 契約の検証であり、実 Device・実 GUI・
//! 実 OS での monitor/latency 受入ではない（NotRun — 実機 gate）。

use ruviform_media_core::ClockEpochId;
use synvelis_player::{
    LatencyBasis, LatencyComponent, LatencyQuality, LatencyReport, LatencyResolutionBasis,
    LowLatencyReadiness, LowLatencyReason, LowLatencyState, MonitorAdapter, MonitorBlock,
    MonitorOutputNotices, MonitorRoute, prepare_low_latency,
};

fn block(route: MonitorRoute, frames: usize, input: Option<Vec<Vec<f32>>>) -> MonitorBlock {
    MonitorBlock {
        route,
        frames,
        input,
        playback: Vec::new(),
    }
}

fn pcm_const(channels: usize, frames: usize, value: f32) -> Vec<Vec<f32>> {
    vec![vec![value; frames]; channels]
}

fn comp(frames: u64, quality: LatencyQuality, basis: LatencyBasis) -> LatencyComponent {
    LatencyComponent {
        frames,
        quality,
        basis,
        clock_epoch: Some(ClockEpochId::new(7)),
    }
}

// ---------- MonitorAdapter: native placement / route PCM ----------

#[test]
fn native_placement_is_bit_exact_without_ramp() {
    // ramp_frames=0: route Input で出力は入力と bit 一致
    // （latency/PDC 補正・オフセットなし）。
    let mut a = MonitorAdapter::new(2, 16, 0);
    let input = vec![
        vec![0.25, -0.5, 0.75, 1.0, -1.0, 0.125, 0.0, -0.0],
        vec![0.5, 0.5, -0.25, 0.0, 1.0, -0.75, 0.33, 0.11],
    ];
    let out = a.process_block(&block(MonitorRoute::Input, 8, Some(input.clone())));
    assert_eq!(out.effective_route, MonitorRoute::Input);
    assert_eq!(out.pcm[0], input[0]);
    assert_eq!(out.pcm[1], input[1]);
    assert!((a.input_gain() - 1.0).abs() < f32::EPSILON);
    assert!(a.playback_gain().abs() < f32::EPSILON);
}

#[test]
fn playback_route_outputs_playback_pcm() {
    let mut a = MonitorAdapter::new(1, 8, 0);
    let mut b = block(MonitorRoute::Playback, 4, None);
    b.playback = vec![vec![0.1, 0.2, 0.3, 0.4]];
    let out = a.process_block(&b);
    assert_eq!(out.effective_route, MonitorRoute::Playback);
    assert_eq!(out.pcm[0], vec![0.1, 0.2, 0.3, 0.4]);
    assert!((a.playback_gain() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn muted_route_produces_silence() {
    let mut a = MonitorAdapter::new(1, 8, 0);
    let mut b = block(MonitorRoute::Muted, 4, Some(pcm_const(1, 4, 0.9)));
    b.playback = pcm_const(1, 4, 0.8);
    let out = a.process_block(&b);
    assert_eq!(out.effective_route, MonitorRoute::Muted);
    assert_eq!(out.pcm[0], vec![0.0; 4]);
}

#[test]
fn input_missing_fails_closed_to_muted() {
    // route Input だが input PCM 未供給 → Muted 降格 + notice。
    // 捏造 PCM は出さず、playback も鳴らさない（fail-closed）。
    let mut a = MonitorAdapter::new(1, 8, 0);
    let mut b = block(MonitorRoute::Input, 4, None);
    b.playback = pcm_const(1, 4, 0.7);
    let out = a.process_block(&b);
    assert_eq!(out.effective_route, MonitorRoute::Muted);
    assert_eq!(
        out.notices,
        MonitorOutputNotices {
            input_missing: true
        }
    );
    assert_eq!(out.pcm[0], vec![0.0; 4]);
}

#[test]
fn gain_ramp_crossfades_over_ramp_frames() {
    // Playback 定常 → Input へ遷移、ramp_frames=4。
    let mut a = MonitorAdapter::new(1, 16, 4);
    let mut b = block(MonitorRoute::Playback, 8, None);
    b.playback = pcm_const(1, 8, 1.0);
    let _ = a.process_block(&b);
    assert!((a.playback_gain() - 1.0).abs() < f32::EPSILON);
    assert!(a.input_gain().abs() < f32::EPSILON);
    // 遷移 block: input=1.0・playback=0.5 を供給し、frame i の gain
    // は (i+1)/4 ずつ目標へ近づく crossfade を観測する。
    let mut b2 = block(MonitorRoute::Input, 8, Some(pcm_const(1, 8, 1.0)));
    b2.playback = pcm_const(1, 8, 0.5);
    let out = a.process_block(&b2);
    // frame ごとの input gain 軌跡（ramp 4 → 以降 1.0 固定）。
    let in_gains = [0.25_f32, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0, 1.0];
    for (i, (&s, &gi)) in out.pcm[0].iter().zip(in_gains.iter()).enumerate() {
        let want = gi * 1.0 + (1.0 - gi) * 0.5;
        assert!((s - want).abs() < f32::EPSILON, "frame {i}: {s} != {want}");
    }
    assert!((a.input_gain() - 1.0).abs() < f32::EPSILON);
    assert!(a.playback_gain().abs() < f32::EPSILON);
}

#[test]
fn ramp_is_per_frame_not_per_channel() {
    // stereo でも gain は frame あたり 1 回だけ進む。
    let mut a = MonitorAdapter::new(2, 16, 4);
    let mut b = block(MonitorRoute::Playback, 8, None);
    b.playback = pcm_const(2, 8, 1.0);
    let _ = a.process_block(&b);
    let out = a.process_block(&block(MonitorRoute::Input, 8, Some(pcm_const(2, 8, 1.0))));
    // 両 channel で同一の gain 軌跡 → 同一出力。
    assert_eq!(out.pcm[0], out.pcm[1]);
    assert!((a.input_gain() - 1.0).abs() < f32::EPSILON);
}

// ---------- LatencyReport::resolve ----------

#[test]
fn round_trip_is_authoritative_total_no_double_add() {
    // round_trip=480 + input=100 + output=200 + reported 2 件
    // → 合計は 480 のみ（580 等にしない）。reported は全て除外。
    let report = LatencyReport {
        round_trip: Some(LatencyComponent {
            frames: 480,
            quality: LatencyQuality::Measured,
            basis: LatencyBasis::RoundTripMeasured,
            clock_epoch: Some(ClockEpochId::new(7)),
        }),
        input: Some(comp(
            100,
            LatencyQuality::Reported,
            LatencyBasis::DeviceTimestamp,
        )),
        output: Some(comp(
            200,
            LatencyQuality::Reported,
            LatencyBasis::DeviceTimestamp,
        )),
        reported: vec![
            comp(50, LatencyQuality::Reported, LatencyBasis::CallbackReported),
            comp(
                30,
                LatencyQuality::Estimated,
                LatencyBasis::CallbackReported,
            ),
        ],
        reported_verified: true,
    };
    let r = report.resolve();
    assert_eq!(r.total_frames, Some(480));
    assert_eq!(r.quality, LatencyQuality::Measured);
    assert_eq!(r.basis, Some(LatencyResolutionBasis::RoundTripMeasured));
    assert_eq!(r.clock_epoch, Some(ClockEpochId::new(7)));
    assert_eq!(r.excluded_reported, 2);
}

#[test]
fn verified_reported_components_are_summed() {
    let report = LatencyReport {
        input: Some(comp(
            100,
            LatencyQuality::Measured,
            LatencyBasis::DeviceTimestamp,
        )),
        output: Some(comp(
            200,
            LatencyQuality::Reported,
            LatencyBasis::DeviceTimestamp,
        )),
        reported: vec![comp(
            50,
            LatencyQuality::Reported,
            LatencyBasis::CallbackReported,
        )],
        reported_verified: true,
        ..LatencyReport::default()
    };
    let r = report.resolve();
    assert_eq!(r.total_frames, Some(350));
    // 最弱品質は Reported。basis は DeviceTimestamp 優先。
    assert_eq!(r.quality, LatencyQuality::Reported);
    assert_eq!(r.basis, Some(LatencyResolutionBasis::DeviceTimestamp));
    assert_eq!(r.excluded_reported, 0);
}

#[test]
fn unverified_reported_is_excluded_not_summed() {
    let report = LatencyReport {
        input: Some(comp(
            100,
            LatencyQuality::Measured,
            LatencyBasis::DeviceTimestamp,
        )),
        reported: vec![
            comp(50, LatencyQuality::Reported, LatencyBasis::CallbackReported),
            comp(30, LatencyQuality::Reported, LatencyBasis::CallbackReported),
        ],
        reported_verified: false,
        ..LatencyReport::default()
    };
    let r = report.resolve();
    assert_eq!(r.total_frames, Some(100));
    assert_eq!(r.excluded_reported, 2);
}

#[test]
fn measured_without_clock_epoch_downgrades_to_reported() {
    // Measured だが Clock Epoch 未確定 → Reported に降格。
    let mut c = comp(100, LatencyQuality::Measured, LatencyBasis::DeviceTimestamp);
    c.clock_epoch = None;
    let report = LatencyReport {
        input: Some(c),
        ..LatencyReport::default()
    };
    let r = report.resolve();
    assert_eq!(r.total_frames, Some(100));
    assert_eq!(r.quality, LatencyQuality::Reported);
    assert_eq!(r.clock_epoch, None);
}

#[test]
fn mixed_clock_epochs_yield_no_single_epoch() {
    let mut a = comp(100, LatencyQuality::Measured, LatencyBasis::DeviceTimestamp);
    a.clock_epoch = Some(ClockEpochId::new(1));
    let mut b = comp(50, LatencyQuality::Measured, LatencyBasis::DeviceTimestamp);
    b.clock_epoch = Some(ClockEpochId::new(2));
    let report = LatencyReport {
        input: Some(a),
        output: Some(b),
        ..LatencyReport::default()
    };
    let r = report.resolve();
    assert_eq!(r.total_frames, Some(150));
    assert_eq!(r.clock_epoch, None);
    // epoch 未確定のため Measured は Reported へ降格。
    assert_eq!(r.quality, LatencyQuality::Reported);
}

#[test]
fn empty_report_resolves_to_unknown() {
    let r = LatencyReport::default().resolve();
    assert_eq!(r.total_frames, None);
    assert_eq!(r.quality, LatencyQuality::Unknown);
    assert_eq!(r.basis, None);
    assert_eq!(r.clock_epoch, None);
}

#[test]
fn overflow_fails_closed_to_unknown() {
    let report = LatencyReport {
        input: Some(comp(
            u64::MAX,
            LatencyQuality::Measured,
            LatencyBasis::DeviceTimestamp,
        )),
        output: Some(comp(
            1,
            LatencyQuality::Measured,
            LatencyBasis::DeviceTimestamp,
        )),
        ..LatencyReport::default()
    };
    let r = report.resolve();
    assert_eq!(r.total_frames, None);
    assert_eq!(r.quality, LatencyQuality::Unknown);
}

// ---------- low-latency monitor route ----------

#[test]
fn low_latency_unavailable_reasons() {
    let report = LatencyReport {
        input: Some(comp(
            100,
            LatencyQuality::Measured,
            LatencyBasis::DeviceTimestamp,
        )),
        ..LatencyReport::default()
    };
    let readiness = |prepared, input_ready, output_ready| LowLatencyReadiness {
        prepared,
        input_ready,
        output_ready,
    };
    let m = prepare_low_latency(readiness(false, true, true), false, &report);
    assert_eq!(m.state, LowLatencyState::Unavailable);
    assert_eq!(m.reason, Some(LowLatencyReason::NotPrepared));
    // latency は状態に依らず保持される。
    assert_eq!(m.latency.total_frames, Some(100));

    let m = prepare_low_latency(readiness(true, false, true), false, &report);
    assert_eq!(m.reason, Some(LowLatencyReason::InputNotReady));
    let m = prepare_low_latency(readiness(true, true, false), false, &report);
    assert_eq!(m.reason, Some(LowLatencyReason::OutputNotReady));
}

#[test]
fn low_latency_ready_and_degraded() {
    let report = LatencyReport::default();
    let ready = LowLatencyReadiness {
        prepared: true,
        input_ready: true,
        output_ready: true,
    };
    let m = prepare_low_latency(ready, false, &report);
    assert_eq!(m.state, LowLatencyState::Ready);
    assert_eq!(m.reason, None);
    let m = prepare_low_latency(ready, true, &report);
    assert_eq!(m.state, LowLatencyState::Degraded);
    assert!(m.tone_difference);
    // unavailable が tone_difference より優先。
    let m = prepare_low_latency(
        LowLatencyReadiness {
            prepared: false,
            ..ready
        },
        true,
        &report,
    );
    assert_eq!(m.state, LowLatencyState::Unavailable);
}
