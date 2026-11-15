---
id: "TASK-034XI5DO7WZZ6NGhbhBooM"
type: "task"
status: "done"
title: "variant_sets restore-as-copy origin identity 修復"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/458"
  changes:
    added:
      - type: "file"
        path: "docs/specs/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/designs/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/plans/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/tasks/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/test-specs/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/impl/ir/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/impl/exp/variant-sets-vcs-restore.md"
      - type: "file"
        path: "evidence/sv458/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/variant/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/src/restore/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_variant_matrix.rs"
      - type: "file"
        path: "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/variant-sets-vcs-restore.md"
  implemented-by:
    - "docs/impl/ir/variant-sets-vcs-restore.md"
  depends-on:
    - "docs/plans/variant-sets-vcs-restore.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/variant-sets-vcs-restore.md"
    - "docs/designs/variant-sets-vcs-restore.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/variant-sets-vcs-restore.md"
  verified-by: []
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/457"
  deferred-by: []
metadata: {}
---

# variant_sets restore-as-copy origin identity 修復 — task

## Work

synvelis#458 の修復 slice。`validate_set_shape` に provenance
規則（`SameProject`/`Persisted`）を導入し、persisted/load
oracle では `VariantSet::origin`/`Branch::created_from` の
`SnapshotIdentity.project_id` が enclosing project と異なること
を許容する（authored provenance の verbatim 維持）。live op
境界（`VariantOp::Fork::validate`）では `SameProject` を維持。
`fork_project` は変更しない。`stable_id_counts` に
`variant-set`/`branch` を追加し、`vcs06_variant_matrix.rs` の
Fail pin を成功系 envelope readback 検証へ flip する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/variant-sets-vcs-restore.md`
- `docs/specs/variant-sets-vcs-restore.md`
- `docs/designs/variant-sets-vcs-restore.md`
- `docs/test-specs/variant-sets-vcs-restore.md`

## Dependencies

- synvelis#457（variant diff 分類）merge 済み — `created_from`/
  `content` の diff field class 確定と整合。

## Affected Files

- `crates/synvelis-project/src/variant/engine.rs`
- `crates/synvelis-project/src/restore/mod.rs`
- `crates/synvelis-project/tests/vcs06_variant_matrix.rs`
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`
- `evidence/sv458/`
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/variant-sets-vcs-restore.md`

## Done When

- [x] `restore_as_copy` on variant-set project が `Published`
  で成功し、envelope readback で `variant_sets` verbatim +
  `project_id` のみ再採番 + provenance source 指し継続が
  assert される。
- [x] `validate_set_shape` の規則が live op 境界
  `SameProject` / persisted oracle `Persisted` で使い分け
  られている。
- [x] `stable_id_counts` が `variant-set`/`branch` を報告する。
- [x] ledger `VCS-RESTORE-10` が `Pass` に flip され、
  `BranchContentStore` capability 限界が記録される。
- [x] `variant_sets` 空 project の restore・apply fail-closed
  pin が非回帰。
- [x] canonical 検証（fmt-check/check/lint/test/test-rt/build/
  check-full/prek）が pass。

## Verification

- [x] `mise run fmt-check` / `mise run check synvelis-project` /
  `mise run lint synvelis-project` / `mise run test
  synvelis-project` / `mise run test-rt` / `mise run build
  synvelis-project` / `mise run check-full` /
  `mise exec -- prek run --all-files` — 全て exit 0
  （`evidence/sv458/verification.md` に実測記録）
