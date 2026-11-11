---
id: "TASK-034VMQRx1r1aqfFdyL3kzv"
type: "task"
status: "done"
title: "ALIB-02 residual closeout — Issue #86 DoD 照合・canonical lane 実測・証拠追記"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "docs/specs/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/designs/alib-02-residual-closeout-design.md"
      - type: "file"
        path: "docs/plans/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-residual-closeout.md"
      - type: "file"
        path: "evidence/issue-86-v5-migration/closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/specs/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/plans/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/test-specs/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat-slice.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/alib-02-residual-closeout.md"
  implemented-by: []
  depends-on:
    - "docs/plans/alib-02-residual-closeout.md"
    - "docs/designs/alib-02-residual-closeout-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ALIB-02 residual closeout — Issue #86 DoD 照合・canonical lane 実測・証拠追記

## Work

Issue #86（ALIB-02）の簿記クローズアウト。プロダクションコードは
変更しない。受入条件/DoD 行と実在 fixture・テスト・lane 結果を
照合し、doc chain・evidence・IR に固定する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/alib-02-residual-closeout.md`
- `docs/specs/alib-02-residual-closeout.md`
- `docs/designs/alib-02-residual-closeout-design.md`

## Dependencies

- canonical lane 実走済みの worktree（`mise` 管理 toolchain）。
- 編集境界: worker spec の占有リスト（`docs/impl/ir/alib-02-*`、
  `docs/specs|plans|tasks|test-specs/alib-02-*`、
  `docs/designs/alib-02-residual-closeout-design.md`、
  `evidence/issue-86-v5-migration/`、
  `crates/synvelis-project/tests/fixtures/`）。

## Affected Files

- 新規: `docs/{specs,designs,plans,tasks,impl/ir}/alib-02-residual-closeout*.md`、
  `evidence/issue-86-v5-migration/closeout-verification.md`
- 更新: `docs/specs|plans/alib-02-v5-snapshot-compat.md`、
  `docs/test-specs/alib-02-streaming-verified-import.md`、
  `docs/impl/ir/alib-02-*.md`（3 件）

## Done When

- [x] Issue 受入条件 5 行・残実装 DoD 4 行が実在テスト/fixture/lane
      付きで [x]/[ ] 記録される（spec Acceptance Criteria + IR）
- [x] canonical lane 7 本の実走結果が Pass/Fail で記録される
- [x] 外部 gate が NotRun + 所有 Issue 記名で記録される
- [x] `evidence/issue-86-v5-migration/closeout-verification.md` に
      lane 要約・再現手順がある
- [x] doc_status/audit_impl_record で変更ファイルに新規 finding が
      ない
- [x] Issue #86 へコメント・PR 作成（close/merge なし）

## Verification

- [x] `mise run fmt-check` / `check` / `lint` / `test` / `build` /
      `test-rt` / `test-msrv`（結果は IR Validation 参照）
- [x] `doc_status.js lint`/`audit`・`audit_impl_record.js` 実走
