//! RCD-01 device-recording headless fixture（fake port 契約証拠）。
//!
//! scripted fake `Device`/`InputStream`（`common`）→ `InputAdapter` →
//! 実 `StreamWriter`（staging media/journal）→ checkpoint/finalize
//! までを一貫して駆動し、PCM fidelity・Punch/Loop 境界・Arm/Monitor
//! 独立性・fail-stop の Verified Prefix/capture-fault を Expected/
//! Observed で検証する。
//!
//! これは fake port 契約の検証であり、実 Device Pass ではない
//! （実 backend は ruviform#251 待ちで `Unsupported`。実機受入・
//! OS 別ドライバ・実 GUI は別 Gate）。

// 各 test binary が common の subset だけを使うため module 単位で許容
// （input_adapter.rs と同じ運用）。
#[allow(dead_code)]
mod common;

use std::path::PathBuf;

use ruviform_audio_core::device::DeviceError;
use ruviform_media_core::ClockDomainId;
use synvelis_project::{MusicalTick, Project, RecordingSessionId};
use synvelis_recording::{
    ArmState, ChannelMap, FAULT_CONTRACT, FAULT_EPOCH_DISCONTINUITY, FAULT_INPUT_DISCONTINUITY,
    FAULT_QUEUE_FULL, FinalizedOutcome, GateSpec, InputBinding, InputStreamSpec, InputTapSpec,
    MonitorState, StopReason, build_payload,
};

use common::*;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-rcd01-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn session_id(raw: u128) -> RecordingSessionId {
    RecordingSessionId::new(raw)
}

/// Normal gate・stop 200 tick（5000 frame）の plan + rig を構築する。
/// `maps[i]` が `stream_index` i の channel map。戻り値は
/// `(rig, device, project)`。
fn normal_rig(
    name: &str,
    lane_count: usize,
    device_channels: u32,
    target_channels: &[u32],
    maps: &[ChannelMap],
    seed: u32,
) -> (DeviceRig, FakeInputDevice, Project) {
    let root = tmp_dir(name);
    let (p, lanes, epoch) = project_with_lanes(lane_count);
    let plan = plan_for(&p, &lanes, target_channels, epoch, 200);
    let device = FakeInputDevice::new(device_channels, ClockDomainId::SAMPLE_CLOCK)
        .with_seed(seed);
    let spec = InputStreamSpec {
        config: input_config(device_channels),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: maps
            .iter()
            .enumerate()
            .map(|(i, m)| {
                tap(
                    &plan,
                    u16::try_from(i).expect("stream index"),
                    m.clone(),
                    ArmState::On,
                    MonitorState::Off,
                )
            })
            .collect(),
    };
    let rig = DeviceRig::open_one(&root, session_id(1), plan, &device, &spec);
    (rig, device, p)
}

/// 完了した Normal 録音の共通検証:
/// - 全 armed stream の journal replay が CleanEnd/TornTail
/// - span が `[0, session_frames)` を隙間なく cover、source が連続
/// - media PCM が channel map + `MockPcm(seed)` と全 sample 一致
/// - finalize ref が Complete で `source_frames == session_frames`
fn assert_complete_normal(
    rig: &DeviceRig,
    report: &synvelis_recording::FinishReport,
    maps: &[ChannelMap],
    seed: u32,
    session_frames: i64,
) {
    assert_eq!(report.refs.len(), maps.len(), "finalize ref count");
    for (i, map) in maps.iter().enumerate() {
        let idx = u16::try_from(i).expect("stream index");
        let (_, result) = report
            .refs
            .iter()
            .find(|(s, _)| *s == idx)
            .expect("finalize ref for stream");
        let r = result.as_ref().expect("finalize must succeed");
        assert_eq!(r.outcome, FinalizedOutcome::Complete, "stream {idx}");
        assert_eq!(
            r.source_frames,
            u64::try_from(session_frames).expect("non-negative frames"),
            "stream {idx}"
        );
        assert!(r.faults.is_empty(), "stream {idx} faults: {:?}", r.faults);

        let cap = capture_of(&rig.area, idx);
        assert!(
            matches!(cap.replay.stop, StopReason::CleanEnd | StopReason::TornTail),
            "stream {idx} journal stop: {:?}",
            cap.replay.stop
        );
        let spans = all_journal_spans(&cap);
        assert_session_coverage(&spans, &[(0, session_frames)]);
        assert_source_contiguous(&spans, session_frames);
        let out_ch = i64::try_from(map.len()).expect("channels");
        let checked = verify_capture_pcm(&cap, map.channels(), seed);
        assert_eq!(
            checked,
            usize::try_from(session_frames * out_ch).expect("sample count"),
            "stream {idx} pcm sample count"
        );
    }
}

