---
id: "DESIGN-034SF01Mo9rSt4uV1wXyZaCdE"
type: "design"
status: "approved"
title: "SF-01 Semantic Routing Model 設計 — 型・整合・Schema v6 Migration"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/104"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/adr/sf-01-routing-model-schema-v6-migration.md"
  implemented-by:
    - "docs/plans/sf-01-semantic-routing-model.md"
  depends-on:
    - "docs/designs/pc-a-parameter-control-design.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/adr/sf-01-routing-model-schema-v6-migration.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-01-e2e-semantic-routing-model.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/ruviform/issues/184"
  defers: []
  deferred-by: []
---

# SF-01 Semantic Routing Model 設計

> 確定契約は Issue #104「SF-01 確定詳細契約」、ADR は
> `docs/adr/sf-01-routing-model-schema-v6-migration.md`。

## 1. 配置と Stable ID(`synvelis-project`)

`src/routing.rs` に全 routing 型を置く。`src/id.rs` の `define_id!` で
以下を追加 — 全て `u128`、serde は `"<kind>-<32hex>"` tagged string:

| 型 | kind tag | 意味 |
| --- | --- | --- |
| `BusId` | `bus` | 論理経路(Group/Return/Master) |
| `StripId` | `strip` | ChannelStrip — DSP/Fader/Pan/Insert/Send の実 Owner |
| `ProcessorId` | `processor` | Insert chain 上の Processor |
| `ChainId` | `chain` | ProcessorChain — 順序付き Processor 所有 |
| `SemanticPortId` | `semantic-port` | Project semantic port(≠ runtime PortId) |
| `RoutingConnectionId` | `connection` | 接続の唯一正本 |
| `SendId` | `send` | SendConfig — 送出設定 |

Runtime `ruviform_graph::{NodeId, PortId}`、plugin local port
identifier、process slot、node generation は別型。Project semantic ID
への自動付替は行わない。

## 2. `RoutingModel` と well-known 起動 Master

```rust
pub const WELL_KNOWN_BASE: u128 = 1 << 96;      // reserved、generator 不到達
pub(crate) const MASTER_BUS/STRIP/CHAIN/IN/OUT: u128 = BASE+1..=5;

pub struct RoutingModel {
    pub master: BusId,
    pub buses: BTreeMap<BusId, Bus>,
    pub strips: BTreeMap<StripId, ChannelStrip>,
    pub processors: BTreeMap<ProcessorId, Processor>,
    pub chains: BTreeMap<ChainId, ProcessorChain>,
    pub ports: BTreeMap<SemanticPortId, SemanticPort>,
    pub connections: BTreeMap<RoutingConnectionId, RoutingConnection>,
    pub sends: BTreeMap<SendId, SendConfig>,
}
```

- `impl Default` = bootstrap: `Bus{kind:Master, name:"Master"}` +
  `ChannelStrip{owner:Bus(master)}` + 空 `ProcessorChain` + Main
  Input/Output port(各 1 件、`PortBinding::Internal`)。Empty/New
  Project の Master exactly-one を `Project::new` から保証。
- reserved 値は `max_issued_id_raw` の watermark 比較対象から除外
  (発行不能な定数)。`check_id_watermark` は `id_watermark >=
  WELL_KNOWN_BASE` を追加拒否。

## 3. Entity 型(serde 形)

