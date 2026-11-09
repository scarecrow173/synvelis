//! RCD-03 Monitoring 意味論: Monitor Off/Auto/On × Arm × Transport ×
//! Punch/Record Gate × Mute/Solo の可聴 route 判定と、Hardware
//! Direct Monitor の状態・notice。
//!
//! 責務分界:
//!
//! - Record/Punch Gate 開閉の権威は [`crate::gate`] の
//!   `GateScheduler`/`evaluate_gate` のみ。本モジュールは gate
//!   ロジックを複製せず、`MonitorProbe` が同じ scheduler/schedule
//!   を駆動して piece（open 区間）を得て閉区間を補完する。
//! - `decide` は純粋関数で、region ごとの可聴 route
//!   （[`MonitorRoute`]）と notice を返す。raw capture
//!   （Capture Gate/Writer）には一切影響しない — Monitor 状態で
//!   Raw Capture を停止・開始・二重補正しない。
//! - Mute/Solo は `Playback` route の可聴性のみに作用する
//!   （pre-fader monitor。`Input` route と raw capture は不変）。
//! - Hardware Direct Monitor は外部事実として入力に取り、DAW の
//!   Mute/Solo や route 判定でその値を変更しない。制御不能を
//!   制御可能と偽らない。
//!
//! PCM の実際の monitor 出力（gain ramp・native placement・
//! latency 報告）は `synvelis-player` の monitor adapter が担う。
//! 本モジュールは「何を鳴らすべきか」の判定のみを所有する。

#![allow(clippy::module_name_repetitions)]

use synvelis_project::{SessionFrame, SessionRange};

use crate::{
    bounded::BoundedVec,
    gate::{CapturePiece, GateSchedule, evaluate_gate},
    input::{ArmState, MonitorState},
    plan::PreparedRecordingPlan,
};

/// Transport 相（monitor 判定の語彙。punch open は `gate_open` が担う）。
///
/// `PunchOpen`/`PostRoll` は Issue #117 の語彙を保持するための
/// variant であり、判定上は `gate_open`（`PunchOpen` 相当）と
/// 「その他の非停止相」（PostRoll を含む）に写像される。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TransportPhase {
    /// Transport 停止。
    Stopped,
    /// Count-in 中。
    CountIn,
    /// Pre-roll 中。
    PreRoll,
    /// Punch 待機（gate 閉・録音 armed）。
    PunchWait,
    /// Punch Gate Open（`gate_open == true` と併用）。
    PunchOpen,
    /// Post-roll 中（punch out 後・gate 閉）。
    PostRoll,
    /// 通常 Playback / 録音 gate 外の再生。
    Playing,
}

/// 可聴 route（`synvelis-player` の monitor adapter と共有する語彙）。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MonitorRoute {
    /// ソフトウェア入力モニタ（input PCM を monitor route へ）。
    Input,
    /// 既存 Playback（track の通常再生信号）。
    Playback,
    /// 無音（playback 抑制・mute/solo・input unavailable 等）。
    Muted,
}

/// Hardware Direct Monitor の既知状態（Issue #117 の 3 分類）。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HardwareDirectState {
    /// デバイス/ドライバが直接制御可能（制御経路が検証済み）。
    DeviceControlled,
    /// ユーザー申告のみ（DAW は制御しない）。
    UserReported,
    /// 不明（capability 未検証/未報告）。制御可能と主張しない。
    Unknown,
}

/// 外部 Hardware Direct Monitor の観測/申告。
///
/// `observed_active` は外部から与えられる観測値であり、本 crate
/// は推測も DAW Mute/Solo による変更も行わない。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct HardwareDirectMonitor {
    /// 既知の状態分類。
    pub state: HardwareDirectState,
    /// 外部監視が実際に動作しているという観測/申告。`None` = 未報告。
    pub observed_active: Option<bool>,
}

impl Default for HardwareDirectMonitor {
    fn default() -> Self {
        Self {
            state: HardwareDirectState::Unknown,
            observed_active: None,
        }
    }
}

/// track 単位の monitor 文脈（region 不変の判定入力）。
// Mute/Solo/ready は独立した外部状態であり、2 値 enum への
// 畳み込みは意味を歪めるため bool 集合を許容する。
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug)]
pub struct TrackMonitorContext {
    /// Record Arm。
    pub arm: ArmState,
    /// Monitor 宣言モード。
    pub monitor: MonitorState,
    /// 入力 Binding + Output Capability の検証済みフラグ
    /// （Monitor On の Software Monitor 可否。未検証を真としない）。
    pub input_output_ready: bool,
    /// track mute（Playback route のみに作用）。
    pub muted: bool,
    /// track solo（Playback route のみに作用）。
    pub soloed: bool,
    /// session 内に solo track が存在するか。
    pub any_solo: bool,
    /// Hardware Direct Monitor の外部状態。
    pub hardware_direct: HardwareDirectMonitor,
}

