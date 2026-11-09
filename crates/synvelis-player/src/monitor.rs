//! RCD-03 非 RT monitor/latency/playback 切替 adapter
//! （synvelis#117 slice）。
//!
//! 責務:
//!
//! - [`MonitorAdapter`]: recording 側 [`synvelis_recording::decide`]
//!   が返す [`MonitorRoute`] に従い、input PCM と playback PCM を
//!   gain ramp 付きで混合する。route 遷移は `ramp_frames` の線形
//!   crossfade で滑らかに行い、`ramp_frames == 0` では即時切替。
//!   出力 frame `i` は入力/playback frame `i` に対応する
//!   **native placement** — latency/PDC 補正・リサンプル・
//!   オフセットを placement に適用しない。ramp は DSP のみで
//!   gate の sample 精度の代替ではない。
//! - latency 報告: [`LatencyReport`] → [`LatencyResolution`] で
//!   Measured/Reported/Estimated/Unknown の品質・Clock Epoch・
//!   basis を保持して一意に解決する。round-trip 実測は合計として
//!   authoritative であり、個別 input/output/reported を再加算
//!   しない（二重加算禁止）。
//! - [`prepare_low_latency`]: 独立した非 RT low-latency monitor
//!   route の準備状態を `Ready`/`Degraded`/`Unavailable` で報告
//!   する。capture・通常 playback・export は変更しない。
//!
//! RT graph への搭載・transport 配線・実デバイス latency 実測は
//! 本 slice の対象外（fixture/headless 検証のみ）。

use ruviform_media_core::ClockEpochId;
pub use synvelis_recording::MonitorRoute;

// ---------- monitor PCM adapter ----------

/// 1 block 分の monitor 入力。
///
/// `input`/`playback` は channel-major の PCM（`channels × frames`）。
/// `route == Input` で `input` が `None` の場合、adapter は
/// `Muted` に fail-closed し `input_missing` notice を立てる
/// （PCM を捏造しない）。`playback` が空でも silence として扱う。
#[derive(Clone, Debug)]
pub struct MonitorBlock {
    /// この block の可聴 route（recording `decide` の結果）。
    pub route: MonitorRoute,
    /// block の frame 数。
    pub frames: usize,
    /// ソフトウェア入力 PCM（`None` = 未供給）。
    pub input: Option<Vec<Vec<f32>>>,
    /// 既存 playback PCM。
    pub playback: Vec<Vec<f32>>,
}

/// monitor adapter の notice。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MonitorOutputNotices {
    /// `route == Input` だったが input PCM が未供給で `Muted`
    /// に降格した。
    pub input_missing: bool,
}

/// `process_block` の結果。
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorOutput {
    /// channel-major の monitor 出力 PCM（`channels × frames`）。
    pub pcm: Vec<Vec<f32>>,
    /// 実際に適用した route（fail-closed 降格を反映）。
    pub effective_route: MonitorRoute,
    /// notice 集合。
    pub notices: MonitorOutputNotices,
}

/// gain を `target` へ `step` ずつ近づける（線形 ramp）。
fn approach(gain: f32, target: f32, step: f32) -> f32 {
    if gain < target {
        (gain + step).min(target)
    } else if gain > target {
        (gain - step).max(target)
    } else {
        gain
    }
}

/// gain 1.0/0.0 を乗算せず通過させ、native placement の bit 一致を
/// 保つ（`x * 1.0 == x` だが `-0.0` 正規化を避けるため明示分岐）。
/// gain は `approach` が目標値へ厳密に到達するため 0.0/1.0 の
/// 完全一致比較が意味を持つ。
#[allow(clippy::float_cmp)]
#[inline]
fn apply(gain: f32, sample: f32) -> f32 {
    if gain == 1.0 {
        sample
    } else if gain == 0.0 {
        0.0
    } else {
        sample * gain
    }
}

/// 非 RT の monitor gain-ramp adapter。
///
/// `block_frames` は 1 block の最大 frame 数。`block.frames` が
/// これを超える場合は先頭 `block_frames` のみ処理する
/// （呼出し側は block をこの容量以下に分割する契約）。
/// 内部 scratch のみを使い、RT 経路での利用は想定しない
/// （出力は呼出し側への複写を返す）。
pub struct MonitorAdapter {
    channels: usize,
    block_frames: usize,
    ramp_step: f32,
    input_gain: f32,
    playback_gain: f32,
    in_gain_scratch: Vec<f32>,
    pb_gain_scratch: Vec<f32>,
    scratch: Vec<Vec<f32>>,
}

