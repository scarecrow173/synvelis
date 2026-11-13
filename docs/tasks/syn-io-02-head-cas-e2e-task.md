---
id: "TASK-034WVJP0tzo7lASwFwzkpG"
type: "task"
status: "done"
title: "SYN-IO-02 HEAD CAS 実 2-process E2E 検証 slice"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/330"
  changes:
    added:
      - type: "file"
        path: "docs/specs/syn-io-02-head-cas-e2e.md"
      - type: "file"
        path: "docs/designs/syn-io-02-head-cas-e2e-design.md"
      - type: "file"
        path: "docs/plans/syn-io-02-head-cas-e2e-plan.md"
      - type: "file"
        path: "docs/tasks/syn-io-02-head-cas-e2e-task.md"
      - type: "file"
        path: "docs/test-specs/syn-io-02-head-cas-e2e.md"
      - type: "file"
        path: "docs/impl/ir/syn-io-02-head-cas-e2e.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/tests/head_cas.rs"
      - type: "file"
        path: "docs/designs/syn-io-02-head-cas-design.md"
      - type: "file"
        path: "docs/test-specs/syn-io-02-head-cas.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/plans/syn-io-02-head-cas-e2e-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/syn-io-02-head-cas-e2e-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "TASK-034T2vRnB8yK3mXw7LpQ"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-io-02-head-cas-e2e.md"
  references:
    - "crates/synvelis-project/tests/head_cas.rs"
    - "crates/synvelis-project/src/store.rs"
  defers: []
  deferred-by: []
---

# SYN-IO-02 HEAD CAS 実 2-process E2E 検証 slice

## Work

synvelis#330 の残件 slice: `save_cas`/`HeadCasError`/
`HEAD.lock`（PR #342 統合済み）の実 2-process 協調を
self re-exec test で検証し、HEAD CAS の保証境界を実測
付きで確定記録する。`src/` 変更・新規 dependency なし。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/syn-io-02-head-cas-e2e-plan.md`
- `docs/specs/syn-io-02-head-cas-e2e.md`
- `docs/designs/syn-io-02-head-cas-e2e-design.md`

## Dependencies

- `save_cas`/`HeadCasError`/`HEAD.lock` advisory mutex
  （PR #342 `d653062` で main 統合済み）。
- `tests/head_cas.rs` の既存 helper（`workdir`/`BundleGuard`）。
- self re-exec 先例: `synvelis-library/tests/import.rs`。

## Affected Files

- `crates/synvelis-project/tests/head_cas.rs`
- `docs/designs/syn-io-02-head-cas-design.md`
- `docs/test-specs/syn-io-02-head-cas.md`
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  syn-io-02-head-cas-e2e*.md`
- `docs/*/README.md`（doc-maintenance index 再生成）

## Done When

- [x] 実 2-process 同時 `save_cas` テストが追加され、
  exactly one winner + loser typed `HeadCasError::Conflict`
  + 最終 HEAD=勝者 + 敗者 snapshot 非 publish +
  `HEAD.lock` 非残滓を assert する。
- [x] `mise run test synvelis-project` で pass する。
- [x] HEAD CAS の保証境界が関連 docs に実測/未検証の区別
  つきで記録される。
- [x] `src/`・範囲外ファイルに差分がない。

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check synvelis-project` — exit 0
- [x] `mise run lint synvelis-project` — exit 0
- [x] `mise run test synvelis-project` — exit 0
- [x] macOS/Linux = CI 実走または NotRun として impl-ir/PR
  に記録。