// ---------- PCM fidelity（Mono / Stereo / MultiChannel / 2Track） ----------

#[test]
fn fake_mono_records_verified_pcm_through_real_writer() {
    let (mut rig, _device, _p) = normal_rig("mono", 1, 1, &[1], &[ChannelMap::mono(0)], SEED);
    rig.drive(5000, 500);
    assert!(!rig.session.fault().is_faulted());
    let report = rig.finish();
    assert_complete_normal(&rig, &report, &[ChannelMap::mono(0)], SEED, 5000);
}

#[test]
fn fake_stereo_records_verified_pcm_through_real_writer() {
    let map = ChannelMap::stereo(0, 1);
    let (mut rig, _device, _p) =
        normal_rig("stereo", 1, 2, &[2], std::slice::from_ref(&map), SEED);
    rig.drive(5000, 500);
    assert!(!rig.session.fault().is_faulted());
    let report = rig.finish();
    assert_complete_normal(&rig, &report, &[map], SEED, 5000);
}

#[test]
fn fake_multichannel_records_verified_pcm_through_real_writer() {
    let map = ChannelMap::identity(4);
    let (mut rig, _device, _p) =
        normal_rig("multichannel", 1, 4, &[4], std::slice::from_ref(&map), SEED);
    rig.drive(5000, 500);
    assert!(!rig.session.fault().is_faulted());
    let report = rig.finish();
    assert_complete_normal(&rig, &report, &[map], SEED, 5000);
}

#[test]
fn fake_two_track_one_stream_binds_channels_to_separate_streams() {
    // 1 stereo device を 2 mono tap が共有し、track ごとに別
    // capture stream / 別 staging media へ書く。
    let maps = [ChannelMap::mono(0), ChannelMap::mono(1)];
    let (mut rig, _device, _p) =
        normal_rig("two-track", 2, 2, &[1, 1], &maps, SEED);
    rig.drive(5000, 500);
    assert!(!rig.session.fault().is_faulted());
    let report = rig.finish();
    assert_complete_normal(&rig, &report, &maps, SEED, 5000);
}

#[test]
fn fake_two_devices_same_clock_record_to_separate_streams() {
    // same-clock multi-device: 同一 ClockDomain の 2 device を
    // `InputSession` で束ね、seed の異なる PCM が別 stream に記録される。
    let root = tmp_dir("two-devices");
    let (p, lanes, epoch) = project_with_lanes(2);
    let plan = plan_for(&p, &lanes, &[1, 1], epoch, 200);
    let dev_a = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK).with_seed(11);
    let dev_b = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK).with_seed(22);
    let spec_a = mono_spec(&plan, epoch, 16);
    let spec_b = InputStreamSpec {
        taps: vec![tap(
            &plan,
            1,
            ChannelMap::mono(0),
            ArmState::On,
            MonitorState::Off,
        )],
        ..mono_spec(&plan, epoch, 16)
    };
    let mut rig = DeviceRig::open(
        &root,
        session_id(2),
        plan,
        &[(&dev_a, &spec_a), (&dev_b, &spec_b)],
        None,
    );
    rig.drive(5000, 500);
    assert!(!rig.session.fault().is_faulted());
    let report = rig.finish();
    assert_eq!(report.refs.len(), 2);
    // stream 0 は dev_a(seed 11)、stream 1 は dev_b(seed 22) の PCM。
    for (idx, seed) in [(0_u16, 11_u32), (1_u16, 22_u32)] {
        let (_, result) = report
            .refs
            .iter()
            .find(|(s, _)| *s == idx)
            .expect("finalize ref");
        assert_eq!(
            result.as_ref().expect("finalize").outcome,
            FinalizedOutcome::Complete
        );
        let cap = capture_of(&rig.area, idx);
        let checked = verify_capture_pcm(&cap, &[0], seed);
        assert_eq!(checked, 5000, "stream {idx} sample count");
    }
}

