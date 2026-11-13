---
id: "DESIGN-034X8sAionwOcsXpKgR0ND"
type: "design"
status: "approved"
title: "Capability / Command Gateway — GW-01〜10 型付き契約・State Transition・Failure Matrix 設計契約 (synvelis#45)"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  changes:
    added:
      - type: "file"
        path: "docs/specs/command-gateway.md"
      - type: "file"
        path: "docs/designs/command-gateway-design.md"
      - type: "file"
        path: "docs/plans/command-gateway.md"
      - type: "file"
        path: "docs/tasks/command-gateway-task.md"
      - type: "file"
        path: "docs/test-specs/command-gateway.md"
      - type: "file"
        path: "docs/impl/ir/command-gateway.md"
      - type: "file"
        path: "docs/impl/exp/command-gateway.md"
      - type: "file"
        path: "evidence/sv45/command-gateway-case-ledger.md"
      - type: "file"
        path: "evidence/sv45/command-gateway-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/specs/command-gateway.md"
  implemented-by: []
  depends-on:
    - "docs/specs/command-gateway.md"
    - "docs/designs/project-persistence-design.md"
    - "docs/designs/playback-graph-design.md"
    - "docs/designs/transport-integration-design.md"
    - "docs/designs/media-render-design.md"
    - "docs/designs/target-architecture-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
    - "docs/designs/midi-source-note-editing-design.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/command-gateway.md"
    - "https://github.com/scarecrow173/synvelis/issues/45"
  derived-by:
    - "docs/plans/command-gateway.md"
    - "docs/test-specs/command-gateway.md"
  verifies: []
  verified-by: []
  references:
    - "docs/specs/command-gateway.md"
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
    - "docs/designs/midi-source-note-editing-design.md"
    - "evidence/sv45/command-gateway-case-ledger.md"
  defers: []
  deferred-by: []
---

# Capability / Command Gateway — GW-01〜10 設計契約 (synvelis#45)

本書は `docs/specs/command-gateway.md` の設計層。**文書のみ**
であり、実装は #46 配下の各実装 Issue が Owner。設計合意・
headless/mock fixture・real transport E2E・製品受入・実機/
GUI/Provider 受入を区別する。

## Context

上流正本は Issue #45 本文の v0.3〜v0.8（提案→GW-06→GW-01〜05
→GW-07〜09→GW-10→最終整合性監査）と #44 の AIB/AHG 不変条件、
#46 の実装計画・G0〜G6。`gw-01-gateway-contract-design.md`・
`gw-02-authorization-policy-design.md`（draft・#53/#54 実装
記録あり）と `midi-source-note-editing-design.md`（#52/#56）・
`project-persistence-design.md`（#17）・`playback-graph-design.md`
（#18）・`transport-integration-design.md`（#20）・
`media-render-design.md`（#21）・`target-architecture-design.md`
の各確定契約を本契約の一部として扱う。

現行実装の実体（main・本 slice で変更しない）:

- `crates/synvelis-gateway/src/`: `capability.rs`・`registry.rs`・
  `request.rs`・`wire.rs`・`schema.rs`・`id.rs`（GW-01）・
  `authorization.rs`・`grant.rs`・`policy.rs`・`durable_policy.rs`
  （GW-02）・`context.rs`・`context_engine.rs`・`snapshot.rs`
  （GW-03）・`prepare.rs`・`prepare_engine.rs`・
  `prepare_policy.rs`（GW-04）・`proposal.rs`・
  `proposal_engine.rs`・`proposal_policy.rs`（GW-05）・
  `commit.rs`・`journal.rs`・`store.rs`（GW-06）・`job.rs`・
  `job_engine.rs`・`media.rs`・`provenance.rs`（GW-07）・
  `mcp.rs`・`mcp_permit.rs`・`mcp_server.rs`・`mcp_tools.rs`
  （GW-08）・`gateway.rs`・`outcome.rs`・`evidence.rs`・
  `oracle.rs`・`schedule.rs`・`trace.rs`・`clock.rs`・
  `fixture.rs`・`stub.rs`（横断/harness）・`bin/gw10_report.rs`
- `crates/synvelis-gateway/tests/`: `contract.rs`・
  `authorization.rs`・`context.rs`・`prepare.rs`・
  `prepare_regressions.rs`・`proposal.rs`・`proposal_
  regressions.rs`・`commit_gate.rs`・`job.rs`・`mcp_adapter.rs`・
  `schema_compat.rs`・`ctl_01.rs`・`fixtures.rs`・`gw10_contract.rs`・
  `gw10_authorization.rs`・`gw10_concurrency.rs`・
  `gw10_persistence.rs`・`gw10_project.rs`・`gw10_mcp.rs`・
  `gw10_job.rs`・`gw10_system.rs`・`gw10_harness.rs`・`rig/`・
  `common/`
