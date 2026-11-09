---
id: "DESIGN-034UtlI2uLUF1lr2g3CUM0"
type: "design"
status: "approved"
title: "RCD-03 Monitor Policy / Punch Gate Probe / Player Monitor-Latency Adapter"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md"
    - "docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  defers: []
  deferred-by: []
---

# RCD-03 Monitor Policy / Punch Gate Probe / Player Monitor-Latency Adapter

## Overview

`synvelis-recording` に monitor 意味論モジュール（`monitor.rs`）を追加し、
`synvelis-player` に非 RT monitor/latency adapter モジュール
（`monitor.rs`）を追加する。両者は `MonitorRoute`
（`Input`/`Playback`/`Muted`）を共有語彙として使う — route 判定の
権威は recording 側、PCM 実行と latency 解決の権威は player 側。

既存資産の再利用:

- `GateScheduler`/`GateSchedule`/`evaluate_gate`（`gate.rs`）が
  Record/Punch Gate 開閉区間の唯一の権威。monitor probe はこれを
  複製せず、block ごとに schedule を extend して piece を得て
  閉区間を補完する。
- `ArmState`/`InputBinding`/`TapRecord`（`input.rs`）の独立状態
  モデルを拡張し、`MonitorState` を `Off`/`Auto`/`On` にする。
- `MockPcm`/`Scenario`/`InspectionWriter`/`verify_log`（`mock.rs`・
  `tests/common`）の fixture 基盤で Raw File Byte を独立比較する。
- player 側は `synvelis-recording` への既存依存で `MonitorRoute`
  を import する（逆方向依存は作らない）。

## 状態・権威モデル

- **Record/Punch Gate 開閉**: `evaluate_gate` が返す `CapturePiece`
  列だけが根拠。monitor probe は同じ `GateScheduler`（plan 由来・
  stream 間共有）で schedule を駆動し、piece が無い区間を閉区間
  とみなす。gate ロジックの第二実装を作らない。
- **可聴 route**: `monitor::decide` が純粋関数として
  `(TrackMonitorContext, TransportPhase, gate_open)` →
  `MonitorDecision` を返す。状態を持たず、RT/非 RT どちらからも
  呼べる（本 slice の呼出しは非 RT probe と fixture のみ）。
- **Hardware Direct**: 外部事実（device/driver 制御可否・観測/
  申告 active）を `HardwareDirectMonitor` として入力に取り、
  DAW の Mute/Solo や route 判定でその値を変更しない。
  `observed_active` は外部から与えられる観測値であり、本 crate
  は推測しない。
- **Latency**: `LatencyReport`（構成要素）→ `resolve()` で
  `LatencyResolution`（合計・品質・basis・epoch）を一意に決める。
  Round-trip 実測が存在する場合はそれを唯一の合計とし、個別
  構成要素との加算を構造的に行わない。

## 1. `synvelis-recording` — `src/monitor.rs`（新規）+ `input.rs` 拡張

### 1.1 `MonitorState` の 3 値化（`input.rs`）

```rust
pub enum MonitorState { Off, Auto, On }  // default Off
```

`InputBinding.monitor`/`TapRecord.monitor` は宣言モードをそのまま
記録する。`Auto` の解決は binding 時ではなく `decide` が region
ごとに行う（Arm/Transport/Gate/Mute/Solo は region 依存のため）。
既存の `Off`/`On` 利用箇所・test は無変更で動く（enum 追加のみ）。

### 1.2 型

```rust
/// transport 相（monitor 判定の粒度。punch open は gate_open が担う）。
pub enum TransportPhase {
    Stopped, CountIn, PreRoll, PunchWait, PunchOpen, PostRoll, Playing,
}

/// 可聴 route（player adapter と共有する語彙）。
pub enum MonitorRoute { Input, Playback, Muted }

/// hardware direct monitor の既知状態。
pub enum HardwareDirectState { DeviceControlled, UserReported, Unknown }

/// 外部 hardware monitor の観測/申告。値は外部事実であり DAW が
/// mute/solo で変更しない。
pub struct HardwareDirectMonitor {
    pub state: HardwareDirectState,
    pub observed_active: Option<bool>,  // None = 未報告
}

/// track 単位の monitor 文脈（判定入力。region 不変部分）。
pub struct TrackMonitorContext {
    pub arm: ArmState,
    pub monitor: MonitorState,
    /// 入力 Binding + Output Capability の検証済みフラグ
    /// （Monitor On の Software Monitor 可否。未検証を真としない）。
    pub input_output_ready: bool,
    /// track mute/solo（Playback route の可聴性のみに作用）。
    pub muted: bool,
    pub soloed: bool,
    /// session 内に solo track が存在するか。
    pub any_solo: bool,
    pub hardware_direct: HardwareDirectMonitor,
}

/// notice（bool 集合。Copy・alloc なし）。
pub struct MonitorNotices {
    pub double_monitoring: bool,      // Software Input 可聴 + 外部監視 active
    pub hardware_direct_unknown: bool,// state == Unknown
    pub input_unavailable: bool,      // Monitor On だが input_output_ready == false
}

pub struct MonitorDecision {
    pub route: MonitorRoute,
    pub notices: MonitorNotices,
}
```

