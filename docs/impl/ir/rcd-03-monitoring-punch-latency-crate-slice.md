---
id: "IMPL-034V6MHZ91AdEfrxqsFgLC"
type: "impl"
status: "completed"
title: "RCD-03 Monitoring/Punch/Latency 意味論 crate 内完結 slice 実装記録"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/src/monitor.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/monitor_gate.rs"
      - type: "file"
        path: "crates/synvelis-player/src/monitor.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/monitor.rs"
      - type: "file"
        path: "docs/specs/rcd-03-monitoring-punch-latency.md"
      - type: "file"
        path: "docs/designs/rcd-03-monitoring-punch-latency.md"
      - type: "file"
        path: "docs/plans/rcd-03-monitoring-punch-latency.md"
      - type: "file"
        path: "docs/test-specs/rcd-03-monitor-punch-latency.md"
      - type: "file"
        path: "docs/tasks/rcd-03-task1-monitor-punch-latency.md"
      - type: "file"
        path: "docs/impl/ir/rcd-03-monitoring-punch-latency-crate-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/src/input.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/rcd-03-task1-monitor-punch-latency.md"
    - "docs/plans/rcd-03-monitoring-punch-latency.md"
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/rcd-03-task1-monitor-punch-latency.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-01-audio-device-headless-slice-fake-port-streamwriter-fixture-recording-stat.md"
    - "docs/impl/ir/rcd-02-project-library-adapter-verified-sha-256-mediareferenceprobe-saved-head.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
    - "docs/designs/rcd-03-monitoring-punch-latency.md"
    - "docs/plans/rcd-03-monitoring-punch-latency.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-03-monitor-punch-latency.md"
  verified-by:
    - "docs/test-specs/rcd-03-monitor-punch-latency.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-03 Monitoring/Punch/Latency 意味論 crate 内完結 slice 実装記録

## Summary

Issue #117（RCD-03 Monitoring / Punch / Latency UX）のうち
`crates/synvelis-recording` と `crates/synvelis-player` 内で完結する
crate 層 slice を実装した（main へ PR #214 として merge 済み）。
`synvelis-recording` に Monitor `Off`/`Auto`/`On` × Arm ×
Transport 相 × Punch/Record Gate × Mute/Solo の可聴 route 判定
（`decide`）と、gate 権威を再利用する非 RT `MonitorProbe`、
Hardware Direct Monitor の状態・notice 型を追加した。
`synvelis-player` に非 RT の `MonitorAdapter`（gain ramp 付き
route 別 PCM 混合・native placement・input 欠落 fail-closed）と
`LatencyReport`/`LatencyResolution`（品質・Clock Epoch・
二重加算禁止）、low-latency route の Ready/Degraded/Unavailable
判定を追加した。GUI/app 統合・実 Device/GUI 実測は対象外であり、
fake port / Fixed PCM fixture 上の契約検証のみを行った。
本記録は spec の In Scope に含まれる impl record が PR #214 で
未作成だったため、merge 済み実装を正本として後追いで補完した
ものである。

## Context

- spec `docs/specs/rcd-03-monitoring-punch-latency.md`
- design `docs/designs/rcd-03-monitoring-punch-latency.md`
- plan `docs/plans/rcd-03-monitoring-punch-latency.md`
- test-spec `docs/test-specs/rcd-03-monitor-punch-latency.md`
- task `docs/tasks/rcd-03-task1-monitor-punch-latency.md`

Issue #117 の crate 層受入条件（Off/Auto/On × Arm × Transport ×
Punch × Mute/Solo の可聴 route・Raw File Byte 独立比較、Hardware
Direct Unknown/二重監視 notice・Input Mono/Stereo、Punch 途中境界
の Gap/重複 0・Loop/Tempo での Raw Placement 二重補正 0、
player monitor adapter の gain ramp/native placement/route 別
PCM/fail-closed、latency 品質・Clock Epoch・二重加算禁止）を
headless fixture でカバーする。`synvelis-app`/`synvelis-workspace`/
`synvelis-project`/Ruviform は変更していない。

## Implementation

### Changes

- `crates/synvelis-recording/src/input.rs`: `MonitorState` に
  `Auto` を追加し Off/Auto/On の宣言モード意味を doc comment で
  固定。Monitor は引き続き capture/Writer と独立した binding
  状態のみを保持する。
