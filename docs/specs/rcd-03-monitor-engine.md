---
id: "SPEC-034WCqwruKIjlaayYRfenJ"
type: "spec"
status: "proposed"
title: "RCD-03 MonitorAdapter engine mount + Loop/PunchInLoop gate slice — synvelis#117"
created: "2026-11-13"
updated: "2026-11-13"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
    - "docs/specs/rcd-03-vcs-diff-monitor-classification.md"
    - "docs/specs/rcd-03-monitor-persistence.md"
    - "docs/specs/rcd-03-app-arm-monitoring-ux.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  defers: []
  deferred-by: []
---

# RCD-03 MonitorAdapter engine mount + Loop/PunchInLoop gate slice — synvelis#117

## What Should Be Built

Issue #117 の統合コメント残存欄で名指しされた headless 到達可能
slice のうち、本 spec が扱うのは次の 2 件のみ（Issue は Open
継続。PR では `Refs #117` を使い `Closes` にしない）。

- **MonitorAdapter の engine 搭載**: `synvelis-player` に、
  recording 側 `MonitorProbe`（gate 観測）と `MonitorAdapter`
  （PCM 制御）を合成し、headless engine driver の実 block 経路
  に monitor bus を載せる `MonitorSession`/`MonitorEngine` を
  実装する。Off/Auto/On × Arm × Transport × Punch × Mute/Solo
  の組合せ結果を headless fixture で pin する。
- **Loop/PunchInLoop gate**: `GateSpec::Loop`/`PunchInLoop` の
  Punch In/Out・Loop 区間境界（block 途中を含む）の Gap/重複 0・
  Raw Placement 二重補正 0 を fixture で検証する。高 Latency
  FX・Tempo 区間の組合せは fixture で扱える範囲を pin する。

## Why Now

PR #214 で `decide`/`MonitorProbe`/`MonitorAdapter`/`LatencyReport`
の crate 内契約は実装済みだが、`MonitorAdapter` は誰からも
構築・駆動されない未接続状態（dead code）であり、Loop/
PunchInLoop gate の monitor 経路検証も未実施。engine の実
block 経路（`rt.process` → master tap PCM）に monitor 観測・
制御を組み込むことで、route 判定が実 PCM に対して機能することを
headless で証明する残件を回収する。

## Users and Value

- 将来の統合層（synvelis-app Arm/Monitoring UX・#114 GUI
  Scenario）: engine 実経路に接続済みの monitor bus を再利用
  可能な契約として得る。app 層の Command/状態投影は別残件。
- 検証者: Off/Auto/On × Arm × Transport × Punch × Mute/Solo
  の可聴 route 結果と Loop/PunchInLoop 境界の Gap/重複 0・
  Raw Placement 二重補正 0 を fixture で実測できる。

## Scope

### In Scope

- `crates/synvelis-player`: `MonitorSession`（`MonitorProbe` +
  `TrackMonitorContext` + `MonitorAdapter` + 入力 PCM FIFO +
  `TransportPhase` + segment evidence の合成）、`MonitorEngine`
  （headless engine driver を block 単位で駆動し各 block の
  実 master 出力 PCM を monitor session へ供給する wrapper）、
  `EngineBlocks` trait（`HeadlessDriver`/`RoutingEngine` の実
  run 経路を抽象化）。`PreparedRecordingPlan` に構築時
  `sample_rate` を保持させ、engine sample rate との一致を
  構築時検証する（`crates/synvelis-recording`）。
- `crates/synvelis-recording/tests/monitor_gate.rs`: Loop/
  PunchInLoop gate の probe segment・Raw Byte 独立比較・
  Tempo 境界での project mapping 検証の追加。
- `crates/synvelis-player/tests/monitor_engine.rs`（新規）:
  engine 実経路に載せた monitor bus の route 別 PCM・
  block 途中境界 Gap/重複 0・Loop/PunchInLoop pass 境界・
  高 Latency FX/Tempo 組合せの fixture 検証。
- `docs/` チェーン（本 spec/design/plan/task/test-spec/impl
  record）。

### Out of Scope

- `synvelis-app`/`synvelis-workspace`/`synvelis-project` の
  変更（app Command/状態投影・project schema は別残件）。
- 実 Device/Capture/Software Monitor/GUI での実測・実 GUI
  受入行（#114 Scenario・別 Oracle）— NotRun。
- RT graph への monitor node 搭載・transport control 配線・
  実 Prepared Route 構築（in-graph RT processor 化は別
  integration slice）。本 slice の monitor bus は engine 実
  経路の非 RT 観測・制御合成であり、RT callback 内では
  動かない。
- 実入力 device からの monitor PCM 供給（fixture は staging
  した PCM block を供給する。実 device monitor 入力経路は
  後続 slice）。
- count-in UX・AudioLens 等の #117 残件。
- `Cargo.lock`・`mise.toml`・`.github/`・app/UI 層。

## Requirements

- `MonitorSession` は `PreparedRecordingPlan` の `GateScheduler`
  を共有する `MonitorProbe` で gate 観測を行い、
  `TrackMonitorContext` + `TransportPhase` + `gate_open` から
  `decide` が返す `MonitorRoute` を block 内 session-frame
  精度で segment 分割する（gate ロジックの第二実装を作らない）。
