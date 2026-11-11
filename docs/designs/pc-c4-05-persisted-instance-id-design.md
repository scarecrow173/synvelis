---
id: "DESIGN-034VvFkFH34loxhN1Sn2tn"
type: "design"
status: "approved"
title: "PC-C4-05 persisted StablePluginInstanceId design — project identity store・schema v13・hydration"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-03-monitor-persistence.md"
  refines:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
  defers: []
  deferred-by: []
---

# PC-C4-05 persisted StablePluginInstanceId design — project identity store・schema v13・hydration

## Context

PC-C4-02 は plugin instance の **authored identity**（`PluginRef`:
descriptor/version/capability_revision）を connect-time 証拠として pin
したが、**instance slot の stable id**（`StablePluginInstanceId`）は
`RoutingControlDispatch.next_instance` が attach ごとに 1 から採番する
runtime-only 値のままだった。pc-c4-02 spec の Out of Scope が明記した
deferred であり、save→reload・engine restart で同一 processor が別 id
を得るため、provenance/binding 記録を restart 横断で照合できない。

採番・復元の正本は project 層に置く。`synvelis-player` は hydration
消費者であり、永続 state の owner にはならない（単一 writer モデル
維持）。entity id（u128 `id_watermark`）とは別ドメインの u64
watermark を持つ独立 store とし、既存の ID 発行権威を混同しない。

VCS-05 apply（`apply/entity.rs::write_all`）は `Processor` entity を
`routing.processors` へ直接移植し `ProjectOp` を経由しない唯一の
processor 変異経路である（Coordinator 承認済みの範囲拡張）。ここの
identity sync を欠くと、apply が「`Resolved` processor に entry が無い」
project を生み、post-apply `validate_references` が必ず reject する
— 今日成功する apply の regression になるため、同じ不変条件をこの
境界でも維持する。

## Scope

### In Scope

- `crates/synvelis-project/src/plugin_instance.rs`（新規）:
  `PluginInstanceId`・`PluginInstanceIdentity`・
  `PluginInstanceIdentities` store。
- `project.rs`: `Project::plugin_instances` field・`Project::new`・
  `validate_references` への oracle 追加。
- `store.rs`: `SCHEMA_VERSION = 13` + schema 履歴 doc。
- `migration.rs`: `v12 → v13` step（`migrate_v12_plugin_instances`）。
- `ops.rs`: `InsertProcessor`/`RemoveProcessor` の identity 維持。
- `apply/entity.rs`: `write_entity` の `ObjectKind::Processor` arm で
  identity sync（ensure 意味論）。
- `lib.rs`: module + 型 export。
- `crates/synvelis-player/src/routing_control.rs`: `vend_nodes` の
  persisted hydration・`next_instance` 撤去。
- `crates/synvelis-player/src/binding.rs`: `StablePluginInstanceId`
  doc 更新（永続 backing の明記）。
- 両 crate の `tests/`。

### Out of Scope

- `driver.rs` の clip-insert FX instance id — project processor では
  ない別 domain（"call-context only" FX）で ephemeral のまま。
- `PluginBindingSession`/`ParamDispatch`/`ProcessBoundarySession` の
  pin・generation・frame-base 契約、`binding.rs` のロジック変更
  （doc コメントのみ）。
- `ProcessorBinding` の正式 rebind op、binding snapshot 形式、
  app/workspace/gateway 配線（pc-c4-04）、実 plugin backend。
- entity `id_watermark`（u128）・`CounterIdGenerator` への統合。

## Components and Boundaries

### `PluginInstanceIdentities`（project、永続正本）

```rust
pub struct PluginInstanceIdentity {
    /// 発行済み stable instance id（u64、1-based、再利用なし）。
    pub id: PluginInstanceId,
    /// この id が pin される authored plugin identity。
    pub plugin: PluginRef,
}

pub struct PluginInstanceIdentities {
    /// 次に発行する id（`1` 始まり）。発行済み id は全て `< watermark`。
    pub watermark: u64,
    /// 生存 processor の identity（`ProcessorId` キー、昇順決定的）。
    pub instances: BTreeMap<ProcessorId, PluginInstanceIdentity>,
}
```

serde は `#[serde(default)]`（`Project` field）+ struct derive。
JSON 形状: `{"watermark": N, "instances": {"processor-<hex>":
{"id": M, "plugin": {descriptor, version, capability_revision}}}}`。
`PluginInstanceId` は `#[serde(transparent)]` の u64 newtype
（entity ID の tagged-string とは別表現 — domain が異なる）。

