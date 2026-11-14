---
id: "DSGN-vcs05-plugin-instance-apply"
type: "design"
status: "approved"
title: "VCS-05 PluginInstance apply surface 拡張 design"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "SPEC-vcs05-plugin-instance-apply"
  verified_by:
    - "TSPEC-vcs05-plugin-instance-apply"
  implemented_by:
    - "TASK-vcs05-plugin-instance-apply"
  derives_from:
    - "SPEC-vcs05-plugin-instance-apply"
    - "DSGN-vcs05-apply-restore-headless"
    - "DESIGN-034VG5a2ikhPyz9RCyjDDT"
  supersedes: []
  superseded_by: []
---

# VCS-05 PluginInstance apply surface 拡張 design

## Overview

VCS-05 apply の entity 転置機構（`entity_value` → fingerprint →
`payload_for` → `write_entity`）に、diff 分類済みの
`ObjectKind::PluginInstance` を載せる。`plugin_instances.
instances` は `BTreeMap<ProcessorId, PluginInstanceIdentity>`
の keyed-synthetic collection で、`stable_id` は owning
`ProcessorId` の tagged 形を借用する（diff `diff_map`・
`render_input` と同一規約）。`TrackMonitorDecl` と同型の
専用 arm で扱い、`keyed!`/`keyed_insert!` macro は使わない
— lookup 先 map・値型・id domain がすべて異なるため。

```
SemanticObjectRef(kind=PluginInstance, stable_id=proc-<tag>)
  → entity_value: ProcessorId::from_tagged(stable_id)
      → project.plugin_instances.instances.get(&proc)
      → Option<PluginInstanceIdentity JSON>
        = {"id": <u64>, "plugin": {descriptor, version,
           capability_revision}}
  → write_entity: ProcessorId::from_tagged(stable_id)
      → from_value::<PluginInstanceIdentity>(payload)
      → project.plugin_instances.instances.insert(proc, identity)
      → watermark = max(live, after.plugin_instances.watermark)
```

## 識別性の決定: byte-faithful transplant + monotone watermark reseed

`docs/designs/plugin-instances-vcs-diff-design.md` の Deferred
Design Concern は「byte-faithful な `id` 値復元 + instance
watermark reseed」について「monotonic never-reissued 契約と
live 採番履歴の統合設計が必要」として apply slice への切り出しを
求めていた。本 slice の決定:

**after-snapshot の `PluginInstanceIdentity` を byte-faithful
に transplant し、watermark は `max(live, after)` で reseed
する。** live 側で再採番（remap）しない。

根拠:

1. **fingerprint CAS / replay reconcile が byte-faithful を要求
   する。** `apply_inner` は選択 object の fingerprint を live と
   after projection の両方で計算し、(a) intent の
   `expected_target_fingerprint` との一致（CAS）、(b) Pending
   journal + `live_fp == after_fp` の ACK-gap/crash reconcile
   （`Replayed` 返却・二重 mutation 防止）に使う。`entity_value`
   が返す entity は `PluginInstanceIdentity{id, plugin}` 全体
   なので、id を remap すると post-apply の live fingerprint が
   after と一致せず、replay reconcile が壊れて同一 intent の
   retry が二重適用されうる。persisted `StablePluginInstanceId`
   は schema v13 の authored state であり、restore-as-copy が
   `plugin_instances` を verbatim carry するのと同じ「外部
   系列の identity 採用」semantics。

2. **`watermark = max(live, after)` が never-reissued を保つ。**
   `PluginInstanceIdentities::allocate` は watermark のみを
   進め、`1 <= id < watermark` の発行済み id を二度と出さない。
   reseed を `max` にすることで:
   - `after.watermark > live.watermark`: live が未発行の id
     空間を after 系列が使っている — after 値を採用して
     transplant される全 entry id を range 内に収める
     （採番 skip は安全、再発行は unsafe）。
   - `after.watermark < live.watermark`: live が retire 済みの
     id を after が知らない — live 値を保持し、将来の
     `allocate` がその範囲を再発行しないことを保証する。
   - watermark は決して後退しない。

3. **一貫した after store では追加の id bound が不要。**
   snapshot validation を通った after store では全 entry が
   `id < after.watermark` を満たすため、`max(live, after)` で
   reseed すれば transplant id は必ず range 内。after が
   非一貫（`id >= watermark`）な場合は oracle
   `validate_against` が `Rejected` で fail-closed する —
   `id + 1` への watermark 押し上げは malformed snapshot を
   oracle 通過させてしまうため採用しない。

4. **`Processor` write との順序非依存。** `write_all` は
   ChangeSet 順に write する。`Processor` write は
   `plugin_instances.sync_processor`（`ensure` semantics:
   plugin 一致なら既存 id 保持、不一致なら新規 allocate、
   `Unresolved` なら remove）を呼ぶ。`PluginInstance` write が
   Processor write より先に走れば、transplant 済み entry の
   `plugin` が processor の after-binding と一致するため
   `ensure` は allocate せず transplanted id を保持する。
   後に走れば `ensure` が mint した entry を byte-faithful
   payload が上書きする。どちらの順でも最終 store は after
   状態に収束し、watermark も `max` により after 支配的な
   場合は一致する（after.watermark が live のどの値より大きい
   とき順序不変 — test-spec で固定）。

