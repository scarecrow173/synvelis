---
id: "DESIGN-034XGA5y1ft0IHe1TEUxj5"
type: "design"
status: "approved"
title: "variant_sets VCS semantic diff 分類設計 — VariantSet/VariantBranch + Variants domain"
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
  implements:
    - "docs/specs/variant-sets-vcs-diff.md"
  implemented-by:
    - "docs/impl/ir/variant-sets-vcs-diff.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
    - "docs/designs/plugin-instances-vcs-diff-design.md"
    - "docs/designs/vcs-06-variant-matrix-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/variant-sets-vcs-diff.md"
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
  derived-by:
    - "docs/plans/variant-sets-vcs-diff.md"
  verifies: []
  verified-by: []
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/458"
  deferred-by: []
metadata: {}
---

# variant_sets VCS semantic diff 分類設計 — VariantSet/VariantBranch + Variants domain

## Context

`docs/discovery/variant-sets-vcs-diff-gap.md`（synvelis#97 で
merge・status resolved）が確定した gap: `Project::variant_sets`
（schema v14・persisted authored state）は VCS-02 semantic diff
の compare surface に存在せず、variant-only 差分が `Equal`/
`Complete`・0 changes を返し、mixed diff で silent drop される。
本 design は discovery の Open Questions 全てに結論を出し、
`plugin-instances-vcs-diff`（synvelis#324）の doc chain を
template として分類を確定する。

## Decisions

### D1: 2 kinds — `VariantSet` + `VariantBranch`（discovery 有力案を採用）

`ObjectKind` 末尾に `VariantSet`・`VariantBranch` を append
（`PluginInstance` の後 — `change_id`/`issue_side_key` が
`object.kind as u8` を bake するため既存 discriminant 維持）。

- `VariantSet` は `project.variant_sets` を `diff_map` で比較。
  `branches` field は `Skip`（branch-level diffing に委譲）。
- `VariantBranch` は `set.branches` を全 set 横断で `BranchId`
  index 化して比較 — `MidiNote`（`midi_sources[].notes`）と同型の
  nested-global diff。owner = 所属 `VariantSetId` を記録し、
  owner が変わった同一 `BranchId` は `AmbiguousIdentity
  {OwnerConflict}`（move ではない）。

1-kind（set whole-object）案は `branches` map 全体を set の
field diff に埋め込み、branch 単位の Added/Removed/`Replaced`
分類・owner 衝突検出・`raw_kind_map` の `BranchId` 登録が全て
粗くなる。2-kind 案は entity 粒度・安定 ID・field evidence が
既存 kind と同じ精度で出るため採用。

### D2: 新 `ComparisonDomain::Variants`（末尾 append）

`VariantScope` は Arrangement/Routing/Comping/Control の対象を
跨ぐ宣言であり、単一既存 domain への帰属は scope 対象の編集が
別 domain の変更として観測される不整合を生む。新 domain
`Variants`（serde `"variants"`）を末尾 append し、
`ComparisonDomains::all()`・`default_adapters()`・
`classify_field` の `match kind.domain()` に
`classify_variants` arm・`VariantsAdapter` を追加。
`VariantSet`/`VariantBranch` の `domain()` は両方 `Variants`。

### D3: field class（discovery Open Question への結論）

`VariantSet`（`classify_variants`、top-level path segment で判定）:

| field | class | 根拠 |
| --- | --- | --- |
| `id` | `Skip` | map key と重複 |
| `branches` | `Skip` | `VariantBranch` entity が個別に diff される |
| `origin` | `Replaced` | fork-origin `SnapshotIdentity` — set の provenance/content-identity 参照。`Clip.source_id`/`Asset.digest`/`PluginInstance.id` と同型 |
| `scope` | `Replaced` | governed subject の identity-defining 参照 — scope swap は set が「何を支配するか」の置換 |
| `adopted`/`revision`/`origin_objects`/`last_adopt` | `Modified` | authored bookkeeping — adopt swap・mutation receipt は通常の属性変更 |

`VariantBranch`:

| field | class | 根拠 |
| --- | --- | --- |
| `id` | `Skip` | map key と重複 |
| `parent` | `Identity` | fork lineage 参照 — 同一 `BranchId` で parent が変わると entity 同一性を確認できない → `AmbiguousIdentity{OwnerConflict}` |
| `created_from` | `Replaced` | fork provenance（`SnapshotIdentity`） |
| `content` | `Replaced` | `BranchContentRef` — `payload_digest`/`scope_digest`/`schema_version`・`Live`↔`Captured` 遷移は candidate content-identity の置換。revision bump を含むあらゆる content 変更が `Replaced`（adopt freeze・candidate edit 共通） |
| `lineage` | `Modified` | `BranchLineage.local_ids` の成長は authored provenance 記録 |

`SnapshotIdentity` field（`origin`/`created_from`）の digest
揺れ: `SnapshotIdentity` は `project_id`/`digest`/`schema_version`/
`projection_version`/`locator` 等を含むが、compare 対象は同一
`project_id` の 2 snapshot のみ（cross-project は engine が
`CrossProject` で先に落とす）。digest/revision の差は provenance
主張の実差であり noise ではない — `Replaced` で観測し field
evidence に残す。`content` の `BranchContentRef` も同様に参照
比較のみで十分: `BranchContentStore` bytes は envelope 外で、
store 側観測経路の新設は本 slice 対象外（pin digest が変われば
`Replaced`、bytes 内容の diff は store/restore 系の責務）。

### D4: `raw_kind_map` 登録（keyed-synthetic ではない）

