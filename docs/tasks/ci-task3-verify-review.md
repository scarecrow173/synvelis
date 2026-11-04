---
id: "TASK-034Rp08nCxirezDOCQ9l0Z"
type: "task"
status: "in-progress"
title: "CI 検証・証跡・最終レビュー"
created: "2026-11-03"
updated: "2026-11-04"
owners: ["scarecrow173"]
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
    - "docs/plans/ci-synvelis-pr-push-regression-gate.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/ci-task2-workflow.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ci-synvelis-pr-push-regression-gate.md"
  references: []
  defers: []
  deferred-by: []
---

# CI 検証・証跡・最終レビュー

## Work

実装済み workflow/mise 追加をローカル検証し、証跡・証跡文書
(impl record)・最終自己レビューを行ってコミットする。

- actionlint / prek / ガード系シェルロジックの単体試験。
- `mise run fmt-check`・`mise run check`・`mise run lint`・
  `mise run test`・`mise run gw10` の実行結果を記録
  (実行したものだけを PASS と書く)。
- `rt_` 非空ガードの計数確認。
- `git fetch origin` → `git merge origin/main` → 生成物再生成 →
  最終検証 → Conventional Commits でコミット。
- `docs/impl/ir/` に impl record を作成し、検証結果と未検証範囲
  (GitHub runner 実測・macOS lane)を明記。
- review-protocol で自己レビューし blocker 0 を確認。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/ci-synvelis-pr-push-regression-gate.md`
- `docs/test-specs/ci-synvelis-pr-push-regression-gate.md`

## Dependencies

- `docs/tasks/ci-task2-workflow.md`

## Affected Files

- `docs/impl/ir/ci-synvelis-pr-push-regression-gate.md`(新規)

## Done When

- [ ] 上記検証がすべて実行・記録済み(未実行は「未検証」+ 理由)
- [ ] origin/main 取り込み済みで最終検証が PASS
- [ ] impl record が証跡・検証・未検証範囲を含む
- [ ] blocker 0 の自己レビュー完了・コミット済み

## Verification

- [ ] `git log` に Conventional Commits 形式のコミットがある
- [ ] `git status` が clean