- `crates/mcp-poc/tests/`: `gw10_transport.rs`・`stdio_e2e.rs`
  （real transport E2E）
- `crates/synvelis-workspace/src/gateway_ui.rs`: GW-09 headless
  UX 境界
- `crates/synvelis-project/src/controller.rs`: ProjectController
  Single Writer（GW-06 の Commit 先）

## Scope

In: GW-R-01〜15 全部、GT-001〜012・JM-01〜05・MCP-01〜06・
UX-01〜06・SEC-01〜14・DATA-01〜10・REC-01〜12・JOB-01〜10・
SYS-01〜10・AC-01〜10・E2E-1/2/3・G0〜G6・GTV-001〜008 の
写像、GW-01〜10 Owner 表、状態機械、Failure Matrix、AUTX/
CLI/adapter 境界、RT boundary。Out: 実装コード、GUI 実装、
実 device/plugin/OS/Provider 受入、AUTX 文書編集、sv#44 文書
編集、Caller 自己申告の権威昇格。

## Components and Responsibilities

### Owner 分離（GW-01〜10 固定表）

| GW | 責務 | 所有する契約 | 所有しない契約 | 実装 Issue | 実体 |
|---|---|---|---|---|---|
| GW-01 | Capability Registry・Typed Invocation | CapabilityDescriptor・OperationRequest・Result/Error | Project 正本・認可判断 | #53 | `capability.rs`/`registry.rs`/`request.rs`/`wire.rs`/`schema.rs`/`id.rs` |
| GW-02 | Authorization Policy | Principal/Grant/Activation/Approval Policy 評価・Decision | Semantic Command の変更意味 | #54 | `authorization.rs`/`grant.rs`/`policy.rs`/`durable_policy.rs` |
| GW-03 | Context Projection | 権限付き Immutable Snapshot Projection・Read/Disclosure 制御 | Project Mutation・UI Selection 正本 | #55 | `context.rs`/`context_engine.rs`/`snapshot.rs` |
| GW-04 | Semantic Preparation | 正規化・Preconditions・実 Effect 導出 | Apply 許可・Project Commit | #56 | `prepare.rs`/`prepare_engine.rs`/`prepare_policy.rs` |
| GW-05 | Proposal/Approval | 不変 Prepared ChangeSet・Semantic Diff Preview・Receipt | Project 正本・Undo/Transaction | #57 | `proposal.rs`/`proposal_engine.rs`/`proposal_policy.rs` |
| GW-06 | Authorized Commit Gate | 最終認可→ProjectController Single Writer・CAS/Revision・Journal・Idempotency/Recovery | 第二 Writer/Store | #58 | `commit.rs`/`journal.rs`/`store.rs`+`synvelis-project` |
| GW-07 | Job/Media/Provenance | Job Admission・結果/送信権限・Operation↔Job/Media/Provenance 参照 | 第二 Job Executor・第二 Media Store | （#21 接続・#62 系） | `job.rs`/`job_engine.rs`/`media.rs`/`provenance.rs` |
| GW-08 | MCP Adapter | Tool/Resource 公開・Protocol 変換・Caller Binding・Bounded Transfer | ProjectController/Grant/Approval/Job 正本 | #59 | `mcp*.rs`+`crates/mcp-poc` |
| GW-09 | Permission/Approval UX | 接続許可/Runtime/Task/Grant/Approval Policy の独立表示契約 | 権限の正本（GW-02/05） | （#114 系） | `synvelis-workspace/src/gateway_ui.rs`（headless） |
| GW-10 | Integration Verification | Evidence Schema・独立 Oracle・Fault Schedule・G0〜G6/GTV | 第二の実行正本 | #62・#83 | `evidence.rs`/`oracle.rs`/`schedule.rs`/`trace.rs`/`bin/gw10_report.rs`+`gw10_*` tests |

原則: Gateway は Capability 宣言であり Authorization ではない。
Caller Principal・Execution Grant・Permission UX は Host 側正本。
Gateway は既存 Project Single Writer を通すだけで第二の
Project/Undo/Transaction/Job/Media/History 正本を作らない。
API に外部 SDK/Transport 型を漏らさない。RT work は
Gateway/MCP adapter 層で禁止。

### 要件写像（spec GW-R-01〜15 → 本設計）

