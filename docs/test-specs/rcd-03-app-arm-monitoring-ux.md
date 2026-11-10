---
id: "TSPEC-034VedrLB13w3phP9Xuh5S"
type: "test-spec"
status: "proposed"
title: "RCD-03 app Arm/Monitoring UX fixture matrix"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-03-app-arm-monitoring-ux.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-03-app-arm-monitoring-ux.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 app Arm/Monitoring UX fixture matrix

## Purpose

`syn.monitoring.*` ops と `syn.monitoring.state` 投影が、crate 層の
`decide`/`MonitorProbe`/`prepare_low_latency` 契約を偽らず投影する
ことを headless fixture で固定する。実 Device/実 GUI/multi-window の
受入は本 spec の対象外（外部 gate、NotRun）。

## Feature

Monitor Arm/Monitoring UX state machine — `syn.monitoring.set` /
`punch` / `transport` / `session` / `latency` / `state` の op 列が、
宣言・位相・gate・mute/solo・外部観測から可聴 route と notice を
投影する。

## Rules

- **R1 route は decide のみが決める**: `syn.monitoring.state` の
  track route は `synvelis_recording::monitor::decide` の返り値と
  一致し、app 独自の route 判定を持たない。
- **R2 session gate は preflight 産物**: `syn.monitoring.session`
  start は `preflight` を通過した `PreparedRecordingPlan` のみを
  保持し、segments は `MonitorProbe` の出力のみを投影する。
  punch gate は半開 `[in, out)`。
- **R3 Arm と Monitor は独立**: Arm Off でも Monitor 宣言・route
  投影は成立し、Arm Off が Writer/capture を捏造しない
  （app 層に capture 経路は存在しない）。
- **R4 Hardware Direct は外部観測**: `Unknown` は
  `hardware_direct_unknown` notice を立て、state を
  `DeviceControlled` と偽らない。`observed_active == Some(true)` かつ
  route `Input` で `double_monitoring`。
- **R5 unavailable は正直**: `input_output_ready == false` の
  Monitor On は `input_unavailable` notice + Playback フォールバック。
  session 未開始・latency 未注入は既定の unavailable 投影。
- **R6 window/focus は gate に影響しない**: view attach/detach・
  window・focus 系 op の前後で `syn.monitoring.state` が不変。
- **R7 session は project に従属しない**: `project.new`/`open` で
  宣言・session・punch・latency が reset される。
- **R8 投影は bounded**: segments は track あたり cap・track 数 cap
  を持ち、打ち切りは `truncated`/`tracks_omitted` で明示。

## Examples

- **E1 Off/Auto/On 行列**: 同一宣言 track で monitor mode を
  Off→Auto→On と変え、arm on/off・transport play/stop・punch
  in/out・strip mute/solo を組み合わせたとき、state の route が
  decide の判定表（Off: gate open+arm→Muted else Playback;
  Auto: arm&&(gate open||Stopped)→Input else Playback;
  On: ready→Input else Playback+notice）と一致する。
- **E2 punch 境界**: punch `[in,out)` 設定の session で segments が
  `[...,gate_open=false] → [gate_open=true] → [gate_open=false]` の
  遷移を半開境界で示し、`out` ちょうどの位置は閉側に属する。
- **E3 Hardware Direct**: `state:"unknown"` 宣言で
  `hardware_direct_unknown` が立ち `observed_active` は `null`。
  `state:"user_reported", observed_active:true` + Monitor On +
  ready で `double_monitoring`。`device_controlled` はそのまま
  投影される。
- **E4 input unavailable**: Monitor On + `input_output_ready:false`
  で `input_unavailable` + route Playback。
- **E5 window/focus 不変**: session+宣言済み状態で
  `syn.view.attach`/`detach`・`syn.window.*`・focus 系 op を実行し、
  `syn.monitoring.state` の JSON が実行前と一致する。
- **E6 latency**: `syn.monitoring.latency` に round_trip component
  （measured, epoch 付き）を注入すると state の latency.resolution
  が `total_frames`/`quality:"measured"`/`clock_epoch` を投影し、
  readiness 未注入なら low_latency は `unavailable/not_prepared`。
- **E7 session 未開始**: 宣言のみで session 未開始の state は
  `session.active:false`、tracks の静的宣言は投影されるが
  segments は空、route は `decide(ctx, Stopped, false)` の結果。

## Guarantees

- `syn.monitoring.state` は決して route/notices/latency を捏造
  しない — 未注入・未開始は明示的な既定投影。
- `syn.monitoring.*` のいかなる op も `Project`/`ProjectController`
  を変更しない（session-only）。
- 既存の `syn.*` ops・`syn.recording.info` の出力は不変。

## Non-goals

- 実 Device/Capture/Software Monitor の可聴計測、実 GPUI GUI・
  multi-window/focus 受入、AudioLens — 外部 gate（NotRun）。
- `MonitorAdapter` PCM ミックス、RT graph monitor node。
- Loop/PunchInLoop gate、count-in/pre-roll UX。
- 永続化・Undo 対象化。

## Risk

- route 投影が decide と乖離すると「見えている route」と実音が
  食い違う。fixture が decide 行列を op 経由で直接検証することで
  乖離を CI で検出する。
- session gate が preflight と乖離すると punch 境界がずれる。
  segments を MonitorProbe 産物のみに限定することで、gate 意味論の
  単一所有者（recording crate）を維持する。