impl MonitorAdapter {
    /// `channels` ch・`block_frames` 上限・`ramp_frames` 遷移幅の
    /// adapter を構築する。gain は 0 開始（初回 route へ ramp）。
    #[must_use]
    pub fn new(channels: usize, block_frames: usize, ramp_frames: u32) -> Self {
        // ramp_frames は block 長程度の小さい値を想定し、f32 への
        // 精度落ちは実用上問題にならない。
        #[allow(clippy::cast_precision_loss)]
        let ramp_step = if ramp_frames == 0 {
            f32::INFINITY
        } else {
            1.0 / ramp_frames as f32
        };
        Self {
            channels,
            block_frames,
            ramp_step,
            input_gain: 0.0,
            playback_gain: 0.0,
            in_gain_scratch: vec![0.0; block_frames],
            pb_gain_scratch: vec![0.0; block_frames],
            scratch: vec![vec![0.0; block_frames]; channels],
        }
    }

    /// 現在の input gain（検査用）。
    #[must_use]
    pub fn input_gain(&self) -> f32 {
        self.input_gain
    }

    /// 現在の playback gain（検査用）。
    #[must_use]
    pub fn playback_gain(&self) -> f32 {
        self.playback_gain
    }

    /// 1 block を処理し monitor 出力を返す。
    ///
    /// route 遷移は frame ごとに `1/ramp_frames` ずつ gain を近づける
    /// 線形 crossfade。`ramp_frames == 0` では block 先頭で目標 gain
    /// へ即時切替する。`Muted` は両 gain を 0 へ ramp する。
    pub fn process_block(&mut self, block: &MonitorBlock) -> MonitorOutput {
        let frames = block.frames.min(self.block_frames);
        let (effective, input_missing) = match block.route {
            MonitorRoute::Input if block.input.is_none() => (MonitorRoute::Muted, true),
            route => (route, false),
        };
        let in_target = if effective == MonitorRoute::Input {
            1.0
        } else {
            0.0
        };
        let pb_target = if effective == MonitorRoute::Playback {
            1.0
        } else {
            0.0
        };
        // gain は frame ごとに 1 回だけ進め、全 channel で共有する
        // （channel 数で ramp が速くならないようにする）。
        for i in 0..frames {
            self.input_gain = approach(self.input_gain, in_target, self.ramp_step);
            self.playback_gain = approach(self.playback_gain, pb_target, self.ramp_step);
            self.in_gain_scratch[i] = self.input_gain;
            self.pb_gain_scratch[i] = self.playback_gain;
        }
        for c in 0..self.channels {
            let in_ch = block.input.as_ref().and_then(|v| v.get(c));
            let pb_ch = block.playback.get(c);
            let out = &mut self.scratch[c];
            for (i, o) in out.iter_mut().enumerate().take(frames) {
                *o = apply(
                    self.in_gain_scratch[i],
                    in_ch.and_then(|ch| ch.get(i)).copied().unwrap_or(0.0),
                ) + apply(
                    self.pb_gain_scratch[i],
                    pb_ch.and_then(|ch| ch.get(i)).copied().unwrap_or(0.0),
                );
            }
        }
        MonitorOutput {
            pcm: self.scratch[..self.channels]
                .iter()
                .map(|ch| ch[..frames].to_vec())
                .collect(),
            effective_route: effective,
            notices: MonitorOutputNotices { input_missing },
        }
    }
}

// ---------- latency resolution ----------

/// latency 値の品質分類（弱い順: `Unknown` < `Estimated` <
/// `Reported` < `Measured`）。
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LatencyQuality {
    /// 品質不明。
    Unknown,
    /// 推定値。
    Estimated,
    /// デバイス/ドライバ申告値（定義検証済みのみ算入）。
    Reported,
    /// timestamp/loopback 実測値。
    Measured,
}

/// latency 値の根拠（ADC/DAC timestamp が最優先）。
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LatencyBasis {
    /// callback timestamp + reported latency の定義検証済み合成。
    CallbackReported,
    /// ADC/DAG ハードウェア timestamp 由来。
    DeviceTimestamp,
    /// loopback round-trip 実測の合計。
    RoundTripMeasured,
}

