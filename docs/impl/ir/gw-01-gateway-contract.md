---
id: "IMPL-0Gw01GateWayContract01"
type: "impl"
status: "completed"
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
      - type: "file"
        path: "docs/designs/gw-01-gateway-contract-design.md"
      - type: "file"
        path: "docs/adr/gw-01-gateway-contract-crate-boundary.md"
      - type: "file"
        path: "docs/impl/ir/gw-01-gateway-contract.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/designs/midi-source-note-editing-design.md"
      - type: "file"
        path: "docs/impl/ir/midi-source-note-editing.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/gw-01-gateway-contract-design.md"
  implemented-by: []
  depends-on:
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
    - "https://github.com/scarecrow173/synvelis/issues/52"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  supersedes: []
  superseded-by: []
  related:
    - "https://github.com/scarecrow173/synvelis/issues/44"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/53"
  derived-by: []
  verifies:
    - "docs/designs/gw-01-gateway-contract-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# GW-01 Gateway Contract 実装記録

Issue #53。`#45` の GTC-001/002/007/008 に基づく型付き内部 Gateway 契約を、
新規 workspace member `crates/synvelis-gateway` として実装した。GW-02〜10 が
積み上げる契約境界であり、認可判断・Project 正本・Transport/Job 実行・
MCP transport・UI は所有しない(非対象は §残課題)。

## 実施した変更