メソッド（`pub(crate)` は crate 内 oracle/維持経路用、`pub` は
消費者 hydration 用）:

- `allocate(processor, plugin) -> Result<PluginInstanceId>` —
  `watermark` を `checked_add(1)` 前進させて発行。`InsertProcessor`
  と migration が使う。overflow は `Error::Overflow`。
- `ensure(processor, plugin) -> Result<()>` — entry が存在し
  `entry.plugin == *plugin` なら既存 id を維持（**再採番しない**）、
  無いか plugin が違えば `allocate` で新規発行して置換。
  VCS apply の sync が使う — apply は繰り返し走るため、無関係な
  apply で id が churn してはならない。
- `remove(processor)` — entry 除去（冪等）。`RemoveProcessor` と
  Unresolved への遷移で使う。id は watermark により二度と発行
  されない。
- `sync_processor(&Processor)` — binding を見て `Resolved`→`ensure`、
  `Unresolved`→`remove` に振り分ける。`apply/entity.rs` が呼ぶ。
- `resolve(processor, plugin) -> Option<PluginInstanceId>` —
  hydration 用の fail-closed lookup: entry 存在 ∧
  `entry.plugin == *plugin` ∧ `1 <= id < watermark` の時だけ
  `Some(id)`。`vend_nodes` はこれ一つを呼ぶ。
- `validate_against(&Project)` — oracle（下記）。

`Default` は `watermark = 1`・空 map（fresh project の初回採番が id=1）。

### `Project::plugin_instances` + oracle

`Project` 末尾に `#[serde(default)] pub plugin_instances:
PluginInstanceIdentities` を追加。`Project::new` は `Default` で初期化。

`validate_references` 末尾（`routing.validate` の後）に
`plugin_instances.validate_against(self)` を追加し、以下を全て検査:

1. `watermark >= 1`。
2. 全 entry: `id` が `1 <= id < watermark`、`instances` 内で id 一意。
3. 全 entry: key の processor が `routing.processors` に存在し、
   `binding` が `Resolved(plugin)` で `entry.plugin == plugin`
   （**`capability_revision` を含む `PluginRef` 完全一致** — revision
   差分は別 plugin instance であり、旧 id の流用を許さない）。
4. 全 `Resolved` processor が entry を持つ（逆向き全単射）。
5. `Unresolved` processor は entry を持たない。

違反は `Error::Validation`（構造的不整合）/`Error::NotFound`
（processor 不在）で typed reject — load・apply・migration 後検証の
全てで同一 oracle が効く。

### Mutation 経路ごとの維持

| 経路 | 動作 |
| --- | --- |
| `InsertProcessor::apply` | `binding` が `Resolved` なら `allocate`（`Unresolved` は何もしない） |
| `RemoveProcessor::apply` | processor 除去時に `remove`（entry が無くても冪等） |
| `ReorderProcessor` 他 | identity に無関係 — 変更なし |
| migration v12→v13 | `routing.processors` を `ProcessorId` 昇順（BTreeMap 反復順）に走査し `Resolved` のみ `allocate` — 同一 v12 body は常に同一 v13 project |
| VCS apply `write_entity` Processor arm | insert 前に `sync_processor(&entity)` — Added/Resolved→新規、plugin 不変 Modified→維持、plugin 差し替え Modified→再採番（rebind=新 instance）、→Unresolved→除去 |
| undo/redo・restore_as_copy | project snapshot ごと store が復元されるため追加処理なし |

`HeadlessDriver::activate_with_attached`（clip-insert FX）は project
processor を参照しない別 domain — `driver.rs` の ephemeral 採番は
維持する（scope 外・変更なし）。

### Player hydration（`vend_nodes`）

```rust
let Some(plugin) = control.plugin else { without_control; continue };
// 旧: StablePluginInstanceId::new(self.next_instance); counter++
// 新:
let Some(instance) = project.plugin_instances.resolve(*processor, &plugin)
else { without_control; continue };
let instance = StablePluginInstanceId::new(instance.get());
```

`resolve` が entry 欠落・plugin 不一致・id 範囲外を全て `None` に
潰すため、`vend_nodes` 側に個別分岐を増やさない。`next_instance`
field・初期化・`wrapping_add` を撤去する。`ProcessBoundarySession`
の `PrecisionProvenance` と `PluginBindingSession::attach` は同一の
persisted id を受け取る（"one identity per surface" の契約維持 —
ただし generation pin が変わるたび session は張り直される既存意味
論は不変）。