/// latency の 1 構成要素。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LatencyComponent {
    /// latency [frame]。
    pub frames: u64,
    /// 品質。
    pub quality: LatencyQuality,
    /// 根拠。
    pub basis: LatencyBasis,
    /// この値が属する Clock Epoch（不明なら `None`）。
    pub clock_epoch: Option<ClockEpochId>,
}

/// monitor latency の生報告（非 RT 構築）。
///
/// `round_trip` は合計として authoritative — 存在する場合、
/// `input`/`output`/`reported` は合計へ再加算しない。
/// `reported` は `reported_verified == true` の場合のみ合計へ
/// 算入し、未検証なら `excluded_reported` に計上する。
#[derive(Clone, Debug, Default)]
pub struct LatencyReport {
    /// round-trip 実測の合計 latency（存在すればこれのみが合計）。
    pub round_trip: Option<LatencyComponent>,
    /// input 側 latency。
    pub input: Option<LatencyComponent>,
    /// output 側 latency。
    pub output: Option<LatencyComponent>,
    /// driver/device 申告の追加 latency 群。
    pub reported: Vec<LatencyComponent>,
    /// `reported` の定義が検証済みか（false なら算入しない）。
    pub reported_verified: bool,
}

/// 解決後の latency basis。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LatencyResolutionBasis {
    /// ADC/DAC timestamp 由来。
    DeviceTimestamp,
    /// callback timestamp + reported latency。
    CallbackReported,
    /// round-trip 実測合計。
    RoundTripMeasured,
}

/// [`LatencyReport::resolve`] の結果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatencyResolution {
    /// 合計 latency [frame]。構成要素 0 件または overflow で `None`。
    pub total_frames: Option<u64>,
    /// 算入要素中の最弱品質。`Measured` は Clock Epoch 確定時のみ
    /// 維持し、未確定なら `Reported` へ降格する。
    pub quality: LatencyQuality,
    /// 採用した basis（構成要素 0 件なら `None`）。
    pub basis: Option<LatencyResolutionBasis>,
    /// 解決に使った Clock Epoch（算入要素の epoch が一致した場合）。
    pub clock_epoch: Option<ClockEpochId>,
    /// 合計へ算入しなかった reported 構成要素の数。
    pub excluded_reported: usize,
}

impl LatencyReport {
    /// 報告を一意の [`LatencyResolution`] へ解決する。
    ///
    /// - `round_trip` 存在時: その `frames` のみが合計
    ///   （input/output/reported を再加算しない）。
    /// - 非 round-trip: `input` + `output` +（`reported_verified`
    ///   なら `reported` 全件）を checked 加算。overflow は
    ///   `total_frames = None` + `Unknown`。
    /// - 品質は算入要素の最弱。`Measured` は算入要素の Clock Epoch
    ///   が一意に確定した場合のみ維持する。
    #[must_use]
    pub fn resolve(&self) -> LatencyResolution {
        if let Some(rt) = &self.round_trip {
            let epoch = rt.clock_epoch;
            let quality = capped_quality(rt.quality, epoch);
            return LatencyResolution {
                total_frames: Some(rt.frames),
                quality,
                basis: Some(match rt.basis {
                    LatencyBasis::RoundTripMeasured => LatencyResolutionBasis::RoundTripMeasured,
                    LatencyBasis::DeviceTimestamp => LatencyResolutionBasis::DeviceTimestamp,
                    LatencyBasis::CallbackReported => LatencyResolutionBasis::CallbackReported,
                }),
                clock_epoch: epoch,
                excluded_reported: self.reported.len(),
            };
        }
        let mut counted: Vec<&LatencyComponent> = Vec::new();
        if let Some(c) = &self.input {
            counted.push(c);
        }
        if let Some(c) = &self.output {
            counted.push(c);
        }
        if self.reported_verified {
            counted.extend(self.reported.iter());
        }
        let excluded_reported = if self.reported_verified {
            0
        } else {
            self.reported.len()
        };
        if counted.is_empty() {
            return LatencyResolution {
                total_frames: None,
                quality: LatencyQuality::Unknown,
                basis: None,
                clock_epoch: None,
                excluded_reported,
            };
        }
        let mut total: u64 = 0;
        for c in &counted {
            match total.checked_add(c.frames) {
                Some(v) => total = v,
                None => {
                    return LatencyResolution {
                        total_frames: None,
                        quality: LatencyQuality::Unknown,
                        basis: None,
                        clock_epoch: None,
                        excluded_reported,
                    };
                }
            }
        }
        // Clock Epoch は算入要素で一意に確定した場合のみ採用。
        let epoch = common_epoch(&counted);
        let quality = counted
            .iter()
            .map(|c| c.quality)
            .min()
            .map_or(LatencyQuality::Unknown, |q| capped_quality(q, epoch));
        let basis = counted.iter().map(|c| c.basis).max().map(|b| match b {
            LatencyBasis::DeviceTimestamp => LatencyResolutionBasis::DeviceTimestamp,
            LatencyBasis::CallbackReported => LatencyResolutionBasis::CallbackReported,
            LatencyBasis::RoundTripMeasured => LatencyResolutionBasis::RoundTripMeasured,
        });
        LatencyResolution {
            total_frames: Some(total),
            quality,
            basis,
            clock_epoch: epoch,
            excluded_reported,
        }
    }
}

