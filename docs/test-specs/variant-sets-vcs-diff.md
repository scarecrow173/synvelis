---
id: "TSPEC-034XGAv6yF1NqnHsAoShC4"
type: "test-spec"
status: "approved"
title: "variant_sets VCS semantic diff fixture matrix"
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
    - "docs/specs/variant-sets-vcs-diff.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/variant-sets-vcs-diff.md"
  derived-by: []
  verifies:
    - "docs/designs/variant-sets-vcs-diff.md"
    - "docs/specs/variant-sets-vcs-diff.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata: {}
---

# variant_sets VCS semantic diff fixture matrix

## Purpose

synvelis#97 が `Blocked` として正直 pin した variant_sets
compare-surface gap（`docs/discovery/variant-sets-vcs-diff-gap.md`）
が synvelis#457 で閉じたことを、headless fixture で恒久的に
保証する。実装を退行させる変更（kind 除去・domain 帰属変更・
field class 変更・policy 戻し）は本 spec の失敗として検出する。

## Feature

`compare_snapshots` は `Project::variant_sets` を
`VariantSet`/`VariantBranch` entity として `Variants` domain で
比較し、presence・field 変更・identity 衝突・domain filter・
apply 境界を既存 VCS-02 契約どおりに報告する。

## Rules

- variant-set/branch の挿入・削除は `Added`/`Removed` の typed
  change（`variant-set-`/`branch-` tagged stable ID）。
- adopt swap は set 上の `Modified`（`adopted`/`revision`/
  `last_adopt` field evidence）+ 両 branch の `content` の
  `Replaced`。
- `BranchContentRef` のみの変更は `VariantBranch` `Replaced`
  （`content.*` field evidence）で set は無変更。
- variant-only 差分は `Different` で `cs.comparison.
  policy_version`/`cs.key.policy_version` が
  `DIFF_POLICY_VERSION`（=4）と一致、`key.domains` に
  `Variants` を含む。
- `VariantSetId`/`BranchId` は `raw_kind_map` 登録済み —
  他 entity kind と raw が衝突すると `AmbiguousIdentity
  {CrossKindId}` で `Equal` に coerce しない。
- `ComparisonSpec.domains` から `Variants` を外すと
  variant-only 差分は `Equal`（domain filter 契約）。
- apply surface は `VariantSet`/`VariantBranch` を
  `ApplyError::Unsupported` で fail-closed し project 不変。

## Examples

- Given track のみの project、When `fork_variant_set` で set +
  2 branch を追加、Then diff は `Different`・`VariantSet`
  `Added`×1 + `VariantBranch` `Added`×2・`Complete`・0 unresolved。
- Given fork 済み set、When `adopt_branch` で candidate を adopt、
  Then set `Modified`（`adopted` field あり）+ branch `Replaced`×2。
- Given adopted `Live` branch、When `content.revision` のみ bump、
  Then `VariantBranch` `Replaced`（`content.revision`）・set 無変更。
- Given variant set の raw が track と衝突する crafted pair、
  Then `AmbiguousIdentity{CrossKindId}`×4・changes 0・`Unknown`。
- Given variant change を含む ChangeSet、When `target_fingerprint`/
  `apply_selected` を実行、Then `ApplyError::Unsupported`・
  `live_revision`/`variant_sets` 不変。

## Guarantees

- silent `Equal`/`Complete`（0 changes）への回帰がない。
- mixed diff で variant change が drop されない。
- 既存 kind の `ChangeKind`/field class/discriminant は不変。
- `vcs06_variant_matrix.rs` の restore-as-copy Fail pin
  （#458 scope）は変更されない。

## Non-goals

- variant change の apply transplant（別 Issue）。
- `restore_as_copy` の nested origin identity 修復（#458）。
- `BranchContentStore` bytes 自体の diff。
- `render_input` の `all_semantic_objects` inventory 搭載
  （variant state は render input ではない — design D6）。
- real GUI/OS/consumer lane。

## Risk

保証を落とすと synvelis#97 が記録した「variant 差分の silent
`Equal`/drop」が再発し、VCS 履歴・apply の前提である
「差分の完全性」が静かに壊れる。`VCS-DIFF-11` が再び
`Blocked`/誤 `Pass` に戻る。