`PluginBindingSession::attach` の signature・`bind`/`dispatch` の
pin 検査は不変。`binding.rs` の `StablePluginInstanceId` doc を
「project 層が採番する persisted identity を composition root が
hydrate する」契約に更新する（型・API 不変）。

## Data and Control Flow

- **Authoring**: `InsertProcessor` → op.apply が `allocate` →
  `validate_references` が全単射を確認 → save が `plugin_instances`
  ごと serialize。
- **Load**: envelope `schema_version` 判定 → v12 以下は
  `migrate_to_current` が backfill → `validate_references` oracle →
  live install → `id_watermark` reseed（instance watermark は
  store 内に永続化済みで reseed 対象外 — entity generator とは独立）。
- **Runtime**: `RoutingControlDispatch::attach/sync` → `vend_nodes`
  → `resolve(processor, control.plugin)` → `StablePluginInstanceId`
  → `PluginBindingSession::attach` + `PrecisionProvenance` →
  既存の bind/dispatch 契約で delivery。
- **Apply**: `plan_changes`/`preflight_plugins`（既存: Added
  Unresolved・became-unresolved・unavailable Resolved を reject）→
  `write_entity` が entity 移植 + `sync_processor` →
  `reconcile_membership` → post-apply `validate_references` が
  全単射を最終確認。

## Non-Functional Constraints

- **決定性**: migration の採番順は `BTreeMap<ProcessorId,_>` の昇順
  反復 — map の構築順・実行環境に依らず同一 v12 body は同一 v13
  project を生む。`ensure` は「同じ plugin なら id 維持」なので
  apply の反復で id が churn しない。
- **単調性**: id は `watermark` からのみ発行され、除去されても
  watermark は戻らない — 古い binding 記録/provenance が指す id は
  永久に stale のまま（fail-closed の前提）。
- **容量**: u64 watermark・`checked_add` — 枯渇は `Error::Overflow`
  で typed fail（実用上到達不能）。
- **RT 安全**: hydration は attach/sync の非 RT 経路のみ。
  `vend_nodes` の lookup は BTreeMap 参照で RT callback に触れない。
- **後方互換**: v12 以下の snapshot は migration で読み込み可能、
  v13 body は `deny_unknown_fields` 下で `plugin_instances` を必須
  としない（`#[serde(default)]` — v12 body の parse と v13 の
  区別は envelope の `schema_version` が担う）。

## Risks and Trade-offs

- **VCS apply による rebind（Resolved→別 PluginRef）**: `ensure` が
  新 id を採番する — 旧 id を指す binding 記録は generation pin と
  合わせて stale 化する（正しい挙動: 別 plugin instance）。preflight
  が unavailable plugin を既に reject するため到達は限定的。
- **apply が Resolved→Unresolved を持ち込む**: preflight_plugins が
  `became_unresolved` を既に reject — sync の remove 分岐は防御的
  一貫性のため残す（oracle が最終確認）。
- **pub field 経由の in-memory 改竄**: `Project` の field は全て pub
  で ops 迂回は可能 — `resolve` の plugin 完全一致 + id 範囲検査が
  runtime 側の最終防衛線。load/apply の oracle が永続経路を塞ぐ。
- **driver.rs との id 空間共有**: clip-insert FX の ephemeral id と
  persisted id が同じ u64 値を取り得るが、provenance/session の
  文脈（node・generation・session 型）が異なり比較経路が交差
  しない — domain 分離を doc に明記。

## Deferred Design Concerns

- `ProcessorBinding` の正式 rebind op — apply の Modified 経由のみ
  現状維持。専用 op が必要になれば別 spec で identity 遷移規則を
  再検討。
- binding record 自体の snapshot 永続化 — pc-c4-04 app 配線以降。

## Verification Notes

- [ ] `mise run test` で project 側新規 test（save/load round-trip・
  watermark 単調・oracle reject・v12→v13 migration・apply 回帰）と
  player 側 hydration/fail-closed test が green。
- [ ] 既存 `vcs05`/`vcs06` apply test・`pc_c4_binding`・`sf03_l6`・
  `ctl_02`・`tim03` 系が regression なく green。
- [ ] `md_a.rs`/`tim_02_smf.rs` 等の `SCHEMA_VERSION` pin 更新が
  必要なら追従（v13 で literal 更新）。
