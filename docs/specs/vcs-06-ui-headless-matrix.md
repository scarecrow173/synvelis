---
id: "SPEC-034VpB76W8P3twLKj0HlHB"
type: "spec"
status: "approved"
title: "VCS-06 UI headless matrix — Unknown/Partial honesty fixtures + ledger split"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "SPEC-vcs06-fixture-fault-matrix"
    - "SPEC-VCS04Hist0ryV1ewUI001"
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
    - "https://github.com/scarecrow173/synvelis/issues/97"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 UI headless matrix — Unknown/Partial honesty fixtures + ledger split

## What Should Be Built

`syn.vcs.view` / `syn.vcs.visual` op surface の headless fixture 行列
`crates/synvelis-app/tests/vcs06_ui_matrix.rs` を新規追加し、
`evidence/reports/vcs-06-fixture-matrix-ledger.md` の VCS-UI-01..05
行を「headless で観測可能な行（Pass*）」と「実機 NotRun 行」に
分割記録する。

## Why Now

前 slice（`vcs06_*` fixture 行列、PR#268）は VCS-SNAP/DIFF/GIT/
APPLY/RESTORE をカバーしたが、VCS-UI-01..05 は「headless lane が
存在しない」として一括 `NotRun` 記録されていた。実際には PR#252 で
app/workspace 層の `vcs04_history` fixture が存在し、`syn.vcs.*`
op 経由の headless 観測 lane は既に成立している。ledger の記述が
実態と不整合なため、観測可能な行を分離して記録する。

## Users and Value

- Issue #97 の受入担当者: VCS-UI 行の headless 証拠を個別 Case ID
  で追跡できる。
- 将来の実機 GUI 検証担当者（#114）: headless で担保済みの行と
  残る `NotRun` 行の境界が明確になる。

## Scope

### In Scope

- `crates/synvelis-app/tests/vcs06_ui_matrix.rs`（新規・test のみ）
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` の VCS-UI 行
  分割と fixture 一覧更新
- 本 spec / design / plan / task / test-spec / impl-ir チェーン

### Out of Scope

- `crates/*/src/` の変更（公開 API で観測可能な範囲のみ。src 変更が
  必要と判明した場合は blocked 報告）
- 既存 `vcs04_history.rs` fixture の重複再実装（A/B pin・merge
  parent・stale・dock 状態保持・lifecycle は参照記録に留める）
- 実 GPUI / IME / keyboard / a11y（#114）、実 OS 2window、VCS
  Writer / 3-way merge、非 git provider、`*-REAL` 行、COMPAT-02..04
- Issue #97 の Close

## Requirements

- 異 `project_id` の committed 端点 pin → `selection.diff.overall ==
  "unknown"`・issues 非空・`equal` / 0 changes に潰れない
  （`different_project_ids_are_unknown_not_diffed` の UI 版）。
- asset bytes が `Absent` の degraded snapshot → `completeness ==
  "Partial"`・`summary.unresolved` / `missing-asset` /
  `unresolved-source` issue 行・`history_screen_spec` の issues
  テーブル非空。
- staged pin + `index_generation` probe 失敗 / 無 backend →
  `freshness == "unknown"` + `freshness_reason`
  （`endpoint_freshness` の Unknown 経路 — 未テスト穴）。
- ledger: VCS-UI-01 A/B/parent → Pass*（vcs04 証拠）、UI-02
  SideBySide/Focus → Pass*、UI-03 2window/dock headless → Pass* +
  実 OS window NotRun、UI-04 Unknown vs zero → Pass*（本テスト）、
  UI-05 keyboard/a11y/実 GUI → NotRun（#114）に分割記録。

## Acceptance Criteria

- [ ] `vcs06_ui_matrix.rs` の fixture が `syn.vcs.view` /
  `syn.vcs.visual` / `vcs_history_view` / `history_screen_spec` の
  外部観測契約を独立 oracle で検証する。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `test-msrv` / `check-app-native` / `lint-app-native` を canonical
  検証として実行し結果を impl-ir に記録する。
- [ ] ledger の VCS-UI 行が headless / 実機で分離記録される。

## Deferred Design Concerns

- 実 GPUI two-window / IME / keyboard / a11y 証拠 — #114 の実機
  GUI lane へ defer。
- 実 OS window / native dock — OS matrix lane へ defer。

## Sources and Relations

- `docs/specs/vcs-06-fixture-fault-matrix.md`（親 matrix spec。
  同 spec は VCS-UI-01..05 を「本 slice の対象外」と明記しており、
  本 spec がその headless 観測部分を引き受ける）。
- `docs/specs/vcs-04-history-visual-diff-ui.md`（UI surface の
  上位 spec）。
- synvelis#97（統合受入 Issue、本 slice は Open のまま）。
