//! RCD-01 input adapter の fake port 検証（synvelis#115 slice）。
//!
//! scripted fake `Device`/`InputStream`（`tests/common`）で
//! capture→work queue→pump→engine→handoff→writer を headless 駆動し、
//! PCM fidelity・binding/channel map・Arm/Monitor 独立・same-clock
//! domain・fail-stop を検証する。
//!
//! これは fake port 契約の検証であり、実 Device Pass ではない
//! （実 backend は ruviform#251 待ちで `Unsupported`）。

// 各 test binary が common の subset だけを使うため module 単位で許容
// （synvelis-library/tests/common と同じ運用）。
#[allow(dead_code)]
mod common;

use common::*;
use ruviform_audio_core::device::{
    BufferSize, DeviceEnumerator, DeviceError, InputStreamConfig, same_clock_domain,
};
use ruviform_audio_core::{SampleFormat, SampleRate};
use ruviform_media_core::ClockDomainId;
use synvelis_project::ClockEpochId;
use synvelis_recording::{
    ArmState, ChannelMap, FAULT_CONTRACT, FAULT_EPOCH_DISCONTINUITY, FAULT_INPUT_DISCONTINUITY,
    FAULT_QUEUE_FULL, FaultLatch, InputAdapter, InputBindError, InputBinding, InputSession,
    InputStreamSpec, InputTapSpec, InspectionWriter, MonitorState,
};

// ---------- tests ----------

#[test]
fn mono_stereo_multichannel_pcm_fidelity() {
    for (channels, map) in [
        (1_u32, ChannelMap::mono(0)),
        (2, ChannelMap::stereo(0, 1)),
        (4, ChannelMap::identity(4)),
    ] {
        let (p, lanes, epoch) = project_with_lanes(1);
        let plan = plan_for(&p, &lanes, &[channels], epoch, 200);
        let device = FakeInputDevice::new(channels, ClockDomainId::SAMPLE_CLOCK);
        let spec = InputStreamSpec {
            config: input_config(channels),
            clock_epoch: epoch,
            max_block_frames: plan.max_block_frames,
            callback_queue: 16,
            taps: vec![tap(&plan, 0, map.clone(), ArmState::On, MonitorState::Off)],
        };
        let fault = FaultLatch::new();
        let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
        let (mut adapter, mut writers) = into_writers(opened, &fault);
        assert_eq!(writers.len(), 1);
        let stream = device.stream(0);
        // session_end = 5000 frames（stop_tick 200）。500 frames × 10 block。
        drive(&stream, &mut adapter, &mut writers, &plan, 5000, 500);
        assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());
        let log = &writers[0].1.log;
        assert_ne!(log.as_slice(), []);
        let total: u32 = log
            .iter()
            .map(|l| l.spans.iter().map(|s| s.frame_count).sum::<u32>())
            .sum();
        assert_eq!(total, 5000);
        for entry in log {
            assert!(
                verify_tap_log(entry, map.channels(), SEED),
                "PCM mismatch in chunk {:?}",
                entry.chunk_id
            );
        }
    }
}

#[test]
fn two_track_one_stream_binds_channels_to_separate_tracks() {
    let (p, lanes, epoch) = project_with_lanes(2);
    let plan = plan_for(&p, &lanes, &[1, 1], epoch, 200);
    let device = FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK);
    let spec = InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![
            tap(
                &plan,
                0,
                ChannelMap::mono(0),
                ArmState::On,
                MonitorState::Off,
            ),
            tap(
                &plan,
                1,
                ChannelMap::mono(1),
                ArmState::On,
                MonitorState::Off,
            ),
        ],
    };
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
    assert_eq!(opened.writer_ends.len(), 2);
    let (mut adapter, mut writers) = into_writers(opened, &fault);
    assert_eq!(writers.len(), 2);
    let stream = device.stream(0);
    drive(&stream, &mut adapter, &mut writers, &plan, 5000, 500);
    assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());
    // track0 は入力 ch0、track1 は入力 ch1 の PCM を持つ。
    for stream_index in 0..2_u16 {
        let src = [u32::from(stream_index)];
        let (_, w) = writers
            .iter()
            .find(|(idx, _)| *idx == stream_index)
            .expect("writer for stream");
        assert_ne!(w.log.as_slice(), []);
        for entry in &w.log {
            assert!(
                verify_tap_log(entry, &src, SEED),
                "track {stream_index} PCM mismatch"
            );
        }
    }
}

