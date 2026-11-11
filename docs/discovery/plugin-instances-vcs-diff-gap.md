---
id: "DISC-034VG2eVX55D1hI610carh"
type: "discovery"
status: "resolved"
title: "plugin_instances VCS semantic diff gap — persisted instance identity invisible to compare_snapshots"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/96"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/designs/pc-c4-05-persisted-instance-id-design.md"
    - "docs/designs/rcd-03-vcs-diff-monitor-classification-design.md"
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
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/plugin-instances-vcs-diff-design.md"
    - "https://github.com/scarecrow173/synvelis/issues/324"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# plugin_instances VCS semantic diff gap — persisted instance identity invisible to compare_snapshots

## Summary

`Project::plugin_instances`（schema v13・PR #313）は VCS-02 semantic
diff のどの domain adapter からも列挙されず、`instances` entry や
`watermark` のみが異なる snapshot 対は `identity.digest` が異なるのに
`changes = 0`・`unresolved = 0`・`Equal`/`Complete` を返す。
結論: persisted instance identity は derived state ではなく authored
state であり、`TrackMonitorDecl`/`SmfDocument` と同じ keyed-synthetic
collection の先例で `ObjectKind::PluginInstance` を追加して diff 対象に
するのが正しい（`watermark` のみ `id_watermark` と同じ bookkeeping として
除外）。設計判断は
`docs/designs/plugin-instances-vcs-diff-design.md`、実装追跡は Issue
#324（`https://github.com/scarecrow173/synvelis/issues/324`）に譲る。

## Exploration Goal

VCS-05 umbrella（#96）由来の発見を spec-level で判定する:

- `plugin_instances`（`BTreeMap<ProcessorId, PluginInstanceIdentity>`
  + `watermark`）を semantic diff の分類対象に加えるべきか、
  それとも `id_watermark` と同じ「diff 非対象の bookkeeping /
  derived state」として `Equal` の正当性を明文化すべきか。
- 加える場合の分類規約（ObjectKind・domain・field class・schema
  suppression・`DIFF_POLICY_VERSION`・apply surface への影響）は
  既存先例（特に #318 monitor 宣言）にどう写すか。

## Key Issues and Assumptions

- `identity.digest` は serialized `SnapshotEnvelope` 全体（`Project`
  全体を含む）の SHA-256 であり、`plugin_instances` を完全にカバーする
  （`snapshot/mod.rs` assemble、`capture_live`）。`id_watermark` も同じ
  envelope 内にある。
- diff engine は curated projection: 7 つの domain adapter が既知
  entity map のみを `diff_map`/singleton で列挙し、`plugin_instances`・
  `id_watermark`・`revision` はどの adapter の view にも含まれない。
- `PluginInstanceIdentities` は 2 種の状態を持つ:
  (a) `instances` の membership + `plugin` pin は
      `validate_against` が強制する「`routing.processors` の完全な一貫
      投影」— 派生的、
  (b) `id` 値と `watermark` は単調採番の永続履歴 — 現状態から派生
      不可能。
- `id_watermark` は「digest 差ありでも `Equal`」の先例となる意図的
  bookkeeping（VCS-02 spec で semantic 対象外と明記済み、
  ProjectMetaAdapter の view にも含めない）。`plugin_instances` が
  この類かは spec に未記載 — schema v13 で追加された際に diff
  分類が更新されなかったギャップ。
- apply 側は `write_entity` の `ObjectKind::Processor` arm が
  `sync_processor` で entry を維持し、post-apply
  `validate_references` が全単射を再検査する — 内部整合は保つが、
  after-snapshot の `id` 値は復元しない（`ensure` は live で再採番
  する。PC-C4-05 design の明示挙動）。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| A. `ObjectKind::PluginInstance` を keyed-synthetic collection として追加（`instances` のみ diff・`watermark` 除外） | persisted identity churn（rebind round-trip・lineage 差による再採番）を `Replaced` で可視化。#318 先例どおり enum 末尾追加 + `raw_kind_map` 除外で実装面積が小さい。`ComparisonKey` の policy bump で旧結果と衝突しない | processor 削除/Resolved→Unresolved で `Processor` change と `PluginInstance` Removed が二重に出る（cascade、先例どおり許容）。pre-v13 側の `id` は migration 採番の投影 artifact なので `Suppressed`→`PartialProjection` 規則が追加で要る | yes |
