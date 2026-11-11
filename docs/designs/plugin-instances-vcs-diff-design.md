---
id: "DESIGN-034VG5a2ikhPyz9RCyjDDT"
type: "design"
status: "approved"
title: "plugin_instances VCS semantic diff classification — keyed-synthetic PluginInstance"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/96"
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "https://github.com/scarecrow173/synvelis/issues/324"
  depends-on:
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/designs/pc-c4-05-persisted-instance-id-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-03-vcs-diff-monitor-classification-design.md"
    - "docs/designs/vcs-02-semantic-diff-engine-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/plugin-instances-vcs-diff-gap.md"
  derived-by:
    - "https://github.com/scarecrow173/synvelis/issues/324"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# plugin_instances VCS semantic diff classification — keyed-synthetic PluginInstance

## Overview

schema v13 の `Project::plugin_instances`（PR #313・PC-C4-05）を
VCS-02 semantic diff の分類対象へ追加する。`instances` entry や
`watermark` のみが異なる snapshot 対は現状 `identity.digest` が
異なるのに `changes = 0`・`Equal`/`Complete` を返す（discovery:
`docs/discovery/plugin-instances-vcs-diff-gap.md`）。

設計判断: **`ObjectKind::PluginInstance` を keyed-synthetic
collection として追加する**（Option A を採用）。entry の
membership・`plugin` pin は `routing.processors` の一貫投影で
派生的だが、`id` 値は `StablePluginInstanceId` の persisted
authority であり save→reload 横断の provenance 照合を担う
authored-facing state — `id_watermark` 類の bookkeeping ではない。
`watermark` のみ allocator bookkeeping として diff 対象外とする。

```
Project::plugin_instances (PluginInstanceIdentities)
  ├─ instances: BTreeMap<ProcessorId, PluginInstanceIdentity>
  │     → ObjectKind::PluginInstance を diff_map で照合
  │       (ProcessorId 空間を借用する keyed-synthetic collection)
  │       Added / Removed / field: id・plugin → Replaced
  └─ watermark: u64
        → diff 対象外（id_watermark と同じ allocator bookkeeping）
domain = ComparisonDomain::Routing
```

## ObjectKind / Domain

- `ObjectKind::PluginInstance` を enum **末尾**（`PunchMarks` の後）
  に追加する。`change_id` の material と `issue_side_key` が
  `object.kind as u8` を使うため、既存 variant の discriminant を
  不変に保ち、既存 kind の change-id 規則・結果を変えない
  （`TrackMonitorDecl`/`PunchMarks` と同じ規約）。
- `ObjectKind::domain()` は `ComparisonDomain::Routing` を返す。
  entry は `routing.processors` の一貫投影であり、instance
  identity の変更は routing processor の authored 意味に随伴する。
  `included_domains` で Routing を外せば instance 差も diff 対象
  から外れる — domain filter の既存契約に自然に乗る。
- `ComparisonDomain` 自体は増やさない。store は独立した authored
  domain ではなく processor の identity 属性であり、独立 domain
  を新設する境界ではない（YAGNI — monitor 宣言を独立 domain に
  しなかった RCD-03 判断と同じ）。

## PluginInstance（keyed-synthetic collection）

`plugin_instances.instances` は `BTreeMap<ProcessorId,
PluginInstanceIdentity>` — `SmfDocument`（`SourceId` key）・
`AppliedCommit`（session key）・`TrackMonitorDecl`（`TrackId`
key）と同じ「他 entity の ID 空間を借用する keyed-synthetic
collection」の先例そのもの。

- `diff_map(ctx, ObjectKind::PluginInstance,
  &before.plugin_instances.instances, &after.plugin_instances.
  instances, no_range, false)` で照合（`RoutingAdapter::diff` 内、
  pre-v6 suppression 判定の後・processor 系 diff_map 群の末尾）。
  `EntityKey` は `ProcessorId` に実装済み。
- `raw_kind_map` には登録しない。登録すると entry を持つ全
  Resolved processor が `{Processor, PluginInstance}` の内部衝突で
  `AmbiguousIdentity` を捏造する。除外コメントを
  `smf_documents`/`applied_commits`/`monitoring.tracks` を含む形へ
  `plugin_instances.instances` を追記する形で更新する。