#[test]
fn arm_off_monitor_on_creates_no_writer_and_captures_nothing() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[2], epoch, 200);
    let device = FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK);
    let spec = InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::stereo(0, 1),
            ArmState::Off,
            MonitorState::On,
        )],
    };
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
    // Arm Off + Monitor On: Writer endpoint は作られない。
    assert!(opened.writer_ends.is_empty());
    let records = opened.adapter.tap_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].arm, ArmState::Off);
    assert_eq!(records[0].monitor, MonitorState::On);
    assert!(!records[0].capturing);
    let (mut adapter, writers) = into_writers(opened, &fault);
    assert!(writers.is_empty());
    let stream = device.stream(0);
    // callback を駆動しても capture path が無いので fault も chunk も無い。
    for _ in 0..10 {
        stream.lock().expect("lock").deliver(500);
        adapter.pump(&plan.scheduler);
    }
    assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());
}

#[test]
fn monitor_on_does_not_change_capture() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[2], epoch, 200);
    let device = FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK);
    let spec = InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::stereo(0, 1),
            ArmState::On,
            MonitorState::On,
        )],
    };
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
    assert_eq!(opened.writer_ends.len(), 1);
    let (mut adapter, mut writers) = into_writers(opened, &fault);
    let stream = device.stream(0);
    drive(&stream, &mut adapter, &mut writers, &plan, 5000, 500);
    assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());
    assert!(
        writers[0]
            .1
            .log
            .iter()
            .all(|l| verify_tap_log(l, &[0, 1], SEED))
    );
}

#[test]
fn same_clock_domain_two_streams_capture_together() {
    let (p, lanes, epoch) = project_with_lanes(2);
    let plan = plan_for(&p, &lanes, &[1, 1], epoch, 200);
    let dev_a = FakeInputDevice::new(1, ClockDomainId::MONOTONIC);
    let dev_b = FakeInputDevice::new(1, ClockDomainId::MONOTONIC).with_seed(99);
    let mk = |idx: u16| InputStreamSpec {
        config: input_config(1),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            idx,
            ChannelMap::mono(0),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    let (mut session, ends_a) = InputSession::open(&dev_a, &mk(0)).expect("open first");
    let ends_b = session.add(&dev_b, &mk(1)).expect("add second");
    assert_eq!(ends_a.len() + ends_b.len(), 2);
    let fault = session.fault().clone();
    let mut writers_a: Vec<_> = ends_a
        .into_iter()
        .map(|w| (w.stream_index, InspectionWriter::new(w.ends, fault.clone())))
        .collect();
    let mut writers_b: Vec<_> = ends_b
        .into_iter()
        .map(|w| (w.stream_index, InspectionWriter::new(w.ends, fault.clone())))
        .collect();
    let stream_a = dev_a.stream(0);
    let stream_b = dev_b.stream(0);
    for _ in 0..10 {
        stream_a.lock().expect("lock").deliver(500);
        stream_b.lock().expect("lock").deliver(500);
        session.pump_all(&plan.scheduler);
        for (_, w) in writers_a.iter_mut().chain(writers_b.iter_mut()) {
            w.pump();
        }
    }
    assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());
    assert!(
        writers_a[0]
            .1
            .log
            .iter()
            .all(|l| verify_tap_log(l, &[0], SEED))
    );
    // dev_b は別 seed: stream ごとの PCM が独立に検算できる。
    assert!(
        writers_b[0]
            .1
            .log
            .iter()
            .all(|l| verify_tap_log(l, &[0], 99))
    );
}

