---
id: "DESIGN-0Gw01GateWayContract01"
type: "design"
status: "draft"
title: "GW-01 Gateway Contract — Capability Registry・Typed Invocation・Versioned Result/Error"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/53"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-gateway"
    modified:
      - type: "file"
        path: "Cargo.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/53"
  implemented-by:
    - "docs/impl/ir/gw-01-gateway-contract.md"
  depends-on:
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
    - "docs/designs/midi-source-note-editing-design.md"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  verifies: []
  verified-by:
    - "docs/impl/ir/gw-01-gateway-contract.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/44"
    - "https://github.com/scarecrow173/synvelis/issues/52"
  defers: []
  deferred-by: []
---

# GW-01 Gateway Contract 設計

Issue #53 — #45 GTC-001/002/007/008 の型付き内部 Gateway を、MCP 非依存の
契約 crate として先行実装する。GW-02〜10 が積み上げる**契約境界**であり、
認可判断・Project 正本・Transport/Job 実行を所有しない。

## 1. crate 配置の決定

採用: **新規 crate `synvelis-gateway`**(workspace member、依存は
`synvelis-project` のみ)。

- Issue #53 は「新crate名は未固定」と明示。`synvelis-*` 命名規約と
  「Synvelis Gateway contract」Owner 表記に従い `synvelis-gateway` とする。
- `synvelis-project` への同居は不採用: GTC-001 で GW-01 は「Project の正本」
  を所有せず、project crate は persisted value の所有 crate。Gateway は
  その型(Typed ID/Revision/Time)を**消費する側**であり、所有境界を混ぜない。
- `synvelis-project` への依存は値型参照のみ(`ProjectId`/`SourceId` 等の
  stable ID、`ProjectRevision`、kind 判定用の tagged 文字列表現)。
  `ProjectController`/`Operation`/`ProjectStore` には依存しない —
  GW-01〜05 が Revision を変更しない(GT-010)を依存構造で裏付ける。

## 2. モジュール構成

```text
synvelis-gateway
├── id.rs            RequestId / OperationId / TaskRef / GrantRef /
│                    ProposalRef / ApprovalRef / SnapshotRef / IdempotencyKey /
│                    AdapterId
├── capability.rs    CapabilityId / CapabilityVersion / CapabilityRef /
│                    OperationDomain / TargetContract / EffectKind /
│                    PermissionKind / ExecutionContract / Availability /
│                    CapabilityDescriptor
├── schema.rs        SchemaVersion / SchemaRef / ValueSchema /
│                    SchemaRegistry / 引数検証・Schema 互換判定
├── wire.rs          WireRequest / WireTarget / WireConsistency /
│                    WireRange — serde wire 型 + parse(authority key 拒否)
├── request.rs       OperationRequest / OperationTarget / ResourceRef /
│                    ResourceKind / Consistency / TimeDomain / TypedRange /
│                    CallerIdentity / Principal / AdapterSeal
├── authorization.rs AuthorizationStage / AuthorizationDecision /
│                    PolicyEvaluator trait / DenyAllPolicy(fail-closed 既定)
├── outcome.rs       GatewayOutcome<T> / GatewayError / ErrorEnvelope /
│                    ErrorCategory / RetryAdvice
├── registry.rs      CapabilityRegistry — register/describe/list/versions
├── gateway.rs       Gateway — parse_wire → bind → invoke の型付き経路、
│                    OperationId 採番、in-memory IdempotencyLedger
└── fixture.rs       FixtureAdapter(AdapterSeal 発行)・4 Domain fixture
                     capability・mock handler・PermitSetPolicy
```

## 3. Capability Registry(GTC-002)

```rust
pub struct CapabilityDescriptor {
    pub id: CapabilityId,                 // stable dotted id, tool名≠id
    pub version: CapabilityVersion,       // { major, minor }
    pub domain: OperationDomain,          // Query|ProjectMutation|RuntimeControl|AsyncOperation
    pub input_schema: SchemaRef,          // { name, version }
    pub output_schema: SchemaRef,
    pub target: TargetContract,           // None|Resource|ResourceSet|Create{parent,creates}
    pub effect_bound: EffectSet,          // Read/Write/Create/Delete/Disclose/ExternalAction の上限
    pub required_permissions: PermissionSet,
    pub execution: ExecutionContract,     // Sync | AsyncAccept
    pub availability: Availability,       // Available|Disabled|Planned|Retired
}
```