- `crates/synvelis-recording/src/monitor.rs`（新規）:
  `TransportPhase`・`MonitorRoute`（Input/Playback/Muted）・
  `HardwareDirectState`（DeviceControlled/UserReported/Unknown）・
  `HardwareDirectMonitor`・`TrackMonitorContext`・
  `MonitorNotices`（double_monitoring/hardware_direct_unknown/
  input_unavailable）・純粋関数 `decide`・非 RT `MonitorProbe`
  を実装。`MonitorProbe` は `PreparedRecordingPlan` の
  `GateScheduler` を共有参照し、block ごとに `GateSchedule` を
  extend → `evaluate_gate` で open piece を得て閉区間を補完し、
  session-frame 精度で route を分割する（block 途中の Punch
  境界を正確に分割・segment は連続非重複の半開区間）。
  `evaluate_gate`/range 構築の契約違反は fail-closed（`faulted`）。
- `crates/synvelis-recording/src/lib.rs`: `monitor` module 公開 +
  re-export。
- `crates/synvelis-recording/tests/monitor_gate.rs`（新規）:
  `decide` 行列（Off の gate-open Muted・Auto の Stopped/gate-open
  Input・On の input_output_ready 要件・Mute/Solo が Playback
  route のみに作用・Hardware Direct notice が外部事実として
  独立）・`MonitorProbe` の block 内 Punch 分割・Loop pass ごとの
  gate open・Raw File Byte/span/PCM が Monitor Off/Auto/On で
  一致（Normal/Punch・stereo/mono binding）・Arm Off + Monitor
  Auto で Writer 未生成・tempo 境界での placement 二重補正なし
  を fake port fixture で検証。
- `crates/synvelis-player/src/monitor.rs`（新規）:
  `MonitorAdapter`（route 別 input/playback PCM を frame ごとの
  線形 gain ramp で crossfade。`ramp_frames == 0` は即時切替。
  出力 frame i は入力 frame i に対応する native placement で
  latency/PDC 補正・オフセットを掛けない。gain は frame あたり
  1 回だけ進め channel 数で ramp が速くならない。`route == Input`
  で input PCM 未供給なら `Muted` へ fail-closed + notice、
  PCM を捏造しない）・`LatencyQuality`
  （Unknown<Estimated<Reported<Measured）・`LatencyBasis`
  （DeviceTimestamp/CallbackReported/RoundTripMeasured）・
  `LatencyComponent`/`LatencyReport`/`LatencyResolution`・
  `prepare_low_latency`（Ready/Degraded/Unavailable + reason、
  latency は状態に依らず保持）を実装。
- `crates/synvelis-player/src/lib.rs`: `monitor` module 公開 +
  export。
- `crates/synvelis-player/tests/monitor.rs`（新規）: native
  placement の bit 一致・route 別 PCM・Muted silence・input
  missing fail-closed・gain ramp 軌跡・channel 非依存 ramp・
  round-trip 実測の authoritative 合計（二重加算なし・reported
  除外計上）・verified reported の合算・未検証 reported の除外・
  Measured の Clock Epoch 未確定時 Reported 降格・epoch 混在時の
  epoch 不採用・空報告 Unknown・overflow fail-closed・
  low-latency Unavailable 理由優先順・Ready/Degraded を Fixed
  PCM/Frame fixture で検証。
- `docs/` チェーン 5 文書（spec/design/plan/test-spec/task）+
  各 README index 追加（PR #214 で実施済み）。

### Decisions During Implementation

- Punch/Record Gate 開閉の権威は `crate::gate` の
  `GateScheduler`/`evaluate_gate` のみに置き、`MonitorProbe` は
  同一 scheduler/schedule を駆動して piece から閉区間を補完する
  設計とした。gate ロジックの複製を避け、block 途中境界でも
  engine と同一の session 位置進行則（`session_end` 飽和）で
  分割する。
- `decide` は純粋関数とし、Monitor 状態は raw capture
  （Capture Gate/Writer）に一切触れない。Monitor Off で Raw
  Capture を停止しない・Monitor On だけで録音を開始しない・
  Raw Placement に二重補正を掛けないことを、Raw File
  Byte/span/PCM の fixture 一致で独立に検証した。