| GW-R | spec 要件 | 本設計の対応 |
|---|---|---|
| GW-R-01 | Capability Registry & Typed Invocation | GW-01・Contract 1 |
| GW-R-02 | Authorization | GW-02・Contract 2 |
| GW-R-03 | Context Projection & Disclosure | GW-03・Contract 3 |
| GW-R-04 | Semantic Preparation & Effects | GW-04・Contract 4 |
| GW-R-05 | Prepared ChangeSet & Approval | GW-05・Contract 5 |
| GW-R-06 | Separated State Machines | State Machines 節 |
| GW-R-07 | Typed Results & GW-06 Boundary | GW-06・Contract 6 |
| GW-R-08 | Authorized Commit Gate & Recovery | GW-06・Contract 6・Failure taxonomy REC |
| GW-R-09 | Job/Media/Provenance Adapter | GW-07・Contract 7・Data/Control Flow |
| GW-R-10 | MCP Adapter & Trusted Binding | GW-08・Contract 8・MCP Adapter 節 |
| GW-R-11 | Permission/Approval UX Contract | GW-09・Contract 9・UX 写像 |
| GW-R-12 | Verification Harness & Evidence | GW-10・Contract 10・Gate 写像 |
| GW-R-13 | Boundary Non-duplication | Owner 表原則・Deferred Design Concerns |
| GW-R-14 | RT Boundary & Consumer Regression | Risks・Deferred・SYS-07/CONSUMER 写像 |
| GW-R-15 | Evidence Honesty | Verification notes・`evidence/sv45/` |

### 主要 Contract（GTC/ACG/GIA の設計対応）

1. **GW-01 Typed Contract**（GTC-001/002/008）:
   `CapabilityDescriptor`（Stable CapabilityId・Version・
   OperationDomain・Input/Output Schema・Target Contract・
   Required Permissions・Possible Effects 上限・Execution
   Contract/Availability）、`OperationRequest`（RequestId・
   CapabilityRef・ProjectId・Explicit Typed Target・Arguments・
   Consistency・Idempotency Key）。Wire 型に caller/authority
   field を持たず、`CallerIdentity` は adapter 発行の内部
   `AdapterSeal` を要求する。Create は Parent ID+Kind+Range。
   `GatewayOutcome` = `Completed(T)`/`ProposalReady`/
   `ApprovalRequired`/`Accepted(OperationRef)`。Error 分類は
   Authentication/Authorization/Approval/Consistency/
   Validation/Capability-Version/Effects/DuplicateRequest/
   Recovery-Indeterminate。実体: `contract.rs` の
   `gt001_*`/`gt002_*`/`gt003_*`/`gt010_*`・`idempotency_*`・
   `unknown_*`/`unsupported_*`/`schema_violation_*` test、
   `gw10_contract.rs` の `typed_ids_*`・`unknown_capability_*`・
   `schema_drift_*`。
2. **GW-02 Authorization**（GTC-003）: AuthorizationStage
   （DiscoverCapability/ReadContext/PrepareProposal/
   RequestApply/Commit/StartRuntime/StartAsync/ReadResult）と
   `PermitForStage`/`ApprovalRequired`/`Deny` の Decision。
   Fixed Exact/Snapshot Scope と Constrained Dynamic Scope
   （明示親・Kind・時間範囲・上限）を区別し暗黙の親子権限
   継承をしない。Apply mode は Grant の Approval Policy から
   導出し request 引数で昇格しない。実体: `authorization.rs`
   の `ahg007_*`/`ahg009_*`/`ahg010_*`/`acg003_*`・
   `data_access_*`・`disclosure_*` test、`gw10_authorization.rs`
   の `sec01`〜`sec14`・`data01`〜`data10`。
3. **GW-03 Context Projection**（GTC-004）: ContextQuery
   （Project・Explicit Resource Selector・Field Selection・
   Consistency・Page Request）→ ContextProjection（Project/
   Revision・許可済み Data・Completeness/Unknown/Unavailable・
   Next Page Cursor）。既存 #7/#17 の Immutable Snapshot を
   再利用し、ページングは固定 Snapshot に紐付き Revision を
   混在させない。Selection は named view+class を要求し
   Grant/Command 対象を変更しない。実体: `context.rs` の
   `pagination_*`/`cursor_*`/`pin_*`/`selector_*`/`selection_*`/
   `expired_*`/`revoked_*`/`foreign_*`/`metadata_only_*`/
   `media_group_*`/`derived_group_*`/`external_envelope_*`/
   `range_*`/`structured_content_*` test、`gw10_authorization.rs`
   の `gt004_*`・`data03`。
