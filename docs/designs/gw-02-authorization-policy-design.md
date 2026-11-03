---
id: "DESIGN-0Gw02AuthorizationPo01"
type: "design"
status: "draft"
title: "GW-02 Principal / Execution Grant / Authorization Policy — Fixed/Dynamic Scope"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/54"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/src/clock.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/grant.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/store.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/policy.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/authorization.rs"
      - type: "file"
        path: "docs/designs/gw-02-authorization-policy-design.md"
      - type: "file"
        path: "docs/impl/ir/gw-02-authorization-policy.md"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/capability.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/request.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/id.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/fixture.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/contract.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/fixtures.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/54"
  implemented-by:
    - "docs/impl/ir/gw-02-authorization-policy.md"
  depends-on:
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  verifies: []
  verified-by:
    - "docs/impl/ir/gw-02-authorization-policy.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/44"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  deferred-by: []
---

# GW-02 Authorization Policy 設計

Issue #54 — #44 AHG-006〜010・#45 GTC-003/ACG-003 の認可契約を、GW-01 の
`PolicyEvaluator` seam に載せる評価器として実装する。**MCP 認証済み ≠
Project/Media/Apply 許可済み**を型と評価経路で強制し、信頼できる境界の外側
(wire 入力・Agent の自己申告)が権限を作れないことを保つ。

## 1. 所有権境界

GW-02 は identity/grant/policy/scope の**評価**を所有する。所有しないもの:

- Project 正本・Revision・書き込み(`synvelis-project` / GW-06)。
- Project 状態の Resource ビュー — `ResourceOracle` trait が host 注入の
  seam であり、gateway は第二の project store を持たない(AHG-008 の事実
  供給は GW-03/04/06 側の snapshot)。
- Semantic Effects の**導出** — `ScopedEffects` は GW-04 が作る入力 view。
  GW-02 は与えられた effect を scope/data-access に照合するだけ。
- Proposal/Approval/Receipt lifecycle(GW-05)、commit fence・永続化・
  Recovery(GW-06)、MCP transport(GW-08)、UX(GW-09)。

## 2. モジュール追加

```text
synvelis-gateway (GW-01 modules unchanged)
├── clock.rs     PolicyInstant / PolicyClock / SystemClock / ManualClock
│                — 評価時刻の注入 seam(決定的 clock で期限判定を検証可能に)
├── grant.rs     Grant / GrantBinding / ApprovalPolicy / GrantLimits /
│                Validity / PolicyEpoch / ResourceScope(Exact|Snapshot|
│                Dynamic) / DynamicScope / OwnerConstraint / MoveArgs /
│                CreateWithin / PlacementConstraint / ScopedEffects /
│                CreateEffect / ResourceAttributes / ResourceFacts /
│                ResourceOracle
├── store.rs     PolicyStore trait / AuthorizedTask / Activation /
│                ProactivePermit / GrantRecord / InMemoryPolicyStore
└── policy.rs    GrantPolicy — PolicyEvaluator 実装 +
                 evaluate_effects / evaluate_commit entry points
```

`capability.rs` には `DataAccessClass`(5 class)・`DataAccessSet` と
descriptor の `data_access` 宣言を追加。`request.rs`/`id.rs` には
`ResourceRef`/`CapabilityRef` の ordering derive(BTreeSet scope 用)、
`ResourceRef::parse_tagged`、`ProactivePermitRef` を追加。

## 3. Authority モデル(AHG-007)

評価は 4 つの別契約を合成する — どれも wire が自己申告できるものではない:

- **Caller** — GW-01 の `CallerIdentity`/`AdapterSeal` 境界で attested。
  `PrincipalKind::Human` は trusted manual authority であり、Agent grant に
  依存しない(通常 DAW 操作を AI 認可へ束ねない)。
- **Activation** — `AuthorizedTask`(UserIssued | Proactive{permit})。
  Agent は live task がなければ動作開始できない。Proactive 起動は
  別契約の `ProactivePermit` が必要 — **Delegated Apply grant は
  proactive 起動権ではない**。
- **Execution Grant** — `Grant`。principal・project・binding(task /
  session / persistent)、明示 capability 集合(exact `id@version`)、
  permissions、data_access、scope、create_within、approval_policy、
  limits、validity、epoch_ceiling、issued_at_revision を保持。
- **Policy Epoch** — store の版 counter。`update_policy` で進み、
  `epoch_ceiling` を持つ grant を失効させる(policy version 更新)。

## 4. Scope モデル(AHG-008)

| Scope | 意味 |
| --- | --- |
| `Exact { resources }` | 明示 stable-id 集合 |
| `Snapshot { resources, captured_revision }` | 発行時 snapshot 由来の固定集合(membership は Exact と同じ、由来のみ記録) |
| `Dynamic(DynamicScope)` | 制約付き動的所属 — owners(Any|OneOf)、kinds、range(同 domain 完全包含)、max_members、include_future、move_args |