### 受容する限界（documented limitation）

byte-faithful transplant は after 系列の id 割当を権威として
採用するため、**divergent lineage 間での retired-id 衝突**を
検出できない: live 系列が発行→退役させた id X を、after 系列が
別の (processor, plugin) pair に割当てている場合、transplant は
id X を「再発行」する。store は retired-id の履歴を watermark
以外に保持しないため、この衝突は原理的に検出不能である。

これは restore-as-copy（`plugin_instances` verbatim carry）や
project file load と同じ「外部系列の identity 採用」
semantics であり、apply 層で新たに導入されるリスクではない。
`max()` reseed により、適用後の `allocate` が退役 id を
再発行することは防がれる。retired-id を追跡する store 拡張は
schema 変更を伴うため本 slice の範囲外（将来必要になれば
別 spec/design）。

## entity_value — keyed-synthetic view

`plugin_instances.instances` は entity map ではなく
`BTreeMap<ProcessorId, PluginInstanceIdentity>` で、
`stable_id` は key の `ProcessorId::tagged()` を借用する
（`ObjectKind::Processor` object と同一 `stable_id` 文字列を
共有するが、`SemanticObjectRef.kind` が異なるため fingerprint
material `{kind}|{stable_id}|{value}` では別 entity として
区別される）。

- `entity_value`: `stable_id` を `ProcessorId::from_tagged` で
  parse し `project.plugin_instances.instances` を参照。
  `TrackMonitorDecl` arm と同じ「stable_id → id → map lookup」
  形だが lookup 先・値型が異なり macro の型パラメータで
  表現できないため専用 arm。parse 失敗は `invalid_id`
  （`ApplyError::InvalidIntent`）で fail-closed。entry 不在は
  `Ok(None)` — absent-entity encoding（fingerprint `null`）。
- serialize は `PluginInstanceIdentity` の導出 `Serialize`
  そのまま: `{"id": <u64 transparent>, "plugin": <PluginRef>}`。
  diff の `value_ref`（`serde_json::to_vec`）と同一 canonical
  形のため fingerprint material は diff evidence と整合する。

## write_entity — typed transplant + watermark reseed

```rust
ObjectKind::PluginInstance => {
    let processor = ProcessorId::from_tagged(&object.stable_id)
        .map_err(|e| Error::InvalidId(e.to_string()))?;
    let identity: PluginInstanceIdentity =
        from_value(payload, "plugin instance identity")?;
    project.plugin_instances.instances.insert(processor, identity);
    if project.plugin_instances.watermark
        < after.plugin_instances.watermark
    {
        project.plugin_instances.watermark =
            after.plugin_instances.watermark;
    }
}
```

- `PluginInstanceIdentity` は `#[serde(deny_unknown_fields)]`
  で `id`+`plugin` のみを持ち、`payload_for` の entity_value
  round-trip と一致する。protected fields なし（membership
  list を持たない）、reconcile 対象外。
- **`max_new_id` に寄与しない**: `PluginInstanceId` は u64 の
  別 id domain であり、entity `id_watermark`（u128
  `IdGenerator`）とは無関係。`stable_id` が借用する
  `ProcessorId` の reseed は owning `Processor` write の
  `keyed_insert!` が担う（`Processor` が同 apply で `Added`
  の場合）。コメントで明記。
- watermark reseed は `PluginInstance` write ごとに評価される
  が `max` は冪等 — 複数 entry の transplant でも最終値は
  `max(live_initial, after.watermark)` に収束する。
- write 自体は store を「形だけ」整える — entry と processor
  binding の整合・id range・一意性・processor 実在は
  `ApplySelectedOp` 経由の `ProjectController::apply` が
  scratch clone に対して実行する `validate_references` →
  `plugin_instances.validate_against` が最終権威
  （oracle fail-closed、既存契約どおり）。

## change_supported — `Replaced` arm への追加

`classify_routing` は `PluginInstance` の `id`/`plugin` field を
`FieldClass::Replaced` に分類し（pre-v13 の `id` は
`Suppressed`）、`diff_matched` は field class ごとに
`ChangeKind` を grouping するため、`PluginInstance` の in-place
差分は常に `ChangeKind::Replaced` 1 件として出る
（`Modified`/`Moved`/`Reordered`/`Reconnected` は生成されない）。
`Added`/`Removed` は `diff_presence` が生成。

- `supported_kind` の `matches!` に `PluginInstance` を追加。
- `change_supported` の `Replaced` arm の許可 kind に
  `PluginInstance` を追加（detail message 更新）。`Removed`
  → `Unsupported`、その他 kind gate は既存のまま。