4. **GW-04 Semantic Preparation**（GTC-005）: PrepareRequest
   （Project ID・Base Revision・Typed Semantic Operation）→
   PreparedOperation（正規化 Commands・Preconditions・導出
   Effects・非破壊候補 Snapshot 参照）。scratch clone 上で
   評価し live mutation しない。Linked Source 書込は Source
   scope を要求し、Independent Copy/新 Source は別明示
   Command。Time domain（Project Musical/Media/Source
   Native）と Range を厳密に区別。実体: `prepare.rs` の
   `prepare_derives_effects_and_mutates_nothing`・
   `batch_*`・`linked_*`・`detach_*`・`creation_requires_*`・
   `wrong_time_domain_*`・`unverifiable_effects_*`・
   `invalid_*`/`absent_*` test、`prepare_regressions.rs`・
   `gw10_project.rs` の `gt005_*`/`gt006_*`/`gw10c_*`。
5. **GW-05 Proposal/Approval**（GTC-006/007）: Prepared
   ChangeSet は不変で ProposalId・ProjectId・Base Revision・
   Versioned Commands・Preconditions・Evaluated Effects・
   Dependency Fingerprint・Schema/Canonicalization Version・
   Content Digest・Expiry を持つ。Digest は意味的 Identity
   （Project/Revision・Commands/対象・時間範囲・Preconditions・
   Effects・依存）に結び付き cosmetic 要素を除外する。
   Approval Receipt は Synvelis 発行で Principal/Project/
   Grant/Task/Proposal+Digest/Expected Revision/Expiry に
   束縛。Proposal/Approval/Apply の状態機械を分離し、
   `changeset_verify_detects_tampering`・`semantic_drift_
   changes_the_digest`・`identical_prepares_digest_
   identically`・`receipt_binding_rejects_*`・`double_
   consumption_can_never_succeed`・`expired_*`/`revoked_*`/
   `declined_*`/`invalidated_*` test が実体。`proposal_
   regressions.rs` の lifecycle/settle/consume 系も実体。
6. **GW-06 Authorized Commit Gate**（ACG-001〜005）: Commit
   確定境界で最新 Grant/Activation/Approval/Policy Epoch・
   Scope/Effects/Limit・Expected Revision を Project
   Controller の Commit と整合評価。Revoke 先→Commit 拒否、
   Commit 先→Mutation 有効。Keyed retry は Receipt を replay
   し Payload 相違は Conflict。crash 各点で Committed または
   Indeterminate に回復し torn state を残さない。File Journal
   + durable policy で restart を越える。実体: `commit_gate.rs`
   の `revoke_*`/`commit_*`/`concurrent_*`/`crash_*`/`keyed_*`/
   `durable_*`/`stale_*`/`torn_*`/`saves_*`/`wire_level_*`/
   `manual_*`/`expired_*`/`apply_awaiting_*`/`approve_to_apply_*`
   test、`gw10_concurrency.rs` の `rec01`〜`rec06`・
   `gtv005_*`・`ac05_*`、`gw10_persistence.rs` の `rec07`〜
   `rec12`・`sys04_*`。
7. **GW-07 Job/Media/Provenance**（GIA-001〜003）: JobBook が
   RequestId/OperationId/JobId/ProposalId/AssetId/SourceId を
   分離し、既存 Job Owner へ接続。Job Execution/Media
   Lifecycle/Project Mutation/Runtime-Persistence を独立状態
   とし、Completed≠Verified≠Committed≠Applied≠Saved。
   Disconnect/Window Close は Job を暗黙 cancel せず、Grant
   Revoke は新規保護操作を拒否し Cancel を要求するが
   Cancel Requested/Cancelled/Unconfirmed を区別。Verified
   Media は Staging→Verification→Verified→ExpectedRevision
   Project Commit を経て明示 Proposal→GW-06 へ接続し自動
   配置しない。Provenance は Model/Rights を Confirmed/
   Declared/Unknown で記録し秘密を埋め込まない。実体:
   `job.rs` の `ids_stay_distinct_*`/`disconnect`/`revocation_*`/
   `verified_media_*`/`provenance_*`/`cancel_*`/`expired_*`/
   `submit_*`/`unknown_*`/`foreign_*`/`result_disclosure_*`/
   `input_snapshot_*`/`progress_*`/`repeated_*`/`batch_*`/
   `failed_*`/`register_*`/`job_book_*`/`pending_*` test、
   `gw10_job.rs` の `job02`〜`job10`・`gtv004_*`・
   `gw10g_*`、`media.rs`/`provenance.rs`。
