---
id: "SPEC-034VdSvFA9X0ESbckrSvBI"
type: "spec"
status: "proposed"
title: "RCD-03 app Arm/Monitoring UX + project projection slice — synvelis#117"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source: []
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
    - "DESIGN-034UtlI2uLUF1lr2g3CUM0"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "SPEC-034UtksmGxvdygOzI1Dp7R"
  refines:
    - "SPEC-034UtksmGxvdygOzI1Dp7R"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 app Arm/Monitoring UX + project projection slice — synvelis#117

## What Should Be Built

`crates/synvelis-app` に、synvelis#117 の残スライスとして、Arm / Monitoring /
Punch の製品 UX 操作と読み取り投影を `syn.*` サービス面に追加する。

- `syn.monitoring.set` — Track 単位の Arm / Monitor mode / Input binding
  (Mono/Stereo/Multichannel) / `input_output_ready` / Hardware Direct
  観測状態の宣言・更新（セッションスコープ、永続化しない）。
- `syn.monitoring.punch` — Punch in/out マーク（project tick）の設定・解除。
- `syn.monitoring.transport` — Monitor セッションの transport 位相
  （play / stop / pause / seek、位置は project tick）操作。
- `syn.monitoring.session` — Monitor セッションの開始・停止（RecordingRequest
  相当のパラメータから `synvelis-recording` の `preflight` で
  `PreparedRecordingPlan` を構築し、Capture Gate / MonitorProbe と同一の
  gate 意味論を共有する）。
- `syn.monitoring.latency` — デバイス報告レイテンシ入力（LatencyReport /
  LowLatencyReadiness / tone-difference）の注入。実デバイス層が後続で供給
  するまでの headless 注入点。
- `syn.monitoring.state` — 上記すべての読み取り投影:
  - セッション位相・位置・gate 状態、
  - track ごとの Arm/Monitor/binding と `decide()` による可聴 route
    （Input / Playback / Muted）と notice（`double_monitoring`、
    `hardware_direct_unknown`、`input_unavailable`）、
  - MonitorProbe による punch 境界をまたぐ route セグメント列
    （bounded、truncate は明示）、
  - 解決済みレイテンシと low-latency readiness。

投影は `synvelis-recording::monitor::decide` / `MonitorProbe` と
`synvelis-player::monitor` の LatencyReport/`prepare_low_latency` を
そのまま消費し、app 側で route/gate 意味論を再実装しない。
プロジェクト集約は単一 writer のまま — Mute/Solo は既存の
strip control target から読み取り、Arm/Monitor 宣言は session-only
（永続化しない）とする。`synvelis-project` の変更は投影に不要と判断し、
行わない。

## Why Now

crate 層の RCD-03 セマンティクス（`decide` / `MonitorProbe` /
`HardwareDirectMonitor` notice、player 側 `MonitorAdapter` /
`LatencyReport` / low-latency readiness）は PR #214 で完了済みだが、
Issue #117 の残件として app 側の UX op・状態投影・project 投影が
未実装のまま残っている。本スライスは headless な app 面の
Arm×Monitor×Transport×Punch 状態機械と投影を先に確定させ、
実 Device/実 GUI/multi-window 受入（外部 gate）の土台とする。

## Users and Value

- GUI ホスト / headless ドライバ / 将来の CLI アダプタが共通で使う
  `syn.*` サービス面の利用者。
- 「Monitor Off でも Raw Capture は止まらない」「Arm と Monitor は独立」
  「Hardware Direct は外部観測であり DAW が制御しない」という製品不変条件を、
  UX 面でも偽らず投影できることが価値。

## Scope

### In Scope

- `crates/synvelis-app/` への `syn.monitoring.*` ops 登録・dispatch・
  `SynvelisApp` 状態（session-scope の monitor 宣言・セッション・
  レイテンシ入力）と `syn.monitoring.state` 投影。
- `synvelis-recording` の `preflight` / `decide` / `MonitorProbe` /
  `TrackMonitorContext` / `TransportPhase` / `MonitorRoute` /
  `HardwareDirectMonitor` の消費（再実装しない）。
- `synvelis-player` の `LatencyReport` / `prepare_low_latency` /
  `LowLatencyReadiness` の読み取り消費（PCM 経路は対象外）。
- Hardware Direct `Unknown` の正直な通知（`hardware_direct_unknown`
  notice）と外部二重監視警告（`double_monitoring` notice）の
  app 状態投影への配線。制御可能と偽らない。
- Input Mono/Stereo（`ChannelMap`）の宣言・投影。
- 多 Window/Focus 変化が monitor/capture-gate 状態に影響しないことの
  headless 検証（view/window/focus op 実行後も `syn.monitoring.state`
  が不変）。
- `crates/synvelis-app/tests/monitor_arm*.rs` の headless fixture。

### Out of Scope

- 実 Device / 実 Capture / 実 Software Monitor の可聴計測（外部 gate、
  NotRun）。
