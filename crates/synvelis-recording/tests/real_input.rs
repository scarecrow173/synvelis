//! real backend composition root（`src/real_input.rs`）と opt-in
//! 実機 lane の検証。
//!
//! headless 部（format 交渉・config 組立・fake device 経由の session
//! 結線）は既定 lane で常時実行する。実機 test は
//! `real-device` feature + `#[ignore]` で opt-in され、既定
//! `cargo test` には含まれない（`mise run test-recording-real-input`）。
//! これらは実 Device Pass の受入ではなく、real backend への到達
//! 経路が契約を満たすことの検証である。

// 各 test binary が common の subset だけを使うため module 単位で許容
// （input_adapter.rs と同じ運用）。
#[allow(dead_code)]
mod common;

use common::*;
use ruviform_audio_core::device::{BufferSize, SupportedBufferSize, SupportedInputFormat};
use ruviform_audio_core::{SampleFormat, SampleRate};
use ruviform_media_core::ClockDomainId;
use synvelis_recording::real_input::{
    negotiate_input_format, real_input_config, select_f32_input_format,
};
use synvelis_recording::{
    ArmState, ChannelMap, FaultLatch, InputAdapter, InputStreamSpec, MonitorState,
};

fn supported(channels: u32, min: u32, max: u32, format: SampleFormat) -> SupportedInputFormat {
    SupportedInputFormat {
        channels,
        min_sample_rate: min,
        max_sample_rate: max,
        buffer_size: SupportedBufferSize::Unknown,
        sample_format: format,
    }
}

#[test]
fn negotiate_prefers_f32_default_format() {
    // fake の default_input_format は F32 → 交渉はそれをそのまま返す。
    let device = FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK);
    let format = negotiate_input_format(&device).expect("negotiate");
    assert_eq!(format.sample_format, SampleFormat::F32);
    assert_eq!(format.layout.channels(), 2);
}

#[test]
fn select_f32_prefers_stereo_then_48k() {
    let ranges = vec![
        supported(1, 44_100, 96_000, SampleFormat::F32),
        supported(2, 44_100, 48_000, SampleFormat::F32),
        supported(6, 44_100, 96_000, SampleFormat::F32),
    ];
    let format = select_f32_input_format(&ranges).expect("select");
    // stereo 優先（6ch より 2ch）。48 kHz は range 上限に clamp。
    assert_eq!(format.layout.channels(), 2);
    assert_eq!(format.sample_rate, SampleRate::new(48_000).expect("rate"));
    assert_eq!(format.sample_format, SampleFormat::F32);
}

#[test]
fn select_f32_rejects_unusable_ranges() {
    let ranges = vec![
        // 非 F32。
        supported(2, 44_100, 96_000, SampleFormat::S16),
        // 逆転 range。
        supported(2, 96_000, 44_100, SampleFormat::F32),
        // rate 0。
        supported(2, 0, 0, SampleFormat::F32),
        // 既知 layout に写像できない channel 数。
        supported(3, 44_100, 96_000, SampleFormat::F32),
    ];
    assert_eq!(select_f32_input_format(&ranges), None);
    assert_eq!(select_f32_input_format(&[]), None);
}

#[test]
fn real_input_config_uses_default_buffer_size() {
    // 実 cpal adapter は Fixed を UnsupportedConfig で拒否し得るため
    // Default 固定。
    let device = FakeInputDevice::new(1, ClockDomainId::SAMPLE_CLOCK);
    let format = negotiate_input_format(&device).expect("negotiate");
    let config = real_input_config(format);
    assert_eq!(config.buffer_size, BufferSize::Default);
    assert_eq!(config.format.sample_format, SampleFormat::F32);
}

#[test]
fn negotiated_config_drives_adapter_end_to_end() {
    // host→device→交渉→config→InputStreamSpec→InputAdapter の
    // composition root が fake device 経由で end-to-end に機能する
    // ことを確認する（実機不要の結線検証）。
    let device = FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK);
    let format = negotiate_input_format(&device).expect("negotiate");
    let config = real_input_config(format);
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[2], epoch, 200);
    let spec = InputStreamSpec {
        config,
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
    drive(&stream, &mut adapter, &mut writers, &plan, 5000, 500);
    assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());
    let log = &writers[0].1.log;
    assert_ne!(log.as_slice(), []);
    for entry in log {
        assert!(verify_tap_log(entry, &[0, 1], SEED));
    }
}

/// 実機 lane — `real-device` feature + `--ignored` でのみ実行。
/// device 不在・権限無し・format 不適合は環境的欠如として loud
/// SKIP（print + return）。device があるのに open/start/callback/
/// 録音経路が契約を満たさない場合は hard fail。
#[cfg(feature = "real-device")]
mod real_lane {
    use std::thread;
    use std::time::{Duration, Instant};

    use ruviform_audio_core::AudioFormat;
    use ruviform_audio_core::device::{
        Device, DeviceEnumerator, DeviceError, InputCallbackInfo, InputDataCallback,
        InputErrorCallback, InputStreamConfig, same_clock_domain,
    };
    use ruviform_audio_core::{AudioBufferView, ChannelLayout, SpeakerMask};
    use synvelis_project::{
        CaptureStream, CaptureStreamId, ClockEpochId, MusicalTick, RecordingSessionId,
    };
    use synvelis_recording::{
        BufferBudget, FAULT_EPOCH_DISCONTINUITY, FAULT_INPUT_DISCONTINUITY, FAULT_QUEUE_FULL,
        FinalizedOutcome, FinishReport, GateSpec, InputBindError, PreparedRecordingPlan,
        StopReason, build_payload,
    };

    use super::*;
    use synvelis_recording::real_input::default_host;

    // ---------- shared helpers ----------

    /// 実機 lane の buffer 上限。cpal `BufferSize::Default` の可変
    /// block は backend 依存（iD4 実績 ~441–960 frame/callback）で、
    /// 既定の 1024 では溢れ得る。`chunk_frames`/`max_block_frames`
    /// を 8192 に拡大し、超過は `FAULT_CONTRACT`/`FAULT_PCM_OVERFLOW`
    /// で正直に fail-stop する。
    const REAL_MAX_BLOCK: u32 = 8192;

    /// negotiated format 向けの `(BufferBudget, max_block_frames)`。
    /// chunk pool は既定 16 ではなく 64 — ~640 ms の pump stall
    /// 耐性を持たせ、負荷環境の scheduling stall が
    /// `FAULT_QUEUE_FULL`/`FREE_EXHAUSTED` を誤誘発しないように
    /// する（容量は plan パラメータであり fault 経路は armed の
    /// まま。queue-full 誘発 test は別途極小 queue を使う）。
    fn real_budget() -> (BufferBudget, u32) {
        let mut b = budget();
        b.chunk_frames = REAL_MAX_BLOCK;
        b.chunks = 64;
        b.filled_capacity = 64;
        (b, REAL_MAX_BLOCK)
    }

