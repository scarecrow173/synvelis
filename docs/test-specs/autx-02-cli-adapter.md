---
id: "TSPEC-034Xeu6Qk5xuw8LRKrRxcZ"
type: "test-spec"
status: "approved"
title: "AUTX-02 CLI Trusted Local Adapter・Unattended認可・Headless Session Host 検証仕様 (synvelis#261)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/261"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/autx-02-cli-adapter.md"
  depends-on:
    - "docs/specs/command-gateway.md"
    - "docs/test-specs/command-gateway.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/autx-02-cli-adapter.md"
    - "evidence/sv261/autx-02-cli-adapter-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/designs/command-gateway-design.md"
  verified-by: []
  references:
    - "docs/impl/ir/autx-02-cli-adapter.md"
    - "docs/impl/exp/autx-02-cli-adapter.md"
    - "evidence/sv261/autx-02-cli-adapter-ledger.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/272"
    - "https://github.com/scarecrow173/synvelis/issues/273"
  deferred-by: []
---

# AUTX-02 CLI Trusted Local Adapter・Unattended認可・Headless Session Host 検証仕様 (synvelis#261)

## Purpose

Issue #261 の headless authorization-boundary スライスが守るべき
fail-closed 不変条件を、`crates/synvelis-gateway/tests/
cli_adapter.rs` の fixture 群として固定する。「ローカル実行・
TTY・`--yes`・env・script 入力・JSON 引数は Human 権限や Grant を
発行・昇格できない」「unattended 実行は常に agent-equivalent で
あり既存 Grant/Approval/Scope/Expiry/Budget を毎回評価される」
「`AdapterSeal` mint は crate-internal に限定される」という 3 つの
契約を、将来の変更が静かに緩めないよう検証仕様として残す。
実装は `docs/impl/ir/autx-02-cli-adapter.md`、試行は
`docs/impl/exp/autx-02-cli-adapter.md`、実測記録は
`evidence/sv261/` が正本。

## Feature

Trusted local CLI adapter が CLI/script/CI/scheduler からの
呼び出しを fail-closed に分類し、verified interactive session
credential のみが `Human` + `OriginKind::LocalCli` を、
それ以外の全経路が `Agent` 相当の delegated caller のみを
mint すること。`HeadlessSessionHost` が GUI 非依存で gateway・
adapter・commit/admin port を単一経路に組み立て、delegated
caller の全操作が既存 `GrantPolicy` の per-call 評価を通ること。

## Rules

### AUTX-T-R-01 CallerIdentity 非偽造性

- `CallerIdentity`/`AdapterSeal` は wire payload・外部 crate・
  CLI 引数・env・TTY・script 入力から mint できない。
  seal mint は `FixtureAdapter`・`McpHost`・`LocalCliAdapter`
  の 3 issuer に限定される。
- wire request の authority 系 field（`principal`・`caller`・
  `authority`・`auth`・`token`・`permissions`・`approved`・
  `approval`・`impersonate`・`as_user`・`role` 等）は parse 段階で
  拒否される（既存 `SelfDeclaredAuthority` 契約の非回帰）。
- `CliSessionClaim` は `deny_unknown_fields` で `yes`・`tty`・
  `as_human`・`interactive` 等の authority 主張 field を拒否する。

### AUTX-T-R-02 fail-closed 分類

- `CliSessionClaim::Unattended` は principal id の見た目に関わらず
  常に `(Agent, LocalCli)` → `CallerClass::UnattendedDelegated`。
  Human を mint する経路・引数は存在しない。
- `CliSessionClaim::Interactive { credential }` は store が
  検証した live human session のみ `(Human, LocalCli)` →
  `CallerClass::VerifiedHuman` を返す。unknown・expired・revoked・
  malformed・non-human record は全て同一の blind
  `GatewayError::Authentication` で失敗する（existence-blind）。
- session issue/verify は Human principal のみを束縛可能
  （`check_session_kind`）。agent principal への session 発行は
  issuance 時と verification 時の両方で拒否される。
- session record は usable secret を保持しない（SHA-256 hash のみ）。
  record clone・serialize 出力に credential が残らない。

### AUTX-T-R-03 分類の独立 fixture