### 1.3 判定規則（`decide`）

`gate_open` は `arm == On` のときのみ意味を持つ（Arm Off では
record gate を評価しない → `Playback` 経路）。

| monitor | 条件 | route |
| --- | --- | --- |
| Off | arm && gate_open | `Muted`（playback 抑制・software input off） |
| Off | 上記以外 | `Playback`（mute/solo で `Muted` 化） |
| Auto | arm && gate_open | `Input` |
| Auto | arm && phase == Stopped | `Input` |
| Auto | arm off、またはその他 phase | `Playback`（mute/solo で `Muted` 化） |
| On | input_output_ready | `Input`（arm/gate/phase 不問） |
| On | !input_output_ready | `Playback`（mute/solo で `Muted` 化）+ `input_unavailable` |

- Mute/Solo: `Playback` 選択時、`muted || (any_solo && !soloed)`
  なら `Muted`。`Input` route と raw capture には作用しない
  （pre-fader monitor）。
- `software_input_active` は `route == Input` で導出可能なため
  独立フィールドにしない。
- notices:
  - `route == Input && hardware_direct.observed_active == Some(true)`
    → `double_monitoring`。
  - `hardware_direct.state == Unknown` → `hardware_direct_unknown`
    （route に依らず常に立つ。制御可否を主張しないための表示値）。
  - `Monitor On && !input_output_ready` → `input_unavailable`。

`TransportPhase::PunchOpen`/`PostRoll` は語彙として保持するが、
判定上は `gate_open`（PunchOpen 相当）と「その他の非停止相」
（PostRoll を含む）に写像する。punch 待機/Count-in/Pre-roll/
通常 Playback は全て Playback 経路で一致する。

### 1.4 `MonitorProbe`（非 RT）

```rust
pub struct MonitorProbe {
    scheduler: GateScheduler,        // plan.scheduler と同一値を共有利用
    schedule: GateSchedule,          // stream ごとの lookahead window
    scratch: BoundedVec<CapturePiece>,
    cursor: SessionFrame,            // session 位置（engine と同じ進行則）
    session_end: SessionFrame,
    ctx: TrackMonitorContext,
}

impl MonitorProbe {
    pub fn new(plan: &PreparedRecordingPlan, ctx: TrackMonitorContext) -> Self;
    /// 1 block 分の route を session-frame 精度で `out` へ書く。
    /// `frames` は入力 block の frame 数（session cursor を進める）。
    /// `phase` はその block 区間の transport 相（block 内で一定と仮定）。
    pub fn feed(&mut self, frames: u32, phase: TransportPhase,
                out: &mut BoundedVec<MonitorSegment>);
}

pub struct MonitorSegment {
    pub session: SessionRange,   // 半開
    pub gate_open: bool,
    pub route: MonitorRoute,
    pub notices: MonitorNotices,
}
```

- `feed` は `cursor..min(cursor+frames, session_end)` を block
  範囲とし、`scheduler.extend(schedule, until)` →
  `evaluate_gate(schedule, block_range, scratch)` で open piece を
  得て、閉区間を補完しながら piece 境界で segment を切る。
  `evaluate_gate` の `Underrun`/`SpanCapacity` は設計上発生しない
  （extend を先に呼ぶ・scratch 容量 = lookahead_spans ≥ piece 数）
  ため、発生時は `debug_assert` + segment 生成を打ち切る
  （fail-closed: 以後の feed は空を返す `faulted` フラグを持つ）。
- block 途中の Punch In/Out は piece 境界で正確に分割され、
  Gap/重複は session 範囲の連続性で保証される（capture engine と
  同一の gate 権威を使うため二重補正の余地がない）。
- `phase` は block 粒度。transport 相の block 内変化は本 slice
  の対象外（app 層が block 境界で供給する契約）。

## 2. `synvelis-player` — `src/monitor.rs`（新規）