    /// 実機 lane の callback→pump in-flight 上限（work queue 容量
    /// かつ PCM slot 数）。callback は ~10 ms 周期なので 256 で
    /// ~2.5 s の pump stall 耐性を持つ — 並行 build 等で test
    /// process が長時間 deschedule されても `FAULT_QUEUE_FULL` を
    /// 誤誘発しないための jitter 余裕（容量は spec パラメータで
    /// あり fault 経路は armed のまま。queue-full 誘発 test は
    /// 別途極小 queue=8 を使う）。
    const REAL_CALLBACK_QUEUE: usize = 256;

    /// continuity 系 fault bit の集合 — device/driver の frame
    /// position 不連続を示す。iD4/WASAPI 実機では raw stream
    /// （adapter/pump 無し）でも再現する実挙動であり、harness
    /// 側の欠陥ではない。fail-stop が verified prefix を守った
    /// ことを確認した上で NotRun/blocked として記録する対象。
    const CONTINUITY_FAULTS: u32 = FAULT_EPOCH_DISCONTINUITY | FAULT_INPUT_DISCONTINUITY;

    /// default input device を取得する。環境的欠如は loud SKIP
    /// （print + `None`）。device があれば `Some`。
    fn default_input() -> Option<Box<dyn Device>> {
        let host = match default_host() {
            Ok(host) => host,
            Err(error) => {
                println!("  SKIP: default_host() failed: {error}");
                return None;
            }
        };
        println!("  host: {}", host.name());
        match host.default_input_device() {
            Ok(Some(device)) => {
                let name = device.name().unwrap_or_else(|_| "<unnamed>".to_owned());
                println!("  device: \"{name}\"");
                Some(device)
            }
            Ok(None) => {
                println!("  SKIP: no default input device on this host");
                None
            }
            Err(error) => {
                println!("  SKIP: default_input_device() failed: {error}");
                None
            }
        }
    }

    /// negotiated F32 format を取得する。取れなければ loud SKIP。
    fn negotiated(device: &dyn Device) -> Option<AudioFormat> {
        if let Some(format) = negotiate_input_format(device) {
            println!("  negotiated format: {format:?}");
            Some(format)
        } else {
            println!("  SKIP: no F32-openable input format on this device");
            None
        }
    }

    /// 録音系 test の共通前段: default input device → negotiated
    /// format → clock-domain probe。いずれかが環境的欠如なら loud
    /// SKIP 済み print + `None`。戻り値は `(device, format)`。
    fn usable_input() -> Option<(Box<dyn Device>, AudioFormat)> {
        let device = default_input()?;
        let format = negotiated(device.as_ref())?;
        probe_clock_domain(device.as_ref(), format)?;
        Some((device, format))
    }

    /// rig の録音駆動〜finalize の共通後段。`run` は `run_for` の
    /// wall 時間、`interval` は pump 間隔。fault があれば panic
    /// （実機 lane の fail 対象）。
    fn drive_and_finish(
        rig: &mut RealDeviceRig,
        run: Duration,
        interval: Duration,
    ) -> FinishReport {
        let (report, bits) = drive_observe(rig, run, interval);
        assert_eq!(bits, 0, "real capture path faulted: bits {bits:#06x}");
        report
    }

    /// `drive_and_finish` の観測版 — fault を assert せず latch の
    /// bit 集合を返す。長時間 stability lane が continuity 系
    /// fault（device/driver の position 不連続）を「検証済み
    /// prefix を伴う観測結果」として分類するために使う。
    ///
    /// pump は専用スレッドで回す — 本番構成の非 RT pump thread を
    /// 模し、test 駆動側の orchestration/断言と pump を timeshare
    /// させない。test thread 上で pump すると OS scheduling の
    /// stall が work queue / chunk pool を実機 fault として
    /// 誤誘発し得る（実機 lane の flake 源）。
    fn drive_observe(
        rig: &mut RealDeviceRig,
        run: Duration,
        interval: Duration,
    ) -> (FinishReport, u32) {
        std::thread::scope(|s| {
            s.spawn(|| {
                rig.start_all()
                    .expect("start on a present device must succeed");
                rig.run_for(run, interval);
                if let Err(error) = rig.stop_all() {
                    println!("  stop() failed: {error}");
                }
                rig.pump_all();
                let bits = rig.session.fault().bits();
                (rig.finish(), bits)
            })
            .join()
            .expect("drive thread panicked")
        })
    }

    /// fault latch の bit 集合を分類する。continuity 系
    /// （device/driver の position 不連続）のみなら verified
    /// prefix を検証して prefix frame 数を返す — caller は
    /// NotRun/blocked として記録して return する。continuity
    /// 以外の bit があれば harness/系の欠陥として hard fail。
    /// prefix が空（fault が capture 開始前）の場合も panic —
    /// 観測として成立しない。
    fn continuity_fault_prefix(rig: &RealDeviceRig, out_channels: u32, fault_bits: u32) -> i64 {
        assert_eq!(
            fault_bits & !CONTINUITY_FAULTS,
            0,
            "non-continuity fault bits {fault_bits:#06x}"
        );
        let cap = capture_of(&rig.area, 0);
        assert!(
            matches!(cap.replay.stop, StopReason::CleanEnd | StopReason::TornTail),
            "journal stop: {:?}",
            cap.replay.stop
        );
        let spans = all_journal_spans(&cap);
        assert_prefix_contiguous(&spans);
        let prefix_frames = span_frame_total(&spans);
        assert!(prefix_frames > 0, "verified prefix must be non-empty");
        assert_source_contiguous(&spans, prefix_frames);
        verify_capture_integrity(&cap, out_channels);
        prefix_frames
    }

    /// adapter 層の決定的 reject を検証する。`validate_spec` が
    /// device open 前に fail-closed で reject することを確認する
    /// （deterministic・hardware 非依存）。
    fn assert_adapter_rejects(
        device: &dyn Device,
        plan: &PreparedRecordingPlan,
        epoch: ClockEpochId,
        format: AudioFormat,
    ) {
        let fault = FaultLatch::new();
        let spec = |callback_queue| {
            real_spec(plan, epoch, format, callback_queue, &[(0, map_for(format))])
        };

        let mut non_f32 = spec(8);
        non_f32.config.format.sample_format = SampleFormat::S16;
        assert!(matches!(
            InputAdapter::open(device, &non_f32, fault.clone()),
            Err(InputBindError::UnsupportedSampleFormat)
        ));

        let mut out_of_range = spec(8);
        out_of_range.taps[0].binding.channel_map = ChannelMap::mono(u32::MAX);
        assert!(matches!(
            InputAdapter::open(device, &out_of_range, fault.clone()),
            Err(InputBindError::ChannelOutOfRange)
        ));

        let zero_queue = spec(0);
        assert!(matches!(
            InputAdapter::open(device, &zero_queue, fault.clone()),
            Err(InputBindError::ZeroCapacity)
        ));

        let mut empty_taps = spec(8);
        empty_taps.taps.clear();
        assert!(matches!(
            InputAdapter::open(device, &empty_taps, fault.clone()),
            Err(InputBindError::EmptyTaps)
        ));
        println!(
            "  adapter-layer rejects: UnsupportedSampleFormat / \
                  ChannelOutOfRange / ZeroCapacity / EmptyTaps — all fail-closed"
        );
    }

