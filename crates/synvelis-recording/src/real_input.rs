//! 実 audio device backend（`ruviform-audio-device` / cpal）への
//! composition root。
//!
//! 本 module は「host → device → F32 format 交渉 →
//! [`InputStreamConfig`]」までを提供する。`InputStreamSpec` の
//! 構築（plan/tap/budget は project 側の知識）と `InputSession`
//! 駆動は caller の責務であり、公開 signature は変えない。
//!
//! `default_host` のみ `real-device` feature で有効化される —
//! cpal は Linux で `libasound2-dev` 等の OS 依存を引くため、
//! 既定の依存グラフ・headless lane を不変に保つ optional dep
//! 分離（synvelis-app の `webview`/`native` feature と同じ方針）。

use ruviform_audio_core::device::{BufferSize, Device, InputStreamConfig, SupportedInputFormat};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};

#[cfg(feature = "real-device")]
pub use ruviform_audio_device::default_host;

/// 実 input device の open 可能な F32 [`AudioFormat`] を交渉する。
///
/// adapter 交渉済みの `default_input_format` が F32 ならそれを採用し、
/// さもなくば `supported_input_formats` の F32 range から選択する
/// （ruviform `tests/real_input_e2e.rs` の同名規則を薄い module と
/// して移植。print/log は持たず `Option` を返す）。
#[must_use]
pub fn negotiate_input_format(device: &dyn Device) -> Option<AudioFormat> {
    match device.default_input_format() {
        Ok(format) if format.sample_format == SampleFormat::F32 => Some(format),
        _ => {
            let supported = device.supported_input_formats().ok()?;
            select_f32_input_format(&supported)
        }
    }
}

/// supported input format 列から open 可能な F32 [`AudioFormat`] を
/// 選ぶ — stereo range・48 kHz 含有を優先し、rate は range 内へ
/// clamp する。候補が無ければ `None`。
#[must_use]
pub fn select_f32_input_format(supported: &[SupportedInputFormat]) -> Option<AudioFormat> {
    supported
        .iter()
        .filter(|range| range.sample_format == SampleFormat::F32)
        .filter(|range| {
            range.min_sample_rate <= range.max_sample_rate && range.max_sample_rate != 0
        })
        .filter(|range| layout_for_channels(range.channels).is_some())
        .max_by_key(|range| {
            (
                range.channels == 2,
                range.min_sample_rate <= 48_000 && 48_000 <= range.max_sample_rate,
            )
        })
        .and_then(|range| {
            let layout = layout_for_channels(range.channels)?;
            Some(AudioFormat {
                sample_rate: SampleRate::new(
                    48_000.clamp(range.min_sample_rate, range.max_sample_rate),
                )
                .ok()?,
                layout,
                sample_format: SampleFormat::F32,
            })
        })
}

/// channel 数 → 既知 `ChannelLayout` の写像（adapter が写像可能な
/// subset のみ。未知の channel 数は `None`）。
fn layout_for_channels(channels: u32) -> Option<ChannelLayout> {
    match channels {
        1 => Some(ChannelLayout::Mono),
        2 => Some(ChannelLayout::Stereo),
        4 => Some(ChannelLayout::Quad),
        6 => Some(ChannelLayout::FivePointOne),
        8 => Some(ChannelLayout::SevenPointOne),
        _ => None,
    }
}

/// 交渉済み format から `InputStreamConfig` を組み立てる。
///
/// `buffer_size` は `BufferSize::Default` 固定 — 実 cpal adapter は
/// `Fixed` を `DeviceError::UnsupportedConfig` で拒否し得るため
/// Default を使う。実効 callback block size は可変であり、
/// `InputStreamSpec::max_block_frames` 超過は `InputAdapter` 側の
/// 既存 `FAULT_CONTRACT` で fail-stop する（十分な上限を与えるのは
/// caller 責務）。
#[must_use]
pub fn real_input_config(format: AudioFormat) -> InputStreamConfig {
    InputStreamConfig {
        format,
        buffer_size: BufferSize::Default,
    }
}