| B. derived/bookkeeping state として明文化（diff 非対象のまま `Equal` を正当化） | 実装不要。membership/plugin は真に派生状態なので「diff が processor を見る」主張は一部成立 | `id` 値は store の存在理由そのもの（PC-C4-05 が runtime 採番をやめて永続権威にした目的 = restart 横断の provenance 照合）。id churn を `Equal` と明文化すると persisted identity drift が VCS 消費者に永久不可視 — VCS-02 spec の「semantic 対象外」列に載る `id_watermark`・revision・capture_generation とは質が違う | no |
| C. membership/`plugin` のみ diff・`id` も bookkeeping 扱い | rebind round-trip の id churn が changelog ノイズにならない | `id` こそこの store が永続化する唯一の非派生値 — ここを隠すと B と同じ blind spot を「意味のある field」で再現する | no |
| D. `watermark` を含む store 全体を singleton で diff | 全永続差分を捕捉できる | `watermark` は allocator bookkeeping（`id_watermark` を semantic 対象外とする既存契約と矛盾）。entry が同一で watermark drift のみの差は観測可能な authored 差を持たないのに `Different` になる noise | no |

## Tentative Conclusions and Hypotheses

- **confirmed — gap は実在し到達可能**: `src/diff/` に
  `plugin_instances` の参照はゼロ。`plugin_instances` のみが異なる
  snapshot 対は次の条件で `Equal` を返す: 同一 `project_id`・digest 差
  あり・adapter 列挙域が全て一致・manifest issue なし。到達経路は
  authored op だけで作れる（下記 Evidence）。
- **confirmed — `Equal` 誤報の具体形**: `overall = Equal` かつ
  `completeness = Complete`・`identity.digest` 不一致。consumer が
  「永続 byte が変わったのに semantic change 0 件」を観測する。
  `id_watermark` でも同型は起きるが、あちらは spec 明記の意図的
  bookkeeping 域で、instance id は authored-facing の永続 identity。
- **hypothesis → design で確定**: Option A 採用。`PluginInstance` を
  enum 末尾に追加し `ComparisonDomain::Routing` に写像、
  `diff_map(ctx, PluginInstance, instances, instances)` 照合、
  `raw_kind_map` 未登録の keyed-synthetic パターン、`id`/`plugin` field
  は `Replaced` 分類、pre-v13 側の `id` は `Suppressed`（migration
  採番 artifact → `PartialProjection`）、`watermark` は比較対象外、
  `DIFF_POLICY_VERSION` 2→3、apply surface は `supported_kind` 非追加
  の fail-closed のまま。
- **confirmed — apply 側は現状で整合**: `write_entity` Processor arm
  の `sync_processor` + post-apply `validate_references` が store の
  不変条件を維持する。after-snapshot の `id` 値を byte 復元しない
  挙動は PC-C4-05 design の明示規則で、diff 可視化とは独立。

## Open Questions

- VCS-06（#97）の diff matrix / ledger fixture に新 kind をどう組み込むか
  — 実装 slice の test-spec で扱う（本 slice では ledger 整合は行わない）。
- byte-faithful な instance id 復元を apply surface に持たせるか —
  本 design は「持たせない（`Unsupported` fail-closed）」を記録するが、
  要求が出れば別 spec 判断（monotonic never-reissued 契約との整合設計が要る）。

## Promotion Candidates

- [x] design-doc: `docs/designs/plugin-instances-vcs-diff-design.md`
  — 採用案・分類規約・watermark/cascade/policy version の扱いを確定。
- [x] GitHub Issue: `plugin_instances` VCS diff 対応の実装 slice 追跡
  — Issue #324（`https://github.com/scarecrow173/synvelis/issues/324`）
  を `.github/ISSUE_TEMPLATE/task.yml` 準拠で起票済み。
- [ ] spec-doc / adr-doc: 不要（VCS-02 spec の既存分類規約内に収まる
  拡張で、新規 product intent / architecture decision ではない —
  design-doc で十分）。

## Sources and Relations

- `https://github.com/scarecrow173/synvelis/issues/96` — VCS-05
  umbrella、本発見の起点。
- `https://github.com/scarecrow173/synvelis/issues/101` — PC-C4、
  persisted `StablePluginInstanceId` store（schema v13・PR #313）の親。
- `https://github.com/scarecrow173/synvelis/issues/117` — RCD-03、
  monitor 宣言の diff 分類追加（PR #318・`DIFF_POLICY_VERSION` 2）の
  同型先例。
- `docs/specs/vcs-02-semantic-diff-changeset.md` — semantic 対象/
  対象外・tri-state・suppression 規約の正本。
- `docs/designs/vcs-02-semantic-diff-engine-design.md` /
  `docs/designs/rcd-03-vcs-diff-monitor-classification-design.md` —
  adapter/keyed-synthetic/singleton 規約。
- `docs/designs/pc-c4-05-persisted-instance-id-design.md` —
  `PluginInstanceIdentities` の不変条件・mutation 経路・apply sync
  規則の正本。
- `docs/designs/plugin-instances-vcs-diff-design.md` — 本発見から
  派生した設計判断の正本。
- `https://github.com/scarecrow173/synvelis/issues/324` — 実装 slice
  の追跡 Issue（本発見と設計判断を根拠に起票）。