// ---------- Punch / Loop 境界（Sample 単位） ----------

#[test]
fn fake_punch_boundaries_are_sample_exact() {
    // punch [10,30) tick = [250,750) frame。block 500 刻みでは
    // 境界が block 途中に落ちるため、span 分割の Sample 単位の
    // 正確さを検証する。
    let root = tmp_dir("punch");
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_gated(
        &p,
        &lanes,
        &[2],
        epoch,
        GateSpec::Punch {
            punch_in: MusicalTick::new(10),
            punch_out: MusicalTick::new(30),
        },
        200,
    );
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
    let mut rig = DeviceRig::open_one(&root, session_id(3), plan, &device, &spec);
    rig.drive(5000, 500);
    assert!(!rig.session.fault().is_faulted());
    let report = rig.finish();
    assert_eq!(report.refs.len(), 1);
    let r = report.refs[0].1.as_ref().expect("finalize");
    assert_eq!(r.outcome, FinalizedOutcome::Complete);
    assert_eq!(r.source_frames, 500);

    let cap = capture_of(&rig.area, 0);
    let spans = all_journal_spans(&cap);
    // session [250,500) + [500,750) が block 途中で正確に分割される。
    assert_session_coverage(&spans, &[(250, 750)]);
    assert_source_contiguous(&spans, 500);
    assert_eq!(
        span_obs(&spans),
        vec![
            (250, 500, 0, 0, 10, 20),
            (500, 750, 0, 250, 20, 30),
        ],
        "punch span session/source/project must be sample-exact"
    );
    // gate 外（[0,250)・[750,5000)）の PCM は media に存在しない。
    assert_eq!(r.media_byte_len, 4_000);
    verify_capture_pcm(&cap, &[0, 1], SEED);
}

#[test]
fn fake_punch_in_loop_pass_boundaries_are_sample_exact() {
    // loop [0,40) tick = 1000 frame、punch [10,30) = [250,750)。
    // stop 100 tick = 2500 frame → pass 0/1/2（最終 pass は
    // session_end で clamp）。
    let root = tmp_dir("punch-loop");
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_gated(
        &p,
        &lanes,
        &[2],
        epoch,
        GateSpec::PunchInLoop {
            loop_start: MusicalTick::ZERO,
            loop_end: MusicalTick::new(40),
            punch_in: MusicalTick::new(10),
            punch_out: MusicalTick::new(30),
        },
        100,
    );
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
    let mut rig = DeviceRig::open_one(&root, session_id(4), plan, &device, &spec);
    rig.drive(2500, 500);
    assert!(!rig.session.fault().is_faulted());
    let report = rig.finish();
    let r = report.refs[0].1.as_ref().expect("finalize");
    assert_eq!(r.outcome, FinalizedOutcome::Complete);

    let cap = capture_of(&rig.area, 0);
    let spans = all_journal_spans(&cap);
    // 各 pass の punch 区間のみ。pass 2 は session_end=2500 で clamp。
    assert_session_coverage(
        &spans,
        &[(250, 750), (1250, 1750), (2250, 2500)],
    );
    assert_source_contiguous(&spans, 1250);
    // pass_index が pass ごとに単調増加し、project 範囲は punch tick。
    let passes: Vec<u32> = spans.iter().map(|s| s.pass_index).collect();
    assert_eq!(passes, vec![0, 0, 1, 1, 2], "pass_index per span");
    // 各 span の session/source/project が Sample 単位で一意。
    assert_eq!(
        span_obs(&spans),
        vec![
            (250, 500, 0, 0, 10, 20),
            (500, 750, 0, 250, 20, 30),
            (1250, 1500, 1, 500, 10, 20),
            (1500, 1750, 1, 750, 20, 30),
            (2250, 2500, 2, 1000, 10, 20),
        ],
        "punch-in-loop span session/source/project must be sample-exact"
    );
    verify_capture_pcm(&cap, &[0, 1], SEED);
}