    /// 全 input device を走査し、negotiated format + clock domain
    /// が観測できた device を `(device, format, domain)` で返す。
    /// probe は open→観測→drop で start はしない。
    fn probe_candidates(
        devices: Vec<Box<dyn Device>>,
    ) -> Vec<(Box<dyn Device>, AudioFormat, ClockDomainId)> {
        let mut candidates = Vec::new();
        for device in devices {
            let name = device.name().unwrap_or_else(|_| "<unnamed>".to_owned());
            println!("  device: \"{name}\"");
            let Some(format) = negotiated(device.as_ref()) else {
                continue;
            };
            let Some(domain) = probe_clock_domain(device.as_ref(), format) else {
                continue;
            };
            candidates.push((device, format, domain));
        }
        candidates
    }

    /// multi-device 候補が same-clock 条件（>=2 台・clock domain
    /// 一致・非 UNKNOWN・同一 negotiated rate）を満たすか検査し、
    /// 共通 sample rate を返す。満たさなければ loud `NotRun` print +
    /// `None`（cross-clock multi-device は issue #119 の範囲）。
    fn same_clock_rate(
        candidates: &[(Box<dyn Device>, AudioFormat, ClockDomainId)],
    ) -> Option<u32> {
        if candidates.len() < 2 {
            println!(
                "  SKIP/NotRun: need >=2 usable input devices, found {}",
                candidates.len()
            );
            return None;
        }
        let base_domain = candidates[0].2;
        if !candidates[1..]
            .iter()
            .all(|(_, _, d)| same_clock_domain(*d, base_domain))
        {
            println!(
                "  SKIP/NotRun: clock domains differ — cross-clock multi-device \
                 is issue #119 scope, not this lane"
            );
            return None;
        }
        let rate = candidates[0].1.sample_rate.get();
        if candidates
            .iter()
            .any(|(_, f, _)| f.sample_rate.get() != rate)
        {
            println!(
                "  SKIP/NotRun: negotiated sample rates differ \
                 ({} vs {}) — single staging rate cannot represent both",
                rate,
                candidates[1].1.sample_rate.get()
            );
            return None;
        }
        Some(rate)
    }

