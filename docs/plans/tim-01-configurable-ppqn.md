---
id: "PLAN-034UHYyMbiLPEtGczKNKYC"
type: "plan"
status: "approved"
title: "TIM-01 Configurable PPQN Project Command — implementation plan"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/designs/tim-01-configurable-ppqn-design.md"
  implemented-by:
    - "docs/tasks/tim-01-configurable-ppqn.md"
  depends-on:
    - "docs/designs/tim-01-configurable-ppqn-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/tim-01-configurable-ppqn-design.md"
  derived-by:
    - "docs/tasks/tim-01-configurable-ppqn.md"
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-configurable-ppqn.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 Configurable PPQN Project Command — implementation plan

## Goal

Issue #125 の headless slice として、`ProjectOp::SetProjectPpqn`
Command（expected_revision + old_ppqn の二重 CAS、Exact/Round
policy、atomic validation、open-recording 拒否）と project
aggregate 全体の rational-beat 再投影を実装し、Undo/Redo・
Save/Reload・非破壊 migration を headless fixture で証明する。

## Scope

単一実装ストリーム。`synvelis-project` の op/ppqn module と
gateway の op 名マッピング、project 不変条件、headless fixture
に閉じる。UI・app 層の前提条件・Ruviform 側変更は対象外。

## Upstream Documents

- `docs/specs/tim-01-configurable-ppqn-project-command.md`

## Design Inputs

- `docs/designs/tim-01-configurable-ppqn-design.md`

## File Map

- `crates/synvelis-project/src/ppqn.rs` - PPQN 範囲・変換 policy・
  project aggregate 再投影（新規）
- `crates/synvelis-project/src/ops.rs` - `SetPpqn` stub を
  `SetProjectPpqn` に置換、validate/apply 実装
- `crates/synvelis-project/src/lib.rs` - `ppqn` module 登録と export
- `crates/synvelis-project/src/project.rs` - PPQN coherence 不変条件
- `crates/synvelis-project/src/tempo.rs` - configurable PPQN 向け doc 更新
- `crates/synvelis-gateway/src/prepare.rs` - op 名 `set-project-ppqn` マッピング
- `crates/synvelis-project/tests/tim_01_ppqn.rs` - headless fixture（新規）

## Implementation Sequence

- [x] `ppqn.rs` に範囲・policy・再投影ヘルパを実装
- [x] `ops.rs` の `SetPpqn` を `SetProjectPpqn` に置換し CAS/検証を実装
- [x] `project.rs` に PPQN coherence 不変条件を追加
- [x] gateway op 名マッピングを更新
- [x] headless fixture 14 件を追加し green 確認
- [x] 全検証 lane（fmt-check/check/lint/test/test-rt/test-msrv）実行

## Dependencies and Risks

- Ruviform checked API（`TickPosition::{to_beat,from_beat}`、
  `Rounding`、checked `BeatPosition`）に依存。重複実装禁止。
- inexact 変換・overflow は operation 全体を拒否（atomic）。
- `SCHEMA_VERSION` bump なし。旧 snapshot 非破壊は fixture で証明。

## Task Breakdown

- `docs/tasks/tim-01-configurable-ppqn.md` — 実装タスク一式

## Verification

- [x] `cargo test -p synvelis-project --test tim_01_ppqn` — 14 tests green
- [x] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` / `test-msrv` — 全て exit 0
- [x] docs チェーン + 索引再生成 + graph route

## Open Questions / Gaps

- 製品 UI の PPQN 選択・transport/automation/merge 前提条件は
  app 層の別 slice（本 slice 対象外、NotRun）。
- 実 OS/実 GUI・製品 Oracle 検証は対象外（NotRun）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（完了済み）
- Independent work streams: none
- User approval required before dispatch: yes

## Review Handoff

- fixture `tim_01_ppqn.rs` の 14 test が spec 受入基準を網羅するか
  reviewer が確認。特に `ppqn_change_survives_save_reload_and_keeps_old_snapshot`
  の旧 snapshot 非破壊証明。