8. **GW-08 MCP Adapter**（GIA-004）: MCP Server は Tool/
   Resource 公開・Protocol/Schema/Result/Error 変換・Caller
   Binding・Bounded Transfer の Adapter。信頼済み境界で
   Principal を決定し（permit record・session binding・
   channel kind・secret hash）、wire の client claim を
   信用しない。Tool allowlist 外 Capability・未知
   Tool/Schema/Version/Typed ID を拒否し、Discovery は権限の
   証明でなく Invocation ごとに GW-02 へ再照会。3 Mode を
   実 transport（stdio helper + 認証付き loopback control
   channel）で E2E 実行し、Disconnect/Retry は冪等キーと
   再認可で二重 Commit を防ぐ。大容量 Media は有界
   Range/Chunk で Read/Disclosure/Budget を検証。実体:
   `mcp_adapter.rs` の `connect_*`/`issue_*`/`catalog_*`/
   `discovery_*`/`rejects_*`/`wire_authority_*`/`suggest_*`/
   `delegated_*`/`approve_to_apply_*`/`revoked_*`/`settle_*`/
   `commit_failure_*`/`operation_handles_*`/`file_store_*`/
   `disabled_*`/`malformed_*`/`notifications_*`/
   `unsupported_*`/`tampered_*`/`control_*`/`issuance_*` test、
   `gw10_mcp.rs` の `gw10f_*`、`mcp-poc/tests/gw10_transport.rs`
   の `gw10f_*_real` 4 test・`stdio_e2e.rs`。
9. **GW-09 Permission/Approval UX**（GIA-005）: 接続許可/
   AI Runtime/Authorized Task/Execution Grant/Approval Policy
   を独立設定・独立状態として表示（既定 OFF）。固定/動的
   Scope・対象・Limit・期間を確認可能にし、Approve-to-Apply
   は Proposal/Digest/Expected Revision/Principal/影響範囲を
   提示。Revoke Requested/Confirmed・Job Cancel
   Requested/Cancelled・Proposal/Committed/Applied/Saved を
   区別表示し第二の Project History を作らない。実体:
   `synvelis-workspace/src/gateway_ui.rs`（headless 契約・
   state fixtures）、`gw10_system.rs` の `sys02_*`/`sys03_*`・
   `sys05_sys06_*`・`e2e1_*`。実 GPUI 承認 UI は #114 が
   Owner で本 slice では NotRun。
10. **GW-10 Verification Harness**（GTV-001〜008）: 5 責務
    （Contract/Security/Concurrency-Recovery/DAW Non-
    regression/Cross-consumer）を GW10-A〜H の論理単位で
    検証。5 層 Test・独立 Oracle（closed-world・linearization
    naming）・Controlled Barrier（scheduled fence）・Fault
    Injection（named fault points）・EvidenceRecord（Test
    ID→Requirements→Owner→Gate→Layer→Fixture→Revision→
    Platform→Backend→Transport→mode→fault_point→Expected/
    Actual→Verdict→Trace）・JSONL sink・`gw10-report` gate
    aggregation。`Pass/Fail/Blocked/NotRun/Unsupported` を
    分離し `required=false`/`real_only` row で mock Pass の
    実環境昇格を防ぐ。実体: `evidence.rs`/`oracle.rs`/
    `schedule.rs`/`trace.rs`/`fixture.rs`・`gw10_harness.rs`
    の `matrix_*`/`oracle_*`/`causal_*`/`crash_hook_*`/
    `evidence_records_*`/`scheduled_fence_*`・`bin/
    gw10_report.rs`。

## Data and Control Flow

### Invocation → Commit（型付き経路）

```
Adapter (MCP/CLI/GPUI/Internal)          Host Authority
  │ AdapterSeal 付き CallerIdentity         Principal/Task/Grant
  ▼                                          ▼
GW-01 Registry: CapabilityRef+Version+Schema+Typed Target+Args
  → GW-02 AuthorizationStage 評価 (Discover/Read/Prepare/Apply/Commit/...)
  → GW-03 ContextProjection (pinned snapshot・field/page・disclosure class)
  → GW-04 PrepareOperation (scratch clone・normalized commands・derived effects)
  → GW-05 PreparedChangeSet (immutable・digest) + ApprovalReceipt (Synvelis発行)
  → GW-06 CommitGate: digest+expected revision+effects+grant/policy+revocation 再評価
  → ProjectController Single Writer (CAS/Revision/Undo/Journal)
  → CommitReceipt / Outcome (Completed/ProposalReady/ApprovalRequired/Accepted)
```

Prepare/Approve/Preview は Revision を変更しない（GT-010）。
Commit 確定のみが Revision を進め、Revoke との線形化は
Commit Gate 内で一意に決まる（REC-01/02）。

### 3 Mode（GIA-005）

- **Suggest**: Agent は Proposal/Preview まで。Apply は
  Synvelis UI の独立操作で Agent Apply 不可（SEC-08・
  `suggest_mode_denies_apply`・`gw10f_suggest_mode_denies_apply`）。
- **Approve-to-Apply**: Synvelis が特定 ChangeSet の Approval
  Receipt を発行し、Apply はその Receipt 消費でのみ成立
  （`approve_to_apply_*`・`receipt_binding_*`・`double_
  consumption_*`）。