    /// device 層 probe: negotiated format を基底に、非 F32・
    /// `Fixed(0)` buffer・非対応 channel layout・range 外 sample
    /// rate の open を試み、backend の実応答（reject/受理）を
    /// 観測・記録する。受理は reject の証拠にならないため
    /// Unsupported として記録し、Pass にはしない。
    fn probe_device_rejects(device: &dyn Device, format: AudioFormat) {
        let mut probes: Vec<(String, InputStreamConfig)> = vec![
            (
                "non-F32 (S16)".to_owned(),
                InputStreamConfig {
                    format: AudioFormat {
                        sample_format: SampleFormat::S16,
                        ..format
                    },
                    buffer_size: BufferSize::Default,
                },
            ),
            (
                "Fixed(0) buffer".to_owned(),
                InputStreamConfig {
                    format,
                    buffer_size: BufferSize::Fixed(0),
                },
            ),
        ];
        match ChannelLayout::custom(SpeakerMask::from_bits(0b111)) {
            Ok(layout) => probes.push((
                "unsupported layout (3ch)".to_owned(),
                InputStreamConfig {
                    format: AudioFormat { layout, ..format },
                    buffer_size: BufferSize::Default,
                },
            )),
            Err(_) => println!("  probe unsupported-layout: NotRun (mask 構成不能)"),
        }
        // range 外 rate: supported 全 range の上限 +1。構成不能なら
        // その probe は NotRun。
        match device.supported_input_formats() {
            Ok(ranges) => {
                let max_rate = ranges.iter().map(|r| r.max_sample_rate).max();
                match max_rate.and_then(|m| m.checked_add(1)).and_then(|r| {
                    SampleRate::new(r).ok().map(|sr| AudioFormat {
                        sample_rate: sr,
                        ..format
                    })
                }) {
                    Some(bad_format) => probes.push((
                        format!("out-of-range rate ({})", bad_format.sample_rate.get()),
                        InputStreamConfig {
                            format: bad_format,
                            buffer_size: BufferSize::Default,
                        },
                    )),
                    None => println!("  probe out-of-range rate: NotRun (rate 構成不能)"),
                }
            }
            Err(error) => {
                println!("  probe out-of-range rate: NotRun (supported_input_formats: {error});");
            }
        }

        let mut rejected = 0usize;
        let mut accepted = 0usize;
        for (name, config) in &probes {
            let data: InputDataCallback =
                Box::new(|_: AudioBufferView<'_, f32>, _: InputCallbackInfo| {});
            let err: InputErrorCallback = Box::new(|e: DeviceError| {
                println!("    probe stream error callback: {e}");
            });
            match device.open_input_stream(config, data, err) {
                Ok(stream) => {
                    accepted += 1;
                    println!("  probe {name}: ACCEPTED — backend converts/ignores (Unsupported)");
                    drop(stream);
                }
                Err(error) => {
                    rejected += 1;
                    println!("  probe {name}: rejected ({error})");
                }
            }
        }
        println!("  device-layer probes: rejected={rejected} accepted={accepted}");
        if accepted == probes.len() {
            println!("  Unsupported: backend accepted every invalid config probed");
        }
    }

    /// negotiated format で probe stream を開き `clock_domain()` を
    /// 観測する。`Some(domain)`: open 成功（UNKNOWN 以外）。
    /// `None`: 環境的 SKIP/Unsupported（print 済み）。present device
    /// が negotiated format を open できない契約違反は panic —
    /// 実機 lane の fail 対象であり、静かに通さない。
    fn probe_clock_domain(device: &dyn Device, format: AudioFormat) -> Option<ClockDomainId> {
        let config = real_input_config(format);
        let data: InputDataCallback =
            Box::new(|_: AudioBufferView<'_, f32>, _: InputCallbackInfo| {});
        let err: InputErrorCallback = Box::new(|e: DeviceError| {
            println!("  probe stream error callback: {e}");
        });
        match device.open_input_stream(&config, data, err) {
            Ok(stream) => {
                let domain = stream.clock_domain();
                let format_ok = stream.format() == config.format;
                drop(stream);
                if domain == ClockDomainId::UNKNOWN {
                    println!(
                        "  SKIP/Unsupported: stream clock domain is UNKNOWN — \
                         continuity is unverifiable on this backend"
                    );
                    return None;
                }
                if !format_ok {
                    println!("  note: opened stream format differs from negotiated format");
                }
                Some(domain)
            }
            Err(error) => match error {
                DeviceError::DeviceBusy
                | DeviceError::DeviceNotFound
                | DeviceError::Unsupported => {
                    println!("  SKIP: probe open_input_stream failed: {error}");
                    None
                }
                _ => panic!("present device rejected negotiated format: {error}"),
            },
        }
    }

    /// negotiated format の channel 数に対応する `ChannelMap`
    /// （1ch→mono(0)、2ch→stereo、それ以外→identity）。
    fn map_for(format: AudioFormat) -> ChannelMap {
        match format.layout.channels() {
            1 => ChannelMap::mono(0),
            2 => ChannelMap::stereo(0, 1),
            n => ChannelMap::identity(n),
        }
    }

    /// negotiated format + plan から armed tap の `InputStreamSpec`
    /// を組み立てる。`taps` は `(stream_index, ChannelMap)` 列。
    fn real_spec(
        plan: &PreparedRecordingPlan,
        epoch: ClockEpochId,
        format: AudioFormat,
        callback_queue: usize,
        taps: &[(u16, ChannelMap)],
    ) -> InputStreamSpec {
        InputStreamSpec {
            config: real_input_config(format),
            clock_epoch: epoch,
            max_block_frames: plan.max_block_frames,
            callback_queue,
            taps: taps
                .iter()
                .map(|&(index, ref map)| {
                    tap(plan, index, map.clone(), ArmState::On, MonitorState::Off)
                })
                .collect(),
        }
    }

    fn session_id(raw: u128) -> RecordingSessionId {
        RecordingSessionId::new(raw)
    }

    /// session の wall-clock 録音時間（秒）+ 余裕。engine は
    /// `session_end` で消費を止めるため、run 時間は session 時間を
    /// 上回る必要がある（callback block が work queue に残っていても
    /// stop 後の `pump_all` で drain される）。
    fn run_duration(session_frames: i64, rate: u32, margin: f64) -> Duration {
        let frames = u32::try_from(session_frames).expect("session frames fit u32");
        let secs = f64::from(frames) / f64::from(rate) + margin;
        Duration::from_secs_f64(secs)
    }

    /// Complete 正常録音の共通検証（実機版）。journal replay・
    /// session/source coverage・media CRC・有限 PCM を構造検証し、
    /// finalize ref の Complete を確認する。PCM の期待値比較は
    /// 行わない — 実機 PCM に deterministic 期待値はない
    /// （`verify_capture_integrity` が構造 Pass の権威）。
    /// `expected` は `(stream_index, 出力 channel 数)` 列。
    fn assert_complete_real(
        rig: &RealDeviceRig,
        report: &FinishReport,
        expected: &[(u16, u32)],
        session_frames: i64,
    ) {
        assert_eq!(report.refs.len(), expected.len(), "finalize ref count");
        for &(index, channels) in expected {
            let (_, result) = report
                .refs
                .iter()
                .find(|(s, _)| *s == index)
                .unwrap_or_else(|| panic!("finalize ref for stream {index}"));
            let r = result.as_ref().expect("finalize must succeed");
            assert_eq!(r.outcome, FinalizedOutcome::Complete, "stream {index}");
            assert_eq!(
                r.source_frames,
                u64::try_from(session_frames).expect("non-negative frames"),
                "stream {index}"
            );
            assert!(r.faults.is_empty(), "stream {index} faults: {:?}", r.faults);
            assert_eq!(
                r.media_byte_len,
                r.source_frames * u64::from(channels) * 4,
                "stream {index} media_byte_len"
            );

            let cap = capture_of(&rig.area, index);
            assert!(
                matches!(cap.replay.stop, StopReason::CleanEnd | StopReason::TornTail),
                "stream {index} journal stop: {:?}",
                cap.replay.stop
            );
            let spans = all_journal_spans(&cap);
            assert_session_coverage(&spans, &[(0, session_frames)]);
            assert_source_contiguous(&spans, session_frames);
            let samples = verify_capture_integrity(&cap, channels);
            assert_eq!(
                samples,
                usize::try_from(session_frames * i64::from(channels)).expect("sample count"),
                "stream {index} pcm sample count"
            );
        }
    }

    #[test]
    #[ignore = "real-device lane: requires audio hardware + capture permission (run `mise run test-recording-real-input`)"]
    fn real_input_device_records_through_input_adapter() {
        println!("real_input lane: enumerate → negotiate → InputAdapter capture");

        let host = match default_host() {
            Ok(host) => host,
            Err(error) => {
                println!("  SKIP: default_host() failed: {error}");
                return;
            }
        };
        println!("  host: {}", host.name());

        let device = match host.default_input_device() {
            Ok(Some(device)) => device,
            Ok(None) => {
                println!("  SKIP: no default input device on this host");
                return;
            }
            Err(error) => {
                println!("  SKIP: default_input_device() failed: {error}");
                return;
            }
        };
        let name = device
            .name()
            .unwrap_or_else(|_| "<name query failed>".to_owned());
        println!("  device: \"{name}\"");

        let Some(format) = negotiate_input_format(device.as_ref()) else {
            println!("  SKIP: no F32-openable input format on this device");
            return;
        };
        println!("  negotiated format: {format:?}");

        let channels = format.layout.channels();
        let map = match channels {
            1 => ChannelMap::mono(0),
            2 => ChannelMap::stereo(0, 1),
            other => {
                println!("  SKIP: channel count {other} has no ChannelMap binding");
                return;
            }
        };

        let (p, lanes, epoch) = project_with_lanes(1);
        let plan = plan_for(&p, &lanes, &[channels], epoch, 200);
        let spec = InputStreamSpec {
            config: real_input_config(format),
            clock_epoch: epoch,
            // cpal の Default buffer は backend 依存で可変 — 十分な
            // 上限を与え、超過は FAULT_CONTRACT で検出する。
            max_block_frames: 8192,
            callback_queue: 256,
            taps: vec![tap(&plan, 0, map, ArmState::On, MonitorState::Off)],
        };

        let fault = FaultLatch::new();
        let opened = InputAdapter::open(device.as_ref(), &spec, fault.clone())
            .expect("open_input_stream on a present device must succeed");
        let (mut adapter, mut writers) = into_writers(opened, &fault);

        adapter
            .stream()
            .start()
            .expect("start() on a present device must succeed");
        // capture 中に定期的に pump して work queue を drain する。
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(50));
            adapter.pump(&plan.scheduler);
            for (_, w) in &mut writers {
                w.pump();
            }
        }
        if let Err(error) = adapter.stream().stop() {
            println!("  stop() failed: {error}");
        }
        adapter.pump(&plan.scheduler);
        for (_, w) in &mut writers {
            w.pump();
        }

        assert!(
            !fault.is_faulted(),
            "real input path faulted: bits {:x}",
            fault.bits()
        );
        let produced = writers
            .iter()
            .flat_map(|(_, w)| w.log.iter())
            .any(|log| !log.spans.is_empty());
        assert!(
            produced,
            "real input path produced no capture chunks — callback never delivered"
        );
        println!("  capture chunks produced: OK");
    }

