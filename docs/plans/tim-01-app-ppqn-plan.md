---
id: "PLAN-034VtBaArtupj0qHG0phVl"
type: "plan"
status: "approved"
title: "TIM-01 app 層 PPQN 統合 — implementation plan"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/specs/tim-01-app-ppqn.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-01-app-ppqn.md"
    - "docs/designs/tim-01-app-ppqn-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-app-ppqn.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 app 層 PPQN 統合 — implementation plan

## Goal

Issue #125 の app 層 slice として、`syn.project.set_ppqn` op
（params 検証・app 層前提条件・`SetProjectPpqn` への写像）と
`project_info` の `ppqn` 投影を `synvelis-app` に実装し、
headless fixture で CAS/stale・前提条件拒否・undo/redo・
save/reload・engine revision 通知を証明する。

## Scope

単一実装ストリーム。`synvelis-app` の `service.rs`/`app.rs` と
新規 test fixture に閉じる。workspace/gateway/project 側の
変更・UI widget・実機検証は対象外。

## Upstream Documents

- `docs/specs/tim-01-app-ppqn.md`

## Design Inputs

- `docs/designs/tim-01-app-ppqn-design.md`

## File Map

- `crates/synvelis-app/src/service.rs` - `OPS` に
  `syn.project.set_ppqn` を追加、`dispatch` に
  `self.app.project_set_ppqn(params)` を追加
- `crates/synvelis-app/src/app.rs` - `project_set_ppqn` 実装
  （params 検証・前提条件 gate・`session.apply`）と
  `project_info` の `ppqn` 投影
- `crates/synvelis-app/tests/tim01_ppqn_app.rs` - headless fixture
  （新規）

## Implementation Sequence

- [x] `tim01_ppqn_app.rs` に失敗 test を先に書く（TDD RED:
      wire CAS/stale・playing 中・transaction open 中・
      record_enabled・範囲外・undo/redo・save/reload・
      engine revision・`project_info.ppqn`）
- [x] `service.rs` に OpInfo + dispatch を追加
- [x] `app.rs` に `project_set_ppqn` と `project_info.ppqn` を実装
- [x] fixture green 確認・全検証 lane 実行

## Dependencies and Risks

- `ProjectSession::{transport, control, apply}`・
  `ControlSession::{recording, record_enabled}` の public
  accessor に依存（完備済み・変更不要）。
- `PpqnConversionPolicy`/`Rounding` の wire 写像は app 層で
  一箇所だけ行う（policy 必須・round 時のみ rounding 必須）。
- 前提条件拒否は `invalid_params`（既存 convention、
  `busy`/`requires_stop` code は wire に存在しない）。

## Task Breakdown

- `docs/tasks/tim-01-task2-app-ppqn.md` — 実装タスク一式

## Verification

- [x] `cargo test -p synvelis-app --test tim01_ppqn_app` — 11 件 green
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `test-msrv` / `check-app-native` /
      `lint-app-native` — 全て exit 0
- [x] docs チェーン + 索引再生成

## Open Questions / Gaps

- PPQN 選択 UI widget・Preview/Confirm は別 slice（本 slice
  対象外、NotRun）。
- 実 OS/実 GUI・製品 Oracle は対象外（NotRun）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution
- Independent work streams: none
- User approval required before dispatch: yes
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

## Review Handoff

- `record_enabled` を前提条件に含める fail-closed 判断と、
  `control.playback` Playing を gate しない判断が妥当か reviewer
  が確認。fixture が前提条件 3 件を独立に検証できているか。
