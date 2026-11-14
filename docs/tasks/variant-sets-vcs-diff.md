---
id: "TASK-034XGAb53fij7Xn1AyjwnA"
type: "task"
status: "completed"
title: "variant_sets VCS semantic diff 分類"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/457"
  changes:
    added:
      - type: "file"
        path: "docs/specs/variant-sets-vcs-diff.md"
      - type: "file"
        path: "docs/designs/variant-sets-vcs-diff.md"
      - type: "file"
        path: "docs/plans/variant-sets-vcs-diff.md"
      - type: "file"
        path: "docs/tasks/variant-sets-vcs-diff.md"
      - type: "file"
        path: "docs/test-specs/variant-sets-vcs-diff.md"
      - type: "file"
        path: "docs/impl/ir/variant-sets-vcs-diff.md"
      - type: "file"
        path: "docs/impl/exp/variant-sets-vcs-diff.md"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs07_variant_diff.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/diff/types.rs"
      - type: "file"
        path: "crates/synvelis-project/src/diff/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/src/diff/adapters.rs"
      - type: "file"
        path: "crates/synvelis-project/src/render_input.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_variant_matrix.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs02_diff.rs"
      - type: "file"
        path: "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/variant-sets-vcs-diff.md"
  implemented-by:
    - "docs/impl/ir/variant-sets-vcs-diff.md"
  depends-on:
    - "docs/plans/variant-sets-vcs-diff.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/variant-sets-vcs-diff.md"
  verified-by: []
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/458"
  deferred-by: []
metadata: {}
---

# variant_sets VCS semantic diff 分類 — task

## Tasks

- [x] `diff/types.rs`: `ComparisonDomain::Variants`・
  `ObjectKind::VariantSet`/`VariantBranch` 末尾 append・
  `domain()`・`DIFF_POLICY_VERSION`=4
- [x] `diff/engine.rs`: `impl_entity_key!`・`raw_kind_map` 登録
- [x] `diff/adapters.rs`: `classify_variants`・`VariantsAdapter`・
  `default_adapters`・`classify_field` arm
- [x] `render_input.rs`: canonical value 解決（inventory 非搭載）
- [x] `tests/vcs07_variant_diff.rs`: 7 test 新規
- [x] `tests/vcs06_variant_matrix.rs`: 2 diff pin flip
- [x] `tests/vcs02_diff.rs`: policy pin 3→4
- [x] ledger `VCS-DIFF-11` flip + env/test 一覧更新
- [x] `evidence/sv457/` verification evidence
- [x] docs index 再生成（doc-maintenance）

## Out of scope

- apply surface transplant（`Unsupported` fail-closed 維持・pin のみ）
- `restore_as_copy` nested origin defect（synvelis#458）
- `src/variant/`・`src/restore/`・`src/apply/` 本体変更
