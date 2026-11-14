---
id: "DISC-034XEEHyrtM1jt747O7iQD"
type: "discovery"
status: "resolved"
title: "variant_sets VCS semantic diff gap — schema v14 persisted variant state invisible to compare_snapshots"
created: "2026-11-14"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "https://github.com/scarecrow173/synvelis/issues/278"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/designs/plugin-instances-vcs-diff-design.md"
    - "docs/discovery/plugin-instances-vcs-diff-gap.md"
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
    - "docs/designs/vcs-02-semantic-diff-engine-design.md"
    - "docs/designs/creative-branch-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "docs/discovery/plugin-instances-vcs-diff-gap.md"
  derived-by:
    - "https://github.com/scarecrow173/synvelis/issues/457"
    - "https://github.com/scarecrow173/synvelis/issues/458"
    - "docs/specs/vcs-06-variant-matrix.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# variant_sets VCS semantic diff gap — schema v14 persisted variant state invisible to compare_snapshots

## Summary

`Project::variant_sets`（schema v14・sv#278 CB-01）は VCS-02
semantic diff のどの domain adapter からも列挙されず、
`ObjectKind` に variant kind が存在しない。variant-set のみが
異なる snapshot 対は `identity.digest` が異なるのに `changes = 0`・
`Equal`/`Complete` を返し、mixed diff（variant 差分 + 通常 entity
差分）では variant 差分が silent drop される。さらに
`restore::fork_project` が `project_id` のみ再採番し nested
`VariantSet::origin`/`Branch::created_from` の
`SnapshotIdentity.project_id` を書き換えないため、
`restore_as_copy` は variant set を含む project を
`RestoreError::Validation` で fail-closed 拒否する（実測 Fail）。
結論: 本 slice（VCS-06 fixture matrix 拡張）の目的は gap の正直な
記録であり、variant diff adapter 本体は VCS-02 系の別 Issue
（#457）・restore defect 修復は別 Issue（#458）に譲る。

## Exploration Goal

VCS-06（#97）の fixture matrix に schema v14 `variant_sets` domain
を追加するにあたり、以下を実測で確定する:

- `variant_sets` が semantic diff の compare surface に載るか、
  載らない場合の観測値（`Equal`/`Complete` か `Unknown`/`Partial`
  か）と silent drop の有無。
- `restore_as_copy` が `variant_sets` を verbatim 保持するか、
  nested identity の `project_id` 不整合で fail-closed するか。
- 観測された挙動を Pass に偽装せず、ledger の Case 行として
  どう記録するか（`Blocked`/`Fail` の正直記録）。

## Key Issues and Assumptions

- `identity.digest` は serialized `SnapshotEnvelope` 全体
  （`Project` 本体を含む）の SHA-256 であり、`variant_sets` を
  完全にカバーする。`variant_sets` のみ差分でも digest は異なる。
- diff engine は curated projection: domain adapter が既知 entity
  map のみを列挙し、`variant_sets` はどの adapter の view にも
  含まれない。`src/diff/` に `variant`/`variant_sets` の参照は
  ゼロ（grep 実測）。
- `ObjectKind` enum の末尾は `PluginInstance`（schema v13・
  synvelis#324）— variant kind は存在しない。`raw_kind_map` も
  `variant_sets` を列挙しない。
- `VariantSet` は `origin: SnapshotIdentity`・
  `branches: BTreeMap<BranchId, Branch>`（各 branch は
  `created_from: SnapshotIdentity`・`content`・lineage）・
  `adopted`・`revision`・`origin_objects`・`last_adopt`・`scope`
  を永続化する。`BranchContentStore` が branch content bytes を
  別途保持し、`Branch.content` は `BranchContentRef` で参照する。
- `validate_set_shape`（`variant/engine.rs`）は
  `VariantSet::origin.project_id == project.project_id` と全
  `Branch::created_from.project_id == project.project_id` を要求
  する — fork で `project_id` だけ再採番するとこの不変条件が
  破れる。
- `restore::fork_project` は `project().clone()` +
  `project_id` 再採番のみ（`restore/mod.rs:707-711`）— nested
  identity を再帰的に書き換えない。
- `restore_as_copy` は publish 前に staged envelope の
  `validate_references`（→ `validate_persisted` →
  `validate_set_shape`）を実行する — validation 失敗は
  `RestoreError::Validation` で fail-closed。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| A. 本 slice で gap を正直記録し adapter 実装は別 Issue（#457）へ defer | fixture が現行契約を pin し、silent `Equal`/silent drop/restore fail-closed を `Blocked`/`Fail` として ledger に固定。`src/` 変更なしで VCS-06 coverage gap を可視化 | variant diff は当面観測不能のまま | yes |