```rust
pub struct Bus { pub id: BusId, pub kind: BusKind, pub name: String, pub strip: StripId }
pub enum BusKind { Group, Return, Master }                 // "group"/"return"/"master"

pub struct ChannelStrip {
    pub id: StripId,
    pub owner: StripOwner,                                // {owner:"track"|"bus", id:"..."}
    pub chain: ChainId,
    pub port_ids: Vec<SemanticPortId>,
}
pub enum StripOwner { Track(TrackId), Bus(BusId) }

pub struct ProcessorChain { pub id: ChainId, pub processors: Vec<ProcessorId> }

pub struct Processor {
    pub id: ProcessorId,
    pub name: String,
    pub binding: ProcessorBinding,                        // tagged "resolved"/"unresolved"
    pub port_ids: Vec<SemanticPortId>,
}
pub enum ProcessorBinding {
    Resolved(PluginRef),                                  // {binding:"resolved", descriptor, version, capability_revision}
    Unresolved { descriptor: String },                    // {binding:"unresolved", descriptor}
}
pub struct PluginRef { pub descriptor: String, pub version: String, pub capability_revision: u64 }

pub struct SemanticPort {
    pub id: SemanticPortId,
    pub owner: PortOwner,                                 // {owner:"strip"|"processor", id}
    pub direction: PortDirection,                         // "input"/"output"
    pub role: PortRole,                                   // "main"/"sidechain"
    pub domain: SignalDomain,                             // "audio"
    pub binding: PortBinding,                             // internal/plugin-declared/unresolved
}
pub enum PortOwner { Strip(StripId), Processor(ProcessorId) }
pub enum PortBinding {
    Internal,
    PluginDeclared { plugin: String, advertised: String, capability_revision: u64 },
    Unresolved { plugin: String, advertised: String },
}

pub struct RoutingConnection {
    pub id: RoutingConnectionId,
    pub source: SemanticPortId,                           // Output
    pub destination: SemanticPortId,                      // Input
    pub kind: ConnectionKind,                             // {kind:"main"|"sidechain"} or {kind:"send", id:"send-..."}
    pub enabled: bool,
    pub channel_map: Option<ExplicitChannelMap>,
}
pub enum ConnectionKind { Main, Send(SendId), Sidechain }
pub struct ExplicitChannelMap { pub entries: Vec<u32> }   // dest ch i ← source ch entries[i]

pub struct SendConfig {
    pub id: SendId,
    pub connection: RoutingConnectionId,
    pub tap: SendTap,                                     // "pre-fader"/"post-fader"
    pub gain_target: Option<ParameterTargetId>,           // 参照のみ(ADR D6)
}
```

全 struct は `#[serde(deny_unknown_fields)]`。enum は既存規約
(`ControlScope` の `tag`+`content` / unit variant は kebab-case
string) に揃える。

**不変条件(routing oracle `RoutingModel::validate(&Project)`):**

1. 全 Map で key == value.id。
2. `master` bus が存在し `kind==Master`、Master kind は全体で厳密 1 件。
3. Bus↔Strip は相互参照一致(`bus.strip` が指す strip の owner がその
   bus)。bus を owner とする strip は 1 件のみ。
4. Track を owner とする strip は Track 1 件につき高々 1 件、owner の
   Track は `project.tracks` に存在。
5. Strip は `chain` が存在し、`port_ids` は全て実在し owner 逆参照一致。
   Strip は Main Input×1・Main Output×1 を厳密に持つ。
6. Chain は高々 1 つの strip から参照され、`processors` は重複なし・
   全実在。各 Processor は全体で高々 1 chain に所属。
7. Processor の `port_ids` は実在・owner 一致。Main Input×1・Main
   Output×1 を持ち、Sidechain role port は Input のみ・Processor のみ。
8. Connection: source/destination 実在、source=Output・dest=Input、
   domain 一致、source.owner ≠ dest.owner、同一 (source,destination)
   重複なし。
9. kind 別制約: `Main`/`Send` は両端が Strip 所有・role Main。
   `Sidechain` は dest が Processor 所有・role Sidechain、source は
   Strip 所有・role Main Output。
10. `kind=Send(sid)` ⇔ `sends[sid].connection == conn.id` の厳密 1:1
    (孤立 Config・孤立 Send edge・二重参照を拒否)。
11. `channel_map` が `Some` なら entries は非空。
12. `SendConfig.gain_target == Some(t)` なら `t ∈ project.control.targets`。
13. Strip グラフ(Main+Send+Sidechain edge の owner-strip 射影)は
    無環(DAG)。初期は遅延有無を問わず全 cycle 拒否(SF-02 ポリシーと
    整合、model 層でも保証)。

## 4. Schema v6 + Migration Registry

`store.rs`: `SCHEMA_VERSION = 6`。
`migration.rs`(新規、crate 内 `pub(crate)`):

```rust
struct MigrationStep { from: u32, migrate: fn(Project) -> Result<Project, Error> }
const STEPS: &[MigrationStep] = &[MigrationStep { from: 5, migrate: migrate_v5_routing }];
pub(crate) fn migrate_to_current(version: u32, project: Project) -> Result<Project, Error>;
```

`controller.load()`:
- `schema_version == SCHEMA_VERSION` → そのまま。
- `5` → `migrate_to_current` 適用 → `live_revision = saved_rev + 1`。
- その他 → `Error::UnsupportedSchema { found }`。
- migrate 適用後も `validate_references` + `check_id_watermark` を必ず通す。

