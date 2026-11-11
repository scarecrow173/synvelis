---
id: "SPEC-034Vtz2CrUpwevHTLxDLm6"
type: "spec"
status: "approved"
title: "TIM-01 app 層 PPQN 統合 — syn.project.set_ppqn + 前提条件検証（synvelis-app slice）"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/designs/tim-01-configurable-ppqn-design.md"
    - "docs/plans/tim-01-configurable-ppqn.md"
    - "docs/tasks/tim-01-configurable-ppqn.md"
    - "docs/test-specs/tim-01-configurable-ppqn.md"
    - "docs/impl/ir/tim-01-configurable-ppqn.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  derived-by:
    - "docs/designs/tim-01-app-ppqn-design.md"
    - "docs/plans/tim-01-app-ppqn-plan.md"
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-app-ppqn.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 app 層 PPQN 統合 — syn.project.set_ppqn + 前提条件検証（synvelis-app slice）

## 背景・目的

Synvelis Issue #125 の app 層 slice。TIM-01 headless Command slice
（`ProjectOp::SetProjectPpqn`・rational beat 再投影・二重 CAS・
open-recording 拒否）は merge 済みだが、製品経路から PPQN を
変更する op が存在せず、設計上 app 層責務とされた前提条件
（transport-stopped・automation-write・merge-commit）も未検証
だった（`docs/test-specs/tim-01-configurable-ppqn.md` の Non-goals
で明示的に defer）。

本 slice は `synvelis-app` の `syn.*` service 面に
`syn.project.set_ppqn` を追加し、headless Command を GUI/headless
共通の op 経路へ公開する。同時に app 層前提条件を public
accessor で実装し、新規 fixture で拒否を証明する。

## 要求

- `syn.project.set_ppqn` op を `service.rs` の `OPS`/`dispatch` と
  `app.rs` の `SynvelisApp` メソッドとして実装する
  （`syn.project.*` は `self.app.*` 呼出の既存パターン）。
- params: `old_ppqn`（u32・必須）、`new_ppqn`（u32・必須）、
  `policy`（`"exact"`|`"round"`・必須 — 暗黙 policy なし）、
  `rounding`（`policy == "round"` のとき必須:
  `"floor"`|`"ceil"`|`"toward_zero"`|`"away_from_zero"`|
  `"nearest_even"`）、`expected_revision`（u64・省略時は live
  revision — 二重 CAS の stale 検証を wire 経路で可能にする）。
- app 層前提条件（いずれか違反で拒否・project 無変更）:
  - transport-stopped: `session.transport().playing == false`
    のときのみ許可。
  - merge-commit in flight なし:
    `session.control().recording().is_none()`。
  - automation-write なし: `!session.control().record_enabled()`。
- `SetProjectPpqn` の `expected_revision` CAS・`old_ppqn`
  ドメイン CAS・`1..=32767` 範囲・open-recording 拒否・scratch
  clone atomic validation が app 層経路でもそのまま維持される
  （app 層は policy を `PpqnConversionPolicy` に写像して
  `session.apply` に渡すのみで、検証ロジックを重複実装しない）。
- `project_info` に `ppqn` を投影する。
- Undo/Redo・Save/Reload・engine revision 通知
  （`session.apply` 内 `on_project_revision`）は既存機構で
  動作し、新規 fixture がそれを証明する。

## 受入基準（本 slice）

- `syn.project.set_ppqn` が headless（`UiRuntime::dispatch_op`）
  で動作し、二重 CAS/stale 拒否・`1..=32767` 検証が app 層経路
  でも維持される。
- app 層前提条件（transport-stopped・automation-write・
  merge-commit）の拒否が新規 fixture
  `crates/synvelis-app/tests/tim01_ppqn_app.rs` で検証される。
- `project_info` の `ppqn` 投影・undo/redo・save/reload・
  engine revision 通知が回帰 test で証明される。
- UI widget・実機/製品 Oracle 行は NotRun 継続であり Pass 扱い
  しない。

## 非目標

- PPQN 選択 UI widget・Preview/Confirm フロー。
- Presentation Driver への時刻正本移動。
- `synvelis-workspace`/`synvelis-gateway`/`synvelis-project`
  側の変更（accessor 完備・op 名登録済みのため不要）。
- 実 OS/実 GUI・製品 Oracle 検証。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/125
- 上流 slice: `docs/specs/tim-01-configurable-ppqn-project-command.md`
  （headless Command 契約）、`docs/designs/
  tim-01-configurable-ppqn-design.md`（責務境界:
  transport-stopped・automation-write・merge-commit は app 層）。