- **Delegated Apply**: 有効 Grant の Scope/期限内で個別承認
  なし。budget は Commit 時に bind され使い切ると以降 deny
  （`ahg009_*`/`ahg010_*`・`delegated_apply_*`）。

### Job/Media/Provenance（GIA-001〜003）

```
GW-07 JobBook (RequestId/OperationId/JobId/ProposalId/AssetId/SourceId 分離)
  → Job Admission (input snapshot pin・scope・transfer class・budget)
  → 既存 Job Owner 実行 (Window/Disconnect 非依存)
  → Media Staging→Verification→Verified (≠Project Commit)
  → 明示 Proposal→GW-04/05→GW-06 Commit (自動配置しない)
  → Provenance (Model/Rights = Confirmed/Declared/Unknown)
```

Job Completed・Media Verified・Project Committed・Engine
Applied・Saved は別状態・別 Receipt（GTV-002・`gtv002_*`・
`sys05_sys06_*`）。

### MCP Adapter（GIA-004）

```
External MCP Host ──stdio──> Local Helper ──auth IPC──> Gateway
  permit record (channel kind・secret hash・session binding)
  → discovery (bound session のみ・catalog は channel scoped)
  → tools/call (allowlist・typed id・schema version)
  → wire authority は permit record から導出 (client claim 非信用)
  → GW-01〜06 同一契約 → bounded media transfer (range/chunk/budget)
  → operation handle (scoped・blind・restart reconcile)
```

## State Machines（GTC-007 分離）

### Proposal Lifecycle

`Submitted → Preparing/Validating → Prepared`（Preview 可能）、
または `Failed / Invalidated / Expired`。`Prepared` は Commit
済み・最新 Project での適用可を意味しない。Base Revision ずれは
Conflict で reject し暗黙 Rebase しない（`expected_revision_
mismatch_is_a_conflict`・`revision_conflict_rejects_apply`・
`stale_proposal_*`・`expired_proposal_*`）。

### Approval Lifecycle

`NotRequired`、または `Requested → Granted / Declined / Expired
/ Revoked / Invalidated`、`Granted → Consumed`（Commit 結果と
整合）。Grant 自体の失効は常に別途判定（`sec12_revoked_grant_
kills_the_unused_approval`・`expired_approval_reopens_as_fresh_
request`・`invalidated_proposal_kills_its_approval`・`rejection_
reopens_the_approval_lifecycle`）。

### Apply Lifecycle

`NotRequested → CommitRequested / AwaitingApproval → Committed
(CommitRef) / Rejected / Conflict / Indeterminate`。Proposal
状態と Apply 結果は別。`settle_*`/`accepted_apply_*`/`waiting_
apply_*`/`live_apply_*`/`superseded_*`/`committed_apply_*` test
が実体。

### Query / Transport / Async

Proposal Lifecycle を必須としない。Query は Read/Disclosure
認可、Transport（RuntimeControl）は #20 の Request→Prepare→
RT Apply→Receipt、AsyncOperation は #21 の Job/Result を使用。
Operation 受付（`Accepted(OperationRef)`）と最終完了を混同
しない。

## Failure taxonomy（横断 — SEC/DATA/REC/JOB/SYS）