- Registry は `(id, version)` をキーに登録。同一キーの再登録は
  `CapabilityConflict` で拒否(暗黙の上書きで契約がずれない)。
- `describe` は metadata の返却のみで**権限を付与しない**(GT-003)。
  request は exact `CapabilityRef` を名指し — version 省略や
  「latest 推測」は型上存在しない。未知 id → `UnknownCapability`、
  未登録 version → `UnsupportedCapabilityVersion`。
- `Availability != Available` な capability への bind は `Unavailable` で
  拒否(Planned capability を推測実行しない)。

## 4. Typed Invocation(wire → internal)

`Gateway::parse_wire(json) -> WireRequest` と
`Gateway::bind(wire, caller) -> OperationRequest` を分離(GTC-002:
「Wire Request の Parse/Schema/Version チェックと Internal
OperationRequest の構築を分離」)。

```rust
// wire.rs — serde で deserialize される唯一の型。caller 系 field は存在しない
#[serde(deny_unknown_fields)]
pub struct WireRequest {
    pub request_id: String,
    pub capability: CapabilityRef,
    pub schema: SchemaRef,            // client が宣言する input schema version
    pub project: String,              // "project-<32hex>"
    pub target: Option<WireTarget>,
    #[serde(default)] pub args: serde_json::Value,
    pub consistency: WireConsistency, // tagged enum
    pub task: Option<String>, pub grant: Option<String>,
    pub idempotency_key: Option<String>,
}
```

