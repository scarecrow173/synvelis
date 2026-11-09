---
id: "SPEC-034UtksmGxvdygOzI1Dp7R"
type: "spec"
status: "proposed"
title: "RCD-03 Monitoring / Punch / Latency semantics — synvelis#117 slice"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-01-recording-input-adapter-channel-binding-arm-monitor-fake-port-fail-stop-s.md"
    - "docs/specs/vs-b-deterministic-mock-capture-engine.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/118"
  defers: []
  deferred-by: []
---

# RCD-03 Monitoring / Punch / Latency semantics — synvelis#117 slice

## What Should Be Built

Issue #117（Monitoring / Punch / Latency UX）のうち、
`crates/synvelis-recording` と `crates/synvelis-player` 内で完結する
製品意味論 slice を実装する。

- `synvelis-recording`: Monitor `Off`/`Auto`/`On` × Record Arm ×
  Transport 相 × Punch/Record Gate 開閉 × Track Mute/Solo に対する
  可聴 route 判定（`Input`/`Playback`/`Muted`）を、既存
  `GateScheduler`/`evaluate_gate` を再利用した非 RT probe として
  提供する。Hardware Direct Monitor の
  `DeviceControlled`/`UserReported`/`Unknown` 状態と、Software +
  Hardware 二重監視・未知状態の notice を型として保持する。
  Monitor 状態は raw capture（Capture Gate/Writer）に一切影響
  しないことを fixture で独立比較する。
- `synvelis-player`: 非 RT の Monitor/Latency/Playback 切替
  Adapter を追加する。Monitor Gain Ramp（DSP 限定・Gate の
  sample 精度の代替にしない）、Native Placement（latency 補正を
  placement に適用しない）、Software Monitor Latency の
  Measured/Reported/Estimated/Unknown 品質区分と Clock Epoch 保持、
  Round-trip 実測と個別 Reported/PDC の二重加算禁止を、Fixed
  PCM/Frame Fixture で検証する。

## Why Now

Issue #117 は Monitoring/Punch/Latency の製品契約を定める。
RCD-01（#115、入力 adapter・Arm/Monitor 独立状態）と RCD-02
（#116、durability/recovery）は main に実装済みで、本 slice は
その上に monitor 意味論と player 側の monitor/latency adapter を
載せる。GUI/app 統合は別 dispatch が `synvelis-app`/`synvelis-workspace`
を編集中のため本 slice の対象外とし、crate 内契約と fixture 検証に
限定する。

## Users and Value

- 将来の統合層（synvelis-app Arm/Monitoring UX・#114 GUI
  Scenario）: Off/Auto/On の可聴 route・Hardware Direct 状態・
  latency 品質を再利用可能な契約として得る。
- 検証者: Fixed PCM/Frame Fixture で可聴結果と Raw File Byte を
  独立比較し、Punch 境界の Gap/重複 0・Raw Placement 二重補正 0
  を実測できる。Mock/fake の結果を実 Device/GUI Pass に昇格
  させない区分を維持できる。

## Scope

### In Scope

- `MonitorState` の `Off`/`Auto`/`On` 3 値化と、Arm/Transport/
  Gate/Mute/Solo を入力とする可聴 route 判定 API
  （`synvelis-recording`）。
- Punch/Record Gate の開閉区間を `evaluate_gate` の piece から
  再構成し、block 途中の境界でも route を session-frame 精度で
  分割する非 RT `MonitorProbe`。
- `HardwareDirectMonitor`（`DeviceControlled`/`UserReported`/
  `Unknown` + 外部観測 active）と `MonitorNotice`
  （`DoubleMonitoring`/`HardwareDirectUnknown`/`InputUnavailable`）
  の型と判定。
- Punch In/Out が block 途中に来る場合の Gap/重複 0、Loop/Tempo
  区間での Raw Placement 二重補正 0 を検証する fixture test
  （`synvelis-recording`）。
- `synvelis-player` の非 RT monitor adapter: route 別 PCM 混合、
  Monitor Gain Ramp、Native Placement、monitor PCM 欠落時の
  fail-closed、`LatencyReport`/`LatencyResolution`（品質・
  Clock Epoch・二重加算禁止）、Low Latency Monitor route の
  Ready/Unavailable/Degraded 状態。
- `docs/` チェーン（spec/design/plan/task/test-spec/impl record）。

### Out of Scope

- `synvelis-app`/`synvelis-workspace` の Arm/Monitoring GUI UX・
  dock/window/focus 連動 — 別 dispatch が編集中。多 Window/Focus
  変化の GUI 側受入は後続 slice。
- `synvelis-project` の変更 — 別 dispatch が tests を実装中。
  project/library 統合 adapter は NotRun。
- 実 Device/Capture/Software Monitor/GUI での実測（#115 実 Input・
  #116 Writer/Recovery・#118 Take/Comp・#114 GPUI Arm/Monitor/
  Punch UI の同 Case ID 別 Oracle 実測）— NotRun（実 OS/Device/
  Backend gate）。fake/mock の検証を実入力 PASS としない。
- Ruviform 側の Device Duplex/Generic Clock/RT/Plugin Host の
  複製・変更。
- RT graph への monitor node 搭載・transport control 配線
  （後続 integration slice）。
- Software Monitor Off で Raw Capture を停止すること、
  Hardware Direct 監視を DAW が制御可能と偽ること — 禁止事項
  として型・文書で固定する。