#[test]
fn mismatched_clock_domain_is_rejected() {
    let (p, lanes, epoch) = project_with_lanes(2);
    let plan = plan_for(&p, &lanes, &[1, 1], epoch, 200);
    let dev_a = FakeInputDevice::new(1, ClockDomainId::MONOTONIC);
    let dev_b = FakeInputDevice::new(1, ClockDomainId::new(99));
    let mk = |idx: u16| InputStreamSpec {
        config: input_config(1),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            idx,
            ChannelMap::mono(0),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    let (mut session, _ends) = InputSession::open(&dev_a, &mk(0)).expect("open first");
    let err = session
        .add(&dev_b, &mk(1))
        .err()
        .expect("must reject mismatched domain");
    assert_eq!(err, InputBindError::ClockDomainMismatch);
}

#[test]
fn mismatched_clock_epoch_is_rejected() {
    let (p, lanes, epoch) = project_with_lanes(2);
    let plan = plan_for(&p, &lanes, &[1, 1], epoch, 200);
    let dev_a = FakeInputDevice::new(1, ClockDomainId::MONOTONIC);
    let dev_b = FakeInputDevice::new(1, ClockDomainId::MONOTONIC);
    let mk = |idx: u16, ep: ClockEpochId| InputStreamSpec {
        config: input_config(1),
        clock_epoch: ep,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            idx,
            ChannelMap::mono(0),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    let (mut session, _ends) = InputSession::open(&dev_a, &mk(0, epoch)).expect("open first");
    // 同一 clock domain でも epoch が違えば拒否（device open 前に fail）。
    let other_epoch = ClockEpochId::new(23);
    let err = session
        .add(&dev_b, &mk(1, other_epoch))
        .err()
        .expect("must reject mismatched epoch");
    assert_eq!(err, InputBindError::ClockEpochMismatch);
    // stream_index 重複も session 横断で拒否。
    let err = session
        .add(&dev_b, &mk(0, epoch))
        .err()
        .expect("must reject duplicate stream_index");
    assert_eq!(err, InputBindError::DuplicateStreamIndex);
}

#[test]
fn unknown_clock_domain_is_rejected() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[1], epoch, 200);
    let device = FakeInputDevice::new(1, ClockDomainId::UNKNOWN);
    let spec = mono_spec(&plan, epoch, 16);
    let fault = FaultLatch::new();
    let err = InputAdapter::open(&device, &spec, fault)
        .err()
        .expect("must reject unknown domain");
    assert_eq!(err, InputBindError::ClockDomainUnknown);
}

#[test]
fn input_frame_gap_faults_and_preserves_verified_prefix() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[2], epoch, 200);
    let device = FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK);
    let spec = InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::stereo(0, 1),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
    let (mut adapter, mut writers) = into_writers(opened, &fault);
    let stream = device.stream(0);
    // 3 block 正常 → gap(+777) → さらに deliver。
    for _ in 0..3 {
        stream.lock().expect("lock").deliver(500);
        adapter.pump(&plan.scheduler);
        for (_, w) in &mut writers {
            w.pump();
        }
    }
    assert!(!fault.is_faulted());
    stream.lock().expect("lock").jump_to(1500 + 777);
    stream.lock().expect("lock").deliver(500);
    adapter.pump(&plan.scheduler);
    for (_, w) in &mut writers {
        w.pump();
    }
    assert!(fault.is_faulted());
    assert_eq!(
        fault.bits() & FAULT_INPUT_DISCONTINUITY,
        FAULT_INPUT_DISCONTINUITY
    );
    // verified prefix: fault 前の 3 chunk のみが writer に届き、
    // 全て PCM 検算を通る（gap 以降は捏造されない）。
    let log = &writers[0].1.log;
    assert_eq!(log.len(), 3, "log: {}", log.len());
    assert!(log.iter().all(|l| verify_tap_log(l, &[0, 1], SEED)));
}

#[test]
fn missing_frame_position_faults() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[1], epoch, 200);
    let device = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK);
    let spec = mono_spec(&plan, epoch, 16);
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
    let (mut adapter, _writers) = into_writers(opened, &fault);
    let stream = device.stream(0);
    stream.lock().expect("lock").set_report_position(false);
    stream.lock().expect("lock").deliver(500);
    adapter.pump(&plan.scheduler);
    assert_eq!(
        fault.bits() & FAULT_INPUT_DISCONTINUITY,
        FAULT_INPUT_DISCONTINUITY
    );
}

#[test]
fn clock_domain_change_mid_stream_faults() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[1], epoch, 200);
    let device = FakeInputDevice::new(1, ClockDomainId::MONOTONIC);
    let spec = mono_spec(&plan, epoch, 16);
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
    let (mut adapter, _writers) = into_writers(opened, &fault);
    let stream = device.stream(0);
    stream.lock().expect("lock").deliver(500);
    adapter.pump(&plan.scheduler);
    assert!(!fault.is_faulted());
    // callback が別 domain を名乗る → EPOCH discontinuity。
    stream
        .lock()
        .expect("lock")
        .set_cb_domain(ClockDomainId::new(42));
    stream.lock().expect("lock").deliver(500);
    adapter.pump(&plan.scheduler);
    assert_eq!(
        fault.bits() & FAULT_EPOCH_DISCONTINUITY,
        FAULT_EPOCH_DISCONTINUITY
    );
}