| Family | Case | 期待 | Fixture（test 関数） | Status |
|---|---|---|---|---|
| SEC | SEC-01 未認証 request | Authentication deny | `gw10_authorization.rs::sec01_unauthenticated_request_denies` | Pass(mock/L2) |
| SEC | SEC-02 wire authority 自己申告 | 拒否・adapter bind のみ | `sec02_wire_authority_fields_cannot_self_declare`・`contract.rs::gt002_*` | Pass(mock/L1-L2) |
| SEC | SEC-03 他 Principal の Grant 使用 | Authorization deny | `sec03_foreign_principal_cannot_spend_the_grant` | Pass(mock/L2) |
| SEC | SEC-04 他 Project 横断 | 存在を秘匿して deny | `sec04_foreign_project_boundary_denies`・`context.rs::foreign_*` | Pass(mock/L2) |
| SEC | SEC-05 共有 Source 書込 | Source scope 要求 | `sec05_shared_source_write_needs_source_scope`・`prepare.rs::linked_*` | Pass(mock/L2) |
| SEC | SEC-06 Dynamic Scope 脱出 | deny | `sec06_dynamic_scope_escape_denies` | Pass(mock/L2) |
| SEC | SEC-07 Selection 変更 | Grant 対象を retarget しない | `sec07_selection_changes_do_not_retarget_the_grant` | Pass(mock/L2) |
| SEC | SEC-08 Suggest mode Apply | deny・Revision 不変 | `sec08_suggest_mode_apply_denies_without_revision_change` | Pass(mock/L3) |
| SEC | SEC-09 ChangeSet tamper | Digest check で deny | `sec09_changeset_tamper_denies_at_digest_check`・`proposal.rs::changeset_verify_*` | Pass(mock/L2) |
| SEC | SEC-10 他人 Approval Receipt | deny | `sec10_foreign_approval_receipt_denies`・`proposal.rs::receipt_for_another_*` | Pass(mock/L2) |
| SEC | SEC-11 Approval 後 Revision drift | Conflict | `sec11_revision_drift_after_approval_conflicts` | Pass(mock/L2) |
| SEC | SEC-12 Grant revoke→未使用 Approval | Commit deny | `sec12_revoked_grant_kills_the_unused_approval` | Pass(mock/L2) |
| SEC | SEC-13 Grant 外 Capability | deny | `sec13_capability_outside_the_grant_denies`・`mcp_adapter.rs::rejects_capability_outside_*` | Pass(mock/L2) |
| SEC | SEC-14 未知 Effect | 自動許可しない | `sec14_unknown_effect_is_never_auto_permitted`・`prepare.rs::unverifiable_effects_*` | Pass(mock/L2) |
| DATA | DATA-01〜10 | Read/Disclosure 分離・pagination revision 固定・bounded・prompt injection 不変・URI 非開示・tool discovery 非拡張 | `gw10_authorization.rs::data01`〜`data10`（`data08`/`data09` は real/L4） | Pass(mock+real/L2-L4) |
| REC | REC-01〜06 | Revoke/Commit 両順序・CAS conflict・keyed retry replay/conflict | `gw10_concurrency.rs::rec01`〜`rec06`・`commit_gate.rs::revoke_*`/`commit_*`/`concurrent_*`/`keyed_*` | Pass(mock/L2) |
| REC | REC-07〜12 | crash 各点 recovery・torn tail・save HEAD 順序・stale proposal 不 commit | `gw10_persistence.rs::rec07`〜`rec12`・`commit_gate.rs::crash_*`/`durable_*`/`torn_*`/`saves_*`/`stale_*`/`restart_*` | Pass(mock/L2) |
| JOB | JOB-01〜10 | ID 分離・disconnect 非依存・revoke→cancel 要求・unverified 非登録・provenance unknown・cancel verdict 正直・drift conflict | `gw10_job.rs::job02`〜`job10`・`job.rs::*`（`job08` は real/L4） | Pass(mock+real/L2-L4) |
| SYS | SYS-01〜10 | AI-off human pipeline・grant 独立性・approval binding・expired proposal・state 非混同・RT boundary 記録・unsupported version・restart・他 | `gw10_system.rs::e2e1_*`/`sys02`〜`sys08`・`gw10_persistence.rs::sys04_*`（`sys09` は real/L4） | Pass(mock+real/L1-L4) |
| SYS | SYS-07-rt | RT 性能 budget 実測 | `sys07_rt_boundary_is_recorded_not_claimed`（記録のみ・budget 未実測） | **Unsupported**（required=false） |
| SYS | SYS-02-ux/SYS-03-ux | 実 GUI での grant 独立表示・approval binding 表示 | （未実装 — #114 GPUI） | **NotRun**（required=false） |
| SYS | SYS-05-powerloss | OS/電源断 durability | （harness 非対応 — `fault::POWER_LOSS` は常に Unsupported） | **NotRun/Unsupported**（required=false） |
| CONSUMER | CONSUMER-RUVIFORM/ AUDIOLENS | Ruviform 共通変更時の両 Consumer 回帰 | （本 slice で Ruviform 変更なし・rf#294/#553 は評価専用） | **NotRun**（required=false・条件未成立） |

詳細な case 別 fixture/oracle/証拠は
`evidence/sv45/command-gateway-case-ledger.md` を正本とする。

## GT / JM / MCP / UX / Gate 写像（概要）