## Evidence

### 2026-11-11 — code-path 計測（Equal 誤報の成立条件）

`compare_snapshots`（`crates/synvelis-project/src/diff/engine.rs`）:

- `before.identity.project_id != after.identity.project_id` →
  `AmbiguousIdentity(CrossProject)` で `Unknown`（本 gap とは無関係）。
- `before.identity.digest != after.identity.digest` のときだけ adapter
  が走る。digest は serialized `SnapshotEnvelope` 全体の SHA-256
  （`snapshot/mod.rs` assemble — `project` 本体を含み
  `plugin_instances`/`id_watermark` をカバー）。
- `default_adapters()` = ProjectMeta / Arrangement / Media / Control /
  Routing / Recording / Comping。いずれも `plugin_instances` ・
  `id_watermark` を読まない（`grep plugin_instances crates/.../src/diff`
  → 0 件）。
- 全 adapter が change 0・issue 0 → `overall = Equal`、
  `completeness = Complete`。

`plugin_instances` のみ差異で `Equal` になる到達経路（全て authored
op / snapshot 経由で再現可能）:

1. **instance id churn（entry 差）**: processor が `Resolved→Unresolved`
   →`Resolved` と遷移すると `sync_processor` が entry を remove→再採番
   する。snapshot 間でこの往復が起きると、binding が同じ processor の
   `instances[pid].id` が異なる値になる。`PluginRef` の rebind
   A→B→A も同型（`ensure` は plugin 不一致で `allocate` し直す）。
   restore-as-copy / apply / 別 lineage の独立 fork で採番履歴が
   分岐しても同じ効果。
2. **watermark-only drift（entry 不変）**: 別の Resolved processor が
   snapshot 間で insert→remove されると `watermark` は前進したまま
   `instances` は元に戻る。issued id は再利用されない設計なので
   `watermark` だけが異なる。
3. **両者の組合せ**。

上記いずれでも `changes = 0`・`unresolved = 0`・`overall = Equal`・
`completeness = Complete` となり、consumer は「digest 差あり /
semantic change 0 件」を観測する。

### 2026-11-11 — `id_watermark` との比較（bookkeeping 先例）

- `id_watermark` も envelope 内で digest 対象・diff 非対象 —「digest
  差ありでも `Equal`」は既に発生しうる。ただし VCS-02 spec が
  「`id_watermark` は semantic 対象外」と明記しており意図的。
  `store.rs` の same-revision 判定も `id_watermark` を 0 正規化して
  比較する（bookkeeping と正式に扱われる先例）。`ProjectMetaAdapter`
  の view は `name`/`ppqn`/`tempo_map` のみで `revision` も除外
  （"revision is bookkeeping, not authored content" コメント）。
- `plugin_instances` は spec で semantic 対象外と明記されていない —
  schema v13 追加時に分類が更新されなかった gap であり、意図的
  除外ではない。
- `PluginInstanceIdentities.watermark` は `id_watermark` と同質の
  allocator bookkeeping（issued range の上限のみ意味を持ち、値自体は
  観測対象でない）。Option A でも diff 対象外とする。
- `PluginInstanceIdentity.id` は `id_watermark` 系 raw と違い、
  `StablePluginInstanceId` として `PrecisionProvenance` /
  `PluginBindingSession` に流入し、save→reload 横断の照合を担う
  authored-facing の値 — bookkeeping とは別類。

### 2026-11-11 — keyed-synthetic 先例と apply surface

- `SmfDocument`（`SourceId` key）・`AppliedCommit`/
  `AppliedImport`（registry key）・`TrackMonitorDecl`（`TrackId`
  key・#318）は全て他 entity の ID 空間を借用し `raw_kind_map` に
  登録しない — 登録すると同じ raw が `{Processor, PluginInstance}` の
  内部衝突で全 processor を `AmbiguousIdentity` にしてしまう。
  `plugin_instances.instances` は `ProcessorId` key の同型。
- `apply/entity.rs::supported_kind` は `matches!` 網羅リスト —
  `TrackMonitorDecl`/`PunchMarks`/`SmfDocument` 等は含まれず
  `ApplyError::Unsupported` で fail-closed。diff 可視化と apply
  対応は別権限であることが先例として確立済み。
- `write_entity` の `ObjectKind::Processor` arm は
  `project.plugin_instances.sync_processor(&entity)` を insert 前に
  呼び、post-apply `validate_references` が `validate_against` の
  全単射を再検査する（`apply/entity.rs` L356-367、`ops.rs`
  InsertProcessor/RemoveProcessor、PC-C4-05 design の mutation 経路表）。
- `migrate_v12_plugin_instances` は `ProcessorId` 昇順に backfill —
  deterministic だが採番値は authored 履歴ではなく投影 artifact
  （pre-v6 routing の projection ID と同類）。