- 実 GPUI GUI・multi-window/focus の受入（#114 系、外部 gate、NotRun）。
- player RT 搻載・`MonitorAdapter` の PCM ミックス経路への統合
  （宣言 scope 外）。`LatencyReport` の解決投影のみ消費。
- #115 実 Input 同 Clock、#116 Writer/Recovery 連携、#118 Take/Comp。
- Arm/Monitor 宣言・Hardware Direct 観測のプロジェクト永続化
  （session-only とし、`synvelis-project` は変更しない）。
- Loop / PunchInLoop gate の app UX（本 slice は Normal / Punch のみ。
  `GateSpec` の Loop 系は後続 slice）。
- Count-in / Pre-roll の UX（位相語彙は `TransportPhase` を再利用するが、
  count-in/pre-roll 区間の宣言 UX は持たない）。

## Requirements

- Arm / Monitor / Punch / transport 位相の遷移は
  `synvelis-recording::monitor::decide` の契約に従う。app は route を
  独自判定しない。
- Monitor セッションの gate は `GateSpec::Normal` / `GateSpec::Punch`
  （半開区間 `[in, out)`）のみ。punch マークが両方揃った場合のみ Punch
  gate、それ以外は Normal（`open = transport_start`）。
- Raw Capture への干渉経路を持たない — monitor 宣言・位相・punch は
  capture / writer / file byte に一切触れない（crate 層で既証明、
  app 層はそもそも capture port を持たない）。
- Hardware Direct `Unknown` は「不明」として通知し、
  `DeviceControlled` / 制御可能とは投影しない。
- `input_output_ready == false` の track は `input_unavailable`
  notice を持ち、route は `Muted` に落ちる（`decide` 契約どおり）。
- `syn.monitoring.state` のセグメント投影は bounded であり、
  上限到達時は `truncated` を明示する。無制限のメモリ・時間消費をしない。
- セッションが無い / project に対象 track/lane/clock epoch が無い場合、
  投影は偽の route やデフォルト値で埋めず、明示的な unavailable /
  empty を返す。

## Acceptance Criteria

- [ ] `syn.monitoring.set` で宣言した track について、Off/Auto/On ×
  Arm × Transport × Punch × Mute/Solo の各組合せで
  `syn.monitoring.state` が `decide` と一致する route
  （Input / Playback / Muted）を投影する（headless fixture）。
- [ ] Punch in/out 設定時、MonitorProbe 由来のセグメント列が
  punch 境界で route を遷移させ、半開区間 `[in, out)` を守る
  （headless fixture）。
- [ ] Hardware Direct `Unknown` で `hardware_direct_unknown` notice が
  立ち、`DeviceControlled` と偽らない。`UserReported` /
  `DeviceControlled` の観測値はそのまま投影される。
- [ ] Hardware Direct 監視 + Software Monitor On の併存で
  `double_monitoring` notice が立つ。
- [ ] `input_output_ready == false` で `input_unavailable` notice と
  `Muted` route が投影される。
- [ ] view attach/detach・focus・window 系 op の実行前後で
  `syn.monitoring.state` が不変（Capture Gate への影響なし）。
- [ ] Monitor セッション未開始時、`syn.monitoring.state` は
  `session.active == false` と宣言済み track の静的投影のみを返し、
  route/segment を捏造しない。
- [ ] `syn.monitoring.latency` に注入した LatencyReport が
  `syn.monitoring.state` で解決済みレイテンシと low-latency
  readiness（`NotPrepared` 優先順位を含む）として投影される。

## Deferred Design Concerns

- 実 Device/実 GUI/multi-window/focus 受入、AudioLens 行 —
  外部 gate（実機・consumer 検証）。本 slice の headless fixture を
  Pass に繰り上げない。
- `MonitorAdapter` PCM 経路・RT graph monitor node — player RT 搭載は
  宣言 scope 外。latency 解決投影のみ本 slice で消費。
- Arm/Monitor 宣言の永続化・Undo 対象化 — session-only が現行契約。
  永続化が必要になった時点で `synvelis-project` 側の投影モジュールを
  再検討する。
- Loop / PunchInLoop gate の app UX — GateSpec は対応済みだが UX
  宣言は未実装。loop 再生 UX が monitor に必要になった時点で追加。

## Sources and Relations

- `SPEC-034UtksmGxvdygOzI1Dp7R`（RCD-03 crate slice spec、本 slice の上流）。
- `DESIGN-034UtlI2uLUF1lr2g3CUM0`（crate slice design: decide/MonitorProbe/
  HardwareDirect 契約の正本）。
- `docs/designs/rcd-03-monitoring-punch-latency.md`、
  `docs/impl/ir/rcd-03-monitoring-punch-latency-crate-slice.md`
  （残件として app UX / project 投影を明記）。
- GitHub issue `scarecrow173/synvelis#117`（Refs。本 spec は Issue を
  Close しない — 実 Device/実 GUI/multi-window 行は外部 gate）。