impl Default for TrackMonitorContext {
    fn default() -> Self {
        Self {
            arm: ArmState::Off,
            monitor: MonitorState::Off,
            input_output_ready: false,
            muted: false,
            soloed: false,
            any_solo: false,
            hardware_direct: HardwareDirectMonitor::default(),
        }
    }
}

/// monitor notice（bool 集合。Copy・alloc なし）。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MonitorNotices {
    /// Software Input route が可聴で外部 hardware monitor が
    /// 観測/申告されている（Software+Hardware 二重監視の警告）。
    pub double_monitoring: bool,
    /// Hardware Direct 状態が `Unknown`（制御可否を主張しない
    /// ための表示値。route に依らず常に立つ）。
    pub hardware_direct_unknown: bool,
    /// Monitor On だが入力 Binding/Output Capability が未検証で
    /// Playback にフォールバックした。
    pub input_unavailable: bool,
}

/// 1 region の monitor 判定結果。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MonitorDecision {
    /// 可聴 route。
    pub route: MonitorRoute,
    /// notice 集合。
    pub notices: MonitorNotices,
}

/// `Playback` route の可聴性を Mute/Solo で解決する。
fn playback_route(ctx: &TrackMonitorContext) -> MonitorRoute {
    if ctx.muted || (ctx.any_solo && !ctx.soloed) {
        MonitorRoute::Muted
    } else {
        MonitorRoute::Playback
    }
}

/// region の可聴 route を判定する（純粋関数）。
///
/// `gate_open` は Record/Punch Gate がその region で開いているか
/// （[`MonitorProbe`] が `evaluate_gate` の piece から供給する）。
/// `arm == Off` では `gate_open` は意味を持たず無視される。
///
/// 規則（spec の製品ポリシー写像）:
///
/// | monitor | 条件 | route |
/// | --- | --- | --- |
/// | Off | arm && gate_open | `Muted` |
/// | Off | 他 | `Playback`（mute/solo で `Muted`） |
/// | Auto | arm && (gate_open || Stopped) | `Input` |
/// | Auto | 他 | `Playback`（mute/solo で `Muted`） |
/// | On | input_output_ready | `Input` |
/// | On | !input_output_ready | `Playback` + `input_unavailable` |
#[must_use]
pub fn decide(
    ctx: &TrackMonitorContext,
    phase: TransportPhase,
    gate_open: bool,
) -> MonitorDecision {
    let armed_gate_open = ctx.arm == ArmState::On && gate_open;
    let route = match ctx.monitor {
        MonitorState::Off => {
            if armed_gate_open {
                MonitorRoute::Muted
            } else {
                playback_route(ctx)
            }
        }
        MonitorState::Auto => {
            if ctx.arm == ArmState::On && (armed_gate_open || phase == TransportPhase::Stopped) {
                MonitorRoute::Input
            } else {
                playback_route(ctx)
            }
        }
        MonitorState::On => {
            if ctx.input_output_ready {
                MonitorRoute::Input
            } else {
                playback_route(ctx)
            }
        }
    };
    MonitorDecision {
        route,
        notices: MonitorNotices {
            double_monitoring: route == MonitorRoute::Input
                && ctx.hardware_direct.observed_active == Some(true),
            hardware_direct_unknown: ctx.hardware_direct.state == HardwareDirectState::Unknown,
            input_unavailable: ctx.monitor == MonitorState::On && !ctx.input_output_ready,
        },
    }
}

/// `MonitorProbe::feed` が返す 1 region 分の判定結果。
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MonitorSegment {
    /// region の session-frame 半開区間。
    pub session: SessionRange,
    /// この region で Record/Punch Gate が開いていたか。
    pub gate_open: bool,
    /// 可聴 route。
    pub route: MonitorRoute,
    /// notice 集合。
    pub notices: MonitorNotices,
}

/// 非 RT の monitor 判定プローブ。
///
/// `PreparedRecordingPlan` の `scheduler` を共有参照し、block ごとに
/// `GateSchedule` を extend → `evaluate_gate` で open piece を得て、
/// 閉区間を補完しながら route を session-frame 精度で分割する。
/// session 位置の進行則は `CaptureEngine` と同一（`cursor` は
/// `min(cursor+frames, session_end)` で飽和）。
pub struct MonitorProbe<'a> {
    scheduler: &'a crate::gate::GateScheduler,
    schedule: GateSchedule,
    scratch: BoundedVec<CapturePiece>,
    cursor: SessionFrame,
    session_end: SessionFrame,
    ctx: TrackMonitorContext,
    /// `evaluate_gate` の契約違反で以後の feed を停止したか
    /// （fail-closed。発生は設計上あり得ない）。
    faulted: bool,
}