| B. 本 slice で `ObjectKind::VariantSet`/`VariantBranch` adapter を実装 | gap を即座に解消 | `src/diff/` 変更はタスク範囲外（「variant diff adapter 本体実装は VCS-02 系の別 Issue」と明示）。`VariantSet` の nested `SnapshotIdentity`・`BranchContentStore` 参照・schema suppression・`DIFF_POLICY_VERSION` bump の設計判断が必要で、本 slice の fixture 拡張とは別の設計単位 | no |
| C. restore defect（#458）も本 slice で修復 | VCS-RESTORE-10 を Pass にできる | `src/restore/` 変更は範囲外。修復方針（nested identity 再採番 vs validate 規則見直し）は CB-01 の origin 意味論の設計判断を要し、diff adapter（#457）との整合も必要 | no |
| D. 観測値を `Pass` に読み替えて ledger に記録 | coverage 追加に見える | silent `Equal`・silent drop・restore 拒否を成功と偽装することになり、VCS-06 ledger の「実測のみ記録」原則に反する | no |

## Tentative Conclusions and Hypotheses

- **confirmed — diff gap は実在し到達可能**: `src/diff/` に
  `variant_sets` 参照ゼロ。`variant_sets` のみ差分の snapshot 対は
  `overall = Equal`・`completeness = Complete`・0 changes・
  0 unresolved を返す（`vcs06_variant_matrix.rs` で実測 pin）。
  `identity.digest` 差・`variant_sets` 実差を assert し AA fast
  path でないことを確認済み。
- **confirmed — mixed diff で variant 差分は silent drop**:
  variant set + track rename の mixed diff は `Different` を返すが
  changes は Track `Modified` 1 件のみ — variant 差分は change
  として emit されず、consumer は「一部しか変わっていない」と
  誤認し得る。
- **confirmed — mutation も不可視**: `fork_variant_set`（set
  作成）→ `adopt_branch`（adopted 付け替え）という authored op
  のみの差分でも `Equal`/`Complete` — variant state の変化は
  diff に一切現れない。
- **confirmed — restore は fail-closed で拒否**: `fork_project`
  が `project_id` のみ再採番するため nested
  `SnapshotIdentity.project_id` が source のまま残り、
  `validate_set_shape` が `origin belongs to a different project`
  で `RestoreError::Validation`。publish 前に拒否されるため
  silent corruption はないが、variant set を持つ project の
  restore-as-copy は一切成功しない capability gap（実測 Fail）。
- **hypothesis → #457/#458 で確定**: `VariantSet`/`Branch` の
  ObjectKind・domain・field 分類・`SnapshotIdentity` の diff
  扱い・`BranchContentStore` 参照の観測方法・schema suppression・
  `DIFF_POLICY_VERSION` bump は #457 の設計判断。restore 側は
  nested identity 再採番 vs validate 規則見直しの境界判断を
  #458 で行う（`SnapshotIdentity` が「どの snapshot を指すか」の
  参照なら fork 後も source を指し続けるのが意味的に正しく、
  validate 規則側の見直しが正しい境界の可能性がある）。

## Open Questions

- `VariantSet`/`Branch` をどの `ObjectKind`・`ComparisonDomain`
  に写像するか — `VariantSetId`/`BranchId` は独自 ID 空間で
  `EntityKey` 実装が要る可能性（#457 で確定）。
- `origin`/`created_from` の `SnapshotIdentity`（`project_id`/
  `digest` を含む）は diff 対象 field か、identity 参照として
  `Replaced`/`Suppressed` のどちらに分類するか（#457）。
- `BranchContentStore` の content bytes は snapshot envelope 外
  — `BranchContentRef` の参照差のみで足りるか、store 側の観測
  経路が要るか（#457）。
- apply surface: diff が variant change を emit した後の
  `supported_kind`/transplant 設計 — nested `SnapshotIdentity`
  再採番・`BranchContentStore` bytes の fork 先複製が必要で
  VCS-05 系の別設計（#457 の Deferred）。
- restore defect の修復方針 — `fork_project` の nested identity
  再採番 vs `validate_set_shape` 規則見直し（#458）。

## Promotion Candidates

- [x] GitHub Issue: variant diff adapter — Issue #457
  （`https://github.com/scarecrow173/synvelis/issues/457`）を
  `.github/ISSUE_TEMPLATE/task.yml` 準拠で起票済み。
- [x] GitHub Issue: restore defect — Issue #458
  （`https://github.com/scarecrow173/synvelis/issues/458`）を
  同テンプレートで起票済み。
- [x] spec-doc: `docs/specs/vcs-06-variant-matrix.md` — 本 slice
  自体の spec（gap の正直記録が目的であり、adapter 実装 spec
  ではない）。
- [ ] design-doc / adr-doc: variant diff adapter の設計判断は
  #457 の slice で行う（本 discovery は gap 記録に留める）。

## Sources and Relations

- `https://github.com/scarecrow173/synvelis/issues/97` — VCS-06
  統合受入 Issue、本 slice の親。
- `https://github.com/scarecrow173/synvelis/issues/278` — CB-01
  variant model、`variant_sets` 永続化 domain の由来。
- `https://github.com/scarecrow173/synvelis/issues/324` —
  plugin_instances の VCS diff 分類追加（同型先例・CLOSED）。
- `https://github.com/scarecrow173/synvelis/issues/457` — 本
  discovery から派生した variant diff adapter Issue。
