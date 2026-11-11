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
    use std::time::Duration;

    use ruviform_audio_core::device::DeviceEnumerator;

    use super::*;
    use synvelis_recording::real_input::default_host;

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
}
