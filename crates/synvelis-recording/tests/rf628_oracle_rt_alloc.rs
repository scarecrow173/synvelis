//! RCD-01 `rf#628-fixture` RT 契約証拠（synvelis#115 SV-6）:
//! `ruviform-audio-test` scripted backend の `pump_input_next` が
//! 発火する data callback（planar→interleave→work queue push）
//! 経路全体 — backend pump + PCM scenario fill + callback — が
//! heap 割当ても dealloc も行わないことを `stats_alloc` の
//! グローバルアロケータで計測する。
//!
//! `cargo test` は同一バイナリ内のテストを並列スレッドで実行し、
//! グローバルアロケータを共有するため、計測はこのファイル内の
//! 単一テストに集約する（他スレッドの割当てを窓に混入させない）。
//! 計測の前に warm-up を行い、初期化系の遅延割当て（stream open・
//! scratch 確保・queue/pool の初回経路）を窓から除外する。
//!
//! scripted backend は deterministic-backend lane の証拠であり、
//! 実 OS audio callback の RT 性能を意味しない。

// 各 test binary が common の subset だけを使うため module 単位で許容
// （synvelis-library/tests/common と同じ運用）。
#[allow(dead_code)]
mod common;

use std::alloc::System;
use std::sync::Arc;

use common::*;
use ruviform_audio_core::device::Device;
use ruviform_audio_core::device::{SupportedBufferSize, SupportedInputFormat};
use ruviform_audio_core::{AudioFormat, SampleFormat, SampleRate};
use ruviform_audio_test::backend::{DeviceSpec, InputSpec, ScriptedBackend};
use ruviform_audio_test::clock::{
    JitterSpec, VirtualClock, VirtualClockConfig, VirtualClockDriver,
};
use ruviform_audio_test::pcm::{PcmFixture, PcmPattern, PcmScenario, PcmSegment};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use synvelis_project::ClockEpochId;
use synvelis_recording::{
    ArmState, ChannelMap, FaultLatch, InputAdapter, InputStreamSpec, MonitorState,
    PreparedRecordingPlan,
};

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const BLOCK: u32 = 256;

/// armed stereo tap の spec（`input_adapter_rt_alloc.rs` と同型）。
fn stereo_spec(plan: &PreparedRecordingPlan, epoch: ClockEpochId) -> InputStreamSpec {
    InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            plan,
            0,
            ChannelMap::stereo(0, 1),
            ArmState::On,
            MonitorState::Off,
        )],
    }
}

/// scripted stereo input device（Noise scenario・256 frame/callback）。
fn scripted_stereo_device(
    backend: &mut ScriptedBackend,
    frames: u64,
) -> ruviform_audio_test::backend::ScriptedDeviceHandle {
    let scenario = PcmScenario::new(vec![PcmSegment {
        frame_start: 0,
        frame_len: frames,
        fixture: PcmFixture::new(
            SampleRate::new(RATE).expect("rate"),
            2,
            PcmPattern::Noise {
                seed: 13,
                amplitude: 0.8,
            },
        )
        .expect("fixture"),
    }])
    .expect("scenario");
    backend
        .add_device(DeviceSpec {
            name: "rt-scripted".to_owned(),
            id: Some("rt-scripted-id".to_owned()),
            clock: None,
            output: None,
            input: Some(InputSpec {
                default_format: AudioFormat {
                    sample_rate: SampleRate::new(RATE).expect("rate"),
                    layout: layout_for(2),
                    sample_format: SampleFormat::F32,
                },
                supported_formats: vec![SupportedInputFormat {
                    channels: 2,
                    min_sample_rate: 44_100,
                    max_sample_rate: 96_000,
                    buffer_size: SupportedBufferSize::Unknown,
                    sample_format: SampleFormat::F32,
                }],
                pcm: scenario,
                stream: ruviform_audio_test::backend::StreamScript::new(BLOCK),
                capture_reporting: true,
            }),
        })
        .expect("add scripted device")
}

#[test]
fn rt_scripted_input_callback_is_alloc_and_drop_free() {
    let clock = Arc::new(
        VirtualClock::new(VirtualClockConfig {
            sample_rate: RATE,
            epoch: 7,
            offset_ticks: 0,
            drift_ppm: 0,
            jitter: JitterSpec::DISABLED,
        })
        .expect("clock"),
    );
    let mut backend = ScriptedBackend::new(Arc::clone(&clock));
    // warm-up + 計測 + drain 分を十分にカバーする scenario。
    let handle = scripted_stereo_device(&mut backend, 65_536);

    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[2], epoch, 200);
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(
        &handle as &dyn Device,
        &stereo_spec(&plan, epoch),
        fault.clone(),
    )
    .expect("open");
    let (mut adapter, mut writers) = into_writers(opened, &fault);
    adapter.stream().start().expect("stream start");
    let mut driver = VirtualClockDriver::new(Arc::clone(&clock));

    // warm-up: open 時の scratch 確保・queue/pool 初回経路・pump/writer
    // の遅延初期化を窓の外で済ませる。
    for _ in 0..8 {
        let report = handle.pump_input_next().expect("warm-up pump");
        driver.advance_frames(u64::from(report.frames));
        adapter.pump(&plan.scheduler);
        for (_, w) in &mut writers {
            w.pump();
        }
    }
    assert!(!fault.is_faulted());

    // backend pump + scenario PCM fill + data callback（planar gather
    // → interleave → work queue push）1 回分だけを Region で計る。
    let reg = Region::new(ALLOC);
    let report = handle.pump_input_next().expect("measured pump");
    let stats = reg.change();
    driver.advance_frames(u64::from(report.frames));
    assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
    assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");

    // 計測した block を pump/writer まで流し、経路全体が健全なことを確認。
    adapter.pump(&plan.scheduler);
    for (_, w) in &mut writers {
        w.pump();
    }
    assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());
    assert_eq!(
        report.direction,
        ruviform_audio_test::backend::StreamDirection::Input
    );
    assert_eq!(report.frames, BLOCK);
    assert_eq!(report.gap_frames, 0);
    assert_eq!(report.errors_fired, 0);
}