/// `Measured` は Clock Epoch 確定時のみ維持し、未確定なら
/// `Reported` へ降格する。
fn capped_quality(q: LatencyQuality, epoch: Option<ClockEpochId>) -> LatencyQuality {
    if q == LatencyQuality::Measured && epoch.is_none() {
        LatencyQuality::Reported
    } else {
        q
    }
}

/// 算入要素の Clock Epoch が全て一致（`Some` 同士）ならその値、
/// それ以外は `None`。
fn common_epoch(counted: &[&LatencyComponent]) -> Option<ClockEpochId> {
    let mut epoch: Option<ClockEpochId> = None;
    let mut seen = false;
    for c in counted {
        match (epoch, c.clock_epoch, seen) {
            (_, None, _) => return None,
            (Some(e), Some(v), true) if e != v => return None,
            (None, Some(v), false) => {
                epoch = Some(v);
                seen = true;
            }
            _ => {}
        }
    }
    epoch
}

// ---------- low-latency monitor route ----------

/// low-latency monitor route の状態。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LowLatencyState {
    /// 利用可能（`tone_difference` なし）。
    Ready,
    /// 利用可能だが通常 monitor と音質差あり（明示的な違い）。
    Degraded,
    /// 利用不可（理由は `reason`）。
    Unavailable,
}

/// low-latency monitor route の準備状態。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct LowLatencyReadiness {
    /// route が prepare 済みか。
    pub prepared: bool,
    /// input 側が準備済みか。
    pub input_ready: bool,
    /// output 側が準備済みか。
    pub output_ready: bool,
}

/// `Unavailable` の理由（優先順: `NotPrepared` > Input > Output）。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LowLatencyReason {
    /// route が未 prepare。
    NotPrepared,
    /// input 側が未準備。
    InputNotReady,
    /// output 側が未準備。
    OutputNotReady,
}

/// 独立した非 RT low-latency monitor route の準備結果。
///
/// latency 報告は状態に依らず保持する（利用不可でも既知の
/// latency を隠さない）。capture・通常 playback・export は
/// 本 route の状態で変更されない。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LowLatencyMonitor {
    /// route の状態。
    pub state: LowLatencyState,
    /// `Unavailable` の理由。
    pub reason: Option<LowLatencyReason>,
    /// 解決済み latency。
    pub latency: LatencyResolution,
    /// 通常 monitor との音質差が既知か（`Degraded` の根拠）。
    pub tone_difference: bool,
}

/// low-latency monitor route の準備可否を判定する（非 RT）。
///
/// `readiness` のいずれかが欠ければ `Unavailable`。
/// `tone_difference` は利用可能時に `Degraded` へ落とす。
/// latency は `report.resolve()` をそのまま保持する。
#[must_use]
pub fn prepare_low_latency(
    readiness: LowLatencyReadiness,
    tone_difference: bool,
    report: &LatencyReport,
) -> LowLatencyMonitor {
    let reason = if !readiness.prepared {
        Some(LowLatencyReason::NotPrepared)
    } else if !readiness.input_ready {
        Some(LowLatencyReason::InputNotReady)
    } else if !readiness.output_ready {
        Some(LowLatencyReason::OutputNotReady)
    } else {
        None
    };
    let state = if reason.is_some() {
        LowLatencyState::Unavailable
    } else if tone_difference {
        LowLatencyState::Degraded
    } else {
        LowLatencyState::Ready
    };
    LowLatencyMonitor {
        state,
        reason,
        latency: report.resolve(),
        tone_difference,
    }
}
