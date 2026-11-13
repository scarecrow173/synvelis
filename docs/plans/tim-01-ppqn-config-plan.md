---
id: "PLAN-TlIO4qpF8SnK0bA3SWQZ5d"
type: "plan"
status: "approved"
title: "TIM-01 PPQN config — punch marks 再投影と受入残件検証 plan"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/tim-01-ppqn-config-design.md"
  implemented-by:
    - "docs/tasks/tim-01-ppqn-config-task.md"
  depends-on:
    - "docs/designs/tim-01-ppqn-config-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/tim-01-ppqn-config-design.md"
  derived-by:
    - "docs/tasks/tim-01-ppqn-config-task.md"
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-config.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN config — punch marks 再投影と受入残件検証 plan

## Goal

Issue #125 残件の `synvelis-project` slice: `reproject_ppqn_with`
の site 列挙に `monitoring.punch` の `punch_in`/`punch_out` を
追加して silent retime を解消し、Issue 受入条件の headless
検証残件（6 PPQN Save/Reload/Undo/Redo 行列・実 v5 snapshot
非破壊 migration・分母上限・同 tick site・異 Division import・
保存対象外データ安全性）を fixture で固定する。

## Scope

`crates/synvelis-project/`（`src/ppqn.rs`・`src/ops.rs` doc・
`tests/tim_01_ppqn.rs`）と本 docs チェーンに閉じる。
`synvelis-app`/`synvelis-workspace`/`synvelis-gateway`、
`Cargo.toml`/`Cargo.lock`、`mise.toml`、他 docs は対象外。

## Upstream Documents

- `docs/specs/tim-01-ppqn-config.md`

## Design Inputs

- `docs/designs/tim-01-ppqn-config-design.md`
- `docs/designs/tim-01-configurable-ppqn-design.md`（先行 slice 設計）

## File Map

- `crates/synvelis-project/src/ppqn.rs` — `reproject_ppqn_with`
  に punch marks site を追加、`PpqnSite`/`PpqnPreviewSite`/
  `Display`/module doc の kind 語彙更新
- `crates/synvelis-project/src/ops.rs` — `SetProjectPpqn` doc
  コメントの再投影対象列挙に punch marks を追記
- `crates/synvelis-project/tests/tim_01_ppqn.rs` — `seeded()` に
  `SetPunchMarks`・automation・plugin instance・SMF import を
  追加、`musical_positions` 拡張、site_count assertion 更新、
  新規 test 5 件追加 + 既存 preservation test 拡張
- `docs/impl/ir/tim-01-ppqn-config.md` — implementation record

## Implementation Sequence

- [x] `ppqn.rs` に punch_in/punch_out site を追加し kind 語彙を更新
- [x] `ops.rs` の `SetProjectPpqn` doc を実態へ合わせる
- [x] `tim_01_ppqn.rs` fixture 拡張と既存 assertion 更新
- [x] 新規 test 追加（6 PPQN 行列・v5 snapshot・分母上限・
  同 tick site・punch preview。保存対象外安全性・異 Division
  SMF import は preservation test の fixture 拡張へ集約）
- [x] `cargo test -p synvelis-project --test tim_01_ppqn` green
  （25 tests pass、補助証拠）
- [x] `mise run fmt-check` / `check synvelis-project` /
  `lint synvelis-project` / `test synvelis-project` exit 0
- [x] docs index 再生成・IR 記録（index apply 済み・IR 記録済み）

## Dependencies and Risks

- 変換は既存 `reproject_ppqn_with`/`reproject_tick` をそのまま
  利用 — Ruviform checked API のみ、重複実装なし。
- `preview_ppqn` は同一 walk を共有するため site 追加は
  dry-run と mutating command の一致を自動維持する。
- schema version・snapshot 形式・op 形状は不変。

## Task Breakdown

- `docs/tasks/tim-01-ppqn-config-task.md` — 実装タスク一式

## Verification

- `cargo test -p synvelis-project --test tim_01_ppqn`（補助証拠;
  canonical は `mise run test synvelis-project`）
- `mise run fmt-check` / `mise run check synvelis-project` /
  `mise run lint synvelis-project` / `mise run test synvelis-project`
- docs index: `doc_maintenance.js index plan|apply` で
  specs/designs/plans/tasks/test-specs/impl/ir を再生成

## Open Questions / Gaps

- GUI PPQN 選択・Preview UI・Workspace 配線は別 slice/別受入
  （NotRun）。
- 実 OS/実 GUI・製品 Oracle は対象外（NotRun）。
- `test-rt`/`test-msrv`/`*-full` は本 slice の必須検証に含めない
  （Daily CI 責務; 変更は headless project crate のみ）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution
- Independent work streams: none
- User approval required before dispatch: review-protocol 代替

## Review Handoff

- `reproject_ppqn_with` の site 列挙が Project aggregate の全
  persisted musical tick を網羅したか、reviewer が確認。
  特に punch marks 追加後の preview site_count と
  `musical_positions` の対応。
