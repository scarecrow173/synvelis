---
id: "SPEC-034VpQd7kM2xN9wR4tY6uJ"
type: "spec"
status: "proposed"
title: "RCD-03 Arm/Monitor/Punch Project 永続投影 slice — synvelis#117"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
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
    - "docs/specs/rcd-03-app-arm-monitoring-ux.md"
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 Arm/Monitor/Punch Project 永続投影 slice — synvelis#117

## What Should Be Built

synvelis#117 の残スライスとして、Arm / Monitor / channel_map / lane
宣言と Punch マークを `synvelis-project` の永続モデルへ POD 型として
投影し、`synvelis-app` の `syn.monitoring.*` ops を write-through
（`ProjectController` 経由の undo 可能 op）に切り替える。

- `synvelis-project` に `monitoring_data` モジュールを追加し、
  authored 宣言のみを保持する POD 型を定義する:
  track ごとの `ArmState` / `MonitorState` / `MonitorChannelMap` /
  `Option<LaneId>` 宣言（`TrackMonitorDecl`）と project 全体の
  `PunchMarks`（`punch_in` / `punch_out`、共に `Option<MusicalTick>`）。
- `Project` に `monitoring: MonitoringData` フィールドを追加し、
  `Project::new`・serde・`validate_references`・ID watermark に
  組み込む。`SCHEMA_VERSION` を 11 → 12 に上げ、v11→v12 の
  migration step（新規フィールドは `#[serde(default)]` で空初期化）
  を追加する。
- `ProjectOp` に monitor 宣言・punch 更新 op（`SetTrackMonitor` /
  `ClearTrackMonitor` / `SetPunchMarks`）を追加し、既存の
  `Operation` validate/apply + `ProjectController` の atomic
  apply/undo/redo 経路に載せる。
- `synvelis-app` の `MonitorUxState` は session 観測値
  （`input_output_ready` / `hardware_direct` / 稼働 session /
  latency 注入）のみを保持するよう再構成し、宣言と punch は
  `project.monitoring` を正本として read-through で投影する。
  `syn.monitoring.set` / `punch` の宣言変更は `ProjectOp` を
  `session.apply` で適用して write-through する。
- `project.new` / `project.open` で session 観測状態は reset され、
  宣言・punch は load 済み Project からそのまま再投影される
  （明示的なコピー不要 — Project が正本）。

## Why Now

先行 slice（`rcd-03-app-arm-monitoring-ux`、PR 済み）は
Arm/Monitor/Punch の UX op と投影を session-only として実装し、
「永続化・Undo 対象化」を明示的に deferred として残した。本 slice は
その deferred 分を回収し、宣言が save/reload・undo/redo・schema
migration を越えて生き残ることを headless fixture で証明する。
これにより monitor 宣言が project の一部として VCS/Undo/Save の
既存契約に自然に乗る。

## Users and Value

- `syn.*` サービス面の利用者（GUI ホスト / headless ドライバ /
  将来の CLI アダプタ）。project を save→open しても monitor
  宣言と punch マークが失われない。
- Undo/Redo が monitor 宣言変更を含むようになり、誤った宣言変更を
  既存の `syn.project.undo` で戻せる。
- `synvelis-project` は POD のみを持ち、route/gate/latency の
  意味論は引き続き `synvelis-recording`/`synvelis-player` が所有
  する（依存方向 `project ← recording` を維持）。

## Scope

### In Scope

- `crates/synvelis-project/`: `monitoring_data.rs`（新規 POD 型）、
  `project.rs`（フィールド・構築・参照検証）、`store.rs`
  （`SCHEMA_VERSION` 12）、`migration.rs`（v11→v12 step）、
  `ops.rs` + `apply/`（新規 `ProjectOp` variants）、`lib.rs`
  export、`tests/`。
- `crates/synvelis-app/src/`: `monitoring.rs` の write-through /
  session 観測値との分離、`app.rs` の project_new/project_open
  rehydrate（session reset + project 正本化）、`service.rs` /
  `ui_ops.rs` の op 引数の最小拡張（wire 形状は原則不変）。
- `crates/synvelis-app/tests/monitor_arm.rs` の更新・拡張。
- 本 slice の spec/design/plan/task/test-spec/impl-ir 文書。

### Out of Scope

- 実 Device / 実 Capture / 実 Software Monitor の可聴計測、実 GPUI
  GUI・multi-window/focus 受入、#114 Scenario — 外部 gate（NotRun）。
- `MonitorAdapter` の RT graph monitor node 搭載、実 PCM monitor
  経路（#106/#115/Ruviform#251 の統合契約依存）。
- Loop / PunchInLoop gate、count-in / pre-roll UX。
- `input_output_ready` / `hardware_direct` / `latency` /
  `low_latency` / 稼働 session / latency 注入の永続化 —
  session 観測値は非永続のまま（外部観測・実行時計測であり
  authored 宣言ではない）。
