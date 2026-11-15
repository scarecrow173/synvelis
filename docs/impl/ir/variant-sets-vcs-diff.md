---
id: "IMPL-034XGSIKsnCqwLSMEFDsUx"
type: "impl"
status: "completed"
title: "variant_sets VCS semantic diff 分類 slice 実装記録"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/457"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/vcs07_variant_diff.rs"
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
    - "docs/tasks/variant-sets-vcs-diff.md"
    - "docs/plans/variant-sets-vcs-diff.md"
    - "docs/designs/variant-sets-vcs-diff.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/variant-sets-vcs-diff.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
    - "docs/impl/ir/plugin-instances-vcs-diff.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/variant-sets-vcs-diff.md"
    - "docs/plans/variant-sets-vcs-diff.md"
  derived-by: []
  verifies:
    - "docs/test-specs/variant-sets-vcs-diff.md"
  verified-by:
    - "docs/test-specs/variant-sets-vcs-diff.md"
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/458"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# variant_sets VCS semantic diff 分類 slice 実装記録

## Summary

Issue #457（VCS-02 umbrella・discovery
`variant-sets-vcs-diff-gap`）の実装 slice として、schema v14 の
`Project::variant_sets`（synvelis#278 CB-01 Creative Branch
モデル）を VCS-02 semantic diff の比較対象 entity 集合へ分類
追加した。`ObjectKind::VariantSet`/`VariantBranch` を enum 末尾に
append し、新 `ComparisonDomain::Variants`・`VariantsAdapter`
（set `diff_map` + branch の `MidiNote` 型 nested-global diff）・
`classify_variants` field policy・`raw_kind_map` 登録・
`impl_entity_key!`・`render_input` canonical value 解決を整備。
`DIFF_POLICY_VERSION` を 3→4 に bump。apply surface は
`Unsupported` fail-closed のまま変更なし。

## Context

synvelis#97（VCS-06 fixture matrix、merge `34387c4`）が
`variant_sets` の compare-surface gap を `vcs06_variant_matrix.rs`
の 2 test と ledger `VCS-DIFF-11 Blocked`/`VCS-APPLY-10 Blocked`
で正直 pin した。本 slice はその gap を閉じ、ledger を
`VCS-DIFF-11 Pass` に flip する。設計は
`docs/designs/variant-sets-vcs-diff.md`（D1–D9）に確定済み。

## Implementation

### Changes

- `diff/types.rs`: `ComparisonDomain::Variants`（serde
  `"variants"`）を末尾 append、`ComparisonDomains::all()` に追加。
  `ObjectKind::VariantSet`/`VariantBranch` を `PluginInstance`
  の後に append（既存 discriminant 維持）。`domain()` で両 kind
  を `Variants` に写像。`DIFF_POLICY_VERSION` = 4（v4 rationale
  コメント追加）。
- `diff/engine.rs`: `impl_entity_key!` に `VariantSetId`/
  `BranchId`。`raw_kind_map` に `variant_sets.keys()`→
  `VariantSet`、各 set の `branches.keys()`→`VariantBranch` を
  登録（entity-issued ID — keyed-synthetic 除外規約の対象外）。
- `diff/adapters.rs`: `classify_variants`（set: `id`/`branches`
  Skip・`origin`/`scope` Replaced・他 Modified。branch:
  `id` Skip・`parent` Identity・`created_from`/`content`
  Replaced・他 Modified）。`VariantsAdapter` — set を `diff_map`、
  branch を全 set 横断 `BranchId` index で `MidiNote` 型
  nested-global diff（owner = `VariantSetId`、owner 変更は
  `OwnerConflict` ambiguous）。`default_adapters()`・
  `classify_field` domain arm に登録。
- `render_input.rs`: `VariantSet` → `keyed_value!`、
  `VariantBranch` → `MidiNote` 型 nested-special
  （`variant_sets.values().find_map`）。`all_semantic_objects`
  inventory には非搭載（variant state は render input ではない
  — design D6）。
- `tests/vcs07_variant_diff.rs`: 新規 7 test — presence
  Added/Removed・adopt swap・content ref swap・policy v4・
  cross-kind collision・domain exclusion・apply fail-closed。
- `tests/vcs06_variant_matrix.rs`: 2 diff pin を新契約に flip
  （rename 込み）。restore-as-copy test は untouched（#458）。
- `tests/vcs02_diff.rs`: `DIFF_POLICY_VERSION` pin 3→4 +
  コメント整合（test 名は v3 導入履歴 anchor として維持）。
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`:
  `VCS-DIFF-11` Blocked→Pass flip、`VCS-APPLY-10`/`VCS-
  RESTORE-10` 維持、env/test 一覧更新。
- `evidence/sv457/`: verification evidence。

### Decisions During Implementation

- design D1–D9 をそのまま実装。field class は design D3 の表
  どおり（`origin`/`scope`/`created_from`/`content` →
  `Replaced`、`parent` → `Identity`、bookkeeping → `Modified`、
  `id`/`branches` → `Skip`）。
- `scope` を `Replaced` としたのは design 確定値 — governed
  subject の identity-defining 参照として `origin` と同格。
- schema suppression なし（design D5）— `variant_sets` は
  pass-through migration で pre-v14 空 map は TRUE absence。
- `render_input` inventory 非搭載（design D6）—
  `shared_context_digest` の過剰 invalidation を回避。

## Related Experiments

### Adopted

`docs/impl/exp/variant-sets-vcs-diff.md` — kind 数・domain
帰属・schema suppression・render-input inventory・apply 境界の
5 つの設計判断を検証メモとして記録。全て design D1–D9 の
結論と一致したため採用。

### Rejected

なし。

## Validation

- `mise exec -- cargo test -p synvelis-project --test
  vcs07_variant_diff --test vcs06_variant_matrix` — 10 tests
  pass（vcs07 7 + vcs06 3）。
- `mise exec -- cargo test -p synvelis-project --test vcs02_diff`
  — 36 tests pass。
- canonical 検証（`mise run fmt-check`/`check`/`lint`/`test`/
  `build`/`test-rt`/`check-full`/`prek`）は PR 本文の検証表に
  実測値を記録。`evidence/sv457/` に証跡。

## Risks

- `ObjectKind`/`ComparisonDomain` は cross-crate public 型 —
  `check-full` で `synvelis-render`/`synvelis-workspace` の
  consumer 非回帰を確認済み（compile 通過）。
- variant change の apply transplant は未実装 — `Unsupported`
  fail-closed を test で pin。別 Issue で対応。
- `restore_as_copy` の nested origin defect（#458）は未解決 —
  ledger `VCS-RESTORE-10` は `Fail` のまま。

## Follow-ups

- synvelis#458: `fork_project` の nested variant origin identity
  再採番（restore-as-copy 修復）。本 PR landing が formal
  blockedBy を unblock。
- variant change の apply surface transplant（別 Issue —
  ledger `VCS-APPLY-10` は `Blocked` のまま）。