- `Added | Modified => Ok` の既存分岐はそのまま — `Modified`
  は生成されないが gate 上は通る（防御的に閉じない）。

## preflight — 変更なし

- `introduced_digests`/`preflight_assets`: payload は
  `{id, plugin}` のみで digest を含まない → `_` arm で素通し。
- `preflight_plugins`: `ObjectKind::Processor` write のみを
  検査する既存契約のまま。`PluginInstance` payload の `plugin`
  field は新規 plugin 依存を導入しない — entry.plugin が
  processor binding と一致しない transplant は oracle が
  `Rejected` にし、一致する transplant（id-only `Replaced`）
  は binding 不変で新規依存ゼロ。plugin field を変える
  `Replaced` は対応する `Processor(Modified)` change と
  セットでのみ成立し、その preflight が既に担う。
- `preflight_ports`: port 参照を含まない → 素通し。

## Failure / Edge Cases

- `stable_id` が `ProcessorId` tagged 形式でない
  `PluginInstance` object → `from_tagged` 失敗 →
  `InvalidIntent`/`InvalidId` で fail-closed（diff が生成
  しない異常入力への防御）。
- `Added` が live 既存 entry と衝突 → `TargetConflict`。
  `Replaced` が live 不在 → `TargetMissing`（plan 層の既存
  existence 規則がそのまま効く）。
- entry のみ選択で processor binding が伴わない
  （plugin field が live binding と不一致）→ write は成功するが
  `validate_against` の「pins a different plugin」で
  `ApplyError::Rejected` — live/journal は honest。
- entry のみ選択で processor が `Unresolved`/不在 →
  「unresolved processor carries a plugin instance」/
  `NotFound` で `Rejected`。
- transplant id が live の別 entry と衝突 → `validate_against`
  の一意性検査で `Rejected`（duplicate issued id）。
- 非一貫 after（`id >= after.watermark`）→ range 検査で
  `Rejected`。watermark を `id+1` へ押し上げる補正はしない
  （malformed snapshot を oracle 通過させない）。
- pre-v13 snapshot 対の `id` field 差分は diff が
  `Suppressed("pre-v13-plugin-instance-id")` →
  `PartialProjection` issue → changeset `Partial` →
  `NotApplicable`（diff 側の既存 fail-closed、本 slice で
  不変）。

## Compatibility / Migration Notes

- `ObjectKind`・serde 名・`DIFF_POLICY_VERSION`（3）は変更
  しない。本 slice は apply surface のみの拡張で diff
  semantics は不変。
- 既存 project の apply 挙動は不変 — `PluginInstance` change
  を含まない selection は同じ plan/payload を通る。
  `PluginInstance` change を含む selection は旧来
  `Unsupported` だったものが適用可能になる意図的な契約変更
  であり、`expected_target_fingerprint` を caller が再計算
  する既存手順どおりに動く（whitelist 追加前は fingerprint
  計算自体が失敗していたため旧 fingerprint との衝突は
  起きない）。
- `sync_processor`/`validate_against`/`resolve` の既存実装は
  無変更 — VCS apply は `ProjectOp` を介さない直接 entity
  write 経路だが、Processor write が既に `sync_processor`
  を呼ぶ境界を持ち、本 slice はその隣に `PluginInstance`
  write を追加するだけで identity store の mutation 規則を
  増やさない。

## Risks / Trade-offs

- **retired-id 衝突の受容**（上記「受容する限界」）— divergent
  lineage での id 再発行は検出不能。restore-as-copy と同じ
  semantics であり、watermark `max` で将来の `allocate`
  再発行は防止。test-spec で「watermark 非減少 + byte-faithful
  id 採用」を実測固定し、design 上の受容判断を明記する。
- `watermark` は fingerprint/entity_value の対象外（allocator
  bookkeeping — diff 設計と同じ位置付け）のため、apply 後の
  watermark が after と一致することは保証しない
  （`max(live, after)` のみ）。live が after より多く採番
  していれば live 値が残る — これは never-reissued のための
  意図的な非対称であり、entry 集合の byte-faithful 性は
  保たれる。
- `Processor` write との write 順で watermark の中間値は
  変わりうるが、最終値は `max(live_initial, after.watermark)`
  またはその上限に収束し、entry 内容は常に after と一致
  する（fingerprint/replay は watermark を見ないため
  観測差にならない）。
- `PluginInstance` の `Added`/`Replaced` が `Processor`
  change なしで選択された場合の oracle reject は「正しい」
  fail-closed だが、caller が entry だけを部分適用しようと
  する UX はありうる — apply 層は依存を推論して Processor
  change を自動同梱しない（selection は caller の明示責務、
  既存契約どおり）。

## Open Questions / Gaps

- `Removed` cascade・recording/comping/SMF 領域の apply・
  retired-id 履歴の store 化は別設計。
- 実 plugin host での opaque state 復元は VCS-01 snapshot /
  plugin-state store の責務であり、本 slice は project
  projection のみを動かす（completeness gate が Unknown/
  Partial を `NotApplicable` で既に排除）。