#[test]
fn device_error_callbacks_map_to_faults() {
    for (err, bit) in [
        (DeviceError::StreamInvalidated, FAULT_INPUT_DISCONTINUITY),
        (DeviceError::DeviceNotFound, FAULT_INPUT_DISCONTINUITY),
        (DeviceError::Xrun, FAULT_INPUT_DISCONTINUITY),
        (DeviceError::ClockDiscontinuity, FAULT_EPOCH_DISCONTINUITY),
    ] {
        let (p, lanes, epoch) = project_with_lanes(1);
        let plan = plan_for(&p, &lanes, &[1], epoch, 200);
        let device = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK);
        let spec = mono_spec(&plan, epoch, 16);
        let fault = FaultLatch::new();
        let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
        let (mut adapter, _writers) = into_writers(opened, &fault);
        let stream = device.stream(0);
        stream.lock().expect("lock").raise(err);
        // fault 後の callback は no-op（fail-stop）。
        stream.lock().expect("lock").deliver(500);
        adapter.pump(&plan.scheduler);
        assert_eq!(fault.bits() & bit, bit, "err {err:?} bit {bit:#x}");
    }
}

#[test]
fn work_queue_full_faults() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[1], epoch, 200);
    let device = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK);
    let spec = mono_spec(&plan, epoch, 1);
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
    let (mut adapter, _writers) = into_writers(opened, &fault);
    let stream = device.stream(0);
    // pump せずに queue 容量を超えて deliver → QUEUE_FULL。
    stream.lock().expect("lock").deliver(500);
    stream.lock().expect("lock").deliver(500);
    adapter.pump(&plan.scheduler);
    assert_eq!(fault.bits() & FAULT_QUEUE_FULL, FAULT_QUEUE_FULL);
}

#[test]
fn filled_queue_full_faults_through_adapter() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[1], epoch, 200);
    let device = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK);
    let mut b = budget();
    b.filled_capacity = 1;
    let spec = InputStreamSpec {
        config: input_config(1),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![InputTapSpec {
            binding: InputBinding {
                channel_map: ChannelMap::mono(0),
                arm: ArmState::On,
                monitor: MonitorState::Off,
            },
            stream_index: 0,
            session_end: plan.session_end,
            budget: b,
        }],
    };
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
    let (mut adapter, _writers) = into_writers(opened, &fault);
    // writer を pump しない → filled queue（容量 1）が 2 個目の
    // chunk で詰まり FAULT_QUEUE_FULL。
    let stream = device.stream(0);
    for _ in 0..3 {
        stream.lock().expect("lock").deliver(500);
        adapter.pump(&plan.scheduler);
    }
    assert!(fault.is_faulted());
    assert_eq!(fault.bits() & FAULT_QUEUE_FULL, FAULT_QUEUE_FULL);
}

#[test]
fn channel_count_change_faults() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[2], epoch, 200);
    let device = FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK);
    let spec = InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::stereo(0, 1),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
    let (mut adapter, _writers) = into_writers(opened, &fault);
    let stream = device.stream(0);
    stream.lock().expect("lock").set_deliver_channels(1);
    stream.lock().expect("lock").deliver(500);
    adapter.pump(&plan.scheduler);
    assert_eq!(fault.bits() & FAULT_CONTRACT, FAULT_CONTRACT);
}

#[test]
fn oversized_block_faults() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[1], epoch, 200);
    let device = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK);
    let spec = InputStreamSpec {
        config: input_config(1),
        clock_epoch: epoch,
        max_block_frames: 512,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::mono(0),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
    let (mut adapter, _writers) = into_writers(opened, &fault);
    let stream = device.stream(0);
    stream.lock().expect("lock").deliver(1024); // > max_block_frames
    adapter.pump(&plan.scheduler);
    assert_eq!(fault.bits() & FAULT_CONTRACT, FAULT_CONTRACT);
}