| Family | Case | Design 被覆 | 実装状態 |
|---|---|---|---|
| GT | GT-001〜003 | GW-01 contract・typed target・adapter authority・stage-scoped permit | 実装済（headless・`contract.rs`/`gw10_contract.rs`） |
| GT | GT-004 | GW-03 pinned projection・disclosure class | 実装済（headless・`context.rs`/`gt004_*`） |
| GT | GT-005/006 | GW-04 effect 導出・prepare 非破壊 | 実装済（headless・`prepare*.rs`/`gt005_*`/`gt006_*`） |
| GT | GT-007/008 | GW-05 immutable changeset・digest・receipt binding | 実装済（headless・`proposal*.rs`/`gt007_*`/`gt008_*`） |
| GT | GT-009 | GW-06 revoke 後未使用 approval commit deny | 実装済（headless・`sec12_*`/`rec01_*`） |
| GT | GT-010 | GW-01〜05 revision 非変更 | 実装済（headless・`gt010_*`） |
| GT | GT-011 | GW-06→ProjectController Single Writer | 実装済（headless・`SYS-01`/`gw10c_*`/`sys01`） |
| GT | GT-012 | AI-off で manual DAW 継続 | 実装済（headless・`E2E-1`/`e2e1_*`）・実 GUI NotRun |
| JM | JM-01〜05 | GW-07 job/media/provenance 分離 | headless Pass・実 Provider/GUI NotRun |
| MCP | MCP-01〜06 | GW-08 adapter・permit binding・3 mode・bounded | PoC real transport Pass・第三者 Host NotRun |
| UX | UX-01〜06 | GW-09 headless 契約 | headless Pass(部分)・実 GPUI #114 NotRun |
| Gate | G0〜G6・GTV | GW-10 harness・evidence・oracle | `mise run gw10` で G0〜G6/GTV PASS（headless+PoC real）・5 件 NotRun/Unsupported |

## Risks

- **R-1 設計契約と実装の乖離**: docs が v0.3〜v0.8 合意を固定
  するが crate API が変わると陳腐化。→ fixture/test 名を
  ledger に pin し、#46 配下の変更時に ledger を更新。
- **R-2 headless/mock Pass の過剰主張**: in-process・mock・
  PoC transport の Pass を実 OS/GUI/Provider/第三者 MCP Host
  受入に繰り上げる誤り。→ mode/layer/required/real_only を
  ledger と evidence schema で分離、NotRun/Unsupported を
  明示。
- **R-3 Owner 境界の曖昧さ**: AUTX/CLI/MCP/GPUI が Gateway
  契約を再実装すると二重権威。→ GW-R-13 で adapter 独立・
  非重複を固定、#259〜#267・rf#553 を参照のみで照合。
- **R-4 Grant/Approval の権威混同**: wire 自己申告・Tool
  Discovery・UI Selection が権限に化ける。→ GT-002/SEC-02・
  DATA-09・SEC-07 で拒否を fixture 化済み、設計でも
  AdapterSeal/permit record 束縛を明記。
- **R-5 Recovery 保証の過大表示**: Journal+durable policy の
  headless 回復を電源断・実 Store atomic と誤認。→
  SYS-05-powerloss は Unsupported、SIGKILL 相当と OS 電源断を
  区別（GTV-005）。
- **R-6 並行 Worker との衝突**: sv#44（ai-native-baseline）
  ・AUTX 系・asset-library 系が並行編集中。本 slice は参照
  のみで編集しない。

## Deferred Design Concerns

spec の Deferred Design Concerns を参照。design 層で追加する
未確定事項:

- 実外部 MCP Host/Provider の Authenticated Session・第三者
  Host 互換 matrix | #46 G4 拡張・#59・#114 | PoC transport
  Pass を実環境 Pass と誤認する誤り。
- 実 GPUI 承認 UI・複数 Window Approval・Dialog 既定値 |
  #114 | headless 契約を実 UI 受入と誤認する誤り。
- OS 別 RT 数値 Budget・電源断 durability・実機 fault
  注入 | 実機 lane・#83 | Unsupported を Pass に繰り上げる
  誤り。
- Journal 物理形式・Durable Fence 方式・Retention/Privacy・
  Provenance Schema 物理保存場所 | #58・#62・PoC | 回復・
  監査保証を過大表示する誤り。
- AUTX Action/Workflow/CLI の具体 API・#267 の同一
  Capability/独立 oracle 受入 | #259〜#267 | Gateway 契約
  の二重実装。

## Verification notes

- 本 slice は docs/ 設計契約 + evidence 台帳のみ。実装・
  GUI・実 device/plugin/OS/Provider 受入は対象外。
- `mise run fmt-check`・doc lint・audit・route・review-
  protocol・`mise run gw10`/`gw10-report`/`check`/`test
  synvelis-gateway`（headless 回帰入口としての実測）を
  `evidence/sv45/command-gateway-verification.md` に記録。
- gw10 evidence JSONL（`target/gw10-evidence/*.jsonl`）の
  verdict を case ledger へ写像し、mock/real・L1〜L5・
  required/real_only を区別する。headless/mock Pass を実機/
  GUI/Provider 受入に繰り上げない。
- `doc-driven-dev-graph` route が pre-existing の
  `artifact-graph`/`broken-relation` blocker を報告する場合は
  baseline として記録し、本 slice の範囲外として修復しない。