- `MonitorEngine` は engine driver を 1 block ずつ実 run 経路
  （`rt.process` + pump policy + transport 観測）で駆動し、
  各 block の実 master 出力 PCM を playback 側として monitor
  session に供給する。monitor 出力は engine の通常出力・
  raw capture・export を変更しない独立 bus とする。
- engine frame ↔ session frame は 1:1（sample rate 一致を構築
  時に検証）。`MonitorEngine` は attach 時の engine cursor を
  frame offset として保持し、以後の block で session 位置との
  ずれを検出した場合は fail-closed（無音 + faulted）とする。
- `TransportPhase` は block 単位で caller が供給する（block 内
  相変化は対象外 — `MonitorProbe::feed` と同一契約）。
- 入力 PCM は caller が block 単位で staging する FIFO。未供給
  の block は `input = None` となり、`route == Input` の
  segment は `Muted` へ fail-closed 降格 + `input_missing`
  notice（PCM を捏造しない）。
- monitor 出力 PCM は segment 連続で frame `i` が playback/
  input frame `i` に対応する native placement を保ち、
  latency/PDC 補正・リサンプル・オフセットを placement に
  適用しない（gain ramp は DSP のみ）。
- segment が block を隙間・重複なく tile しない場合、および
  probe/adapter の契約違反は fail-closed（無音出力 + faulted
  観測可能）とする。
- session 終端を越えた block 区間は Playback 経路へ戻す
  （録音 session 終了後は通常再生が可聴になる製品規則）。
- Loop/PunchInLoop gate で block 途中の Punch In/Out・Loop
  pass 境界が来ても monitor 出力の Gap/重複 0・Raw Placement
  （session/source frame・project tick 写像）二重補正 0 を
  fixture で検証する。高 Latency FX は monitor の native
  placement が playback 経路 latency を二重補正しないこと、
  および `LatencyReport` が reported latency を一度だけ算入
  することを fixture で pin する。Tempo 区間は gate template
  分割と project mapping の対応を fixture で検証する。
- fake/fixture の検証結果を実 Device/実 GUI Pass と偽装しない。

## Acceptance Criteria

- [ ] `MonitorEngine` が `HeadlessDriver`/`RoutingEngine` の実
      run 経路に monitor bus を載せ、headless fixture で
      Off/Auto/On × Arm × Transport × Punch × Mute/Solo の
      組合せ route 結果（PCM・effective route・notice）が
      pin される。
- [ ] Punch In/Out・Loop 区間の境界が block 途中でも monitor
      出力の Gap/重複 0、Raw Placement 二重補正 0 が fixture
      で検証される。`GateSpec::Loop`/`PunchInLoop` の pass ごとの
      gate open 区間が session-frame 精度で正しく route に
      反映される。
- [ ] 高 Latency FX・Tempo 区間の組合せが fixture で扱える範囲
      で pin される（monitor native placement が FX latency を
      二重補正しない・reported latency の一度だけ算入・tempo
      境界での span project mapping）。
- [ ] `PreparedRecordingPlan` が構築 sample rate を保持し、
      engine sample rate との不一致が構築時エラーになる。
- [ ] 追加・変更したテストが `mise run test` で全 pass し、
      canonical 9 lane（fmt-check/check/lint/test/test-rt/
      check-app-native/lint-app-native/test-msrv/build）が
      すべて exit 0。
- [ ] #117 の残件（実 device monitor/latency・実 GUI 受入行・
      count-in UX・AudioLens・RT graph monitor node・app 層
      Command/状態投影との実接続）が PR 本文の「未対応の作業」
      に記載される。

## Deferred Design Concerns

- RT graph への monitor node 搭載・transport control 配線・
  実 Prepared Route 構築 | 本 slice は engine 実経路の非 RT
  観測・制御合成のみ | 実 device/RT 統合 slice | adapter と
  RT 経路の接続が未検証のまま残る
- 実入力 device からの monitor PCM 供給・per-strip pre-fader
  monitor tap | fixture は staging PCM・master tap のみ |
  実 device monitor slice | monitor 入力経路と strip 別 tap が
  未接続
- synvelis-app からの monitor session 構築・transport phase
  導出・状態投影接続 | app 層は別残件（本 slice の編集範囲外）
  | app 側 dispatch | engine monitor bus が app に未接続
- count-in UX・AudioLens 等の #117 残件 | 本 Spec の対象外 |
  後続 slice | Issue #117 が Open 継続

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/117>
  （RCD-03 Monitoring / Punch / Latency UX — 統合コメント残存欄:
  MonitorAdapter の engine 搭載・Loop/PunchInLoop gate）
- 前提 slice: `docs/specs/rcd-03-monitoring-punch-latency.md`
  （decide/MonitorProbe/MonitorAdapter/LatencyReport の crate
  内契約 — PR #214）、`docs/specs/rcd-03-vcs-diff-monitor-
  classification.md`（slice A — PR #318）、
  `docs/specs/rcd-03-monitor-persistence.md`（PR #307）、
  `docs/specs/rcd-03-app-arm-monitoring-ux.md`（PR #269）
- 関連 Issue: #114（実 GUI/OS Scenario 受入）、#115（実
  Input）、#106（Routing/PDC 契約）— いずれも統合・受入 gate
  であり本スライスの着手条件ではない