- Mute/Solo は `Playback` route の可聴性のみに作用させ、
  `Input` route と raw capture を不変に保つ（pre-fader
  monitor）。
- Hardware Direct Monitor は外部事実として入力に取り、DAW の
  Mute/Solo や route 判定でその値を変更しない。`Unknown` は
  制御可能と主張せず `hardware_direct_unknown` notice を常時
  立て、Software Input 可聴 + 外部監視観測/申告時のみ
  `double_monitoring` を立てる。
- `MonitorAdapter` の gain ramp は DSP の crossfade に限定し、
  Gate の sample 精度の代替にしない。native placement は
  latency/PDC 補正を placement に適用しない契約として、
  ramp 無し時の出力が入力 PCM と bit 一致することを検証した。
- `LatencyReport::resolve` は `round_trip` 存在時にその frames
  のみを合計とし input/output/reported を再加算しない
  （round-trip + 個別 reported/PDC の二重加算禁止）。
  `reported` は `reported_verified == true` の場合のみ合計へ
  算入し、未検証分は `excluded_reported` に計上する。
  `Measured` は算入要素の Clock Epoch が一意に確定した場合のみ
  維持し、未確定・混在時は `Reported` へ降格する。合算は
  checked_add で overflow 時 `Unknown` に fail-closed する。
- review-protocol で spec の channel 数非依存の主張が stereo
  のみで検証されていると指摘され、fixture helper を
  channel 数・ChannelMap 指定可能に一般化して mono binding の
  case を追加した（PR #214 の修正 commit）。

## Related Experiments

### Adopted

<!-- 実験 log なし（既知解の機械的実装 + fixture 作成）。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク）:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check` — Pass
- `mise run lint` — Pass
- `mise run test` — Pass（`monitor_gate.rs`・`monitor.rs` の
  新規 test を含む）
- `mise run test-rt` — Pass
- `mise run build` — Pass

（PR #214 merge 時の検証結果に加え、本記録追加時に最新
`origin/main` で再検証した。）

NotRun / Unsupported（test-spec/spec 宣言どおり、fake/fixture
は以下を証明しない）:

- 実 OS/Device/GUI での monitor・latency 実測（#115 実 Input・
  #116 Writer/Recovery・#118 Take/Comp・#114 GPUI Arm/Monitor/
  Punch UI の同 Case ID 別 Oracle 実測）— NotRun（実機 gate・
  後続 slice）
- `synvelis-app` Arm/Monitoring GUI UX・多 Window/Focus 受入・
  RT graph への monitor node 搭載・transport control 配線・
  low-latency route の実 Prepared Route 構築・project/library
  への monitor/latency 状態投影 — 後続 slice（本 slice は
  非 RT adapter の契約と fixture 検証のみ）
- Mock 録音を実入力 PASS としない区分を維持（fake port/fixture
  の検証を実 Device/実 GUI Pass に昇格させない）

## Risks

- `MonitorProbe` は `evaluate_gate` の piece を閉区間補完で
  分割するが、transport 相の block 内変化は本 slice の対象外
  （`feed` の `phase` は block 内で一定と仮定）。実 transport
  配線時に block 内相変化が必要になれば別 slice で拡張が必要。
- `LatencyReport` の各 `LatencyComponent` は呼出し側が構築する
  入力であり、ADC/DAC timestamp の実取得・callback timestamp +
  reported latency の定義検証自体は device/driver 層の責務。
  本 crate は与えられた報告の品質区分と二重加算禁止の解決則
  のみを所有する。
- `MonitorAdapter` は非 RT 前提であり、RT graph 搭載・実
  device への monitor 出力経路は未接続。fixture の PCM 一致は
  実聴・実 latency を証明しない。

## Follow-ups

- Issue #117 残件: `synvelis-app` Arm/Monitoring GUI UX・
  #110〜112 GUI Shell 連携・多 Window/Focus 受入（app/workspace
  crate は別 dispatch が占有中のため後続 slice）、実
  Device/Capture/Software Monitor/GUI 実測（実機 gate）、
  project/library への monitor/latency 状態投影、RT graph
  への monitor node 搭載と low-latency route の実 Prepared
  Route 構築。