- processor raw が snapshot 間で異 kind に衝突する場合（processor
  削除→raw 再発行等）は既存の `raw_is_ambiguous` が
  `AmbiguousIdentity(CrossKindId)` を出力する — entry の key 元の
  identity が曖昧なら entry も確定できない、という正しい fail。
- field 分類（`classify_routing` に arm 追加）:
  - `id` → `FieldClass::Replaced`。persisted `StablePluginInstanceId`
    の差し替えは `Clip.source_id`・`Asset.digest` と同じ
    「content/source identity swapped」の意味。
  - `plugin` → `FieldClass::Replaced`。pin される authored
    `PluginRef`（`capability_revision` を含む完全一致）の変更は
    別 instance identity の mint を意味し、`id` も必ず同時に
    変わる（`ensure` の再採番規則）。`Identity` にはしない —
    entry の同一性は `ProcessorId` key で確定しており
    `AmbiguousIdentity` を捏造しない。
  - その他 → `Modified` 既定。
- `watermark` は `instances` map の外にある store 直上の値であり、
  `diff_map` の比較対象に一切入らない。entry が同一で watermark
  drift のみの差は観測可能な authored 差を持たない（issued range
  の上限だけが意味を持ち、live entry の値に影響しない）—
  `id_watermark`・`revision` を semantic 対象外とする既存契約と
  同じ bookkeeping として扱う。

## Removed cascade との整合

`plugin_instances` entry の presence は Resolved processor の
presence に oracle 上完全に連動する（`validate_against`:
`Resolved` ⇔ ちょうど 1 entry、`Unresolved` ⇔ entry なし）。
したがって instance diff は processor の change と必ず併発する:

- processor `Removed` → `Processor(Removed)` + `PluginInstance(Removed)`。
- binding が `Resolved→Unresolved` → `Processor(Modified)` +
  `PluginInstance(Removed)`。`Unresolved→Resolved` は
  `PluginInstance(Added)`（新 id mint）。
- plugin rebind → `Processor(Modified)` + `PluginInstance(Replaced)`
  （`id`/`plugin` field 差）。

二重報告は suppress しない。`TrackMonitorDecl`/`Track`、
`SmfDocument`/`MidiSource` と同じく、kind の異なる object それぞれ
の presence/content 証拠として正しい — kind を絞る consumer
（ObjectKind 別 view）は既存の分類粒度で読み分けられる。

## Schema-version / migration 取り扱い

`plugin_instances` は `#[serde(default)]` で、pre-v13 snapshot は
`migrate_v12_plugin_instances` が `ProcessorId` 昇順に backfill
する。membership・`plugin` は Resolved binding の faithful 投影で
authored 値だが、**`id` 値は migration が mint する投影 artifact**
（pre-v6 routing の projection ID・pre-v8 master fader と同類 —
snapshot は resolve 時に検証済みでも、その値が authored 履歴を
表さない点で投影由来）。

- `before_id.schema_version < 13 || after_id.schema_version < 13`
  の間、`PluginInstance` の `id` field diff を
  `FieldClass::Suppressed("pre-v13-plugin-instance-id")` とし、
  差異があれば `PartialProjection`（`classify_control` の
  `pre_v7` と同じ either-side 規則。`classify_routing` に
  schema-version 条件を渡す形で実装する）。
- `plugin` field と Added/Removed の presence は version に依らず
  通常 diff（Resolved binding は v6 以降 authored で、pre-v6 は
  routing domain 全体が既存 suppression で止まる）。
- v13 同士の比較では `id` を通常 `Replaced` 分類 — persisted
  identity の実差として報告する。

## DIFF_POLICY_VERSION

`DIFF_POLICY_VERSION` を 2 → 3 に進める。`PluginInstance` が分類
対象に加わることで、同一 snapshot 対に対する `ChangeSet` の正解
が変わる（instance-only 差分が v2 では `Equal`・v3 では
`Different`）。`ComparisonKey` は `policy_version` を束縛するため、
version bump は旧 policy で計算・保存された結果との key 衝突を
防ぐ fail-closed の既定動作（RCD-03 が v1→v2 で取った同じ根拠）。
`ComparisonSpec::new` は常に `DIFF_POLICY_VERSION` を使い、既存
呼び出し側の変更は不要。

## Apply surface への影響

