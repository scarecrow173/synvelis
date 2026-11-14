---
id: "SPEC-034XGzLZmToUqCCAnOgwpK"
type: "spec"
status: "approved"
title: "variant_sets VCS semantic diff 分類 — VariantSet/VariantBranch kinds (synvelis#457)"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/457"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/vcs-06-variant-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
  derived-by:
    - "docs/designs/variant-sets-vcs-diff.md"
  verifies: []
  verified-by:
    - "docs/test-specs/variant-sets-vcs-diff.md"
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/458"
  deferred-by: []
metadata: {}
---

# variant_sets VCS semantic diff 分類 — VariantSet/VariantBranch kinds (synvelis#457)

## Context

synvelis#97（VCS-06 fixture matrix）の実測で、schema v14 の
`Project::variant_sets`（synvelis#278 CB-01 Creative Branch
モデル）は VCS-02 semantic diff のどの domain adapter からも
列挙されず、`ObjectKind` に variant kind が存在しないことが
確定した（`docs/discovery/variant-sets-vcs-diff-gap.md`）。
その結果、variant-set のみ差分の snapshot 対は `identity.digest`
が異なるのに `changes = 0`・`Equal`/`Complete` を返し、mixed
diff では variant 差分が silent drop されていた。本 spec は
その compare-surface gap を閉じる分類契約を定義する。

## Requirements

- `ObjectKind` に `VariantSet`・`VariantBranch` を**末尾
  append** で追加する（既存 discriminant を維持 — `change_id`・
  `issue_side_key` が `object.kind as u8` を bake するため）。
- 新 `ComparisonDomain::Variants` を末尾 append で追加し、
  `ComparisonDomains::all()`・`default_adapters()`・
  `classify_field` の domain arm を整備する。`VariantScope` が
  Arrangement/Routing/Comping/Control に跨るため単一既存
  domain への帰属はしない。
- `VariantsAdapter` は `project.variant_sets` を `diff_map` で
  `VariantSet` entity として比較し、`set.branches` を `MidiNote`
  型の nested-global index で `VariantBranch` entity として比較
  する（owner = 所属 `VariantSetId`、owner 変更は
  `AmbiguousIdentity{OwnerConflict}`）。
- `VariantSetId`/`BranchId` は `fork_variant_set` が
  `controller.draw_raw_id()` で発行する entity-issued ID であり
  keyed-synthetic ではないため、`raw_kind_map` に登録し
  cross-kind raw 衝突を `AmbiguousIdentity{CrossKindId}` に落とす。
- field class: set の `origin`/`scope` は `Replaced`（identity-
  defining 参照）、`adopted`/`revision`/`origin_objects`/
  `last_adopt` は `Modified`、`id`/`branches` は `Skip`。branch
  の `parent` は `Identity`（lineage 参照）、`created_from`/
  `content` は `Replaced`（provenance/content-identity）、
  `id` は `Skip`、他は `Modified`。
- `variant_sets` は `#[serde(default)]` + v13→v14 pass-through
  migration で、pre-v14 側の空 map は TRUE absence — suppression
  /`PartialProjection` は不要（`plugin_instances` の migration-
  minted `id` とは構造が異なる）。
- `DIFF_POLICY_VERSION` を 3→4 に bump する（variant-only
  `Equal`→`Different` は observable change で `ComparisonKey`
  collision 規約上必須）。
- apply surface は変更しない — `supported_kind` が新 kind を
  `Unsupported` fail-closed で扱い続けることを test で pin する
  （variant change の transplant は本 Issue 対象外）。
- `restore_as_copy` の nested origin identity defect は
  synvelis#458 scope — 本 slice では触らない。

## Acceptance Criteria

- variant-set のみ差分の snapshot 対が `Equal`/`Complete` ではなく
  typed `VariantSet`/`VariantBranch` change を伴う `Different`
  を返すことが test で検証される。
- set Added/Removed・branch Added/Removed・set/branch field
  変更・adopt swap・`BranchContentRef` swap が期待する
  `ChangeKind`/field evidence で観測される。
- `Variants` domain を `ComparisonSpec.domains` から外すと
  variant-only 差分が再び `Equal` になる（domain filter 契約）。
- `VariantSetId`/`BranchId` と他 entity の cross-kind raw 衝突が
  `AmbiguousIdentity{CrossKindId}` になり `Equal` に coerce
  されない。
- variant change の `target_fingerprint`/`apply_selected` が
  `ApplyError::Unsupported` で fail-closed し project 不変。
- 既存 VCS-02/VCS-06 契約の非回帰（policy pin 更新を除く）。