## Requirements

- Monitor `Off`: Software Input Off。通常は既存 Playback を可聴
  route とし、Record/Punch Gate Open 区間では当該 track の既存
  Playback を抑制（`Muted`）する。
- Monitor `Auto`: Arm 有効時、Transport 停止中は Input、
  Playback/Count-in/Pre-roll/Punch 待機/Post-roll は既存
  Playback、Gate Open 区間は Input。Arm Off 時は Input 無効
  （Playback 経路にフォールバック）。
- Monitor `On`: 入力 Binding/Output Capability が有効なら
  Arm なしでも Software Monitor 可（`InputUnavailable` notice で
  未検証を偽らない）。Recording は Arm に従い、Monitor On だけで
  開始しない。同 track 既存 Playback は原則抑制。
- Track Mute/Solo は Playback route の可聴性のみに作用し、
  Software Input route と raw capture に影響しない（pre-fader
  monitor として明示）。
- Capture Gate（Raw Writer）と Input Monitor の音量/FX/PDC/
  Mute/Solo/Hardware Direct は独立。Monitor 状態の如何に関わらず
  Raw File Byte は同一でなければならない（fixture で独立比較）。
- Punch は `[in, out)`。Count-in/Pre/Post Roll は初期既定で
  未 Gate の隠れた PCM を録音しない（既存 gate 契約を維持）。
  Monitor Gain Ramp は DSP に限定し、Gate の sample 精度の
  代替にしない。
- Hardware Direct Monitor は `DeviceControlled`/`UserReported`/
  `Unknown` を区別して保持し、制御不能な場合に DAW の
  Mute/Solo で止まったと宣言しない。Software Input route が
  可聴で外部監視が観測/申告されている場合は `DoubleMonitoring`
  notice を出す。
- Latency は ADC/DAC 実基準 timestamp を優先し、callback
  timestamp + reported latency は定義検証済みの場合のみ補助
  利用する。Round-trip 実測と個別 Reported Latency/PDC の
  二重加算を行わない。品質（Measured/Reported/Estimated/
  Unknown）と Clock Epoch を報告に保持する。
- Low Latency Monitor は非 RT の Prepared Route を独立に検証し、
  `Unavailable`/音色差（`Degraded`）を明示する。Capture/通常
  Playback/Export を無断変更しない。
- fake/mock/fixture の検証結果を実 Device/実 GUI Pass と
  偽装しない。実測不能な OS/Device/Backend 組合せは
  NotRun/Unsupported として記録する。

## Acceptance Criteria

- [ ] Off/Auto/On × Arm × Transport × Punch × Mute/Solo の
      可聴 route 判定が fixture で網羅され、同一入力系列に対する
      Raw File Byte が Monitor 状態に依らず独立に一致する
      （`synvelis-recording` fixture test）。
- [ ] Hardware Direct `Unknown`/外部二重監視の notice が型として
      区別され、Input Mono/Stereo binding で monitor 判定が
      channel 数に依らず成立する（crate 意味論層）。
- [ ] Punch In/Out が block 途中に来ても Gap/重複 0 で、Loop/
      Tempo 変化区間で Raw Placement（session/source frame・
      project tick 写像）に二重補正がないことを fixture で検証
      する。
- [ ] `synvelis-player` の monitor adapter が Fixed PCM/Frame
      Fixture で Monitor Gain Ramp・Native Placement・route 別
      PCM 出力・monitor PCM 欠落時の fail-closed を検証する。
- [ ] Software Monitor Latency が Measured/Reported/Estimated/
      Unknown と Clock Epoch を区別し、Round-trip 実測 + 個別
      Reported/PDC の二重加算を行わないことを fixture で検証
      する。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `build` が全て Pass する。

## Deferred Design Concerns

- synvelis-app Arm/Monitoring GUI UX・多 Window/Focus 受入・
  #110〜112 GUI Shell 連携 | 別 dispatch が app/workspace crate
  を編集中でスコープ外 | app 側 dispatch 完了後 | crate 契約と
  GUI 表示の齟齬
- 実 Device/Capture/Software Monitor/GUI 実測（#115/#116/#118/
  #114 の同 Case ID 別 Oracle）| 実 OS/Device/Backend gate |
  実機環境整備後 | fake/fixture 検証を実入力 PASS と誤認する
  リスク（文書で明記して抑止）
- RT graph への monitor node 搭載・transport control 配線・
  Low Latency Monitor の実 Prepared Route 構築 | 本 slice は
  非 RT adapter の契約のみ | integration slice | adapter と
  graph/RT 経路の接続が未検証のまま残る
- project/library への monitor/latency 状態投影 | project 側
  dispatch 稼働中・スコープ外 | project dispatch 完了後 |
  製品状態への反映経路が未接続

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/117>
  （RCD-03 Monitoring / Punch / Latency UX）
- 前提契約: RCD-01 spec/design（input adapter・Arm/Monitor
  独立状態・fake port）、RCD-02 spec/design（durability/
  recovery review）、VS-B（deterministic mock capture engine・
  Gate schedule）、#106 SF-03（Routing/PDC 契約）、#12 §16
  （製品設計正本）
- 関連 Issue: #114（実 GUI/OS Scenario 受入）、#115（実 Input）、
  #116（Writer/Recovery）、#118（Take/Comp）— いずれも統合・
  受入 gate であり本スライスの着手条件ではない