`crates/synvelis-project/src/apply/`（VCS-05）は新 kind に
**変更なし**で fail-closed のまま（`TrackMonitorDecl` 先例と同一）:

- `entity::supported_kind` は `matches!` 網羅リスト — 新 kind は
  `false` を返し `ApplyError::Unsupported`。
- `entity_value`/`write_entity`/`protected_fields`/
  `reconcile_membership`/`check_singleton_fields`/`preflight_*` は
  すべて `_` arm で Unsupported/無視に倒れる。
- `PluginInstance` change を含む selection の apply は計画時に
  `Unsupported` で拒否される — 未実装機能を黙って適用しない
  正しい既定。

`supported_kind` に**追加しない**判断の根拠:

- instance id は monotonic・never-reissued の採番規則を持つ。
  after-snapshot の `id` 値を直接 transplant する経路は、live で
  既に retire 済みの id を再導入し得る（「id は二度と発行され
  ない」契約と provenance 照合の前提を崩す）。watermark reseed を
  足して機械的整合を取る設計は可能だが、live 採番履歴と外来 id
  の意味論をどう統合するかは別の spec-level 問題。
- processor entity の write 経路（`ObjectKind::Processor` arm）が
  `sync_processor` で store を維持し、post-apply
  `validate_references` が全単射を最終確認する — apply 後の
  project は内部一貫状態を保つ（after-snapshot の `id` 値を byte
  復元はしない。`ensure` が live で再採番する PC-C4-05 の明示
  規則で、本判断とは独立した既存挙動）。

## Compatibility / Migration Notes

- `ObjectKind` は serde `kebab-case` — `plugin-instance`。既存
  variant の serde 名・並び・`as u8` は不変。
- `DIFF_POLICY_VERSION` bump により `ComparisonKey::digest` は
  同一 snapshot 対でも新旧で異なる。`ChangeSetCache` は
  in-memory の derived cache であり正本ではないため、version 差
  による再計算は許容範囲（仕様どおりの invalidation）。
- `plugin_instances` を持たない project（全 processor
  `Unresolved`、または pre-v13 由来で entry なし）の比較結果は
  byte 同一ではない（policy_version が key digest・ChangeId 材料
  に入る）が、changes/unresolved/overall の意味論は不変。

## Risks and Trade-offs

- rebind round-trip（A→B→A）で `PluginInstance(Replaced)` が出る
  のは semantic truth（persisted identity が実際に差し替わった）
  であり noise ではない — changelog の圧縮表現は view 層の責務で
  あり、engine は persisted contract の実差を報告する。
- `Removed`/`Added` cascade で `Processor` change と併発するのは
  既存 keyed-synthetic kind と同一の許容済み挙動。
- pre-v13 比較で `id` field 差があれば `PartialProjection` —
  instance-only の差では `Unknown`/`Partial` に倒れる。projector
  artifact を authored change と誤記しないための意図的な保守性
  （既存 suppression 規約と同じ fail-closed 方向）。
- `instances` の entry value（`PluginInstanceIdentity`）に将来
  field が追加された場合、`Modified` 既定で diff に現れる —
  新 field の意味に応じた分類見直しはその時の変更で行う。

## Deferred Design Concerns

- `PluginInstance` change の apply 対応（byte-faithful な `id` 値
  復元 + instance watermark reseed）: monotonic never-reissued
  契約と live 採番履歴の統合設計が必要で、本 slice の「diff
  分類追加」スコープを超える。要求が出た時点で別 spec/design に
  切り出す。
- VCS-06（#97）の diff matrix / ledger fixture への新 kind 組込み:
  実装 slice の test-spec で扱う。

## Verification Notes

- [ ] `mise run fmt-check` / `check` / `lint` / `test` 系 lane が
  実装後に pass（本 doc は design 記録のみで code 変更を含まない）。
- [ ] `tests/vcs02_diff.rs` に plugin-instance 分類セクション:
  entry Added/Removed、`id`/`plugin` → `Replaced`、pre-v13 `id`
  suppression、`watermark`-only drift → `Equal` 維持、
  cascade 併発の検証。
- [ ] `raw_kind_map` に `plugin_instances.instances` が未登録の
  まま（cross-kind 衝突を捏造しない）ことを test で pin。
- [ ] review-protocol で blocker なし。
