---
id: "SPEC-PCc4030bs3vN1pQ8wXyZ2aB"
type: "spec"
status: "approved"
title: "PC-C4-03 reported observation 消費 — host drain → ParamDispatch ledger（headless slice）"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
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
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
  refines:
    - "docs/specs/pc-c4-plugin-param-binding.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  defers: []
  deferred-by: []
---

# PC-C4-03 reported observation 消費 — host drain → ParamDispatch ledger

## 背景

PC-C4 の下部機構は実装済み: `ParamDispatch::observe`/`observe_cycle`/
`record_reported`/`mark_reporting_unavailable`/`stage`（`dispatch.rs`）、
`HeadlessDriver::observe_attached_block`/`param_observations`、generation
gate（`routing_control.rs`）、および PC-C4-02 の session→driver/routing 配線。

欠落しているのは **consumer glue** のみ:
`PluginHost::drain_output_reports(handle, &mut Vec<Event>) ->
Result<ReportDrain, _>`（ruviform pin `b02942c2`）を呼び、
`record_reported`/`mark_reporting_unavailable`/`observe_cycle` を駆動する
production コードがゼロである。

## 要件

- `RoutingControlDispatch`（generation pin 済みの唯一の ownership 層）に
  generation-scoped の report drain surface を追加する。
- host 側 handle 集合は caller 注入型:
  `PluginHostProcessors::host()` / `RoutingEngine::plugin_state_handles
  (generation)` 経由の取得を想定し、host 型への直接依存は薄く保つ。
- `ReportDrain::Drained` → `record_reported`/`observe_cycle` 経由で
  `stage()`/`ParamValueObservation` 投影を前進させる。
- `ReportDrain::Unavailable` → `mark_reporting_unavailable` で正直に終端
  する（success 偽装禁止）。
- `ParamId → ParameterTargetId` 逆引きは既存 binding table を利用する。
- generation 切替との整合（旧 generation 由来 report の reject/gate）を
  維持する。
- L1 slot の wire report verb（RF#253 PR #564）は pin `b02942c2` に未含有
  のため、L1 slot は `Unavailable` → `mark_reporting_unavailable` 終端を
  仕様とし「pin bump 待ち `NotRun`」とする（L1 実値の検証は行わない）。
- `attached`/`tick`/`fast_control` 等の既存公開 API の意味論は変更しない
  （app/workspace 側の接続は後続 slice pc-c4-04 の範囲）。

## 受入条件（本 slice 分）

- [x] host output report の drain → `record_reported`/`observe_cycle` →
  `stage()` 投影の経路が `RoutingControlDispatch` に generation-scoped で
  実装され、新規 headless test が Pass。
- [x] `ReportDrain::Unavailable` が `mark_reporting_unavailable` で正直に
  終端する（success 偽装なし）。L1 slot の実値検証は pin bump 待ち
  `NotRun` として明記。
- [x] generation 切替で旧 generation の report が誤配達されない
  （gate 回帰）。
