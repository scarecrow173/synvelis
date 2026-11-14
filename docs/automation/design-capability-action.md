# Capability→Action 対応契約（AUTX-01 境界）

一次正本: `crates/synvelis-gateway/src/capability.rs`・`request.rs`・
`outcome.rs`・`gateway.rs`。本書は投影・対応関係のみを定義し、
第二 Registry を作らない。

## 1. Action の定義

**Action は `CapabilityRef { id, version }` の presentation 投影である。**

- ActionId = `CapabilityRef.id`（`CapabilityId`: 小文字 `[a-z0-9._-]`、
  最大 128 文字）。版は `CapabilityVersion { major, minor }` で厳密解決。
- Action の実行意味論は常に `Gateway::bind` → `Gateway::invoke` を通る。
  Action 独自の Executor・検証・認可経路を持たない。
- Action Metadata が保持してよいのは検索名・カテゴリ・UI input hint のみ。
  入出力 Schema・TargetContract・EffectSet・DataAccessSet・PermissionSet・
  Availability・ExecutionContract の定義元は `synvelis-gateway` の
  `CapabilityDescriptor` のみであり、Action 側で複写・上書きしない。

## 2. `CapabilityDescriptor` → Action/CLI/Workflow 対応表

| Gateway 正本（capability.rs） | Action/CLI/Workflow 側の扱い | Owner |
| --- | --- | --- |
| `CapabilityDescriptor.id` / `.version` | ActionId + 版 pin。CLI/Workflow は `CapabilityRef` をそのまま名前付ける | AUTX-01 (#260) |
| `.domain: OperationDomain` | 実行経路の分岐（Query/Proposal/ProjectMutation/RuntimeControl/AsyncOperation）。Action は domain を変更しない | GW-01（既存） |
| `.input_schema` / `.output_schema: SchemaRef` | 型付き引数・結果の検証。Script/CLI/Workflow の引数はこの schema に適合必須 | GW-01/03（既存） |
| `.target: TargetContract` | 明示ターゲット必須。UI 選択/フォーカス/active window の暗黙解決は禁止（`ImplicitTargetRejected`） | GW-01（既存） |
| `.effect_bound: EffectSet` | GW-04 が導出する実効果の上限。Action/Script は自称効果を宣言できない | GW-04（既存） |
| `.required_permissions: PermissionSet` | capability 存在 ≠ 権限。`DenyAllPolicy` が既定 | GW-02（既存） |
| `.data_access: DataAccessSet` | Grant が被覆すべき data 分類の上限 | GW-02（既存） |
| `.execution: ExecutionContract` | `Sync`→`Completed`（Proposal domain は `ProposalReady`）、`AsyncAccept`→`Accepted{operation}`。policy が `ApprovalRequired` を返す場合は別途 `ApprovalRequired` | GW-01/07（既存） |
| `.availability: Availability` | 後述 §3。表示用 status と実行権限を分離 | GW-01（既存）+ AUTX-01 表示投影 |

## 3. Capability 状態（Availability）と表示 status の分離

`Availability`（登録・実行の正本）は 4 状態:

| `Availability` | 意味 | 呼び出し可否 |
| --- | --- | --- |
| `Available` | 現在呼び出し可能 | 可（認可通過時） |
| `Disabled { reason }` | 管理者により無効化 | 不可（`Unavailable`） |
| `Planned` | 将来 Milestone 用に宣言済み | 不可。推測呼び出し禁止 |
| `Retired` | 提供終了。schema/audit 連続性のため残存 | 不可 |

表示側（Action カタログ・CLI `action list`・GUI）が加えて使う表示専用
status（実行権限を与えない）:

| 表示 status | 対応する正本 | 備考 |
| --- | --- | --- |
| `Unknown` | Registry に存在しない id | 表示のみ。`UnknownCapability` で拒否 |
| `Hidden/Planned` | `Availability::Planned` | 発見可能だが実行不可と明示 |
| `Available` | `Availability::Available` かつ policy が許可 | 「表示可能」≠「実行可能」。実行は毎回 `invoke` で再認可 |

契約: **どの表示 status も Grant・Approval・実行許可を生成しない。**
発見（`DiscoverCapability` stage）と実行（domain 別 stage）は別々の
`AuthorizationStage` で評価される。

## 4. 起動パイプライン（全アダプタ共通）

```
json → Gateway::parse_wire → WireRequest
     → Gateway::bind(wire, caller: CallerIdentity) → OperationRequest
     → Gateway::invoke(&req) → GatewayOutcome<Value> | ErrorEnvelope
```

- `parse_wire`: `SelfDeclaredAuthority` / `ImplicitTargetRejected` /
  `MalformedRequest` を拒否。wire に caller identity・権限を自己申告する
  フィールドは存在してはならない。
- `bind`: capability 解決 → availability → schema 互換 → target/consistency
  → args 検証の順で fail-closed。`CallerIdentity` は trusted adapter の
  `AdapterSeal` 経由でのみ生成（`request.rs`）。
- `invoke`: descriptor の domain から `AuthorizationStage` を導出し、
  `PolicyEvaluator::decide` が `PermitForStage` の場合のみ executor へ。
  `ApprovalRequired` → `GatewayOutcome::ApprovalRequired`、
  `Deny` → `Denied{stage}`。
- Idempotency: `invoke` が副作用前に dedupe し、成功後にのみ記録。
  `DuplicateRequest`（同一 payload）/ `IdempotencyConflict`（別 payload）。

## 5. Outcome / Error の共有契約

GUI・MCP・CLI は以下を**同一の型・同一の意味**で受け取る:

| `GatewayOutcome<T>` | 意味 | CLI/Workflow での扱い |
| --- | --- | --- |
| `Completed { result }` | 型付き結果（output schema 検証済み） | 成功。receipt に operation id を記録 |
| `ProposalReady { proposal }` | 非破壊 prepare 完了。commit ではない | preview/approval へ誘導 |
| `ApprovalRequired { approval }` | 適用に承認が必要 | 承認待ちとして surface。script/CLI から承認を捏造しない |
| `Accepted { operation }` | 非同期受理。完了は job owner が報告 | `Await` step または job status 再照会へ |

| `ErrorCategory` | 例 | RetryAdvice の目安 |
| --- | --- | --- |
| `Authentication` | `SelfDeclaredAuthority` | `Never`/`AfterCorrection` |
| `Authorization` | `Denied` | `AfterCorrection`（Grant 付与後） |
| `Approval` | `ApprovalInvalid` | `AfterCorrection`（再承認） |
| `Consistency` | `RevisionConflict`/`SnapshotExpired`/`DigestMismatch` | `AfterCorrection`（再 prepare/再 pin） |
| `Validation` | `SchemaViolation`/`ImplicitTargetRejected` | `AfterCorrection` |
| `Capability` | `UnknownCapability`/`UnsupportedCapabilityVersion`/`IncompatibleSchema` | `Never`（版確認） |
| `Effects` | effect bound 超過 | `AfterCorrection` |
| `DuplicateRequest` | `DuplicateRequest`/`IdempotencyConflict` | `Never`/`ReconcileState` |
| `Recovery` | `Indeterminate` | **`ReconcileState` 必須。推測で成功扱いしない** |
| `Internal` | `Internal`/`NoExecutor` | `Transient`/`ReconcileState` |

CLI の機械可読出力（stdout JSON / stderr 診断）はこの `ErrorEnvelope`
（`code`/`category`/`retry_advice` を含む）をそのまま serialize する。
GUI/MCP が独自の error 語彙を被せない。

## 6. 第二正本ゼロの照合（本契約の不変条件）

| 概念 | 唯一の正本 | 禁止される第二正本 |
| --- | --- | --- |
| Capability 識別・版・schema・effect/permission/data-access 上限 | `synvelis-gateway` `CapabilityRegistry`/`SchemaRegistry` | Action Registry、CLI 独自 catalog、Script 独自 registry |
| 実行認可 | GW-02 `PolicyEvaluator` + Grant/Approval（GW-05/06） | CLI 独自 auth、Script 内 Grant、Workflow 独自 policy |
| Project 変更 | GW-06 `CommitGate`（単一 writer） | Macro/Workflow/CLI の直接 mutate、第二 Transaction Engine |
| Job 実行状態 | provider/coordinator（`JobPort` 先） | adapter/CLI/Workflow 側の job truth 複写 |
| Proposal/Approval/Apply 状態 | GW-05/06 proposal store | Workflow journal への状態複写（参照のみ保持） |

## 7. 検証可能な受入（AUTX-01 向け）

- Action 一覧が `CapabilityRegistry` の投影のみから生成され、
  registry 外の id を発明できない。
- `Availability::{Disabled,Planned,Retired}` の capability が
  表示されても `invoke` は `Unavailable` で拒否される。
- CLI/GUI/MCP の同一 `CapabilityRef` 呼び出しが同一の
  `GatewayOutcome`/`ErrorEnvelope` を返す（fake adapter で契約試験可能）。
- wire 引数に caller/principal/grant を自己申告するフィールドを
  追加した場合 `SelfDeclaredAuthority` で拒否される（既存 GW-01 契約の再確認）。