- Verified Human（verified CLI session・MCP control・fixture
  human）/ Unattended Delegated（local CLI agent）/ External Agent
  （MCP client）/ Fixture Agent（fixture/manual origin）を独立に
  分類し、`authority_class()` が `(PrincipalKind, OriginKind)` の
  みに写像されることを検証する。

### AUTX-T-R-04 delegated 権限の per-call 評価

- delegated caller の全操作は既存 `GrantPolicy` を通り、task/grant
  なしでは `no applicable authorization` で deny される。
- scope・grant/task revocation・grant expiry（policy clock）・
  apply budget・approval policy（`ApproveToApply` →
  `ApprovalRequired`）が delegated 経路でも迂回されない。
- delegated caller の権限は principal に束縛され、他 principal の
  grant では authorize されない。

### AUTX-T-R-05 HeadlessSessionHost composition

- `HeadlessSessionHost::handle_claim` が claim → trusted caller →
  `Gateway::handle` の単一経路を提供し、GUI/windowing/transport
  依存を持たない。
- `without_commit_boundary` は `NoCommitPort`/`NoAdminPort` の
  fail-closed 既定を返し、commit 権限を捏造しない。
- `CommitGate` が `CommitPort`/`AdminPort` を満たし、host 配線が
  実 commit fence・journal・recovery 契約を迂回しない。

## Examples

- Given `CliSessionClaim::Unattended { principal: "user-1" }`
  （human らしい id）, When `attest` を呼ぶ, Then 常に
  `Agent` + `LocalCli` が返り `Human` にはならない
  （`unattended_claim_cannot_mint_human_regardless_of_principal_id`）。
- Given 未知・malformed・wrong-prefix・空の credential, When
  `caller_verified` を呼ぶ, Then 全て同一の
  `GatewayError::Authentication` が返る
  （`interactive_claim_requires_a_verified_credential`）。
- Given 有効な task+grant を持つ delegated caller, When add-note
  を呼ぶ, Then `GrantPolicy` 通常経路で `Completed`;
  grant revoke 後の同一 request は `"grant revoked"`、expiry 後は
  `"grant expired"` で deny される（per-call 再評価）。
- Given authority 系 field を追加した wire JSON, When
  `Gateway::parse_wire`/`handle_claim` を呼ぶ, Then parse/bind 段階で
  拒否され gateway の policy 評価に到達しない
  （`wire_payloads_cannot_assert_authority_or_identity`）。
- Given `ApproveToApply` grant を持つ delegated caller, When apply
  を呼ぶ, Then silent success ではなく `GatewayOutcome::
  ApprovalRequired` が返る
  （`delegated_approval_policy_is_not_bypassed`）。

## Guarantees

- 外部 crate・wire・CLI 入力から `CallerIdentity` を偽造できない
  （`AdapterSeal` が crate-internal mint のまま）。
- unattended/agent 実行が `Human` の無条件許可経路に到達しない。
- delegated 実行の権限は session ではなく per-call の
  task/grant/scope/expiry/approval/budget 評価で決まる。
- credential 検証失敗は session の存在を漏らさない blind verdict。
- headless composition は既存の単一 commit fence・journal・
  recovery 契約を再利用し、第二の権限源を作らない。

## Non-goals

- 実 OS session provenance（Windows logon/macOS/Linux console
  session）の検証 — 後続スライス。
- durable `CliSessionStore`（file/OS keychain 永続化）— 後続スライス。
- `synvelis-project` port adapter・実 project open/lock/reopen・
  実 2-process commit/crash-restart 検証 — 後続スライス。
- Script/Luau 実バイナリ・Workflow 実行経路（synvelis#272/#273）—
  後続スライス。
- `synvelis-runtime` 新設 crate — 本スライスでは作成しない。

## Risk

- `authority_class()` の `Human → VerifiedHuman` 写像は「Human
  principal を mint できるのは信頼 issuer のみ」の不変条件に依存
  する。将来 seal issuer を追加する変更は class 写像と
  provenance 契約を再監査しないと Human/Delegated の区別を
  静かに崩しうる。
- `MemoryCliSessionStore` は process restart で session を失う。
  durable store 実装前に production 経路へ配線してはならない
  （session 消失は fail-closed だが可用性を損なう）。
- `CliSessionClaim` の serde 表現は in-process 契約であり、
  将来 transport をまたぐ場合は別途 wire 契約のレビューが必要。
