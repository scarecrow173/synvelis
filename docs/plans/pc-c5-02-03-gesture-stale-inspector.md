---
id: "TSPEC-034UxY8XCaH0Kk6JNotIAO"
type: "test-spec"
status: "approved"
title: "PC-C5 slice2 テスト仕様 — Gesture 固定・stale drop・Inspector mode（UI-C5-02/03・headless）"
created: "2026-11-09"
updated: "2026-11-09"
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
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/pc-c5-02-03-gesture-stale-inspector.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice2 テスト仕様 — Gesture 固定・stale drop・Inspector mode（UI-C5-02/03・headless）

## Purpose

UI-C5-02/03 の受入条件を headless fixture で固定する。実 OS GUI を伴わない
workspace レベルの semantics を保証し、後続 native slice の回帰基盤とする。

## Feature

Control Layer edge gesture と Inspector mode が、project 変更・selection・
window/focus/dock 変化に対して正しい固定 semantics を持つ。

## Rules

- Rule: control edge gesture は Begin で view・両 typed endpoint・project revision・
  window epoch を固定し、以降の入力で対象をすり替えない
- Rule: Preview と Cancel は project revision・undo 履歴を変更しない
- Rule: Commit は固定 revision で `AddModulationAssignment` を 1 回だけ適用し、
  undo 1 回で取り消せる。window epoch/revision が stale なら drop する
- Rule: control edge gesture は Audio Routing gesture（`gesture.rs`）と独立で、
  互いに混線しない
- Rule: inspect 操作は `InspectionFocus` のみ更新し、`object_selections`・
  project・undo を変更しない
- Rule: `InspectionMode::Explore`/`Pinned` 中は link deliver が target を
  すり替えない。Follow は follow_source 一致時のみ追従する
- Rule: explore back/forward は Inspector 内履歴のみを辿り、selection に触れない
- Rule: Pinned target 削除後も別対象へ暗黙に切り替わらない
- Rule: multi-selection（per-domain）は inspect/mode 変更で失われない

## Examples

- Example: Begin→Preview→Commit で assignment が 1 回適用され undo で戻る
- Example: Preview/Cancel 後に project revision・undo が不変
- Example: Begin 後に dock transfer → commit が Stale で drop
- Example: Begin 後に別経路で project 変更 → commit が Stale（CAS）
- Example: Begin 中に selection/focus を変更 → commit が fixation 対象に適用
- Example: inspect → selection/project/undo 不変、mode が Explore に遷移
- Example: Follow+follow_source 一致で link deliver が target を更新、
  Explore/Pinned/不一致 source では更新しない
- Example: explore で target を辿り back/forward が履歴通り動く
- Example: Pinned target を削除 → target が unresolved のまま残る
- Example: Audio Routing gesture と control edge gesture が同一 view で共存/排他
  ルール通りに動く

## Guarantees

- 正しい実装は上記 Rule を全て満たし、fixture が外部観測可能な挙動で証明する
- `object_selections`（UI selection）と gesture fixation/command target は
  型レベルで区別され、inspect が command target を生まない

## Non-goals

- 実 OS GUI/GPUI 描画/IME/DPI/a11y（UI-C5-05、NotRun）
- Audio Routing gesture 自体の semantics 変更
- ControlsUi parameter gesture の変更（既存機構の再確認のみ）
- Recording Conflict（UI-C5-04）

## Risk

- これらの保証が落ちると、edge drag の誤 routing 化・inspect の編集化・
  undo 汚染・window 変化での対象すり替えが headless でも検出不能になり、
  実画面でのデータ破壊的バグに直結する。