    /// 診断 lane — backend が callback 毎に実際に報告する
    /// `frame_position`/`capture`/`clock_domain` を観測する。
    /// placement fallback の実機挙動を記録するための probe であり、
    /// 成否判定は行わない（観測値を print するのみ）。
    #[test]
    #[ignore = "real-device lane: requires audio hardware (run `mise run test-recording-real-input`)"]
    fn real_input_probe_reports_timestamp_fields() {
        use std::sync::Mutex;

        use ruviform_audio_core::AudioBufferView;
        use ruviform_audio_core::device::{
            DeviceError, InputCallbackInfo, InputDataCallback, InputErrorCallback,
        };

        println!("real_input lane: timestamp field probe");

        let host = match default_host() {
            Ok(host) => host,
            Err(error) => {
                println!("  SKIP: default_host() failed: {error}");
                return;
            }
        };
        let Ok(Some(device)) = host.default_input_device() else {
            println!("  SKIP: no default input device");
            return;
        };
        let Some(format) = negotiate_input_format(device.as_ref()) else {
            println!("  SKIP: no F32-openable input format");
            return;
        };
        println!("  format: {format:?}");

        let observed = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
        let observed_cb = std::sync::Arc::clone(&observed);
        let data: InputDataCallback = Box::new(
            move |view: AudioBufferView<'_, f32>, info: InputCallbackInfo| {
                let stamp = info.capture_stamp();
                let mut v = observed_cb.lock().expect("lock");
                if v.len() < 8 {
                    v.push(format!(
                        "  cb: frames={} pos={:?} cb_t={:?} cap={:?} stamp={:?} domain={:?}",
                        view.frames(),
                        info.frame_position,
                        info.timestamp.callback.value(),
                        info.timestamp.capture.map(|t| t.time.value()),
                        stamp.map(|s| s.ticks),
                        info.clock_domain,
                    ));
                }
            },
        );
        let err: InputErrorCallback = Box::new(|e: DeviceError| {
            println!("  stream error callback: {e}");
        });
        let stream = match device.open_input_stream(&real_input_config(format), data, err) {
            Ok(stream) => stream,
            Err(error) => {
                println!("  SKIP: open_input_stream failed: {error}");
                return;
            }
        };
        println!(
            "  stream.now(): {:?}",
            stream.now().map(ruviform_media_core::MediaTime::value)
        );
        println!("  stream.clock_domain(): {:?}", stream.clock_domain());
        if let Err(error) = stream.start() {
            println!("  SKIP: start() failed: {error}");
            return;
        }
        thread::sleep(Duration::from_millis(300));
        let _ = stream.stop();
        for line in observed.lock().expect("lock").iter() {
            println!("{line}");
        }
    }

    /// AC1 2Track 行: 1 device の 2 入力を `ChannelMap::mono(0)`/
    /// `mono(1)` の 2 armed tap に binding し、別 staging stream /
    /// 別 media / 別 journal へ記録する。入力 channel が 2 未満の
    /// device では loud SKIP（NotRun）。
    #[test]
    #[ignore = "real-device lane: requires >=2ch audio hardware (run `mise run test-recording-real-input`)"]
    fn real_two_track_capture_records_both_inputs() {
        println!("real_input lane: 2-track capture via mono(0)+mono(1)");
        let Some((device, format)) = usable_input() else {
            return;
        };
        let channels = format.layout.channels();
        if channels < 2 {
            println!(
                "  SKIP/NotRun: device has {channels} input channel(s); \
                 2-track needs >= 2"
            );
            return;
        }

        let rate = format.sample_rate.get();
        let (p, lanes, epoch) = project_with_lanes(2);
        let (b, mbf) = real_budget();
        // stop 2000 tick ≈ 1.04 s（120 BPM/PPQN960 で 1 tick = 1/1920 s）。
        let plan = plan_at(
            &p,
            &lanes,
            &[1, 1],
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            2000,
            rate,
            b,
            mbf,
        );
        let session_frames = plan.session_end.value();
        let spec = real_spec(
            &plan,
            epoch,
            format,
            REAL_CALLBACK_QUEUE,
            &[(0, ChannelMap::mono(0)), (1, ChannelMap::mono(1))],
        );
        let root = tmp_dir("real-two-track");
        let mut rig =
            RealDeviceRig::open_one(&root, session_id(10), plan, rate, device.as_ref(), &spec);
        let report = drive_and_finish(
            &mut rig,
            run_duration(session_frames, rate, 0.5),
            Duration::from_millis(2),
        );
        assert_complete_real(&rig, &report, &[(0, 1), (1, 1)], session_frames);
        println!(
            "  2-track capture: streams 0/1 Complete, {session_frames} frames each @ {rate} Hz"
        );
    }

    /// 実 device → `InputSession`/`InputAdapter` → 実 `StreamWriter` /
    /// staging → checkpoint/finalize の end-to-end。negotiated
    /// channel 数の map で 1 stream を記録し、commit 層 payload が
    /// recoverable なしで構成できることを確認する。
    #[test]
    #[ignore = "real-device lane: requires audio hardware (run `mise run test-recording-real-input`)"]
    fn real_device_records_end_to_end_through_stream_writer() {
        println!("real_input lane: device → adapter → StreamWriter/staging e2e");
        let Some((device, format)) = usable_input() else {
            return;
        };

        let map = map_for(format);
        let out_channels = u32::try_from(map.len()).expect("channels");
        let rate = format.sample_rate.get();
        let (mut p, lanes, epoch) = project_with_lanes(1);
        let (b, mbf) = real_budget();
        let plan = plan_at(
            &p,
            &lanes,
            &[out_channels],
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            2000,
            rate,
            b,
            mbf,
        );
        let session_frames = plan.session_end.value();
        let spec = real_spec(&plan, epoch, format, REAL_CALLBACK_QUEUE, &[(0, map)]);
        let sid = session_id(11);
        // commit 層は staging の capture_stream_id が project の
        // recording.streams に登録済みであることを要求する（本番
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
        let root = tmp_dir("real-e2e-writer");
        let mut rig = RealDeviceRig::open(
            &root,
            sid,
            plan,
            rate,
            &[(device.as_ref(), &spec)],
            Some(&[stream_id]),
        );
        let report = drive_and_finish(
            &mut rig,
            run_duration(session_frames, rate, 0.5),
            Duration::from_millis(2),
        );
        assert_complete_real(&rig, &report, &[(0, out_channels)], session_frames);

        // commit 層: fault なし session は recoverable 分離されず、
        // Complete stream が payload に乗る。
        let (payload, recoverable) =
            build_payload(&p, &rig.area, sid, &mut || 0).expect("build_payload");
        assert!(
            recoverable.is_empty(),
            "clean session must not produce recoverable streams: {recoverable:?}"
        );
        assert_eq!(payload.streams.len(), 1, "one committed stream");
        println!(
            "  e2e: media/journal/checkpoint/finalize verified, \
             {session_frames} frames, {} chunk(s)",
            capture_of(&rig.area, 0).chunks.len()
        );
    }

    /// AC2 実機行: 極小 `callback_queue` + pump 停止で RT queue-full
    /// を決定的に誘発し、verified prefix 保持・capture-fault
    /// marker・recoverable 分離を実機で確認する。fault を誘発
    /// できなければ fail（NotRun にはしない）。
    #[test]
    #[ignore = "real-device lane: requires audio hardware (run `mise run test-recording-real-input`)"]
    fn real_rt_queue_full_fails_stop_and_preserves_prefix() {
        println!("real_input lane: RT queue-full fail-stop");
        let Some((device, format)) = usable_input() else {
            return;
        };

        let map = map_for(format);
        let out_channels = u32::try_from(map.len()).expect("channels");
        let rate = format.sample_rate.get();
        let (p, lanes, epoch) = project_with_lanes(1);
        let (b, mbf) = real_budget();
        let plan = plan_at(
            &p,
            &lanes,
            &[out_channels],
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            2000,
            rate,
            b,
            mbf,
        );
        // callback→pump queue と PCM pool を 8 に絞る。pump を止めると
        // ~callback 数回分（数十〜数百 ms）で必ず溢れる。
        let spec = real_spec(&plan, epoch, format, 8, &[(0, map)]);
        let sid = session_id(12);
        let root = tmp_dir("real-queue-full");
        let mut rig = RealDeviceRig::open_one(&root, sid, plan, rate, device.as_ref(), &spec);
        rig.start_all()
            .expect("start on a present device must succeed");

        // Phase 1: pump しながら ~0.8 s 録音し verified prefix を作る
        // （session ~1.04 s の途中で止め、prefix が session 全体に
        // ならないようにする）。
        rig.run_for(Duration::from_millis(800), Duration::from_millis(2));
        assert!(
            !rig.session.fault().is_faulted(),
            "warm phase must not fault: bits {:x}",
            rig.session.fault().bits()
        );

        // Phase 2: pump を完全に止め、callback が queue を溢れさせる
        // のを待つ。15 s で誘発できなければ induction 失敗として
        // fail する（pass にはしない）。
        let deadline = Instant::now() + Duration::from_secs(15);
        while !rig.session.fault().is_faulted() {
            assert!(
                Instant::now() < deadline,
                "FAULT_QUEUE_FULL did not fire within 15 s of stopped pump"
            );
            thread::sleep(Duration::from_millis(5));
        }
        let bits = rig.session.fault().bits();
        assert_eq!(
            bits & FAULT_QUEUE_FULL,
            FAULT_QUEUE_FULL,
            "expected FAULT_QUEUE_FULL, got bits {bits:#06x}"
        );
        println!("  FAULT_QUEUE_FULL observed: bits {bits:#06x}");

        // Phase 3: 溜まった work を drain（engine は faulted で
        // no-op）→ journal は fault 時点で凍結する。
        rig.pump_all();
        let frozen = span_frame_total(&all_journal_spans(&capture_of(&rig.area, 0)));
        assert!(frozen > 0, "warm prefix must be non-empty");
        thread::sleep(Duration::from_millis(300));
        rig.pump_all();
        let after = span_frame_total(&all_journal_spans(&capture_of(&rig.area, 0)));
        assert_eq!(
            after, frozen,
            "post-fault input must not be recorded (journal grew {frozen} -> {after})"
        );

        if let Err(error) = rig.stop_all() {
            println!("  stop() failed: {error}");
        }
        rig.pump_all();
        let report = rig.finish();
        assert_eq!(report.refs.len(), 1, "finalize ref count");

        // verified prefix: marker durable・journal intact・session 0
        // からの連続 prefix・source 連続・構造検証。
        assert!(
            rig.area.capture_fault_marker_exists(),
            "capture-fault marker must be recorded"
        );
        let detail = rig.area.capture_fault().expect("marker detail");
        assert!(detail.contains("fault bits"), "marker detail: {detail}");
        let cap = capture_of(&rig.area, 0);
        assert!(
            matches!(cap.replay.stop, StopReason::CleanEnd | StopReason::TornTail),
            "journal must remain intact: {:?}",
            cap.replay.stop
        );
        let spans = all_journal_spans(&cap);
        assert_prefix_contiguous(&spans);
        assert_source_contiguous(&spans, frozen);
        verify_capture_integrity(&cap, out_channels);

        // commit 層: capture-fault marker により stream は
        // recoverable に分離され、Complete media として commit
        // されない。
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
        println!(
            "  fail-stop verified: {frozen} frame prefix preserved, \
             marker durable, recoverable classified"
        );
    }

    /// AC2: format/rate 不適合 open の正直な reject。
    /// adapter 層の決定的 reject（`validate_spec` — device open 前）
    /// を assert し、device 層は backend の実応答を probe として
    /// 記録する。backend が不正 config を受理した場合は
    /// Unsupported/NotRun として print し、Pass にはしない。
    #[test]
    #[ignore = "real-device lane: requires audio hardware (run `mise run test-recording-real-input`)"]
    fn real_format_rate_reject_is_fail_closed() {
        println!("real_input lane: format/rate reject probes");
        let Some((device, format)) = usable_input() else {
            return;
        };

        let rate = format.sample_rate.get();
        let (p, lanes, epoch) = project_with_lanes(1);
        let (b, mbf) = real_budget();
        let plan = plan_at(
            &p,
            &lanes,
            &[format.layout.channels()],
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            200,
            rate,
            b,
            mbf,
        );
        assert_adapter_rejects(device.as_ref(), &plan, epoch, format);
        probe_device_rejects(device.as_ref(), format);
    }

    /// AC3 実機昇格: punch 境界を negotiated rate の Sample 単位で
    /// 検証する。期待 session 区間は `plan.mapping.tick_to_media`
    /// （Gate scheduler と同一の権威）から計算し、journal span の
    /// session/source/project 座標が半開 `[punch_in, punch_out)` に
    /// 一致すること、media が punched 区間のみを含むことを確認する。
    /// continuity 系 fault（device/driver の position 不連続）は
    /// verified prefix を確認した上で NotRun/blocked として記録
    /// する — sample-exact 検証には gap-free capture が必要。
    #[test]
    #[ignore = "real-device lane: requires audio hardware (run `mise run test-recording-real-input`)"]
    fn real_punch_boundaries_are_sample_exact() {
        println!("real_input lane: punch boundary sample-exact verification");
        let Some((device, format)) = usable_input() else {
            return;
        };

        let map = map_for(format);
        let out_channels = u32::try_from(map.len()).expect("channels");
        let rate = format.sample_rate.get();
        let (p, lanes, epoch) = project_with_lanes(1);
        let (b, mbf) = real_budget();
        // punch [480,1440) tick（0.25 s–0.75 s 相当）、stop 2000 tick。
        let punch_in = MusicalTick::new(480);
        let punch_out = MusicalTick::new(1440);
        let plan = plan_at(
            &p,
            &lanes,
            &[out_channels],
            epoch,
            GateSpec::Punch {
                punch_in,
                punch_out,
            },
            2000,
            rate,
            b,
            mbf,
        );
        // 期待 session 区間 — GateScheduler の template と同一計算
        // （`tick_to_media - transport_start_frame`）。
        let base = plan.transport_start_frame;
        let s_in = plan
            .mapping
            .tick_to_media(punch_in, 1, rate)
            .expect("tick_to_media")
            .value()
            - base;
        let s_out = plan
            .mapping
            .tick_to_media(punch_out, 1, rate)
            .expect("tick_to_media")
            .value()
            - base;
        let expected_frames = s_out - s_in;
        assert!(expected_frames > 0, "punch region must be non-empty");
        let session_frames = plan.session_end.value();
        let spec = real_spec(&plan, epoch, format, REAL_CALLBACK_QUEUE, &[(0, map)]);
        let sid = session_id(13);
        let root = tmp_dir("real-punch");
        let mut rig = RealDeviceRig::open_one(&root, sid, plan, rate, device.as_ref(), &spec);
        let (report, fault_bits) = drive_observe(
            &mut rig,
            run_duration(session_frames, rate, 0.5),
            Duration::from_millis(2),
        );
        // continuity 系 fault（device/driver の position 不連続）
        // は verified prefix を確認した上で NotRun/blocked —
        // sample-exact punch 検証には gap-free capture が必要。
        if fault_bits != 0 {
            let prefix_frames = continuity_fault_prefix(&rig, out_channels, fault_bits);
            println!(
                "  PUNCH NotRun/blocked: continuity fault bits \
                 {fault_bits:#06x}; verified prefix {prefix_frames} \
                 frames is journal/media intact — sample-exact punch \
                 verification requires a gap-free capture on this device"
            );
            return;
        }
        assert_eq!(report.refs.len(), 1, "finalize ref count");
        let r = report.refs[0].1.as_ref().expect("finalize");
        assert_eq!(r.outcome, FinalizedOutcome::Complete);
        assert_eq!(
            r.source_frames,
            u64::try_from(expected_frames).expect("frames")
        );
        assert_eq!(
            r.media_byte_len,
            r.source_frames * u64::from(out_channels) * 4,
            "media must contain only the punched interval"
        );

        let cap = capture_of(&rig.area, 0);
        assert!(
            matches!(cap.replay.stop, StopReason::CleanEnd | StopReason::TornTail),
            "journal stop: {:?}",
            cap.replay.stop
        );
        let spans = all_journal_spans(&cap);
        // journal span が半開 punch 区間を Sample 単位で正確に cover。
        assert_session_coverage(&spans, &[(s_in, s_out)]);
        assert_source_contiguous(&spans, expected_frames);
        // project 座標の端点は punch tick に一致（内側は線形補間）。
        assert_eq!(spans.first().map(|s| s.project_in), Some(punch_in.value()));
        assert_eq!(spans.last().map(|s| s.project_out), Some(punch_out.value()));
        verify_capture_integrity(&cap, out_channels);
        println!(
            "  punch verified: session [{s_in},{s_out}) = {expected_frames} frames, \
             project [{},{}), {} span(s)",
            punch_in.value(),
            punch_out.value(),
            spans.len()
        );
    }

    /// 長時間 stability 行: ~30 s の bounded capture で frame 連続性・
    /// fault・journal 連続性を観測し、実測 frame 数・chunk 数・
    /// wall 時間を記録する。連続性の Pass 権威は journal coverage
    /// （全 session frame が 1 回ずつ記録される）と fault 非発生。
    /// continuity 系 fault（device/driver の position 不連続）は
    /// 検証済み prefix を確認した上で NotRun/blocked として記録
    /// する — iD4/WASAPI 実機では raw stream でも再現する実挙動で
    /// あり、fail-stop が prefix を守ること自体が観測対象。
    #[test]
    #[ignore = "real-device lane: requires audio hardware (run `mise run test-recording-real-input`)"]
    fn real_long_capture_monitors_frame_continuity() {
        println!("real_input lane: ~30 s long capture continuity");
        let Some((device, format)) = usable_input() else {
            return;
        };

        let map = map_for(format);
        let out_channels = u32::try_from(map.len()).expect("channels");
        let rate = format.sample_rate.get();
        let (p, lanes, epoch) = project_with_lanes(1);
        // 長時間 lane は pump stall 耐性を上げるため buffer を拡大
        // する — 既定 pool（16 chunk ≈ 160 ms）では負荷環境の
        // scheduling stall が `FAULT_QUEUE_FULL`/`FREE_EXHAUSTED`
        // を誤誘発する。容量は plan パラメータであり、本番も
        // jitter に応じて sizing する。fault 経路は armed のまま
        // （容量拡大は検証を弱めない）。
        let mut b = budget();
        b.chunk_frames = REAL_MAX_BLOCK;
        b.chunks = 256;
        b.filled_capacity = 256;
        let mbf = REAL_MAX_BLOCK;
        // 1920 tick/s × 30 s = 57600 tick → session ≈ 30 s（rate 不変）。
        let plan = plan_at(
            &p,
            &lanes,
            &[out_channels],
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            57_600,
            rate,
            b,
            mbf,
        );
        let session_frames = plan.session_end.value();
        let spec = real_spec(&plan, epoch, format, REAL_CALLBACK_QUEUE, &[(0, map)]);
        let sid = session_id(14);
        let root = tmp_dir("real-long-capture");
        let mut rig = RealDeviceRig::open_one(&root, sid, plan, rate, device.as_ref(), &spec);
        let wall_start = Instant::now();
        // session 時間 + 余裕で bounded に駆動する。
        let (report, fault_bits) = drive_observe(
            &mut rig,
            run_duration(session_frames, rate, 1.0),
            Duration::from_millis(5),
        );
        let wall = wall_start.elapsed();

        // continuity 系 fault（`FAULT_EPOCH_DISCONTINUITY` /
        // `FAULT_INPUT_DISCONTINUITY`）は device/driver の frame
        // position 不連続 — iD4/WASAPI 実機で観測される実挙動で
        // あり、raw stream（adapter/pump 無し）でも再現する。
        // fail-stop が verified prefix を守ったことを検証した上で
        // 「このデバイスでは bounded 長時間 gap-free capture が
        // 達成不能」という観測結果として記録する（NotRun/blocked
        // 相当 — Pass ではない）。continuity 以外の bit は
        // harness/系の欠陥として hard fail。
        if fault_bits != 0 {
            let prefix_frames = continuity_fault_prefix(&rig, out_channels, fault_bits);
            let prefix_secs =
                f64::from(u32::try_from(prefix_frames).expect("prefix frames")) / f64::from(rate);
            println!(
                "  LONG CAPTURE NotRun/blocked: continuity fault bits \
                 {fault_bits:#06x} after {wall:.2?}; verified prefix \
                 {prefix_frames} frames ({prefix_secs:.2} s media) is \
                 journal/media intact — gap-free {session_frames}-frame \
                 capture not achievable on this device/driver"
            );
            return;
        }

        assert_complete_real(&rig, &report, &[(0, out_channels)], session_frames);

        // 観測記録: 実測 frame 数・chunk/span 数・wall 時間・
        // media 時間との比較（drift の正直な記録。連続性 Pass は
        // coverage + fault 非発生が権威）。
        let cap = capture_of(&rig.area, 0);
        let spans = all_journal_spans(&cap);
        let frames = span_frame_total(&spans);
        let frames_f = f64::from(u32::try_from(frames).expect("frames fit u32"));
        let media_secs = frames_f / f64::from(rate);
        println!(
            "  long capture: {frames} frames / {} chunk(s) / {} span(s) in \
             {:.2?} wall (media {:.2} s @ {rate} Hz, window {:.2?})",
            cap.chunks.len(),
            spans.len(),
            wall,
            media_secs,
            wall
        );
        assert_eq!(frames, session_frames, "every session frame captured once");
    }

    /// multi-device 行: >=2 の input device が列挙でき、かつ
    /// same-clock 条件（clock domain 一致・非 UNKNOWN・同一
    /// negotiated rate）を満たす場合のみ 2 device capture を試行
    /// する。条件を満たさなければ loud NotRun（cross-clock は
    /// issue #119 の範囲であり本 lane では扱わない）。
    #[test]
    #[ignore = "real-device lane: requires >=2 same-clock audio devices (run `mise run test-recording-real-input`)"]
    fn real_multi_device_same_clock_records() {
        println!("real_input lane: multi-device same-clock capture");
        let host = match default_host() {
            Ok(host) => host,
            Err(error) => {
                println!("  SKIP: default_host() failed: {error}");
                return;
            }
        };
        let devices = match host.input_devices() {
            Ok(devices) => devices,
            Err(error) => {
                println!("  SKIP: input_devices() failed: {error}");
                return;
            }
        };
        println!("  {} input device(s) enumerated", devices.len());

        // 各 device の negotiated format と clock domain を probe
        // する（open→観測→drop。start はしない）。
        let candidates = probe_candidates(devices);
        let Some(rate) = same_clock_rate(&candidates) else {
            return;
        };

        let map0 = map_for(candidates[0].1);
        let map1 = map_for(candidates[1].1);
        let ch0 = u32::try_from(map0.len()).expect("channels");
        let ch1 = u32::try_from(map1.len()).expect("channels");
        let (p, lanes, epoch) = project_with_lanes(2);
        let (b, mbf) = real_budget();
        let plan = plan_at(
            &p,
            &lanes,
            &[ch0, ch1],
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            2000,
            rate,
            b,
            mbf,
        );
        let session_frames = plan.session_end.value();
        let spec0 = real_spec(
            &plan,
            epoch,
            candidates[0].1,
            REAL_CALLBACK_QUEUE,
            &[(0, map0)],
        );
        let spec1 = real_spec(
            &plan,
            epoch,
            candidates[1].1,
            REAL_CALLBACK_QUEUE,
            &[(1, map1)],
        );
        let sid = session_id(15);
        let root = tmp_dir("real-multi-device");
        let mut rig = RealDeviceRig::open(
            &root,
            sid,
            plan,
            rate,
            &[
                (candidates[0].0.as_ref(), &spec0),
                (candidates[1].0.as_ref(), &spec1),
            ],
            None,
        );
        let report = drive_and_finish(
            &mut rig,
            run_duration(session_frames, rate, 0.5),
            Duration::from_millis(2),
        );
        assert_complete_real(&rig, &report, &[(0, ch0), (1, ch1)], session_frames);
        println!(
            "  multi-device capture: streams 0/1 Complete, \
             {session_frames} frames each @ {rate} Hz (same clock domain)"
        );
    }

    /// PCM fidelity 行: deterministic source（既知内容の OS
    /// loopback や注入可能な仮想 device）が無い限り `NotRun`。
    /// 実機 lane の default input device は内容が不定であり、
    /// `MockPcm` 相当の deterministic 期待値を構成できない —
    /// 構造検証（CRC・有限値・連続性）のみが本 lane の Pass 権威。
    /// device 名を列挙して loopback 候補の有無を記録するが、
    /// 内容の決定性は保証できないため本 row は `NotRun` のまま。
    #[test]
    #[ignore = "real-device lane: deterministic PCM source availability probe (run `mise run test-recording-real-input`)"]
    fn real_deterministic_pcm_source_records_not_run() {
        println!("real_input lane: deterministic PCM fidelity");
        let host = match default_host() {
            Ok(host) => host,
            Err(error) => {
                println!("  SKIP: default_host() failed: {error}");
                return;
            }
        };
        match host.input_devices() {
            Ok(devices) => {
                for device in &devices {
                    let name = device.name().unwrap_or_else(|_| "<unnamed>".to_owned());
                    println!("  input device: \"{name}\"");
                }
                if devices.is_empty() {
                    println!("  (no input devices enumerated)");
                }
            }
            Err(error) => println!("  input_devices() failed: {error}"),
        }
        println!(
            "  NotRun: no deterministic PCM source — an OS loopback's content \
             is not a verifiable expectation, and MockPcm equality is not \
             claimed for real PCM (structural verification is the Pass authority)"
        );
    }
}
