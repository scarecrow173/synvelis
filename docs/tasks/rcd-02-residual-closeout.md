---
id: "TASK-034XHPOcaY2TsdPi7kkJte"
type: "task"
status: "done"
title: "RCD-02 residual closeout — Issue #116 AC 照合・canonical lane 実測・証拠固定・簿記 drift 修正"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added:
      - type: "file"
        path: "docs/specs/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/designs/rcd-02-residual-closeout-design.md"
      - type: "file"
        path: "docs/plans/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/test-specs/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/exp/rcd-02-residual-closeout.md"
      - type: "file"
        path: "evidence/sv116/rcd-02-closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/tasks/rcd-02-task3-stagedfile-publish-migration.md"
      - type: "file"
        path: "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
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
    - "docs/plans/rcd-02-residual-closeout.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-02-residual-closeout.md"
    - "docs/designs/rcd-02-residual-closeout-design.md"
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
    - "docs/test-specs/rcd-02-residual-closeout.md"
  references:
    - "evidence/sv116/rcd-02-closeout-verification.md"
  defers: []
  deferred-by: []
---

# RCD-02 residual closeout — Issue #116 AC 照合・canonical lane 実測・証拠固定・簿記 drift 修正

## Work

Issue #116（RCD-02）の簿記クローズアウト。プロダクションコード・
test・fixture は変更しない。受入条件 4 行と実在 test・既存
ledger・canonical lane 結果を照合し、doc chain・evidence・
IR/exp に固定する。併せて task3 status・docs index の簿記
drift を修正する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-02-residual-closeout.md`
- `docs/specs/rcd-02-residual-closeout.md`
- `docs/designs/rcd-02-residual-closeout-design.md`
- `docs/test-specs/rcd-02-residual-closeout.md`

## Dependencies

- canonical lane 実走済みの worktree（`mise` 管理 toolchain）。
- 編集境界: worker spec の占有リスト（新規 doc chain
  `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/
  rcd-02-residual-closeout*.md`、`evidence/sv116/
  rcd-02-closeout-verification.md`、`docs/tasks/
  rcd-02-task3-stagedfile-publish-migration.md` の status、
  生成 `docs/*/README.md` index、任意 additive note
  `evidence/reports/rcd-02-durability-recovery-review-ledger.md`）。

## Affected Files

- 新規: `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/rcd-02-residual-closeout*.md`（7 file）、
  `evidence/sv116/rcd-02-closeout-verification.md`
- 更新: `docs/tasks/rcd-02-task3-stagedfile-publish-migration.md`
  （status のみ）、`docs/*/README.md`（index 再生成）、
  `evidence/reports/rcd-02-durability-recovery-review-ledger.md`
  （closeout への additive note のみ・既存記述は不変）

## Done When

- [x] Issue 受入条件 4 行が実在テスト/ledger/lane 付きで
      [x]/[ ] 記録される（spec Acceptance Criteria + IR +
      `evidence/sv116/rcd-02-closeout-verification.md`）
- [x] canonical lane（`mise run test synvelis-recording` /
      `synvelis-player` / `synvelis-project` / `test-rt`）の
      実走結果が Pass/Fail で記録される
- [x] 外部 gate が NotRun/Deferred + 所有 Issue 記名で記録される
- [x] `docs/tasks/rcd-02-task3-stagedfile-publish-migration.md`
      の status が `done` になり、`docs/tasks/README.md` が
      再生成される
- [x] `docs/test-specs/README.md` の managed index row が
      `approved`（正本 frontmatter と一致）に揃う。marker
      管理外 tail 領域の stale `proposed` 重複行は generator
      が verbatim 保存するため残存 finding として PR 本文に
      記録
- [x] doc_status/audit_impl_record/doc_maintenance index plan で
      変更ファイルに新規 finding がない
- [x] Issue #116 へ進捗コメント・PR 作成（close/merge なし）

## Verification

- [x] `mise run fmt-check` / `prek run --all-files`（軽量
      チェック）
- [x] `mise run test synvelis-recording` / `synvelis-player` /
      `synvelis-project` / `test-rt`（結果は IR Validation 参照）
- [x] `mise run test-recording-real-fs`（opt-in・任意 — 実行
      した場合は結果を正直に記録）
- [x] `doc_status.js lint`/`audit`・`audit_impl_record.js`・
      `doc_maintenance.js index plan/apply` 実走