### 2.1 PCM adapter（非 RT・alloc-free 処理部）

```rust
pub struct MonitorAdapter {
    channels: usize,
    block_frames: usize,
    ramp_frames: u32,
    input_gain: f32,     // 現在 gain（route 目標へ向けて ramp）
    playback_gain: f32,
    scratch: Vec<Vec<f32>>,   // 出力 planar（構築時確保）
}

pub struct MonitorBlock<'a> {
    pub playback: &'a [Vec<f32>],       // planar、block_frames
    pub input: Option<&'a [Vec<f32>]>,  // monitor input PCM（None = 未供給）
    pub route: MonitorRoute,
}

pub struct MonitorOutput {
    pub pcm: Vec<Vec<f32>>,       // scratch の複写（非 RT 呼出し側へ）
    pub effective_route: MonitorRoute,  // input missing 降格を反映
    pub notices: MonitorOutputNotices,  // input_missing 等
}
```

- route → gain 目標: `Input` → input 1.0/playback 0.0、
  `Playback` → その逆、`Muted` → 両方 0.0。
- 各 sample で gain を目標へ `1/ramp_frames` ずつ線形に進める
  （`ramp_frames == 0` は即時）。遷移 block では 2 source の
  gain が同時に中間値を取る crossfade — DSP 限定の ramp であり、
  Gate の sample 精度の代替ではない（gate 開閉の正確さは
  recording 側 `evaluate_gate` が担う）。
- `route == Input && input.is_none()` → effective `Muted` +
  `input_missing` notice。捏造 PCM を出さない（fail-closed）。
- **Native Placement**: 出力 frame `i` は入力/playback frame `i`
  に対応し、latency・PDC 補正・リサンプルを placement に適用
  しない。latency は `LatencyReport` として別経路で報告する
  （二重補正の構造的排除）。
- `process_block` は確保済み scratch のみを使い、RT 経路でも
  alloc/dealloc しない（本 slice の呼出しは非 RT fixture のみ）。
  出力は `Vec<Vec<f32>>` の複写を返す（非 RT）。

### 2.2 Latency

```rust
pub enum LatencyQuality { Unknown, Estimated, Reported, Measured }
pub enum LatencyBasis { DeviceTimestamp, CallbackReported, RoundTripMeasured }

pub struct LatencyComponent {
    pub frames: u64,
    pub basis: LatencyBasis,
    pub quality: LatencyQuality,
}

pub struct LatencyReport {
    pub input: Option<LatencyComponent>,    // ADC 側
    pub output: Option<LatencyComponent>,   // DAC 側
    pub round_trip: Option<LatencyComponent>,
    /// 補助 reported latency（plugin PDC・callback+reported 等）。
    /// `reported_verified` が true のときのみ算入。
    pub reported: Vec<LatencyComponent>,
    /// reported latency の定義検証済みフラグ。
    pub reported_verified: bool,
    /// latency 系列の Clock Epoch。
    pub clock_epoch: Option<ClockEpochId>,
}

pub struct LatencyResolution {
    pub total_frames: Option<u64>,   // None = Unknown
    pub quality: LatencyQuality,
    pub basis: Option<LatencyBasis>,
    pub clock_epoch: Option<ClockEpochId>,
    /// 定義未検証で除外した reported 構成要素数。
    pub excluded_reported: usize,
}
```

`LatencyReport::resolve()` の規則（この順で一意に決まる）:

1. `round_trip` が存在 → `total = round_trip.frames`、
   `basis = RoundTripMeasured`、`quality = round_trip.quality`。
   `input`/`output`/`reported` は**加算しない**（二重加算禁止。
   個別 reported/PDC を足すと round-trip 実測を二重計上する）。
2. `round_trip` なし → `input + output`（存在分）に、
   `reported_verified == true` の場合のみ `reported` 各要素を
   加算。`reported_verified == false` の reported は除外し
   `excluded_reported` に計上。合計は `checked_add`、
   overflow は `total_frames = None` + `quality = Unknown`。
   `basis` は算入要素の優先順位（`DeviceTimestamp` >
   `CallbackReported` > `RoundTripMeasured` 不在）で代表値を選ぶ。
3. `quality` は算入要素の最弱（`Unknown < Estimated < Reported <
   Measured`）。`quality == Measured` は `clock_epoch.is_some()`
   の場合のみ維持し、未確定なら `Reported` に降格する
   （epoch を表示できない Measured を名乗らない）。
4. 構成要素 0 件 → `total_frames = None`、`quality = Unknown`。

