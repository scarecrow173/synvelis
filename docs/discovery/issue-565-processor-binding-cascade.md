---
id: "DISC-565ProcessorBindingCascade01"
type: "discovery"
status: "resolved"
title: "Issue #565 discovery — SetProcessorBinding sync_processor の PluginInstanceId 退役 cascade 監査"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/565"
    - "https://github.com/scarecrow173/synvelis/issues/561"
    - "https://github.com/scarecrow173/synvelis/issues/559"
    - "https://github.com/scarecrow173/synvelis/issues/128"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-565-processor-binding-cascade.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
---

# Issue #565 discovery — SetProcessorBinding sync_processor の PluginInstanceId 退役 cascade 監査

## 背景

synvelis#561（`docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md`
残件節）で「`SetProcessorBinding` の `sync_processor` が
`plugin_instances` の旧 `PluginInstanceId` を退役させる cascade」
が別クラスの persisted identity として本 Issue `synvelis#565`
に切り出された。本 discovery は main `b73018bd` のコードで
退役経路を再確認し、persisted identity の不変条件 3 軸
（再読み込み・復元・外部参照）への影響を監査する。

監査の主問い: **退役する persisted identity を外部が参照でき、
dangling が起き得るか**。起き得るなら fail-closed
（`unsupported`/op_facts）または契約明記で修正、起き得ない
なら根拠を doc chain/evidence に記録して完了とする。

## 退役経路（main `b73018bd` で再確認）

`ProjectOp::SetProcessorBinding` の apply arm
（`crates/synvelis-project/src/ops.rs:2383-2398`）は
`proc.binding` を差し替えた直後に
`project.plugin_instances.sync_processor(proc)?` を呼ぶ。
`sync_processor`（`plugin_instance.rs:161-169`）は
`Resolved(plugin)` → `ensure`、`Unresolved` → `remove` に
分岐する単一 primitive で、以下の全 mutation 経路が共有する:

- op 経路: `InsertProcessor`（`ops.rs:2349`）・
  `SetProcessorBinding`（`ops.rs:2394`）・`RemoveProcessor`
  （`ops.rs:2368` は `remove` 直接呼出）
- VCS apply 経路: `apply/entity.rs:505` の `ObjectKind::Processor`
  直接書込みが同じ `sync_processor` を呼ぶ
- migration 経路: v12→v13 backfill（`migration.rs:467-472`）が
  同一 primitive を `ProcessorId` 順で再利用

`ensure`（`plugin_instance.rs:136-145`）は entry の `PluginRef`
が一致すれば id を維持し、descriptor/version/
`capability_revision` のいずれかが異なれば `allocate` で新 id
を発行する — rebind は新しい authored identity であり、旧 id
は entry ごと退役する。`remove`（:150-152）は entry を落とすが
watermark は戻さないため発行済み id は**二度と再発行されない**
（`allocate` :106-125 の単調 watermark・`remove` のコメント
明記）。

post-apply oracle `validate_against`（`plugin_instance.rs:203-254`）
は `Project::validate_references`（`project.rs:986`）内で
`ProjectController::apply`（`controller.rs:170`）と `load`
（`controller.rs:306`）の双方から走り、store が
`routing.processors` の全射的投影であること（全射・id 範囲・
一意性・plugin 一致・Unresolved 非所持）を検査する。

## 不変条件 3 軸の照合結果

### (1) 再読み込み — 健全

- schema v13 の `Project::plugin_instances`
  （`project.rs:502`）が persisted authority。serde default で
  旧 schema も deserialize 可能。
- v12→v13 backfill は同一 `sync_processor` primitive を再利用
  し、`ProcessorId` 順の決定的発行で冪等（`migration.rs:449-471`）。
- save→load roundtrip・oracle 拒否系は既存 pin
  （`plugin_instance_persist.rs` :115-:135, :195-:246,
  :258-:359）が担保。
- **本 slice で追加する pin**: `SetProcessorBinding` op 経路の
  rebind で新 id 発行・旧 id 退役・watermark 前進・
  `resolve`(旧 plugin)→`None`・oracle 通過を固定する
  （apply 経路 pin は :576/:603/:639 に既存、op 経路は未 pin）。

### (2) 復元 — 健全（fail-closed）

- `resolve`（`plugin_instance.rs:178-188`）は missing entry・
  `PluginRef` drift（`capability_revision` 含む）・範囲外 id を
  全て `None` に畳み、「missing」と「stale」を区別しない。
- 消費側は typed fail-closed: player
  `synvelis-player/src/routing_control.rs:1407-1419` は
  `None` → `without_control` evidence、project
  `synvelis-project/src/host_param.rs:180-221` は
  `None` is data で `PluginMismatch` が instance 比較より
  先に報告される ordering を明記。
- undo/redo は project 全体 snapshot を復元するため store も
  同 revision の状態に戻る（既存 pin :167-:190）。

### (3) 外部参照 — dangling は構造的に起きない