#[test]
fn fake_loop_gate_records_each_pass() {
    // loop [0,40) = 1000 frame を stop 2500 frame まで反復。
    // pass ごとに別 pass_index・連続 source で記録される。
    let root = tmp_dir("loop");
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_gated(
        &p,
        &lanes,
        &[2],
        epoch,
        GateSpec::Loop {
            loop_start: MusicalTick::ZERO,
            loop_end: MusicalTick::new(40),
        },
        100,
    );
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
    let mut rig = DeviceRig::open_one(&root, session_id(5), plan, &device, &spec);
    rig.drive(2500, 500);
    assert!(!rig.session.fault().is_faulted());
    let report = rig.finish();
    let r = report.refs[0].1.as_ref().expect("finalize");
    assert_eq!(r.source_frames, 2500);

    let cap = capture_of(&rig.area, 0);
    let spans = all_journal_spans(&cap);
    assert_session_coverage(&spans, &[(0, 1000), (1000, 2000), (2000, 2500)]);
    assert_source_contiguous(&spans, 2500);
    let passes: Vec<u32> = spans.iter().map(|s| s.pass_index).collect();
    // block 500 刻みで pass 境界をまたぐため、pass ごとの span 数は
    // gate template 次第。pass_index が単調非減少で 0→1→2 を踏むこと。
    assert_eq!(passes.first(), Some(&0));
    assert_eq!(passes.last(), Some(&2));
    assert!(
        passes.windows(2).all(|w| w[0] <= w[1]),
        "pass_index must be monotone: {passes:?}"
    );
    verify_capture_pcm(&cap, &[0, 1], SEED);
}

// ---------- Arm / Monitor 独立性・Writer 不作成 ----------

#[test]
fn fake_arm_off_monitor_on_creates_no_writer_and_no_media() {
    // Arm Off + Monitor On: capture path が構築されず、staging に
    // stream 自体が登録されない（writer 不作成条件）。
    let root = tmp_dir("arm-off-monitor-on");
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
    let mut rig = DeviceRig::open_one(&root, session_id(6), plan, &device, &spec);
    assert!(rig.writers.is_empty(), "Arm Off must not create writers");
    assert!(
        rig.area.manifest.streams.is_empty(),
        "unarmed session registers no capture streams"
    );
    // callback を駆動しても capture/fault は発生しない。
    rig.drive(2000, 500);
    assert!(!rig.session.fault().is_faulted());
    let report = rig.finish();
    assert!(report.refs.is_empty());
    // staging dir に media/journal は存在しない。
    assert!(!rig.area.media_path(0).exists());
    assert!(!rig.area.journal_path(0).exists());
}

#[test]
fn fake_monitor_on_does_not_alter_raw_pcm() {
    // 同一 device/seed で Monitor Off/On の 2 rig を録音し、
    // media bytes が一致すること（Monitor は raw capture に影響しない）。
    let build = |name: &str, monitor: MonitorState| {
        let root = tmp_dir(name);
        let (p, lanes, epoch) = project_with_lanes(1);
        let plan = plan_for(&p, &lanes, &[2], epoch, 200);
        let device =
            FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK).with_seed(SEED);
        let spec = InputStreamSpec {
            config: input_config(2),
            clock_epoch: epoch,
            max_block_frames: plan.max_block_frames,
            callback_queue: 16,
            taps: vec![tap(&plan, 0, ChannelMap::stereo(0, 1), ArmState::On, monitor)],
        };
        DeviceRig::open_one(&root, session_id(7), plan, &device, &spec)
    };
    let mut off = build("monitor-off", MonitorState::Off);
    let mut on = build("monitor-on", MonitorState::On);
    off.drive(5000, 500);
    on.drive(5000, 500);
    assert!(!off.session.fault().is_faulted());
    assert!(!on.session.fault().is_faulted());
    let media_off = std::fs::read(off.area.media_path(0)).expect("media off");
    let media_on = std::fs::read(on.area.media_path(0)).expect("media on");
    assert_eq!(media_off, media_on, "monitor state must not alter raw PCM");
    let _ = off.finish();
    let _ = on.finish();
}

// ---------- fail-stop（Verified Prefix + capture-fault marker） ----------