`CallbackReported` basis の構成要素は `reported_verified`
ゲートを通る `reported` vec にのみ現れる想定だが、型上は
`input`/`output`/`round_trip` にも置ける。その場合も basis を
品質評価に反映する（`Reported` 以下に cap しない — basis は
出所、quality は確度として独立に扱う）。

### 2.3 Low Latency Monitor

```rust
pub struct PreparedMonitorRoute {
    pub route_id: u64,
    /// 非 RT で prepare 済みか。
    pub prepared: bool,
    /// 入力 Binding/Output Capability 検証済みか。
    pub input_ready: bool,
    /// 出力先 readiness。
    pub output_ready: bool,
    /// 音色差の明示（true = 差あり。隠さない）。
    pub tone_difference: bool,
}

pub enum LowLatencyReason { NotPrepared, InputUnavailable, OutputUnavailable }
pub enum LowLatencyState { Ready, Unavailable(LowLatencyReason), Degraded }

pub struct LowLatencyMonitor {
    pub state: LowLatencyState,
    pub latency: LatencyResolution,
}

pub fn prepare_low_latency(route: &PreparedMonitorRoute,
                           latency: &LatencyReport) -> LowLatencyMonitor;
```

- `PreparedMonitorRoute` は monitor route 固有の readiness のみを
  持ち、capture/通常 playback/export への参照を型として持たない
  （独立性は構造で保証。実 route 構築・graph 配線は後続 slice）。
- 判定順: `!prepared` → `Unavailable(NotPrepared)`、
  `!input_ready` → `Unavailable(InputUnavailable)`、
  `!output_ready` → `Unavailable(OutputUnavailable)`、
  `tone_difference` → `Degraded`、他 → `Ready`。
- `latency` は状態に依らず resolve して保持する（Unavailable
  の理由と latency 品質を別々に観測できる）。

## File Map

- `crates/synvelis-recording/src/input.rs`: `MonitorState` に
  `Auto` 追加（doc 更新）。
- `crates/synvelis-recording/src/monitor.rs`（新規）: §1.2–1.4。
- `crates/synvelis-recording/src/lib.rs`: `monitor` module 公開 +
  export 更新。
- `crates/synvelis-recording/tests/monitor_gate.rs`（新規）:
  decide 行列・MonitorProbe・Raw Byte 独立比較・Punch 途中境界
  Gap/重複 0・Loop/Tempo 二重補正 0・Hardware Direct notice・
  Mono/Stereo。
- `crates/synvelis-player/src/monitor.rs`（新規）: §2.1–2.3。
- `crates/synvelis-player/src/lib.rs`: `monitor` module 公開 +
  export 更新。
- `crates/synvelis-player/tests/monitor.rs`（新規）: gain ramp・
  native placement・route 別 PCM・input missing fail-closed・
  latency resolve 行列・low latency 状態。

## Risk Register

| risk | 対応 |
| --- | --- |
| monitor 判定が gate ロジックを複製して乖離 | `evaluate_gate`/`GateScheduler` を唯一の権威として再利用。probe は piece の補完のみ |
| `MonitorState::Auto` 追加で既存 match が壊れる | 使用箇所は recording crate 内のみ（grep 確認済み）。網羅 match なし |
| Mute/Solo を Input route に誤適用 | pre-fader monitor として文書化し、decide で Input route に作用させない実装 + テストで固定 |
| Round-trip + 個別 reported の二重加算 | resolve の第 1 規則で round_trip 存在時は他を加算しない構造 + fixture で合計値を検証 |
| Monitor On + Arm Off で録音開始と誤認 | `arm` と `monitor` を独立フィールドのまま維持し、decide が Recording 開始を一切行わないことをテストで固定 |
| player adapter が RT graph に未接続のまま「実装済み」と誤認 | 非 RT adapter と明記し、graph/transport 配線は Deferred として文書化 |
| fixture Pass を実 Device Pass と誤認 | spec/design/test-spec/PR に fake/fixture 区分と NotRun 一覧を明記 |

## Verification Matrix

| 検証 | 手段 |
| --- | --- |
| fmt/lint/check | `mise run fmt-check` / `lint` / `check` |
| 単体/統合 | `mise run test`（recording `tests/monitor_gate.rs`、player `tests/monitor.rs` 追加） |
| RT noalloc | `mise run test-rt`（monitor probe/adapter の処理部は alloc-free 設計。既存 RT 契約を維持） |
| build | `mise run build` |
| 実 OS/Device/GUI | NotRun（実機 gate・後続 slice） |