#[test]
fn sample_rate_change_is_detected_by_verify_format() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[1], epoch, 200);
    let device = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK);
    let spec = mono_spec(&plan, epoch, 16);
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&device, &spec, fault.clone()).expect("open");
    let (adapter, _writers) = into_writers(opened, &fault);
    // device 側で rate が変わった場合、control path の照合で検出する。
    let mut bad = input_format(1);
    bad.sample_rate = SampleRate::new(44_100).expect("rate");
    device.stream(0).lock().expect("lock").set_format(bad);
    adapter.verify_format();
    assert_eq!(fault.bits() & FAULT_CONTRACT, FAULT_CONTRACT);
}

#[test]
fn binding_validation_rejects_bad_specs() {
    let (p, lanes, epoch) = project_with_lanes(2);
    let plan = plan_for(&p, &lanes, &[1, 1], epoch, 200);
    let fault = FaultLatch::new();
    let device = FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK);

    // channel map が stream channel 数を超える参照。
    let spec = InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::mono(5),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    assert_eq!(
        InputAdapter::open(&device, &spec, fault.clone())
            .err()
            .expect("must reject"),
        InputBindError::ChannelOutOfRange
    );

    // armed tap の stream_index 重複。
    let spec = InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![
            tap(
                &plan,
                0,
                ChannelMap::mono(0),
                ArmState::On,
                MonitorState::Off,
            ),
            tap(
                &plan,
                0,
                ChannelMap::mono(1),
                ArmState::On,
                MonitorState::Off,
            ),
        ],
    };
    assert_eq!(
        InputAdapter::open(&device, &spec, fault.clone())
            .err()
            .expect("must reject"),
        InputBindError::DuplicateStreamIndex
    );

    // F32 以外の sample format。
    let mut fmt = input_format(1);
    fmt.sample_format = SampleFormat::F64;
    let spec = InputStreamSpec {
        config: InputStreamConfig {
            format: fmt,
            buffer_size: BufferSize::Default,
        },
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::mono(0),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    assert_eq!(
        InputAdapter::open(&device, &spec, fault.clone())
            .err()
            .expect("must reject"),
        InputBindError::UnsupportedSampleFormat
    );

    // taps 空。
    let spec = InputStreamSpec {
        config: input_config(1),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![],
    };
    assert_eq!(
        InputAdapter::open(&device, &spec, fault)
            .err()
            .expect("must reject"),
        InputBindError::EmptyTaps
    );
}

#[test]
fn device_open_error_is_mapped() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[1], epoch, 200);
    let device = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK);
    device.fail_open(DeviceError::DeviceBusy);
    let spec = mono_spec(&plan, epoch, 16);
    let fault = FaultLatch::new();
    let err = InputAdapter::open(&device, &spec, fault)
        .err()
        .expect("must fail");
    assert_eq!(err, InputBindError::Device(DeviceError::DeviceBusy));
}

#[test]
fn enumerator_and_input_stream_lifecycle() {
    // DeviceEnumerator 経路でも fake device が列挙できること。
    let dev = FakeInputDevice::new(2, ClockDomainId::MONOTONIC);
    let enum_ = FakeEnumerator {
        devices: vec![dev.clone()],
    };
    let devices = enum_.input_devices().expect("enum");
    assert_eq!(devices.len(), 1);
    assert!(enum_.default_input_device().expect("default").is_some());
    assert!(enum_.output_devices().expect("outputs").is_empty());
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[2], epoch, 200);
    let spec = InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::stereo(0, 1),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    let fault = FaultLatch::new();
    let opened = InputAdapter::open(&*devices[0], &spec, fault.clone()).expect("open");
    // format/clock_domain/start/stop のライフサイクル API。
    let stream = opened.adapter.stream();
    assert_eq!(stream.format(), input_format(2));
    assert!(same_clock_domain(
        stream.clock_domain(),
        ClockDomainId::MONOTONIC
    ));
    assert_eq!(stream.buffer_size(), Ok(BufferSize::Default));
    stream.start().expect("start");
    assert!(dev.stream(0).lock().expect("lock").is_started());
    stream.stop().expect("stop");
    assert!(!dev.stream(0).lock().expect("lock").is_started());
    // teardown: into_parts で engine ends を回収できる。
    let parts = opened.adapter.into_parts();
    assert!(parts.stream.is_some());
    assert_eq!(parts.taps.len(), 1);
    assert!(parts.fault.is_some());
}
