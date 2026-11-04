---
id: "TASK-034Rp06VWlp4iyjC6X8NzA"
type: "task"
status: "done"
title: "mise 基盤: test-rt タスク・actionlint tool・lint-actions タスク追加"
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
    - "docs/plans/ci-synvelis-pr-push-regression-gate.md"
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

# mise 基盤: test-rt タスク・actionlint tool・lint-actions タスク追加

## Work

`mise.toml` に CI 用の最小追加を行う。既存タスクの意味は変えない。

- `[tools]` に `actionlint` を追加(`minimum_release_age = "7d"` を
  満たす公開版に pin)。
- `[tasks.test-rt]`: `cargo test --workspace --locked rt_`
  (RT no-alloc/no-block 契約テスト。`rt_` 接頭辞フィルタ)。
- `[tasks.lint-actions]`: `actionlint`(引数なしで
  `.github/workflows/*.{yml,yaml}` を検査)。
- `mise install` で `mise.lock` を再生成し、差分を確認してコミット
  対象に含める。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/ci-synvelis-pr-push-regression-gate.md`
- `docs/designs/ci-synvelis-pr-push-regression-gate.md`

## Dependencies

- なし(先行する task は無い)。

## Affected Files

- `mise.toml`
- `mise.lock`

## Done When

- [ ] `mise run test-rt` が `cargo test --workspace --locked rt_` を実行する
- [ ] `mise run lint-actions` が actionlint を起動する
- [ ] `mise.lock` が actionlint を含み `mise install --locked` と整合する
- [ ] 既存タスク定義が無改変である

## Verification

- [ ] `mise run test-rt` がローカルで PASS(2 件の rt_ テストを含む)
- [ ] `mise run lint-actions` が PASS
- [ ] `mise tasks` に 2 タスクが列挙される