/// fail-stop 後の共通観測を検証する:
/// - capture-fault marker が durable に存在する
/// - journal replay の intact record が全て検証を通る（verified prefix。
///   fault が最初の chunk 確定前に起きた場合は空 prefix も許容する）
/// - span が session 0 からの連続 prefix で、media PCM が fault 前
///   chunk 分だけ Expected と一致する
/// - finalize ref が存在する
fn assert_fail_stop_prefix(
    rig: &DeviceRig,
    stream_index: u16,
    channel_map: &[u32],
    seed: u32,
) -> StreamCapture {
    assert!(
        rig.area.capture_fault_marker_exists(),
        "capture-fault marker must be recorded"
    );
    let detail = rig.area.capture_fault().expect("marker detail");
    assert!(detail.contains("fault bits"), "marker detail: {detail}");
    let cap = capture_of(&rig.area, stream_index);
    assert!(
        matches!(cap.replay.stop, StopReason::CleanEnd | StopReason::TornTail),
        "journal must remain intact: {:?}",
        cap.replay.stop
    );
    let spans = all_journal_spans(&cap);
    assert_prefix_contiguous(&spans);
    verify_capture_pcm(&cap, channel_map, seed);
    assert_source_contiguous(&spans, span_frame_total(&spans));
    cap
}

#[test]
fn fake_input_drop_fails_stop_and_preserves_verified_prefix() {
    let (mut rig, _device, p) = normal_rig("drop", 1, 1, &[1], &[ChannelMap::mono(0)], SEED);
    let sid = session_id(1);
    // 3 block 正常 → gap(+777) → fail-stop。
    for _ in 0..3 {
        rig.deliver_all(500);
        rig.pump_all();
    }
    assert!(!rig.session.fault().is_faulted());
    rig.streams[0].lock().expect("lock").jump_to(1500 + 777);
    rig.deliver_all(500);
    rig.pump_all();
    assert_eq!(
        rig.session.fault().bits() & FAULT_INPUT_DISCONTINUITY,
        FAULT_INPUT_DISCONTINUITY
    );
    // 以降の入力は fail-stop で捨てられる（連続を捏造しない）。
    rig.drive(2000, 500);
    let report = rig.finish();
    assert_eq!(report.refs.len(), 1);

    let cap = assert_fail_stop_prefix(&rig, 0, &[0], SEED);
    // fault 前の 1500 frame のみが verified prefix として残る。
    assert_eq!(span_frame_total(&all_journal_spans(&cap)), 1500);

    // commit 層の分類: capture-fault marker により stream は
    // recoverable（Partial/Unknown）に分離され、Complete media
    // としては commit されない。
    let (payload, recoverable) =
        build_payload(&p, &rig.area, sid, &mut || 0).expect("build_payload");
    assert_eq!(recoverable.len(), 1, "faulted stream must be recoverable");
    assert!(payload.streams.is_empty(), "no stream may commit");
    assert!(payload.passes.is_empty(), "no pass may commit");
    assert!(
        recoverable[0].reason.starts_with("capture-fault:"),
        "reason: {}",
        recoverable[0].reason
    );
}

#[test]
fn fake_clock_epoch_change_fails_stop() {
    let (mut rig, _device, _p) = normal_rig("epoch", 1, 1, &[1], &[ChannelMap::mono(0)], SEED);
    rig.drive(1500, 500);
    assert!(!rig.session.fault().is_faulted());
    // callback が別 clock domain を名乗る → epoch discontinuity。
    rig.streams[0]
        .lock()
        .expect("lock")
        .set_cb_domain(ClockDomainId::new(42));
    rig.deliver_all(500);
    rig.pump_all();
    assert_eq!(
        rig.session.fault().bits() & FAULT_EPOCH_DISCONTINUITY,
        FAULT_EPOCH_DISCONTINUITY
    );
    let _report = rig.finish();
    let cap = assert_fail_stop_prefix(&rig, 0, &[0], SEED);
    assert_eq!(span_frame_total(&all_journal_spans(&cap)), 1500);
}