- **GT-002**: parse はまず top-level の authority 系 key
  (`principal`/`caller`/`authority`/`auth`/`token`/`permissions`/
  `approved`/`impersonate`/`as_user`/`role`)を走査し、含有すれば
  `SelfDeclaredAuthority` で拒否。implicit-targeting 系 key
  (`selection`/`focus`/`active_window`/`current_selection`)は
  `ImplicitTargetRejected` で拒否。`deny_unknown_fields` が残りの未知
  top-level field を `MalformedRequest` で拒否し、さらに wire schema が
  所有する入れ子 object(`capability`/`schema`/`target`/`consistency`)
  内の未知 key も明示 scan で `MalformedRequest` にする(internally
  tagged enum では serde の `deny_unknown_fields` が効かないため —
  serde#2123)。wire 上で権限・対象を自己申告する経路を型+検査の両方で
  閉じる。
- **`CallerIdentity`** は private field(実装上 `pub(crate)`)+ 限定
  ctor を持ち、構築には crate が発行する **`AdapterSeal`**(private
  field を持つ capability token)が必要。GW-01 では `FixtureAdapter` が
  唯一の発行元で、origin は `OriginKind::TestFixture` に pin される —
  wire payload・外部 crate のどちらからも caller を捏造できず、fixture
  caller が trusted session に見せかけることもない。
  `CallerIdentity { principal: Principal{kind: Human|Agent, id},
  origin: OriginKind, adapter: AdapterId }` — Human Authority と Agent
  Authority の認可根拠を型で区別(GTC-001)。`Principal::new` は空・
  長過ぎ(256 chars 超)の id を拒否する。
- `OperationRequest` は `Deserialize` を実装せず、全 field が
  `pub(crate)` — crate 外では構築・書換不能で、外部からは read-only
  getter 経由のみ。必ず `bind()` の検証を通った値だけが
  `CapabilityOperation` に届く。

### bind 検証順序(全て型付き Error、推測・補正なし)

1. capability id 解決 → `UnknownCapability`
2. version 解決 → `UnsupportedCapabilityVersion`(supported 一覧付き)
3. `Availability::Available` 確認 → `Unavailable`
4. `SchemaRef` 解決 + 互換判定 → `UnknownSchema` / `UnsupportedSchemaVersion`
5. `project` tagged 文字列 → `ProjectId`(`InvalidId`)
6. target: contract 必須性・ResourceKind 一致(tagged id の kind と宣言
   kind の両方を検査)→ `TargetRequired` / `WrongResourceKind`
7. consistency が domain で許可されるか → `ConsistencyNotAllowed`
   (`ExpectedRevision` 欠落も同 variant — mutation に `Latest` 系を
   許可しない単一の規則として扱う)
8. args を input `ValueSchema` で検証 → `SchemaViolation` /
   `WrongTimeDomain`(TimeRange node の domain 不一致)
9. → `OperationRequest` 構築(OperationId 採番、caller 添付)

idempotency 照合は **invoke 側**に置く:bind は副作用を持たず、
`PermitForStage` 判定後・handler 実行前に ledger を引き、handler 成功後
にのみ key を記録する。deny/失敗/未実行の request が key を燃やすことは
ない(`DuplicateRequest.original` は必ず実際に実行された operation を
指す)。fingerprint は capability/schema/project/target/args/consistency/
task/grant を含む。

### Consistency(domain 別、混同しない — AC2)

```rust
pub enum Consistency {
    Latest,                                  // Query のみ
    ReadAt { revision: ProjectRevision },    // Query のみ
    FixedSnapshot { snapshot: SnapshotRef }, // Query のみ
    ExpectedRevision { revision: ProjectRevision }, // ProjectMutation のみ必須
    Unconditional,                           // RuntimeControl / AsyncOperation
}
```

- `ProjectMutation` は `ExpectedRevision` **必須** — `Latest`/`ReadAt`/
  `FixedSnapshot`/`Unconditional` はすべて `ConsistencyNotAllowed` で
  拒否する(別 variant は持たない)。
- `Query` は `ExpectedRevision` を拒否 — mutation の expected と query の
  pin-read を型 variant で分離し、同一 revision 値でも意味が混同しない。

## 5. Result / Error Contract(GTC-007/008)

```rust
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum GatewayOutcome<T> {
    Completed { result: T },                  // capability 処理完了 ≠ Project Committed
    ProposalReady { proposal: ProposalRef },
    ApprovalRequired { approval: ApprovalRef },
    Accepted { operation: OperationRef },     // = OperationId
}
```

全 variant が named-field — internally tagged enum は scalar/newtype
payload を serialize できない(serde#1183)ため、`status` tag + named
field の表現で wire 形状を安定させる。

```rust
pub struct ErrorEnvelope {
    pub code: GatewayError,                // 型付き分類(§4 の全 variant)
    pub category: ErrorCategory,           // Authentication|Authorization|Approval|
                                           // Consistency|Validation|Capability|
                                           // Effects|DuplicateRequest|Recovery|Internal
    pub retry: RetryAdvice,                // Never|AfterCorrection|Transient|ReconcileState
    pub message: String,                   // 外部開示可メッセージ
    pub diagnostic: Option<String>,        // 内部診断 — to_external() で除去
}
```

- `category()`/`retry()` は `GatewayError` variant から導出 — envelope 組立
  で分類を間違えない。`to_external()` は diagnostic を落とした wire 投影を
  返し、内部診断と公開エラーを分離(GTC-003 の情報漏えい対策)。
- State machine は型で分離(GTC-007): GW-01 は `GatewayOutcome` の
  Proposal/Approval/Accepted **分岐点**だけを所有し、Proposal/Approval/
  Apply lifecycle の遷移管理は GW-05/06 に defer。

## 6. 認可 seam(GT-003、GW-02 境界)

```rust
pub enum AuthorizationStage {
    DiscoverCapability, ReadContext, PrepareProposal, RequestApply,
    Commit, StartRuntimeOperation, StartAsyncOperation, ReadOperationResult,
}
pub enum AuthorizationDecision { PermitForStage, ApprovalRequired, Deny { reason } }
pub trait PolicyEvaluator {
    fn decide(&self, req: &OperationRequest, stage: AuthorizationStage,
              capability: &CapabilityDescriptor) -> AuthorizationDecision;
}
```

- `invoke` は**登録済み descriptor の domain**に応じた stage(Query→
  ReadContext、Mutation→RequestApply、Runtime→StartRuntimeOperation、
  Async→StartAsyncOperation)で evaluator を呼ぶ — request 側の主張では
  なく registry の記述が stage を決める。availability も invoke で再確認
  し、bind 時点の状態を信用しない。`PermitForStage` のみ handler へ進む。
  `ApprovalRequired` → `GatewayOutcome::ApprovalRequired`、
  `Deny` → `Denied{stage, reason}` error(evaluator の理由を保持する)。
- registry の `resolve`/handler 参照は `pub(crate)` — discovery
  (`list`/`describe`)は metadata のみを公開し、executor へ到達する唯一の
  経路が authorization-gated な `invoke` になる。
- handler が返した outcome は境界を出る前に検査される:
  `ExecutionContract::Sync` が `Accepted` を返す・`Completed` payload が
  宣言 `output_schema` を満たさない、は executor の bug として `Internal`
  error になる(呼出し側に不正な結果を渡さない)。
- `register_capability` は coherence 検査を兼ねる:宣言 input/output
  schema が store で解決できること、Query domain が mutating effect
  (`Write`/`Create`/`Delete`)を持たないこと。違反は
  `UnknownSchema`/`IncoherentDescriptor` で登録自体が失敗する。
- **既定 evaluator は `DenyAllPolicy`(fail-closed)** — capability が
  registry に存在しても、明示 policy なしに実行経路は開かない
  (Tool Discovery ≠ Permission の実行証拠)。
- Grant 実装そのものは非対象 — GW-01 は decision/stage の**型と呼出
  タイミング**のみを固定し、評価実装は GW-02 が置き換える。

## 7. Idempotency / Identity の意味分離

- `RequestId`(wire 相関、caller 採番)と `OperationId`(gateway 採番の
  operation 実体)を別型にする — wire の request_id を operation の
  identity と混同しない。
- `IdempotencyKey` は (principal kind+id+attesting adapter)+project+key
  で ledger 照合 — 同名 id の Human/Agent や別 adapter が scope を共有
  しない:same key+same fingerprint → `DuplicateRequest`、same key+diff
  fingerprint → `IdempotencyConflict`。ledger は上限付き(4096 entry、
  古い順 eviction)の in-memory guard で、record-after-success のため
  deny/未実行で key は燃えない。永続 Record/Retry 契約は
  GW-06(ACG-004)に defer。
- `TaskRef`/`GrantRef` は opaque 参照型 — 所持は認可の証明にならず、
  GW-01 は存在だけを型付けし評価しない(GTC-002)。

## 8. Fixture / Contract Test 構造(AC4)

`fixture.rs` が 4 domain の静的 capability を登録可能にする
(本番 Operation 不在で契約テスト可能 — Issue スコープ):

| capability | domain | target | consistency | 補足 |
| --- | --- | --- | --- | --- |
| `synvelis.context.source-notes` | Query | Resource{Source} | Latest/ReadAt/FixedSnapshot | args.range = TimeRange{SourceNative} |
| `synvelis.midi.add-note` | ProjectMutation | Resource{Source} | ExpectedRevision 必須 | args.position = TimeRange{SourceNative} |
| `synvelis.midi.create-source` | ProjectMutation | Create{parent:Project,creates:Source} | ExpectedRevision | Create 経路の検証用 |
| `synvelis.transport.set-position` | RuntimeControl | None | Unconditional | |
| `synvelis.job.render-export` | AsyncOperation | None | Unconditional | Accepted outcome |

- mock handler は `EchoHandler`(bind 済み OperationRequest を結果へ
  echo — typed target/args が executor に届いた証拠)と `AcceptHandler`
  (async `Accepted` outcome)。
- 別テストで `ProjectController` を handler 裏に差し込み、expected
  revision 一致→apply 成功 / stale→`RevisionConflict` mapping を検証 —
  GW-06 が接続する seam が実際に機能することの証拠(handler 経由でのみ
  Project が書かれ、GW-01 自身は書かない)。

## 9. 非対象(Issue #53 明示範囲外)

- Grant/Policy 実装(実 evaluator、永続 grant store、scope 評価) — GW-02
- Project Writer 接続(GW-06 が ProjectController へ接続する実行実装)
- MCP Transport / Server、client 認証(GW-08)
- UI / UX(GW-09)、Ruviform への汎用化
- Proposal/Approval/Apply lifecycle の遷移実装(GW-05/06)、durable
  idempotency / recovery journal(ACG-004)