impl<'a> MonitorProbe<'a> {
    /// `plan` の gate scheduler を共有して probe を構築する。
    /// `ctx` は判定に使う track の monitor 文脈。
    #[must_use]
    pub fn new(plan: &'a PreparedRecordingPlan, ctx: TrackMonitorContext) -> Self {
        let lookahead = plan.budget.lookahead_spans;
        Self {
            scheduler: &plan.scheduler,
            schedule: GateSchedule::new(lookahead),
            scratch: BoundedVec::with_capacity(lookahead),
            cursor: SessionFrame::ZERO,
            session_end: plan.session_end,
            ctx,
            faulted: false,
        }
    }

    /// 現在の session 位置。
    #[must_use]
    pub fn cursor(&self) -> SessionFrame {
        self.cursor
    }

    /// `session_end` に到達したか。
    #[must_use]
    pub fn finished(&self) -> bool {
        self.cursor >= self.session_end
    }

    /// 契約違反で停止したか。
    #[must_use]
    pub fn faulted(&self) -> bool {
        self.faulted
    }

    /// 1 block 分（`frames`）の route を `out` へ書く（非 RT）。
    ///
    /// `phase` はその block 区間の transport 相（block 内で一定と
    /// 仮定。block 内の相変化は本 slice の対象外）。block 途中の
    /// gate 境界は piece 境界で正確に分割され、segment は連続・
    /// 非重複・半開 `[in, out)` を保つ。
    ///
    /// `out` は呼出し側が容量を確保して渡す（内部で再利用する
    /// ため事前に `clear` する）。
    pub fn feed(
        &mut self,
        frames: u32,
        phase: TransportPhase,
        out: &mut BoundedVec<MonitorSegment>,
    ) {
        out.clear();
        if self.faulted || self.finished() {
            return;
        }
        let block_start = self.cursor;
        let full_end = block_start
            .checked_add(i64::from(frames))
            .unwrap_or(self.session_end);
        let eval_end = full_end.min(self.session_end);
        // session 前進（engine と同じく session_end で飽和）。
        self.cursor = eval_end;
        if eval_end <= block_start {
            return;
        }
        let Ok(block_range) = SessionRange::try_new(block_start, eval_end) else {
            self.faulted = true;
            return;
        };
        // lookahead 補充 → gate 評価（engine/pump と同じ順序）。
        if self.scheduler.extend(&mut self.schedule, eval_end).is_err() {
            self.faulted = true;
            return;
        }
        self.scratch.clear();
        if evaluate_gate(&mut self.schedule, block_range, &mut self.scratch).is_err() {
            self.faulted = true;
            return;
        }
        // open piece（block 範囲に clip 済み・昇順）で区間を分割し、
        // 閉区間を補完する。piece は `self.scratch` を借用するため
        // 判定は field を個別に渡す helper で行う。
        let mut pos = block_start;
        let mut overflow = false;
        for piece in self.scratch.as_slice() {
            if piece.session_in > pos
                && !emit_segment(
                    &self.ctx,
                    phase,
                    false,
                    pos,
                    piece.session_in,
                    out,
                    &mut overflow,
                )
            {
                break;
            }
            if !emit_segment(
                &self.ctx,
                phase,
                true,
                piece.session_in,
                piece.session_out,
                out,
                &mut overflow,
            ) {
                break;
            }
            pos = piece.session_out;
        }
        if !overflow && pos < eval_end {
            let _ = emit_segment(&self.ctx, phase, false, pos, eval_end, out, &mut overflow);
        }
        // out 容量不足は契約違反として fail-closed（部分出力は
        // faulted() を見た呼出し側が破棄する）。
        self.faulted |= overflow;
    }
}

/// `[start, end)` の region を判定して `out` へ push する。
///
/// 空区間は push しない。`out` 満杯時は `overflow` を立てて
/// `false` を返す。
fn emit_segment(
    ctx: &TrackMonitorContext,
    phase: TransportPhase,
    gate_open: bool,
    start: SessionFrame,
    end: SessionFrame,
    out: &mut BoundedVec<MonitorSegment>,
    overflow: &mut bool,
) -> bool {
    let Ok(range) = SessionRange::try_new(start, end) else {
        return true;
    };
    if range.is_empty() {
        return true;
    }
    let d = decide(ctx, phase, gate_open);
    if out
        .push(MonitorSegment {
            session: range,
            gate_open,
            route: d.route,
            notices: d.notices,
        })
        .is_err()
    {
        *overflow = true;
        return false;
    }
    true
}
