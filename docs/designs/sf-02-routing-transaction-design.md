---
id: "DESIGN-034SF02Vy9qNo6pS9uA4bCeFg"
type: "design"
status: "approved"
title: "SF-02 RoutingTransaction/TempRef Staging・二層検証・Mute/Solo Audibility 設計"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/105"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-01-semantic-routing-model-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
    - "docs/adr/sf-02-routing-transaction-port-channels-mute-solo-authority.md"
  derived-by:
    - "docs/plans/sf-02-routing-transaction.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-02-e2e-routing-transaction.md"
  references: []
---

# SF-02 設計 — RoutingTransaction・TempRef Staging・二層検証・Audibility

## 1. 目的と境界

SF-01 の `RoutingModel` 正本に対し、UI gesture を表す編集列を
atomic に検証・Commit する層を `synvelis-project` に追加する。
書き込み経路は `ProjectController::apply(Command)` だけであり、
runtime への lowering/publish(SF-03)は含まない。

## 2. 型設計(`crates/synvelis-project/src/routing_tx.rs` 新規)

### 2.1 TempRef と参照型

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TempBusId(pub u32);
// 同様に TempSendId / TempConnectionId

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BusRef { Existing(BusId), Temp(TempBusId) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StripRef { Existing(StripId), Bus(BusRef) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SendRef { Existing(SendId), Temp(TempSendId) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConnectionRef { Existing(RoutingConnectionId), Temp(TempConnectionId) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PortRef { Existing(SemanticPortId), OfStrip(StripRef, PortRole) }
```

`PortRef::OfStrip` は strip 作成直後の `main_in/main_out` など role 一意の
自動生成 port を指すため、gesture 内の新 bus への接続で使う。
send_out/in・sidechain_in は明示 PortId が必要(role 非一意)。

### 2.2 Intent / Edit

```rust
pub struct RoutingEditIntent {
    pub operation_id: String,
    pub expected_port_capability_generation: u64,
    pub edits: Vec<RoutingEdit>,
    pub approved_conversion: ExplicitConversionPolicy,
}

pub enum ExplicitConversionPolicy { None, ExplicitMaps }

pub enum RoutingEdit {
    CreateBus { temp_id: TempBusId, name: String, kind: BusKind },
    RemoveBus { bus: BusRef },
    CreateConnection { temp_id: TempConnectionId, source: PortRef,
        destination: PortRef, kind: ConnectionKind, channel_map: Option<ExplicitChannelMap> },
    RemoveConnection { connection: ConnectionRef },
    SetConnectionEnabled { connection: ConnectionRef, enabled: bool },
    ChangeChannelMap { connection: ConnectionRef, channel_map: Option<ExplicitChannelMap> },
    CreateSend { temp_id: TempSendId, source: PortRef, destination: PortRef,
        tap: SendTap, level: SendGainSpec, channel_map: Option<ExplicitChannelMap> },
    RemoveSend { send: SendRef },
    SetSendTap { send: SendRef, tap: SendTap },
    SetSendGainTarget { send: SendRef, target: ParameterTargetId },
    ReorderProcessor { chain: ChainId, from: usize, to: usize },
    SetMute { strip: StripRef, muted: bool },
    SetSolo { strip: StripRef, soloed: bool },
    ChangeBusConfiguration { bus: BusRef, name: Option<String>, kind: Option<BusKind> },
}

pub enum SendGainSpec {
    /// Strip-level target adopt(既存 ParameterTarget を参照)
    Adopt(ParameterTargetId),
    /// 新規 target を transaction で生成し ControlOwner::Send に紐付ける
    Create { initial_db: f64 },
}
```

`expected_project_revision` は `Command::expected_revision` が担い intent
内には置かない(単一権威の重複を避ける)。`ChangeBusConfiguration` は
初期では name/kind のみ。`CreateSend` は routing connection + SendConfig +
level target を 1 transaction で生成する。

### 2.3 Error / Receipt

```rust
pub enum RoutingEditError {
    StaleRevision { expected: u64, actual: u64 },
    StalePortCapability { expected: u64, actual: u64 },
    DuplicateOwnership { detail: String },
    UnresolvedPort { port: SemanticPortId },
    UnresolvedReference { detail: String },
    InvalidDomainOrDirection { detail: String },
    UnsupportedChannelMap { detail: String },
    FeedbackCycle { detail: String },
    DanglingReference { detail: String },
    DependencyInUse { detail: String },
    RequiresUserDecision { detail: String },
    ResourceBudget { detail: String },
    GenericGraphRejected { violations: Vec<String> },
    Validation(Error),
}
```

```rust
pub struct RoutingPlan {
    pub project: Project,            // staging 済み候補(完全検証済み)
    pub receipt: RoutingReceipt,     // TempRef→Stable ID・割当 ID
}
pub struct RoutingReceipt {
    pub operation_id: String,
    pub bus_ids: BTreeMap<TempBusId, BusId>,
    pub send_ids: BTreeMap<TempSendId, SendId>,
    pub connection_ids: BTreeMap<TempConnectionId, RoutingConnectionId>,
    pub gain_targets: Vec<ParameterTargetId>,
}
```

### 2.4 Staging/preflight API

```rust
pub fn preflight(
    base: &Project,
    intent: &RoutingEditIntent,
    ids: &mut IdGenerator,
) -> Result<RoutingPlan, RoutingEditError>
```

- `base.clone()` から開始し、事前割当した TempRef を Stable ID へ解決して
  `RoutingEdit` を既存 routing apply 経路(内部 `apply_*` helper)へ順次適用。
- 全 edit 適用後に `RoutingModel::validate` + 意味検証 + 汎用
  `GraphSpec`→Validator を通す。失敗時は候補を破棄。
- `IdGenerator` は controller のものを使い、CAS 成功時のみ消費を確定
  (validate 側は clone を使うため失敗時に水位は進まない)。

### 2.5 `ProjectOp` 統合

```rust
ProjectOp::Routing { intent: RoutingEditIntent }
```

- `validate`: live revision と一致確認は controller 側 CAS で担保。
  ここでは `preflight` を IdGenerator clone で実行し、結果 plan を
  保持(内部 `OnceCell` 相当は持たず、apply で再度 staging する方が
  単純なため apply 内で同一関数を実行し deterministic であることを
  テストで pin)。
- `apply`: `preflight` 実行 → 成功した候補を project へ適用。
- `receipt()` op 後の TempRef→Stable 対応を返す(gateway op_facts の
  resource 記録用)。

## 3. `SemanticPort.channels`

```rust
pub struct SemanticPort {
    /* 既存フィールド */
    #[serde(default = "default_port_channels")]
    pub channels: u32, // default: 2(stereo)
}
```

- bootstrap/binding で port 生成時に owner に応じて設定
  (strip main_in/out: bus kind/strip 設定、binding input/output: binding capability)。
- 接続両端 `channels` 不一致: `channel_map` 必須。map 提示なしで
  `approved_conversion = ExplicitMaps` なら `RequiresUserDecision`、
  同意+map 正規(map.size == src.channels, 各 index < dst.channels)なら受理。
  map なし & consent None なら `UnsupportedChannelMap`/`RequiresUserDecision`。

## 4. Mute/Solo audibility

```rust
impl RoutingModel {
    pub fn audible_strips(&self, control: &ControlModel) -> BTreeSet<StripId>
    pub fn strip_muted(&self, control: &ControlModel, strip: StripId) -> bool
    pub fn strip_soloed(&self, control: &ControlModel, strip: StripId) -> bool
}
```

- mute/solo 値は control model の Boolean target(owner=strip, role=Mute/Solo)。
- solo 集合非空時: solo 指定 strip から enabled main/sidechain/sends を
  たどり master strip に到達する経路の strip 集合を可聴とする。
  sidechain edge は「destination processor を持つ strip を可聴にするために
  source strip が必要」として依存に含める(初期は source 追従)。
- mute は自 strip の可聴 main/send 出力を止める。sidechain send の
  source strip が mute でも destination strip 経路は維持(逆に source mute は
  sidechain 供給を止める: sidechain edge を持つ send/connection の
  source strip が muted ならその供給は止まる。初期ポリシーは source mute
  追従= sidechain send も止まる、を audibility で表現)。
- 評価は問い合わせ API のみ。接続の `enabled` や SendConfig は変更しない。

## 5. 汎用検証(ruviform-graph 投影)

```rust
fn to_graph_spec(routing: &RoutingModel) -> GraphSpec
```

- node: strip owner(strip id 文字列)+bus node。edge: enabled な
  RoutingConnection。port: SemanticPort(direction/domain/channels/
  binding capability 由来の max edge 数)。
- `Validator::validate` の violation を `GenericGraphRejected` に写像。
- DAW 規則(kind/role 意味・Send 1:1・duplicate pair・ownership)は
  `RoutingModel::validate` が担い、汎用と重複実装しない。

## 6. 世代検査

- `expected_port_capability_generation` = 全 `ProcessorBinding::PluginResolved`
  の `capability_revision` 総和(決定的・単調)。不一致は StalePortCapability。
- `PluginDeclared`/`Unresolved` の processor port を CreateConnection の
  端点に使うと `UnresolvedPort`。
- Plugin reconfigure で port が再解決されると新 ID となり、旧 ID は
  dangling として reject(自動配線しない)。

## 7. Undo / 保存 / 既存契約

- 1 transaction = 1 op = 1 undo snapshot(既存機構)。Undo/Redo は
  Project-only。
- 新 op variant は既存 `op_name` / gateway op_facts に `"routing"` として追加。
- `RoutingModel::validate`・永続 schema・既存 ops の意味は維持。