- `https://github.com/scarecrow173/synvelis/issues/458` — 本
  discovery から派生した restore defect Issue。
- `docs/specs/vcs-02-semantic-diff-changeset.md` — semantic
  対象/対象外・tri-state・suppression 規約の正本。
- `docs/designs/vcs-02-semantic-diff-engine-design.md` /
  `docs/designs/plugin-instances-vcs-diff-design.md` /
  `docs/designs/creative-branch-design.md` — adapter/keyed-
  synthetic 規約・CB-01 variant model の設計正本。
- `docs/discovery/plugin-instances-vcs-diff-gap.md` — 同型の
  gap 記録先例。
- `crates/synvelis-project/tests/vcs06_variant_matrix.rs` —
  観測値 pin fixture（VCS-DIFF-11・VCS-RESTORE-10）。

## Evidence

### 2026-11-15 — code-path 計測（Equal 誤報の成立条件）

`compare_snapshots`（`crates/synvelis-project/src/diff/engine.rs`）:

- `before.identity.project_id != after.identity.project_id` →
  `AmbiguousIdentity(CrossProject)` で `Unknown`。
- `before.identity.digest != after.identity.digest` のときだけ
  adapter が走る。digest は serialized `SnapshotEnvelope` 全体の
  SHA-256（`project` 本体を含み `variant_sets` をカバー）。
- `default_adapters()` の domain adapter は
  `variant_sets` を読まない（`grep variant crates/.../src/diff`
  → 0 件）。
- `ObjectKind` enum（`src/diff/types.rs`）末尾は `PluginInstance`
  — variant kind 無し。`raw_kind_map`（`engine.rs`）も
  `variant_sets` を列挙しない。
- 全 adapter が change 0・issue 0 → `overall = Equal`、
  `completeness = Complete`。

実測（`cargo test -p synvelis-project --test
vcs06_variant_matrix` — 3 tests pass、Windows・rustc 1.99.1）:

- `variant_set_presence_and_mutation_are_invisible_to_diff`:
  `variant_sets` 空 → `fork_variant_set` で set 作成（adopted
  live branch + captured candidate branch）→ `adopt_branch` で
  adopted 付け替え、の 2 段階の authored op 差分で
  `overall=Equal`・`completeness=Complete`・0 changes・
  0 unresolved。`identity.digest` 差と `variant_sets` map の
  実差（`len`/`id`/`revision`/`adopted`）を assert。
- `mixed_diff_reports_track_change_but_drops_variant_set`:
  variant set 作成 + track rename の同時差分で `Different`・
  changes は Track `Modified`（`name` field evidence）のみ —
  variant 差分は emit されない。
- `restore_as_copy_fails_closed_on_variant_set_origin_mismatch`:
  後述。

### 2026-11-15 — restore fail-closed の成立条件

`restore::fork_project`（`src/restore/mod.rs:707-711`）:

```rust
let mut fork = snapshot.projection.project().clone();
fork.project_id = new_id;
fork
```

`project_id` のみ再採番。`variant_sets` 以下の nested identity
（`VariantSet::origin`・全 `Branch::created_from`）は source
project の `project_id` を保持したまま clone される。

`validate_set_shape`（`src/variant/engine.rs:405-474`）:

- L432-436: `branch.created_from.project_id !=
  project.project_id` → `Error::Validation("branch {bid} origin
  belongs to a different project")`
- L461-466: `set.origin.project_id != project.project_id` →
  `Error::Validation("variant set {} origin belongs to a
  different project")`

`restore_as_copy` は publish 前に staged envelope の
`validate_references`（`restore/mod.rs:1237`・`1397`）を実行し、
`Project::validate_references` → `validate_persisted`（
`variant/engine.rs:477`）→ `validate_set_shape` の経路で拒否。
結果: `Err(RestoreError::Validation)`、staging 保持・destination
不在・source 不変の fail-closed。envelope readback による
verbatim 確認は restore 成功が前提のため未実施（VCS-RESTORE-10
は **Fail** として ledger に記録）。

`variant_sets` が空の project では nested identity が存在しない
ため影響なし — `BTreeMap` 空なら `validate_persisted` の loop が
回らない。

### 2026-11-15 — `SnapshotIdentity` の意味論（#458 の設計論点）

`SnapshotIdentity` は `project_id`・`revision`・`digest`・
`origin` を持つ「どの snapshot を指すか」の参照。variant set の
`origin`/`created_from` は fork 元 snapshot を指す authored
provenance であり、fork 後 project でも「source project の
snapshot X から派生した」という事実は変わらない。したがって
`project_id` だけを書き換えると「別 project の同じ digest の
snapshot」を指す虚偽の参照になる可能性があり、修復は
(a) `fork_project` が nested identity を再帰的に書き換える、
(b) `validate_set_shape` が「enclosing project と異なる
`project_id` の origin を持つ variant set」を fork 産と見なして
許容する、のどちらが CB-01 の origin 意味論と整合するかは
#458 の設計判断（`SnapshotIdentity` 全体の整合 — `digest`/
`origin` 参照 — も検討要）。
