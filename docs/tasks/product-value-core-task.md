---
id: "TASK-034X3k0wEnZD3OtrbVBneg"
type: "task"
status: "done"
title: "Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価計画タスク"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/1"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/product-value-core-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/product-value-core-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/product-value-core.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/product-value-core.md"
  references: []
  defers: []
  deferred-by: []
---

# Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価計画タスク

## Work

Issue #1 の製品価値・中核体験・Gate A/B/C/AI・UX 評価計画を
`docs/` canonical 文書チェーンと `evidence/sv1/` の根拠として
正本化する。文書のみのタスクで、コード・CI・mise 設定は変更
しない。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/product-value-core-plan.md`
- `docs/designs/product-value-core-design.md`
- `docs/specs/product-value-core.md`
- Source: synvelis#1

## Dependencies

- なし（Blocked by なし）。

## Affected Files

- `docs/specs/product-value-core.md`（新規）
- `docs/designs/product-value-core-design.md`（新規）
- `docs/plans/product-value-core-plan.md`（新規）
- `docs/tasks/product-value-core-task.md`（新規）
- `docs/test-specs/product-value-core.md`（新規）
- `docs/impl/ir/product-value-core.md`（新規）
- `docs/impl/exp/product-value-core.md`（新規）
- `evidence/sv1/acceptance-matrix.md`（新規）
- `evidence/sv1/issue-1-scope-map.md`（新規）
- `evidence/sv1/verification.md`（新規）
- `docs/specs/README.md`・`docs/designs/README.md`・
  `docs/plans/README.md`・`docs/tasks/README.md`・
  `docs/test-specs/README.md`・`docs/impl/ir/README.md`・
  `docs/impl/exp/README.md`（生成 index の再生成のみ）

## Tasks

- [x] Issue #1 本文と関連 Issue の Owner/受入条件を確認。
- [x] 既存 docs との重複・矛盾を調査。
- [x] spec → design → plan → test-spec → task の文書チェーンを作成。
- [x] `evidence/sv1/` の根拠ファイルを作成。
- [x] review-protocol レビュー（blocker なしまで）。
- [x] docs index 再生成・`mise run fmt-check`・doc-status lint/audit。
- [x] impl/ir・impl/exp を完了状態に更新。

## Done When

- 文書チェーン 7 file と `evidence/sv1/` が揃い、Acceptance Matrix
  の 6 項目が測定方法・合格基準・検証手段・現状の達成度を持つ。
- 実装 Issue（#8/#23・#4/#6・#102/#103・#108/#109・#114・#2/#3・
  #44/#46・#277〜#288）への相互参照がある。
- 設計合意のみで OS/RT/実 GUI 受入を済ませない記述があり、
  `NotRun`/`Unsupported` が正直に記録されている。
- docs index が再生成され、軽量チェックが通る。

## Verification

- [x] `mise run fmt-check` exit 0。
- [x] `doc_status.js lint`/`audit` で新規 docs 起因の error なし
  （spec の `traceability-missing-upstream` warning は Issue 起点
  spec 共通の既存パターン。repo 全体の既存 finding は本タスク
  範囲外として `evidence/sv1/verification.md` に記録）。
- [x] `doc_maintenance.js index plan` で触った dir が `in-sync`。
