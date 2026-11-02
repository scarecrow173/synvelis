---
id: "PLAN-034PCB04Pk9ySt5zBcPdEf"
type: "plan"
status: "completed"
title: "PC-B Parameter Control dispatch/note-scope/RT/offline 実装計画"
created: "2026-11-04"
updated: "2026-11-04"
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
  implements:
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-a-parameter-control.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-b-dispatch-boundary-note-entity-prepared-eval.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-b-e2e-parameter-dispatch.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-B Parameter Control 実装計画

## Goal

Issue #43 の PC-B-01..04 を headless PoC として実装し、`mise run
fmt-check`/`check`/`lint`/`test` 全 green + impl record まで完走する。

## Scope

- synvelis-project: `Note` entity + `resolve_scope_pos` Note arm +
  `EvalPlan` prepared eval(`eval_rt`)。schema v4。
- synvelis-workspace: 記録経路の scope 正規化(`record_points` に
  `&Project`)、session 呼び出し整合。
- synvelis-player: `dispatch` モジュール(`ParamDispatch`)+
  `ruviform-plugin-host`/`ruviform-audio-plugin-bridge` 依存追加。
- tests: project `pc_b_rt.rs`(rt_ alloc-free)、workspace `pc_b.rs`
  (note scope)、player `pc_b.rs`(dispatch + offline parity)。

## Task breakdown

1. **T1: Note entity + scope 解決**(project)
   `Note`/`Project.notes`/ops `AddNote`/`RemoveNote`/
   `resolve_scope_pos` Note arm/validate/schema v4。
2. **T2: 記録経路 scope 正規化**(workspace)
   `record_points`/`record_write`/`gesture_end` に `&Project`、
   session.rs 適合、pc_a 既存テスト適合。
3. **T3: prepared eval + rt_ test**(project)
   `EvalPlan` prepared データ + `eval_rt`、`tests/pc_b_rt.rs` で
   `assert_no_alloc` 測定。
4. **T4: dispatch 経路**(player)
   `dispatch.rs` `ParamDispatch` + 依存追加 + `tests/pc_b.rs` で
   bridge 経路配送・拒否伝播を検証。
5. **T5: offline parity + 全検証 + impl record**
   render() parity テスト、merge origin/main、生成物再生成、
   mise 全タスク、docs/impl/ir 記録。

## Verification matrix

| 受入 | 検証 |
| --- | --- |
| PC-B-01 | player pc_b.rs: endpoint への ParamEvent 配送 + bind/resolve/queue 拒否の型付き伝播 |
| PC-B-02 | workspace pc_b.rs: note-relative 評価 + 記録点の note-relative 格納 + domain 外 skip |
| PC-B-03 | project pc_b_rt.rs: `assert_no_alloc` で eval_rt/lower_event 成功パス alloc=0 |
| PC-B-04 | player pc_b.rs: playback drive と render() の配送イベント一致 |
| 全体 | `mise run fmt/fmt-check/check/lint/test` 全 green |