#[test]
fn fake_sample_rate_change_fails_stop() {
    let (mut rig, device, _p) = normal_rig("rate", 1, 1, &[1], &[ChannelMap::mono(0)], SEED);
    rig.drive(1500, 500);
    assert!(!rig.session.fault().is_faulted());
    // device 側で sample rate が変わった → control path 照合で検出。
    let mut bad = input_format(1);
    bad.sample_rate = ruviform_audio_core::SampleRate::new(44_100).expect("rate");
    device.stream(0).lock().expect("lock").set_format(bad);
    rig.verify_formats();
    assert_eq!(
        rig.session.fault().bits() & FAULT_CONTRACT,
        FAULT_CONTRACT
    );
    let _report = rig.finish();
    let cap = assert_fail_stop_prefix(&rig, 0, &[0], SEED);
    assert_eq!(span_frame_total(&all_journal_spans(&cap)), 1500);
}

#[test]
fn fake_device_disconnect_fails_stop() {
    let (mut rig, _device, _p) = normal_rig("disconnect", 1, 1, &[1], &[ChannelMap::mono(0)], SEED);
    rig.drive(1500, 500);
    assert!(!rig.session.fault().is_faulted());
    // device 切断を error callback で通知 → fail-stop。
    rig.streams[0]
        .lock()
        .expect("lock")
        .raise(DeviceError::DeviceNotFound);
    rig.deliver_all(500);
    rig.pump_all();
    assert_eq!(
        rig.session.fault().bits() & FAULT_INPUT_DISCONTINUITY,
        FAULT_INPUT_DISCONTINUITY
    );
    let _report = rig.finish();
    let cap = assert_fail_stop_prefix(&rig, 0, &[0], SEED);
    assert_eq!(span_frame_total(&all_journal_spans(&cap)), 1500);
}

#[test]
fn fake_rt_queue_full_fails_stop() {
    // callback→pump の work queue 容量 1。pump 前に 2 block
    // deliver すると 2 個目で FAULT_QUEUE_FULL。
    let root = tmp_dir("rt-queue-full");
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[1], epoch, 200);
    let device = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK);
    let spec = InputStreamSpec {
        config: input_config(1),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 1,
        taps: vec![tap(
            &plan,
            0,
            ChannelMap::mono(0),
            ArmState::On,
            MonitorState::Off,
        )],
    };
    let mut rig = DeviceRig::open_one(&root, session_id(8), plan, &device, &spec);
    rig.deliver_all(500);
    rig.deliver_all(500); // queue 満杯 → fault（pump 前）
    rig.pump_all();
    assert_eq!(
        rig.session.fault().bits() & FAULT_QUEUE_FULL,
        FAULT_QUEUE_FULL
    );
    let _report = rig.finish();
    let cap = assert_fail_stop_prefix(&rig, 0, &[0], SEED);
    // fault は最初の chunk 確定前に起きたため verified prefix は
    // 空か queue に残った 1 block 分まで。いずれにせよ fault 後の
    // 入力は記録されない。
    assert!(
        span_frame_total(&all_journal_spans(&cap)) <= 500,
        "verified prefix must not exceed the queued block"
    );
}

#[test]
fn fake_filled_queue_full_fails_stop() {
    // engine→writer の filled queue 容量 1。writer を pump しない
    // 間に engine が chunk を生産し続けると FAULT_QUEUE_FULL。
    let root = tmp_dir("filled-queue-full");
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[1], epoch, 200);
    let device = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK);
    let mut tap_budget = budget();
    tap_budget.filled_capacity = 1;
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
            budget: tap_budget,
        }],
    };
    let mut rig = DeviceRig::open_one(&root, session_id(9), plan, &device, &spec);
    // writer.pump を呼ばず engine のみ回す → filled queue 溢れ。
    for _ in 0..3 {
        rig.deliver_all(500);
        rig.pump_engines();
    }
    assert_eq!(
        rig.session.fault().bits() & FAULT_QUEUE_FULL,
        FAULT_QUEUE_FULL
    );
    // 溜まった分を drain して finalize。
    rig.pump_all();
    let _report = rig.finish();
    let cap = assert_fail_stop_prefix(&rig, 0, &[0], SEED);
    assert!(span_frame_total(&all_journal_spans(&cap)) > 0);
}