| 範囲 | 内容 |
| --- | --- |
| `Cargo.toml` / `Cargo.lock` | workspace member に `crates/synvelis-gateway` を追加(依存は `synvelis-project` + `serde`/`serde_json`/`thiserror`) |
| `id.rs` | `RequestId`/`OperationId`(u128 tagged newtype、`to_tagged()`/`parse`)、`TaskRef`/`GrantRef`/`ProposalRef`/`ApprovalRef`/`SnapshotRef`/`IdempotencyKey`/`AdapterId`(bounded ctor、空・chars 長超過は `MalformedRequest`) |
| `capability.rs` | `CapabilityId`(lowercase `[a-z0-9._-]`)、`CapabilityVersion`、`CapabilityRef`、`OperationDomain`(Query/ProjectMutation/RuntimeControl/AsyncOperation)、`TargetContract`(None/Resource/ResourceSet/Create — `shape` discriminator)、`EffectSet`/`PermissionSet`/`ExecutionContract`/`Availability`/`CapabilityDescriptor` |
| `schema.rs` | `SchemaVersion`、`SchemaRef`(+ `is_compatible_with` — 同名・同 major・要求 minor ≤ 宣言 minor)、`ValueSchema`(Bool/Integer/Number/String/Id/Enum/Array/Object/Optional/TimeRange — 値検証は `SchemaViolation`/`WrongResourceKind`/`WrongTimeDomain`/`InvalidId`)、`SchemaRegistry`(同一 (name,version) 再登録は conflict) |
| `wire.rs` | `WireRequest`/`WireTarget`/`WireConsistency`/`WireRange` — serde wire 型。`parse` は先に authority key(`principal`/`caller`/`authority`/`auth`/`token`/`permissions`/`approved`/`impersonate`/`as_user`/`role`)を走査して `SelfDeclaredAuthority`、implicit targeting key(`selection`/`focus`/`active_window`/`current_selection`)は `ImplicitTargetRejected`。top-level は `deny_unknown_fields`、さらに `capability`/`schema`/`target`/`consistency` 入れ子 object の未知 key も明示 scan で `MalformedRequest`(internally-tagged enum では `deny_unknown_fields` が効かない serde#2123 対策) |
| `request.rs` | `OperationRequest`(Deserialize 不実装 + 全 field `pub(crate)` — crate 外は構築・書換不能で read-only getter のみ。`bind()` 検証通過値だけが `CapabilityOperation` に届く)、`OperationTarget`/`ResourceRef`/`ResourceKind`(全 13 kind + `parse`/`expect` で kind/tag 二重検査、ResourceSet は 1024 ids 上限)、`Consistency`(domain 別 5 variant — `ExpectedRevision`≠`ReadAt`)、`TimeDomain`/`TypedRange`、`CallerIdentity`/`Principal`/`AdapterSeal`(private field の capability token、`Principal::new` は 256 chars 上限) |
| `authorization.rs` | `AuthorizationStage`(8 stage)、`AuthorizationDecision`(PermitForStage/ApprovalRequired/Deny)、`PolicyEvaluator` trait、`DenyAllPolicy`(fail-closed 既定) |
| `outcome.rs` | `GatewayOutcome<T>`(Completed/ProposalReady/ApprovalRequired/Accepted — `status` tag + named-field、scalar payload も serialize 可能)、`GatewayError`(全型付き variant、`category()`/`retry()` を variant から導出、`Denied{stage,reason}`/`SchemaConflict`/`IncoherentDescriptor`/`TargetShapeMismatch` 含む)、`ErrorEnvelope`(flatten した code+category+retry+message+`diagnostic` — `to_external()` で内部診断を除去した公開投影) |
| `registry.rs` | `CapabilityRegistry`(register/describe/list/versions — `resolve`/`Registration.handler` は `pub(crate)` で executor 到達経路を `invoke` に限定、重複登録は `CapabilityConflict`)、`CapabilityOperation` trait(Send+Sync の実行 seam) |
| `gateway.rs` | `Gateway` — `parse_wire`→`bind`→`invoke` の型付き経路。bind 検証 9 段(capability→version→availability→schema→project→target→consistency→args→採番)。`register_capability` は宣言 schema 解決可能性 + Query≠mutating-effect の coherence 検査。`invoke` は availability 再確認・descriptor の domain→stage policy 評価(`Deny` は reason 保持)・handler 実行・outcome の `ExecutionContract`/output-schema 適合検査までを担保。in-memory `IdempotencyLedger`((principal kind+id+adapter)+project+key、fingerprint は task/grant 含む、上限 4096・record-after-success — deny/未実行で key を燃やさない) |
| `fixture.rs` | `FixtureAdapter`(唯一の `AdapterSeal` 発行元、origin は `TestFixture` に pin)、4 domain fixture capability+input/output schema(add-note.input は 1.0/1.1/2.0 で互換判定を検証)、`EchoHandler`/`AcceptHandler`/`PermitSetPolicy`、`install()` |
| `tests/contract.rs` | 40 テスト — GT-001/002/003/010、unknown cap/version/schema/kind/id/time-domain、implicit targeting/authority key/入れ子未知 field 拒否、consistency domain 規則、create/resource-set target、idempotency(重複・衝突・scope・deny 後の key 再利用)、outcome 全 variant serialize/envelope 契約、availability/duplicate registration、incoherent descriptor・Sync≠Accepted・output-schema 違反・principal bound |
| `tests/fixtures.rs` | 8 テスト — 4 domain roundtrip、`ProjectController` への executor seam(ExpectedRevision→`Command.expected_revision`→commit、stale→`RevisionConflict`)、resource-set kind 検査 |
| `tests/schema_compat.rs` | 7 テスト — SchemaRef 互換規則、SchemaRegistry 解決、ValueSchema 全 variant |
| `docs/` | 設計書 + ADR(crate 境界決定)。併せて sv#52 文書 2 件の malformed `defers` front-matter を `[]` へ正規化(artifact-graph blocker 解消、本文の defer 記述は温存) |

## 受入条件との対応

| AC | 対応 |
| --- | --- |
| 未知 capability/version/schema・kind・time domain を typed error で拒否 | bind 検証 1–6 段が `UnknownCapability`/`UnsupportedCapabilityVersion`/`UnknownSchema`/`UnsupportedSchemaVersion`/`IncompatibleSchema`/`WrongResourceKind`/`UnknownResourceKind`/`InvalidId`/`WrongTimeDomain`/`TargetShapeMismatch` を返す。推測・暗黙変換なし |
| `ProjectMutation` は explicit expected revision 必須、Query snapshot/latest 区別維持 | `Consistency` variant 分離 + domain 規則。mutation の non-`ExpectedRevision` は `ConsistencyNotAllowed`、query の `ExpectedRevision` も同様に拒否(同値でも variant が違えば混同しない) |
| discovery が permission を含意しない、capability 追加で既存 grant が拡大しない | `describe` は metadata のみ。既定 evaluator は `DenyAllPolicy`(fail-closed)。`PermitSetPolicy` の set は登録内容に依存せず、後から登録した capability の必須 permission で引き伸ばされないことをテストで証明 |
| Context Query・MIDI Mutation・Transport・Job の typed fixture | `fixture_descriptors()` 5 件で 4 domain を網羅、`Echo`/`Accept` mock handler 経由で production operation なしに invocation を検証 |
| GT-001/002/003/010 相当の contract/schema 互換 test + 実行証拠 | `tests/contract.rs`(GT-001 explicit target・GT-002 adapter 由来 caller・GT-003 discovery≠permission・GT-010 bind≠execute)、`tests/schema_compat.rs`、下記検証コマンド |

## 実行した検証(証拠)

- 環境: Windows、rust 1.99.0(mise pin)、base revision `a4937eb` + 本変更
- `mise run fmt-check` — Pass
- `mise run check` — Pass(workspace 全 target)
- `mise run lint`(`cargo clippy --workspace --all-targets -- -D warnings`) — Pass
- `mise run test`(`cargo test --workspace --locked`) — Pass、workspace 全 41 suite 緑(gateway 新規 55 テスト: contract 40 + fixtures 8 + schema_compat 7)

## Review-protocol 実施結果

独立 reviewer 2 系(cooperative / adversarial、adversarial 側は gate 未実行の
ため code review のみ)を並行適用し、blocking 指摘を全て修正した上で再検証
(§実行した検証は修正後の結果)。

| 指摘 | 重大度 | 対応 |
| --- | --- | --- |
| `OperationRequest`/`CallerIdentity`/`Principal` が public field で外部から構築・書換可能(bind bypass・caller 偽造) | P0 | 全 field を `pub(crate)` 化 + read-only getter 追加。`Principal::new` に 256 chars 上限、`CallerIdentity::new` は `AdapterSeal` 必須 |
| `CapabilityRegistry::resolve`/`Registration.handler` が public で evaluator を bypass して handler を直接実行可能 | P0 | `pub(crate)` 化。discovery(`list`/`describe`)は metadata のみ、handler 到達は authorization-gated `invoke` のみ |
| `FixtureAdapter::new` が任意 `OriginKind` を受け fixture caller を trusted session に見せかけ可能 | P1 | origin を `OriginKind::TestFixture` に pin |
| `invoke` が bind 時点の availability/domain を信用し、`Deny` の reason を捨てていた | P1 | invoke で availability 再確認、stage は登録 descriptor の domain から導出、`Denied{stage, reason}` に evaluator の理由を保持 |
| `GatewayOutcome` の internally-tagged repr は scalar/newtype payload を serialize 不能 | P0 | 全 variant を named-field + `status` tag へ変更、全 variant serialize テスト追加 |
| idempotency ledger が bind 時点で key を記録(未実行・deny で key 焼失、`original` が実行されなかった operation を指す)+ fingerprint に task/grant 不含 | P1 | check/record 分離で invoke 側へ移動、handler 成功後のみ記録。ledger key を (principal kind+id+adapter)+project 化、上限 4096・古い順 eviction |
| `ExecutionContract`/`output_schema` が宣言のみで検査されない | P1 | `check_outcome` 追加 — Sync≠`Accepted`・`Completed` result は output schema 適合を境界送出前に検査、違反は `Internal` |
| 入れ子 object(capability/schema/target/consistency)の未知 key が黙って捨てられる(`deny_unknown_fields` が internally-tagged enum で不効 — serde#2123) | P2 | wire parse で明示 scan、`MalformedRequest` で拒否 |
| envelope `code` の二重入れ子、schema 再登録が `CapabilityConflict` に mislabel、`Denied` に safe reason 不在 | P2 | `code` flatten、`SchemaConflict` 追加、`Denied{stage,reason}` |
| その他系統: stable message の `Debug` 依存・char/byte 長不一致・version 型の `deny_unknown_fields` 欠落・ResourceSet 無制限 | P2 | `.name()` helper(kebab-case)導入、chars カウント、`deny_unknown_fields` 付与、1024 ids 上限 |
| 非 blocking(P2/P3): async consistency model・discovery の多段 stage・typed-args 投影・query 用 `OperationRef` 命名 | 報告のみ | GW-02〜08 の設計論点として残課題に記録、GW-01 の契約形状は維持 |

## 未実施の検証

- `mise run test-msrv`(rust 1.89 toolchain 未導入のため未実行)
- GW-05/06 lifecycle・GW-08 transport との結合 — Issue スコープ外

## 残課題

- `ProposalReady`/`ApprovalRequired`/`Accepted` は GW-01 では outcome の
  **分岐点**のみ — Proposal/Approval/Apply lifecycle の遷移・receipt・
  durable ledger は GW-05/06 で実装(`ApprovalRef` は deterministic
  placeholder `approval-for-<operation>`)。
- `PolicyEvaluator` の実装(Grant store・scope 評価)は GW-02。
- `IdempotencyLedger` は in-memory の契約検証用 — 永続 Record/Retry 契約は
  GW-06(ACG-004)に defer。
- `OperationRequest` は現状 synchronous bind のみ — GW-08 adapter は
  `parse_wire`/`bind`/`invoke` を呼び分ける責務を持つ。
- Review で挙がった非 blocking 論点(GW-02〜08 で検討): AsyncOperation の
  consistency 意味付け、discovery を複数 stage に分けるか、`args` の
  typed 投影(executor 向けの deserialize 補助)、`Accepted` の operation
  参照命名と read-result 参照の関係。