原則:

- **親子継承なし** — membership は各 resource 単位で評価。共有 Source は
  `ResourceAttributes::related` として独立評価される(clip 権限は source
  権限を含意しない、SEC-05)。関連先の追跡は capability の effect_bound が
  mutating(Write/Create/Delete)を含む場合のみ — read-only は effect-aware
  stage が実読み込みを照合する。
- **新規 object は黙って所属しない** — `created_revision <=
  issued_at_revision` で証明できる既存 member のみ暗黙所属。
  `include_future: false` が既定で、後発 object は future 加入の明示が必要。
- **move/reparent は after-state を再評価** — `MoveArgs` が指す args field
  (parent/range)から after-state を再導出し同じ scope 制約へ照合。
  宣言 field が malformed なら検証不能 → deny(SEC-06)。
- **create は別軸** — `CreateWithin`(parents・kinds・placement bound)。
  grant が create_within を持たなければ create は一律 deny。create 対象の
  parent は oracle で resolvable なもののみ — 未解決 parent を暗黙信頼しない。
- **range は同一 domain 完全包含** — `checked_add` で境界を計算、
  overflow・domain 不一致は non-membership(AHG-008 の domain 混同禁止)。

## 5. Data Access モデル

`DataAccessClass` — `MetadataRead` / `MediaBytesRead` / `DerivedDataRead` /
`ExternalDisclosure` / `ProviderTransfer`。capability descriptor が**宣言する
class 集合**(`data_access`)と grant が許す集合の両方を持ち、評価は
descriptor ⊆ grant を要求(SEC 系の「宣言した以上の触り方をしない」を型で
担保)。effect-aware stage は `effects.disclosures` と
`external_transfers` を grant.data_access に照合 — ProviderTransfer を含まな
い grant で外部送信 effect が 1 件でもあれば deny(DATA-10)。

## 6. 評価チェーン(GTC-003)

`evaluate_with` は全 entry point 共通:

```text
Human principal?  ──yes──> PermitForStage (manual authority)
        │no
authorize_agent ──> task resolve + principal/project/live 確認
                ──> proactive permit 再確認(UserIssued 以外)
                ──> grant resolve + principal/project/binding 確認
                ──失敗は全て "no applicable authorization" (存在を漏らさない)
check_grant_dimensions ──> revoked / expired / epoch / capability∈ /
                           required_permissions ⊆ / data_access ⊆
check_target_scope ──────> target 種別ごと: create axis / resource(集合)
                           の oracle 解決 + scope coverage + related 追跡
check_effects ───────────> ScopedEffects がある時:
                           unsupported 非空 → deny (SEC-14)
                           reads/writes/deletes → scope coverage
                           creates → create_within
                           disclosures/external_transfers → data_access
stage_gate ──────────────> RequestApply / Commit のみ approval_policy で分岐:
                           Suggest → Deny
                           ApproveToApply → ApprovalRequired
                           DelegatedApply → RequestApply は枯渇を早期報告
                           するだけで消費しない。unit は Commit 認可で
                           try_consume_apply が revoked/expiry/epoch/
                           budget を原子再検証して消費(ACG-003) —
                           論理 apply 1 回につき 1 unit、commit を伴わない
                           preflight 評価は何度でも free
```

Deny reason の情報量制御: binding が証明される前の失敗はすべて
`no applicable authorization` — task/grant の存在有無を probe できない。
証明後は policy 次元名(revoked/expired/scope/data access)までで、
resource の存在は名指さない。`ResourceFacts::Unresolved` は「不在」と
「不可視」を区別しない。

## 7. Revoke/Commit 線形化(ACG-003 → GW-06)

`PolicyStore` の各 method は 1 つの原子操作。`evaluate_commit` は Commit
stage の評価を named entry point として公開し、**呼び出し時点の store 状態**
で再評価する。契約:

- revoke が commit-read **より先**に確定 → read が観測し deny。
- commit が revoke **より先**に確定 → commit は有効(revoke は自動 undo
  ではない)。

GW-06 の fence が「policy 読み(`grant`/`policy_epoch`/`try_consume_apply`)
と project commit」の直列化点を構成する。`InMemoryPolicyStore` は単一
`RwLock` で全操作を直列化し mock レベルで同契約を満たす。durable store・
cross-store atomicity・recovery は GW-06。

## 8. 非対象

- 第二 ProjectController / project store / 独立 Job executor。
- 永続 grant store・commit fence・durable idempotency/recovery(GW-06)。
- UI Grant ダイアログ・approval receipt 検証(GW-05/09)。
- `ScopedEffects` の導出そのもの(GW-04)。
