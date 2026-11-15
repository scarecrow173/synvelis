---
id: "EXP-variant-sets-vcs-diff"
title: "variant_sets VCS semantic diff 分類 slice 検証メモ (synvelis#457)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: variant_sets を VCS-02 semantic diff の分類対象に追加（synvelis#457）

## E1 kind 数: `VariantSet` 単体 vs `VariantSet`+`VariantBranch` 2-kind

**仮説**: variant-only 差分を typed change として報告するには、
set 単位の whole-object 比較で十分か、branch を独立 entity と
して比較する必要があるか。

- 案 A（`VariantSet` 1-kind）: `set.branches` map を field として
  serialize 比較。branch 追加/削除は set の `Modified`/`Replaced`
  に潰れ、どの branch が変わったか stable ID で追跡できない。
  branch の presence transition を `Added`/`Removed` として
  独立報告できない。
- 案 B（2-kind、採用）: `VariantBranch` を `MidiNote` 型の
  nested-global diff（`midi_sources[].notes` 先例）で比較。
  `BranchId` は `id_watermark` 発行の entity-issued ID で
  keyed-synthetic ではないため `raw_kind_map` 登録対象。
  branch 単位の `Added`/`Removed`/`Modified`/`Replaced` と
  owner conflict（`OwnerConflict` ambiguous）を既存機構で
  そのまま表現できる。

**観察**: 案 B で `adopt_branch` swap が「set の `adopted`
field `Modified` + 両 branch の `content` `Replaced`」という
意味のある typed change 列に分解された（test
`adopt_swap_reports_set_modified_and_branch_replaced`）。
案 A では branch content ref の差分が set 全体の
`Replaced` に埋没し、apply 境界・undo 粒度の判断材料が
失われる。`BranchId` の owner は `VariantSetId` で一意に
決まるため `MidiNote` の owner=track 先例と同型で実装できる。

## E2 `ComparisonDomain`: 新設 `Variants` vs 既存 domain 帰属

**仮説**: `VariantScope` は arrangement/routing/comping/
control の 4 scope を跨ぐ variant set を含みうる。既存
`ComparisonDomain` のいずれかに帰属させると domain filter
（`ComparisonSpec.domains`）の意味が歪む。

- 案 A（既存 domain 帰属）: 例えば `Arrangement` に寄せると、
  `Variants` を含まない filter で variant 差分が消える一方、
  arrangement-only の意図と variant を分離できない。
- 案 B（新 `Variants` domain、採用）: `#[serde(rename_all =
  "kebab-case")]` で `"variants"`。scope が複数 domain に跨る
  ため独立 domain が整合的。`ComparisonDomains::all()` に
  末尾 append で追加し既存 domain の順序・discriminant を
  維持。domain exclusion test で「`Variants` を外すと
  variant-only 差分が `Equal`」を pin。

**観察**: 新 domain 追加で `classify_field` の exhaustive
match が compile error になり、arm 追加を強制する設計が
機能した。既存 domain consumer（`synvelis-render`/
`synvelis-workspace`）は `matches!`/field 型使用のみで
非回帰（`check-full` で確認）。

## E3 schema suppression: pre-v14 空 map の扱い

**仮説**: `plugin_instances` は migration-minted `id` field の
関係で pre-migration 側を `Suppressed` 扱いする先例がある。
`variant_sets`（schema v14・`#[serde(default)]` +
v13→v14 pass-through migration）でも同様の suppression が
必要か。

- `plugin_instances` の `Suppressed` は、旧 schema で
  migration が `id` を後付け mint するため「diff 上は
  同一だが identity が違う」ノイズを抑えるための措置。
- `variant_sets` は `#[serde(default)]` で旧 schema に
  存在しない field は真の absence（empty map）を意味し、
  migration は pass-through で ID を再採番しない。
  したがって pre-v14 側の空 map は「variant が存在しない」
  という真の状態であり、`Suppressed`/`PartialProjection`
  は不要。

**観察**: suppression なしで variant-only 差分が
`Different` + typed `Added` として報告されることを test
`variant_set_and_branch_presence_are_typed_changes` で確認。
`plugin_instances` との非対称は struct 上の migration 意味
論の違いに起因し、field 単位の個別判断が正しい。

## E4 `render_input` inventory 搭載の要否

**仮説**: `canonical_keyed_entity_value` の exhaustive match
に新 kind を追加する必要があるが、`all_semantic_objects`
の inventory（`shared_context_digest` 計算対象）にも
variant kinds を載せるべきか。

- variant state は render input（音声/MIDI レンダリングの
  依存 entity）ではなく、branch bookkeeping（fork/adopt/
  lineage の管理情報）。live scope objects は既に各 domain
  map 経由で inventory に含まれる。
- inventory に載せると variant 操作だけで shared-context
  fingerprint が変化し、無関係な render cache が
  invalidation される過剰反応になる。

**観察**: canonical value 解決のみ追加（`VariantSet` →
`keyed_value!`、`VariantBranch` → `MidiNote` 型
nested-special の `find_map`）し、inventory には非搭載。
`check synvelis-project` で exhaustive match の compile
充足を確認。将来 variant が render dependency になった
場合は別途 inventory 追加が必要（design D6 に記録）。

## E5 apply surface: fail-closed `Unsupported` 維持

**仮説**: `supported_kind`（`apply/entity.rs`）は
`matches!` で新 kind を `Unsupported` に落とす。apply
surface を本 slice で拡張するか。

- Issue #457 は compare surface のみが対象。apply
  transplant（variant set/branch を別 snapshot に
  適用する設計）は identity lineage・branch content store
  の整合が複雑で別 Issue が適切。
- `entity_value`/write-insert/reconcile は `_` arm で
  compile-safe fail-closed。

**観察**: `variant_changes_stay_fail_closed_on_apply_surface`
test で `target_fingerprint`/`apply_selected` が
`ApplyError::Unsupported` を返し `live_revision`/
`variant_sets` が不変であることを pin。ledger
`VCS-APPLY-10` は `Blocked` のまま維持。