`VariantSetId`/`BranchId` は `variant/engine.rs` の
`fork_variant_set` が `controller.draw_raw_id()`（=
`id_watermark` 空間）から発行する entity-issued ID。keyed-
synthetic 収集（`smf_documents`/`applied_commits`/
`monitoring.tracks`/`plugin_instances.instances` — 他 entity の
ID 空間を借用）は `raw_kind_map` から除外する規約だが、variant
ID は独立発行なので **登録対象**。`raw_kind_map` に
`variant_sets.keys()`→`VariantSet`、各 set の
`branches.keys()`→`VariantBranch` を追加し、既存
`ambiguous_raws` ロジックがそのまま cross-kind 衝突を
`AmbiguousIdentity{CrossKindId}` に落とす。`impl_entity_key!`
に両 ID を追加。

### D5: schema suppression なし（plugin_instances との非対称）

`variant_sets` は `#[serde(default)]` + v13→v14 **pass-through**
migration — v14 導入以前の schema には variant model が存在
せず、migration が値を mint しない。pre-v14 側の空 map は
「まだ authored されていない」TRUE absence であり、
`plugin_instances`（v12→v13 migration が `id` を backfill し
pre-v13 側の値が migration-minted projection artifact）のような
`Suppressed`/`PartialProjection` は不要。v14 以降で fork され
た set が pre-v14 snapshot に対して `Added` として観測される
のは authored history として正しい。

### D6: `render_input.rs` — canonical value 解決のみ、inventory 非搭載

`canonical_keyed_entity_value`/`canonical_special_entity_value`
の exhaustive match を compile-safe に埋める
（`VariantSet` → `keyed_value!(project.variant_sets, VariantSetId)`、
`VariantBranch` → `MidiNote` 型 nested-special:
`variant_sets.values().find_map(|s| s.branches.get(&id))`）。
`all_semantic_objects`（`ins_map!` 群）には **載せない**:
variant set/branch は render input（描画依存 entity）ではなく
branch 管理帳簿であり、resolved project が実体 entity を運ぶ。
inventory に載せると `canonical_fingerprint_except` の
`shared_context_digest` が variant 編集のたびに全 branch で
過剰 invalidation を起こす。value 解決は全 `ObjectKind` の
exhaustive 契約として残す。

### D7: apply surface — fail-closed `Unsupported` 維持

`apply/entity.rs` の `supported_kind` は `matches!` で新 kind を
`false` とし、`entity_value`/`fingerprint` は `_` arm で
`ApplyError::Unsupported`、write-insert/reconcile も `_` arm で
compile-safe fail-closed。**変更なし**。variant change の
transplant 設計は Issue 対象外で、`vcs07_variant_diff.rs` が
`target_fingerprint`/`apply_selected` の `Unsupported` と
project 不変を pin する。ledger `VCS-APPLY-10` は `Blocked`
のまま（apply 対応は別 Issue）。

### D8: `DIFF_POLICY_VERSION` 3→4

variant-only `Equal`→`Different` は observable change。v2
（monitoring）・v3（plugin instance）と同じ `ComparisonKey`
collision 規約: cached/persisted result が旧 policy key と衝突
しないよう bump 必須。`vcs02_diff.rs` の pin は literal 3→4
更新のみ（test 名 `..._at_policy_v3` は v3 導入履歴の anchor
として維持 — plugin-instance 契約自体の assert は不変）。

### D9: VCS-06 pin flip

`vcs06_variant_matrix.rs` の 2 diff test を新契約に flip
（rename: `..._are_invisible_to_diff`→`..._are_typed_changes`、
`mixed_diff_reports_track_change_but_drops_variant_set`→
`mixed_diff_reports_track_and_variant_changes`）。ledger の
test 名参照と整合。`restore_as_copy_fails_closed_on_variant_
set_origin_mismatch` は **UNTOUCHED**（#458 scope）。

## File map

- `crates/synvelis-project/src/diff/types.rs` — `Variants`
  domain・2 kinds・`domain()`・`DIFF_POLICY_VERSION`=4
- `crates/synvelis-project/src/diff/engine.rs` —
  `impl_entity_key!`・`raw_kind_map`
- `crates/synvelis-project/src/diff/adapters.rs` —
  `classify_variants`・`VariantsAdapter`・`default_adapters`
- `crates/synvelis-project/src/render_input.rs` — canonical
  value 解決（inventory 非搭載）
- `crates/synvelis-project/tests/vcs07_variant_diff.rs` — 新規
  7 test（presence/adopt/content/policy-v4/collision/domain-
  exclusion/apply-fail-closed）
- `crates/synvelis-project/tests/vcs06_variant_matrix.rs` — 2
  diff pin flip（restore test untouched）
- `crates/synvelis-project/tests/vcs02_diff.rs` — policy pin
  literal のみ
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` —
  VCS-DIFF-11 flip・VCS-APPLY-10/RESTORE-10 維持・env 更新
- `evidence/sv457/` — 本 slice の verification evidence

## Risks / rollback

- 既存 kind の discriminant・field class・adapter 挙動は不変
  （追加のみ）。rollback は本 slice の revert で完結。
- `ObjectKind`/`ComparisonDomain` は cross-crate public 型 —
  `check-full` で `synvelis-render`/`synvelis-workspace` の
  consumer 非回帰を確認（`matches!`/field 型使用のみで
  exhaustive match なし・検証済み）。
- variant state が render input でない判断（D6）は
  `resolve_render_project` の resolved project 経路を確認済み
  — 誤りなら `all_semantic_objects` への追加は別 slice。