- `PluginInstance` は wire `ResourceKind`
  （`request.rs:31-58`・16 variants）にも
  `ENUMERATED_KINDS`（`snapshot.rs:264-280`・15 kinds）にも
  非存在 — `ResourceRef` で名指せず、op target/precondition/
  effect resource として外部から参照する経路が存在しない。
- `op_facts`/`derive_effects` も同じ観測モデルを使うため、
  退役そのものは effect に現れない。ただし `Processor` の
  `detail_of`（`prepare.rs:1596-1600`）は `binding` フィールドを
  含む `Processor` 全体を serialize するため、**rebind 自体は
  `Processor` `Write` 効果の before/after detail で観測可能**
  — `insert-processor` の「ports/plugin-instance identity は
  internal structure」posture と整合する設計。
- VCS diff/apply 面では `ObjectKind::PluginInstance`
  （`diff/types.rs:199`・`diff/adapters.rs:761` で
  `plugin_instances.instances` を `ProcessorId` key の
  keyed-synthetic collection として比較、`watermark` は
  allocator bookkeeping で比較対象外）。`render_input.rs:680-682`
  も同じ key 規約で hash するため rebind で render key が変わり
  honest。`removal_supported_kind`（`apply/entity.rs:113-125`・
  sv#559 占有ファイル・読み取りのみ）は `PluginInstance` を
  含まず、adopt 側の `Removed` は typed `Unsupported`
  fail-closed。`write_plugin_instance`（`entity.rs:665-678`）の
  transplant は byte-faithful + watermark の `max(live, after)`
  単調 reseed。
- variant 経路も同じ primitive に収束: `SetProcessorBinding`
  は instrument-slot/processing-range/strip-Inserts mask の
  candidate edit op として受理され（`variant/adapter.rs:237,
  545, 760`）、適用は `ProjectOp::apply` → `sync_processor`。
  adopt は `plan_adopt_writes` が scope object のみを計画し
  `ObjectKind::PluginInstance` を含まないため Processor write
  → `sync_processor` のみで store を live 側再導出する
  （`write_plugin_instance` transplant は VCS `apply_selected`
  が `PluginInstance` change を明示選択した経路専用）。

## 残候補論点の判定

### (a) set-binding 向け posture 明記 — 契約明記で対応（本 slice の主変更）

`prepare.rs:1412-1419` の `SetProcessorBinding` op_facts arm と
`prepare_engine.rs:703-709` の op-table entry には、rebind で
persisted plugin-instance identity が退役・再発行される
「internal structure」posture が未転記（`AddSend`/
`RemoveSend` には同型コメント :1430-1435・:710-720 が既存、
`insert-processor` は entry 内に「ports/plugin-instance
identity are internal structure」の parenthetical あり）。
実害が確認されないため `unsupported` 追加は行わず、
insert-processor/send 系に倣うコメント明記で着地する。

### (b) `PluginStateProbe` staleness — `synvelis#128` へ defer

`crates/synvelis-render/src/dependency.rs:105-131` の
`PluginStateEvidence{digest, quality}` は plugin/instance
identity を持たず、`PluginStateProbe::evidence(processor:
ProcessorId)` は `ProcessorId` キーのみで evidence を引く
（:543）。rebind 後、旧 plugin 用に capture された evidence が
同一 `ProcessorId` の新 plugin に attach し得る staleness 契約
の問題。in-tree 実装は `NoPluginStates`/`StaticPluginStates`
fixture のみで実害実測はない。`dependency.rs` は本 slice の
lease 外のため変更せず、「`synvelis#128`（TIM-04 plugin state
snapshot）へ defer」と判定して記録する — plugin state 永続化の
責務 Issue であり、evidence が instance generation を区別
できない問題は TIM-04 の port 契約設計に属する。

### (c) `unsupported`（fail-closed 拒否）の要否 — 不要と判定

fail-closed が必要になる分岐点は「`ResourceRef` で名指せない
永続効果が外部参照を dangling させる」実害の確認だが、
外部から persisted id を参照する経路（wire op・`ResourceRef`・
VCS `Removed` 適用）が構造的に存在しないため実害なし。
store cascade 自体への `unsupported` は op を無効化する
過剰拒否であり、既存受理の回帰になる — 立てない。

## companion Issue との境界

- `synvelis#559`（domain adopt 側 fail-closed）は別 Worker が
  `apply/entity.rs`・`variant/*` 等を占有中。本 slice は op 経路の
  退役監査で layer が別・lease 無交差。占有パスは読み取り参照のみ。
- `synvelis#561` は本 Issue の発見元（IR 残件節が起票根拠）。
- `synvelis#283`（CB-06 tracker）は OPEN のまま。ledger 追記は
  本 slice の lease 外。
- `synvelis#128`（TIM-04）は `PluginStateProbe` staleness の
  defer 先。

## 結論

退役経路は main 上で不変条件付き実装済みであり、persisted
identity の不変条件 3 軸（再読み込み・復元・外部参照）は全て
健全。store cascade への `unsupported` は不要。着地は
(a) の契約明記（コメント）+ op 経路/wire の test pin +
(b) の defer 判定記録 + doc chain/evidence の監査記録。
production コードの動作変更はなし。