`migrate_v5_routing`:
- `project.routing == RoutingModel::default()` でなければ
  `Validation` 拒否(v5 は routing を持ち得ない = 偽造入力)。
- `base = max(max_issuable_id_raw(project), project.id_watermark)`。
- Track を `TrackId` 昇順に走査、連番で strip→chain→port_in→port_out→
  conn を発行(strip.out→master.in, kind=Main, enabled=true,
  channel_map=None)。
- `base + issued` が `WELL_KNOWN_BASE` に達したら `Validation` 失敗。
- `project.id_watermark = base + issued_count`。
- 同一 v5 bytes の再投影は同一結果(入力決定的・出力決定的)。

## 5. `ProjectOp` 追加(全て 1 CAS + 1 Undo)

| Op | validate(op 側) | apply |
| --- | --- | --- |
| `AddBus{kind,name}` | kind≠Master | Bus+Strip(owner=bus)+Chain+2 Ports 生成 |
| `RemoveBus{bus_id}` | 存在・kind≠Master | bus+strip+chain+ports cascade、dangling は oracle 拒否 |
| `AddTrackStrip{track_id}` | track 存在・strip 未所有 | strip+chain+2 ports+Main conn→master.in(5 draw) |
| `AddConnection{source,dest,kind,channel_map}` | 両 port 存在・kind∈{Main,Sidechain} | conn(enabled=true) 挿入 |
| `RemoveConnection{connection_id}` | 存在 | conn 削除;`Send` なら SendConfig も除去(1:1) |
| `AddSend{source,dest,tap,gain_target}` | 両 port 存在・gain_target 存在 | SendId→conn(kind=Send)→SendConfig 同時作成 |
| `RemoveSend{send_id}` | 存在 | config+connection 除去 |
| `SetSendTap{send_id,tap}` | 存在 | tap 更新 |
| `SetConnectionEnabled{connection_id,enabled}` | 存在 | flag 更新 |
| `InsertProcessor{strip_id,index,name,binding,sidechain_input}` | strip 存在・index 範囲 | processor+ports(main in/out [+sidechain in])、chain 挿入 |
| `RemoveProcessor{processor_id}` | 存在 | processor+ports cascade、dangling は oracle |
| `ReorderProcessor{chain_id,processor_id,new_index}` | chain/processor 存在・index 範囲 | 順序更新 |

- `RemoveTrack` は従来の lanes 検査 + strip/chain/ports の cascade を
  追加。参照中は oracle が拒否 → composite
  `[RemoveConnection…, RemoveTrack]` が正規手順(ADR D4)。
- `RemoveParameterTarget`: `sends` の `gain_target` に参照される
  target は oracle 拒否で保護(validate 追加)。

## 6. `Project` 変更

- `pub routing: RoutingModel` を `control` の後に追加
  (`#[serde(default)]` = v5 body parse 用。完全 Migration とは誤認
  しない — ADR D3)。
- `validate_references()` 末尾で `self.routing.validate(self)`。
- `max_issued_id_raw` に routing 7 Map を fold(reserved 値除外)。
- `check_id_watermark` に `id_watermark < WELL_KNOWN_BASE` 条件追加。

## 7. Legacy Lowering 保持(SF-03 境界)

`synvelis-player::lower`/`lower_with_inserts` は routing を読まず従来の
Clip→Master fan-in のまま(SF-03 #106 が model 起点 lowering を担当)。
Golden 比較は「同一 Project へ routing op 適用前後で `lower()` 出力が
byte-identical」で担保する。master `insert_count` は引数のまま →
persisted processor を捏造しない。

## 8. SF-02 接続点(文書化)

- `RoutingTransaction`(Temp Ref→Stable ID)は `Composite` に詰めず
  独立 preflight/scratch staging。SF-01 op は primitive。
- Mute/Solo field・send level target 同時生成・異 Layout 承認・
  Feedback cycle policy(詳細)は SF-02/CTL-F02。model 層の DAG 保証は
  SF-01 で既に保持。
- `ControlOwner::{Strip,Send,Processor}`・`AutomationTimeScope`・旧
  `ControlScope` migration は CTL-01。`gain_target` 存在検証のみ本件。

## 9. 検証方針

test-spec `docs/test-specs/sf-01-e2e-semantic-routing-model.md` の
Feature/Rule/Example に従い `crates/synvelis-project/tests/routing_sf01.rs`
で headless 検証。`mise run test` workspace 全体 + `lint`/`fmt-check`/
`build`/`check` を PASS。
