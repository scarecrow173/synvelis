---
id: "TSPEC-034VNrSyPxIGK6pnJSonNR"
type: "test-spec"
status: "proposed"
title: "plugin_instances VCS semantic diff 分類 fixture matrix"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source: []
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
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/plugin-instances-vcs-diff-design.md"
  derived-by: []
  verifies:
    - "docs/designs/plugin-instances-vcs-diff-design.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# plugin_instances VCS semantic diff 分類 fixture matrix

## Purpose

schema v13 で永続化される plugin-instance identity
（`Project::plugin_instances.instances`）が VCS-02 semantic diff
の `ObjectKind`/field 分類に正しく現れ、instance-only 差分が
`Equal` と誤報しないこと、apply surface の fail-closed が維持
されることを headless fixture で固定する。実 GUI・VCS-05 apply
適用は対象外。

## Feature

Plugin instance diff classification —
`ObjectKind::PluginInstance`（`plugin_instances.instances` map
照合・`ProcessorId` 空間を借用する keyed-synthetic collection）が
`Routing` domain の `SemanticChange` として決定的に現れる。

## Rules

- **R1 entry 存在分類**: `instances` の key 存在差は `Added`/
  `Removed`。entry は owning `Processor` の presence と oracle
  上連動するため `Processor` change と併発する（suppress しない
  keyed-synthetic cascade）。`watermark` は比較対象に入らない。
- **R2 field 分類**: v13 同士の比較で `id`（persisted
  `StablePluginInstanceId`）と `plugin`（pinned `PluginRef`）の
  差は `Replaced` で field evidence を持つ。その他 field 差は
  `Modified` 既定。`id` 差のみでも owning `Processor` は無差分
  のまま（instance-only 差分）。
- **R3 pre-v13 suppression**: `before`/`after` いずれかの
  `schema_version < 13` なら `id` field 差は
  `Suppressed("pre-v13-plugin-instance-id")` →
  `PartialProjection`（`Partial`/`Unknown`）。`plugin` field と
  presence 分類は version に依らず通常 diff。
- **R4 keyed-synthetic identity**: `instances` は
  `raw_kind_map` に登録しない — 登録すると全 Resolved
  processor が `{Processor, PluginInstance}` 衝突で
  `AmbiguousIdentity` を捏造する。entry key の raw が snapshot
  間で別 kind に衝突した場合は既存 `raw_is_ambiguous` が
  `AmbiguousIdentity(CrossKindId)` を出し、PluginInstance の
  change は出ない。
- **R5 policy version・apply 境界**: `DIFF_POLICY_VERSION == 3`。
  instance-only 差分は `Different`（v2 の `Equal` 誤報を解消）。
  `PluginInstance` change は apply surface 外 —
  `target_fingerprint` が `ApplyError::Unsupported` で
  fail-closed。既存 kind の分類・change-id・summary 規則は不変。

## Examples

- **E1 entry add/remove**: Resolved processor insert で
  `PluginInstance Added`（`Processor Added` と併発、
  `stable_id` = processor tagged ID）。remove で `Removed`。
- **E2 id/plugin 差し替え**: `instances[pid].id` のみ変更 →
  `Replaced` + `id` evidence、Processor 無差分。`plugin` pin
  変更（binding・`plugin`・`id` 同時 drift）→ `Replaced` +
  `id`/`plugin.*` evidence + `Processor Modified` 併発。
- **E3 watermark-only drift**: entry 集合が同一で `watermark`
  のみ前進 → `Equal`/`Complete`、changes 0 件。
- **E4 pre-v13 比較**: v12-labeled before × v13 after で `id`
  差 → PluginInstance change 0 件、
  `PartialProjection(pre-v13-plugin-instance-id)` issue、
  `completeness = Partial`、`overall = Unknown`。
- **E5 identity 衝突・apply 境界**: processor raw が track に
  再発行 → `AmbiguousIdentity(CrossKindId)` で PluginInstance
  change なし。`PluginInstance` object を含む selection の
  `target_fingerprint` は `Unsupported`。

## Guarantees

- `plugin_instances` のみが異なる snapshot 対は `Different`
  （`Equal`/`Complete` に誤昇格しない）。
- `PluginInstance` change は `Processor` change を suppress
  せず、逆も同様 — kind 別 view がそれぞれの証拠を読む。
- `ChangeSet` の sort・`ChangeId`・summary 規則は既存と同一
  （deterministic 順序に PluginInstance change が参加する）。
- `DIFF_POLICY_VERSION` bump により旧 policy の
  `ComparisonKey` と衝突しない。

## Non-goals

- `PluginInstance` change の VCS-05 apply/restore — apply
  surface 外（fail-closed `Unsupported`、Deferred Design
  Concern）。
- `watermark` の diff — allocator bookkeeping（`id_watermark`
  と同じ semantic 対象外）。
- VCS-06 ledger fixture への組込み — 別 worker slice。
- 実 plugin instance・実 GUI — 非永続/外部 gate。

## Risk

- `plugin_instances.instances` を `raw_kind_map` に登録し忘れ
  ても登録してしまっても壊れる — 未登録が正しいが、登録すると
  全 Resolved processor が `AmbiguousIdentity` を捏造する。
  E1 の `unresolved = 0` pin と E5 の衝突 fixture が両方向を
  固定する。
- pre-v13 `id` を通常 `Replaced` にすると migration 採番の投影
  artifact を authored change と誤記する — E4 が
  `PartialProjection` を固定する。
- select-all apply fixture が新 kind を拾うと
  `target_fingerprint` で panic — fixture 側の絞り込みが
  apply-surface 契約を pin する（E5）。
