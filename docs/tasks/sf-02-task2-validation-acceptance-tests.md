---
id: "TASK-034SF02Zc3uRs0tW3yE8fGiJk"
type: "task"
status: "done"
title: "SF-02 T2 — 受入/Reject/不変/Undo 契約テストと canonical 検証"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/105"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-02-routing-transaction.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/sf-02-task1-routing-transaction-staging-model.md"
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
    - "docs/test-specs/sf-02-e2e-routing-transaction.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-02 T2 — 契約テスト・検証・記録

## T2.1 テスト実装

- `crates/synvelis-project/tests/routing_sf02.rs` に test-spec の
  Feature/Rule/Example を pin。
- preview 不変・commit 1 undo・失敗時全不変・stale revision/generation・
  dangling/invalid role/map consent/cycle/unresolved・Bass→Bus→Master・
  Pre/Post send→return・sidechain・mute/solo audibility・undo/redo・
  save/load 往復。

## T2.2 canonical 検証

- `mise run fmt-check` → `check` → `lint` → `test` → `build`。
- MSRV 範囲内であること(必要に応じて `test-msrv`)。
- `git fetch origin` → `git merge origin/main` 後に `Cargo.lock` 再解決
  (`mise run check` 等)・再検証。

## T2.3 記録

- `docs/impl/` に implementation record を作成し spec/design の
  `implemented-by`/`verified-by` を整合。
- 各 index README へ新規 docs を登録。
- 未検証項目(実機/実 Plugin/GUI)を NotRun/Unsupported で明記。

## 完了条件

- mise 全緑・受入テスト pass・doc 整合・PR 作成まで完了。