- `synvelis-recording` / `synvelis-player` / `synvelis-workspace` /
  `synvelis-library` / `ui/` の変更。`synvelis-gateway` は新規
  `ProjectOp` variant の `op_name` 命名 arm 追加のみ（網羅 match
  維持のための Coordinator 承認済み範囲拡張）。
- Issue #117 の Close（残件は外部 gate）。

## Requirements

- 永続化対象は authored 宣言のみ: Arm/Monitor 状態、
  channel_map、lane 宣言、punch marks。session 観測値
  （`hardware_direct` / `latency` / `low_latency` /
  `input_output_ready` / 稼働 session）は永続化しない。
- 永続型は `recording_data.rs` と同じ「POD only」パターンで
  `synvelis-project` 側に置く（serde derive + `deny_unknown_fields`）。
  `synvelis-recording` の型を project が参照しない
  （依存方向 `project ← recording` の制約）。
- 新規 map フィールドは `#[serde(default)]` とし、v11 以前の
  snapshot がそのまま load できる。`SCHEMA_VERSION` bump に対応する
  migration step を `migration.rs` の STEPS に追加する。
- 宣言変更は `ProjectOp` + `ProjectController::apply` 経由のみで
  行い、revision・undo/redo・atomicity の既存契約に従う。
  `synvelis-app` が Project を直接 mutate する経路を作らない
  （単一 writer 維持）。
- `validate_references` は monitor 宣言の参照整合を検証する:
  宣言 track は `project.tracks` に存在し、`lane` 宣言はその
  track の `lane_ids` に属し、`channel_map` は非空であること。
- route/gate/latency の判定は `synvelis-recording::decide` /
  `MonitorProbe` / `preflight` と `synvelis-player` の
  `LatencyReport::resolve` / `prepare_low_latency` が引き続き
  唯一の権威。`synvelis-project` にそれらの意味論を複製しない。
- 既存の `syn.monitoring.*` wire 契約（op 名・params・state
  投影の形状）は原則維持し、永続化は内部経路の変更として
  実現する。

## Acceptance Criteria

- [ ] Arm/Monitor/channel_map/lane 宣言と punch marks が
  `Project.monitoring` に POD として保存され、ProjectController
  の save→load round trip で復元される（headless fixture）。
- [ ] v11 snapshot が v12 へ migration され、monitoring は空既定値で
  load される。v12 snapshot を v11 として読むことは拒否される
  （前方非互換は fail-loud）。
- [ ] `SetTrackMonitor` / `ClearTrackMonitor` / `SetPunchMarks` が
  validate 失敗時に project を変更せず、成功時に revision を
  進め、undo/redo で宣言・punch が復元される。
- [ ] `syn.monitoring.set` / `punch` の宣言変更が write-through で
  `project.monitoring` に反映され、`syn.project.undo` で戻る。
- [ ] `project.new` / `project.open` 後、`syn.monitoring.state` が
  load 済み宣言・punch を投影し、session 観測値は既定に reset
  される。
- [ ] session 観測値（`input_output_ready` / `hardware_direct` /
  latency / 稼働 session）は save/load・undo を越えて残らない。
- [ ] 既存の route/gate/latency 投影（decide/MonitorProbe/
  preflight 消費）は本 slice で意味論を変えない
  （`monitor_arm.rs` 既存 fixture が引き続き pass）。

## Deferred Design Concerns

- 実 Device/実 GUI/multi-window/focus 受入、AudioLens — 外部 gate
  （NotRun）。
- `MonitorAdapter` PCM 経路・RT graph monitor node、low-latency
  route の実 Prepared Route 構築 — 宣言 scope 外。
- Loop/PunchInLoop gate・count-in/pre-roll UX — 後続 slice。
- VCS semantic diff への monitor 宣言の分類追加 — 本 slice では
  永続化・Undo・Save/Reload を優先し、diff adapter の拡張は
  follow-up とする（diff engine は既知 entity のみ列挙するため
  未対応でも破損しない）。

## Sources and Relations

- `SPEC-034VdSvFA9X0ESbckrSvBI`（RCD-03 app Arm/Monitoring UX spec —
  本 slice がその deferred「永続化・Undo 対象化」を回収する上流）。
- `SPEC-034UtksmGxvdygOzI1Dp7R`（RCD-03 crate slice spec —
  decide/MonitorProbe/Latency 契約の正本）。
- `docs/designs/rcd-03-app-arm-monitoring-ux.md`（session-only 契約の
  現行正本 — 本 slice で宣言部のみ project 正本へ移行）。
- `docs/impl/ir/rcd-03-app-arm-monitoring-ux-slice.md`（先行 slice の
  実装記録）。
- GitHub issue `scarecrow173/synvelis#117`（Refs。本 spec は Issue を
  Close しない）。
