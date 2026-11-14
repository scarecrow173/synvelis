---
id: "SPEC-034X8szMIas9VjSANAwgwa"
type: "spec"
status: "approved"
title: "Capability / Command Gateway v0.3 — 横断整合性・型付き契約・責務境界 設計契約 (synvelis#45)"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/45"
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
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/specs/project-model.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/transport-integration.md"
    - "docs/specs/musical-time.md"
    - "docs/specs/media-render.md"
    - "docs/specs/workspace-model.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/recording-take-comping.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
    - "docs/designs/target-architecture-design.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/midi-source-note-editing-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
  derived-by:
    - "docs/designs/command-gateway-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/command-gateway.md"
  references: []
  defers: []
  deferred-by: []
---

# Capability / Command Gateway v0.3 — 横断整合性・型付き契約・責務境界 設計契約 (synvelis#45)

## What Should Be Built

Issue #45 の canonical 設計契約: Capability / Command Gateway
v0.3〜v0.8 で合意された横断整合性・型付き契約・責務境界を、
GW-01〜10 の Owner・型・状態遷移・Failure/Recovery Matrix・
G0〜G6 受入 Gate・GT-001〜012 / JM-01〜05 / MCP-01〜06 /
UX-01〜06 の受入ケースへ写像する 7 文書鎖 + evidence 台帳。

上流正本は Issue #45 本文（v0.3 提案 + v0.4 GW-06 ACG +
v0.5 GW-01〜05 GTC + v0.6 GW-07〜09 GIA + v0.7 GW-10 GTV +
v0.8 最終整合性監査）と、[#44] の AIB-001〜007 / AHG-001〜011
不変条件、[#46] の実装計画・G0〜G6 Gate。既存の確定済み縦断
契約（project-persistence・playback-graph・transport-integration・
musical-time・media-render・workspace-model・vcs-02 semantic
diff・gw-01/gw-02 design・midi-source-note-editing）を本契約の
一部として扱う。

本 slice は **docs/ 設計契約 + evidence 台帳のみ**。実装は
#46 配下の各実装 Issue（#53〜#59・#62・#83・#114）が Owner。
`crates/synvelis-gateway/` のコード・実 MCP Adapter・OS UI を
本 Issue だけで変更しない。設計完了を実装・実 OS・実
Plugin・実 Provider・GUI-L2/EXT-L3 受入とみなさない。

## Why Now

- Issue #45 は v0.4〜v0.7 の設計方向合意と v0.8 の実装監査を
  持つが、canonical な spec/design/plan/task/test-spec/impl の
  7 文書鎖と evidence 台帳が未整備であり、「何が合意済みで
  何が未決定か」「どの Case がどの fixture/Oracle/Owner/
  status で検証されるか」を一枚の契約表に固定する必要がある。
- GW-01〜10 の個別実装 Issue（#53〜#59）と統合検証（#62・
  #83）は merge 済みで `crates/synvelis-gateway/`・
  `crates/mcp-poc/`・`crates/synvelis-workspace/src/gateway_ui.rs`
  に実体が存在する。実装状態と設計契約の対応を照合し、
  headless/mock/real transport の証拠を正直に分類する必要が
  ある。
- AUTX 系 Issue（#259〜#267）と Ruviform #553 が Gateway の
  責務境界を前提に並行設計されており、Action/Workflow/CLI/
  MCP/GPUI の adapter 独立性と非重複を固定しないと将来の
  二重実装・権威混同が起きる。
- #44（AI 製品原則・Execution Grant/Approval 不変条件）・
  #46（G0〜G6 実装受入）・#114（GPUI 承認 UI）・実 Provider/
  OS gate との責務分界を Issue/Fixture レベルで照合する必要が
  ある。

## Users and Value

- 実装者（#46 配下の各 Issue Owner）: GW-01〜10 の型・状態・
  不変条件・Failure/Recovery の判断基準と Owner 境界。
- Reviewer: 「設計合意」「headless/mock fixture 成功」「real
  transport E2E」「製品受入」「実機/GUI/Provider 受入」を
  区別できる evidence 語彙。
- AUTX/CLI/MCP/GPUI adapter 設計者: Gateway 契約を再実装せず
  adapter として接続するための境界。
- Ruviform Consumer 調整: 共通化評価（rf#553）と Consumer
  回帰（rf#294）の適用条件。

## Scope

### In Scope

- Capability Descriptor / Typed OperationRequest / Result・
  Error の型付き契約（GTC-001/002/008）。
- Authorization Stage・Grant/Activation/Approval Policy・
  effect-aware authorization（GTC-003）。
- 権限付き Immutable Context Projection と Read/Disclosure
  分離（GTC-004）。
- Semantic Preparation と Synvelis 側 Effect 導出
  （GTC-005）。
- 不変 Prepared ChangeSet・Content Digest・Approval Receipt
  （GTC-006）。
- Proposal/Approval/Apply/Query/Transport/Async の分離された
  State Machine と 3 Mode（GTC-007・GIA-005）。
- Authorized Commit Gate・Grant Revoke vs Commit 線形化・
  Idempotency・Recovery/Indeterminate（ACG-001〜005）。
- Job/Media/Provenance adapter と状態分離（GIA-001〜003）。
- MCP adapter・信頼済み Local Principal Binding・3 Mode
  real transport E2E（GIA-004）。
- Permission/Approval UX 契約（GIA-005）。
- GW-10 検証 Harness・Evidence Schema・5 層 Test・独立
  Oracle・Fault Schedule・G0〜G6/GTV Gate（GTV-001〜008）。
- GT-001〜012・JM-01〜05・MCP-01〜06・UX-01〜06・SEC/DATA/
  REC/JOB/SYS failure matrix の case 写像・Owner・Fixture・
  status の台帳化。
- AUTX（#259〜#267）・CLI local issuer・MCP/CLI/GPUI adapter
  独立性・Ruviform #553 評価専用範囲の境界固定。
- RT boundary（Gateway/MCP 層で RT work 禁止）と Cross-
  consumer 回帰の適用条件。

### Out of Scope

- 実装コード（`crates/`、`Cargo.toml`、`Cargo.lock`、
  `mise.toml`、CI、Ruviform）。実装 Owner は #46 配下の各
  Issue。
- GUI 実装・実 device/plugin・実 OS 受入（GUI-L2/EXT-L3）。
  #114（GPUI 承認 UI）・#83（独立 CI 再現性）が Owner。
- 実外部 MCP Host/Provider の Authenticated Session 受入
  （#46 G4 拡張・#114 系の後続試験）。
- AUTX 系文書（#259〜#267）の編集・作成 — 別 Issue/Worker の
  範囲（参照のみ）。
- sv#44（ai-native-baseline docs）の編集・作成 — 並行 Worker
  の範囲（参照のみ）。
- Caller 自己申告を Trusted Authority へ昇格する設計変更。
- 設計だけによる実装・実 OS・実 Plugin・実 Provider・実機
  PASS の宣言。

## Requirements

- **GW-R-01 Capability Registry & Typed Invocation**: Stable
  CapabilityId/Version、OperationDomain（Query /
  ProjectMutation / RuntimeControl / AsyncOperation）、型付き
  Input/Output Schema、Target Contract、Required Permissions、
  Possible Effects 上限、Execution Contract/Availability を
  CapabilityDescriptor が保持する。OperationRequest は
  RequestId・CapabilityRef・ProjectId・Explicit Typed Target・
  Typed Arguments・Consistency Requirement・Idempotency Key
  を持ち、Principal/Origin は信頼できる Adapter/Session から
  別途 Bind する。Create は明示 Parent ID + Kind + Position/
  Range。未知 Capability/Version/Schema・Typed ID 不一致は
  推測変換せず拒否し、新 Capability は既存 Grant へ暗黙追加
  しない。UI Selection/Focus/Active Window への暗黙依存を
  禁止（GT-001/002/003）。
- **GW-R-02 Authorization**: DiscoverCapability / ReadContext /
  PrepareProposal / RequestApply / Commit / StartRuntimeOperation
  / StartAsyncOperation / ReadOperationResult の論理 Stage
  を区別し、PermitForStage は Commit 許可 Token でも永続的
  権限証明でもない。Preflight→GW-04 Effect 導出→Effect-aware
  Authorization→GW-06 Commit-time Revalidation を分離する。
  Suggest / Approve-to-Apply / Delegated Apply は Invocation
  引数で昇格させず、Synvelis が有効 Grant の Approval Policy
  から導出する。権限外 Resource の存在有無を外部 Error で
  開示しない（SEC-01〜14）。
- **GW-R-03 Context Projection & Disclosure**: 権限付き
  Immutable Snapshot/Projection を ContextQuery（Project・
  Explicit Resource Selector・Field Selection・Consistency・
  Page Request）へ応答として返す。Metadata / Structured
  Content / Media Bytes / Derived Data / Workspace Selection /
  External Disclosure を区別し、内部 Read と外部 Disclosure
  は別権限。ページングは固定 Snapshot に紐付き Revision を
  混在させず、Snapshot 失効時は再取得を求め暗黙に最新へ
  切替えない。Selection 公開は Grant/Command 対象を変更
  しない（DATA-01〜10・GT-004）。
- **GW-R-04 Semantic Preparation & Effects**: PrepareRequest
  （Project ID・Base Revision・Typed Semantic Operation）に
  対し、既存 Semantic Validator で正規化した Commands・
  Preconditions・Synvelis 側導出の Read/Write/Create/Delete/
  Disclosure/External Action Effects・非破壊候補 Snapshot 参照
  を PreparedOperation として返す。Clip≠Source≠Asset・Linked
  vs Independent MIDI を維持し、Linked Source 更新は実際に
  書き込む Source ID を認可対象とする。Project Musical /
  Media / Source Native Time・TempoMap/Source Revision・配置
  範囲を厳密に区別。Prepare 成功≠Apply 許可≠Commit 成功
  （GT-005/006）。
- **GW-R-05 Prepared ChangeSet & Approval**: Prepared
  ChangeSet は不変で ProposalId・ProjectId・Base Revision・
  Versioned Commands・Preconditions・Evaluated Effects・
  Dependency Fingerprint・Schema/Canonicalization Version・
  Content Digest・Expiry を持つ。Digest は Project/Revision・
  Command/対象・時間範囲・Preconditions・Effects・関連依存の
  意味的 Identity に結び付き、レイアウト・生成日時等の非意味
  要素と区別する。Approval Receipt は Synvelis 発行で
  Principal/Project/Grant/Task/Proposal+Digest/Expected
  Revision/Expiry に束縛し、改竄・他主体流用・期限切れ・
  Rebase なし再利用を拒否。Digest の所持は認可・承認の証明
  ではない（GT-007/008）。
- **GW-R-06 Separated State Machines**: Proposal
  （Submitted→Preparing/Validating→Prepared / Failed /
  Invalidated / Expired）、Approval（NotRequired または
  Requested→Granted/Declined/Expired/Revoked/Invalidated→
  Consumed）、Apply（NotRequested→CommitRequested /
  AwaitingApproval→Committed(CommitRef)/Rejected/Conflict/
  Indeterminate）を分離する。Query/Transport/Async は
  Proposal Lifecycle を必須とせず、Operation 受付と最終完了
  を混同しない。Suggest は Agent Apply 不可、Approve-to-
  Apply は個別 Receipt、Delegated Apply は有効 Grant 内で
  個別承認なし。
- **GW-R-07 Typed Results & GW-06 Boundary**: GatewayOutcome
  は Completed(T)/ProposalReady/ApprovalRequired/Accepted
  (OperationRef) を区別し、Completed は当該 Capability 処理
  の完了であって Project Committed を一般に意味しない。Error
  は Authentication/Authorization/Approval/Consistency/
  Validation/Capability-Version/Effects/DuplicateRequest/
  Recovery-Indeterminate に分類し、再試行可否と安全な外部
  Message を区別する。GW-01〜05 は Semantic Project Revision
  を変更せず、Project Mutation の確定は GW-06→Project
  Controller Single Writer のみ（GT-009/010/011）。
- **GW-R-08 Authorized Commit Gate & Recovery**: GW-06 は
  実際の Project Mutation 確定境界で最新の Grant/Activation/
  Approval/Policy Epoch・Scope/Effects/Limit・Expected
  Revision を ProjectController の Commit と整合して評価
  する。Revoke 確定が先なら Commit 拒否、Commit 確定が先な
  ら既存 Mutation は有効（Revoke は自動 Undo ではない）。
  事前 Check と直前 Check の反復だけで TOCTOU 解消を宣言
  しない。Commit Identity・承認消費・Idempotency Record は
  確定結果と整合し、同一 Principal/Project/Key+同一 Payload
  の再試行で二重変更せず、Key 同一で Payload 相違は Conflict。
  判別不能は Indeterminate として照合・Recovery し、根拠な
  く成功/未適用を断定しない（REC-01〜12・AC-01〜10）。
- **GW-R-09 Job/Media/Provenance Adapter**: GW-07 は Job
  Admission・結果取得権限・外部送信権限・Gateway
  Operation↔Job/Media/Provenance 参照を管理し、第二の汎用
  Job Executor・Window 所有 Job・第二 Media Store を新設
  しない。RequestId/OperationId/JobId/ProposalId/AssetId/
  SourceId は意味を分離する。Job Execution・Media Lifecycle・
  Project Mutation・Runtime/Persistence を独立状態とし、Job
  Completed≠Media Verified≠Project Committed≠Engine
  Applied≠Saved。入力 Read・External Provider 送信・Result
  開示・Project 登録を別保護境界として認可する。AI 生成
  Provenance は Model/Rights を Confirmed/Declared/Unknown
  の根拠区別で記録し、Grant 資格情報・秘密 Token・Prompt
  全文を素材 Provenance へ無条件に埋め込まない（JM-01〜05・
  JOB-01〜10・E2E-3）。
- **GW-R-10 MCP Adapter & Trusted Binding**: MCP Server は
  Tool/Resource 公開・Protocol/Schema/Result/Error 変換・
  Caller Binding・Bounded Data Transfer を担う Adapter で
  あり、ProjectController・Grant・Approval・Job の正本では
  ない。外部 Host/Client の申告名称/Task/Approval/Principal
  を認証の証明とせず、信頼済み境界で Principal を決定する。
  初期 MCP 連携は Local stdio Helper + 認証付き Local IPC
  を第一候補とする比較 PoC（Localhost Streamable HTTP を
  比較対象に残す）。Tool Discovery は利用権限の証明ではなく
  Invocation ごとに GW-02 へ再照会する。Suggest/Approve-to-
  Apply/Delegated Apply の 3 Mode を実動作させ、外部 MCP
  Host の承認 UI だけで Synvelis Approval Receipt を発行
  しない。Disconnect/Retry は冪等キーと再認可で結果を照合
  し二重 Commit を防ぐ（MCP-01〜06・E2E-2-suggest/approve/
  delegated/restart）。
- **GW-R-11 Permission/Approval UX Contract**: 接続許可/
  Synvelis 管理 AI Runtime/Authorized Task/Execution Grant/
  Approval Policy を独立設定・独立状態として表示し、AI/MCP
  は既定 OFF。Workspace/Selection は Grant 発行時の対象選択
  支援に留め、発行時に Project/Typed Stable ID/固定 Scope
  または明示条件付き動的 Scope として確定し、その後の
  Selection/Active Window 変更で Grant/Command 対象を変更
  しない。固定/動的 Scope・対象・将来追加 Resource・
  Capability・時間/数量 Limit・期間を確認可能にする。
  Approve-to-Apply は Proposal/Content Digest/Expected
  Revision/Principal/影響範囲（Linked MIDI Source を含む）
  を提示して特定 Receipt を発行する。Revoke Requested≠
  Revoke Confirmed≠Job Cancel Requested≠Job Cancelled を
  区別し、Proposal/Committed/Applied/Saved を Semantic
  Diff/History と関連付けて表示する。第二の Project History
  を作らず、AI 無効でも通常 DAW は完全に機能する（UX-01〜
  06・SYS-02-ux/03-ux）。
- **GW-R-12 Verification Harness & Evidence**: GW-10 は
  Contract/Security/Concurrency-Recovery/DAW Non-regression/
  Cross-consumer Compatibility の 5 責務を持ち、Project/
  Policy/Job/RT の第二実行正本を持たない。5 層 Test（L1
  Unit/Contract/Property・L2 Component Integration・L3
  In-process E2E・L4 Process/Fault E2E・L5 System Regression）
  と独立 Oracle・Controlled Barrier・Fault Injection・
  Deterministic Fixture・Causal Event Trace を用いる。Test
  ID→Requirements→GW Owner→Fixture/Revision/Platform/
  Transport/Backend→Expected/Actual→Evidence→Gate 判定を
  追跡し、Pass/Fail/Blocked/NotRun/Unsupported を分離する。
  Unsupported/未実行/Mock 限定/実機未検証を Pass にしない
  （G0〜G6・GTV-001〜008・SEC/DATA/REC/JOB/SYS）。
- **GW-R-13 Boundary Non-duplication**: #259（AUTX-00）の
  自動化層を最新 Owner/DAG とし、#260 Action は
  CapabilityRef/Descriptor の Presentation 投影であって別
  Action Registry/Executor ではない。#263 Atomic Macro は
  既存 GW-04 `synvelis.operation.batch`→ProjectOp::
  Composite→GW-05/06 Commit で一度だけ CAS/Undo。#264/
  #265 Workflow Definition/Coordinator は Gateway の外側で
  非 RT Step・Plan/Resume cursor を担当し、GW-02 Grant・
  GW-05 Approval・GW-06 ProjectWriter/Journal・GW-07
  Provider Job Truth を再実装しない。#261 CLI local issuer
  は AdapterSeal 付き信頼境界を維持し TTY/`--yes`/JSON で
  PrincipalKind::Human を偽造しない。#262 CLI は Native/
  WebView に依存しない独立 Adapter。#267 で MCP/CLI/GPUI
  の同一 Capability と独立 Policy/Commit oracle を受入
  する。Ruviform #553 は共通化を設計評価するだけで
  Gateway/DAW 型の移植なし。
- **GW-R-14 RT Boundary & Consumer Regression**: Gateway/
  MCP adapter 層で RT work（Policy/Network/Project Mutation/
  Blocking/無期限 Wait）を禁止し、旧 Plan/Transport
  Generation への誤配送を禁止する。Ruviform 共通 API/Time/
  Job/Media/UI へ変更する場合のみ Synvelis・AudioLens 双方の
  機能・精度・性能回帰と対応 Issue を要求し、Synvelis 固有
  の Grant/MCP 意味論を Ruviform へ自動移管しない（SYS-07・
  CONSUMER-RUVIFORM/CONSUMER-AUDIOLENS）。
- **GW-R-15 Evidence Honesty**: 各 case に実行 Owner・
  コマンド/Fixture・Revision・Platform・mode（mock/real）・
  layer（L1〜L5）・Pass/Fail/NotRun/Unsupported と検証証拠
  を付す。headless/mock の Pass を実 OS/GUI/Plugin/Provider/
  実機受入に繰り上げず、設計レビュー Pass と実測 Pass を
  区別する。要求状態（Required/Deferred/NotApplicable）と
  実行結果（Pass/Fail/NotRun/Unsupported）を別軸で管理する。

## Acceptance Criteria

Issue #45 の受入条件を GW-AC-01〜12 として転記・対応付ける。
各 GT/JM/MCP/UX/SEC/DATA/REC/JOB/SYS/G0〜G6 case の現在の
被覆状況は `evidence/sv45/command-gateway-case-ledger.md` を
正本とし、本 spec は要求のみを定義する。

- [ ] **GW-AC-01** GT-001〜003: 明示 Typed Stable ID/範囲で
  対象指定し UI Selection から暗黙解決しない、外部入力から
  Trusted Authority を自己申告できない、Capability の存在と
  利用権限を分離する設計・写像が存在する。
- [ ] **GW-AC-02** GT-004〜006: 内部 Context Read と外部
  Data Disclosure を分離し、Proposal 作成権限と Apply 権限を
  分離し、実際の Effect Set を Synvelis 側が導出する設計・
  写像が存在する。
- [ ] **GW-AC-03** GT-007〜009: ChangeSet は不変で Digest は
  意味的変更内容と Revision を識別し、Approval は
  Principal/Project/ChangeSet/Revision に束縛し、Grant 失効
  後は未使用 Approval で Commit できない設計・写像が存在する。
- [ ] **GW-AC-04** GT-010〜012: GW-01〜05 で Semantic Project
  の Revision を変更せず、Project Mutation の確定は GW-06→
  ProjectController Single Writer のみとし、MCP/AI Disabled
  でも手動 UI 編集・通常 DAW 機能が成立する設計・写像が存在
  する。
- [ ] **GW-AC-05** JM-01〜05: Disconnect/Window Close 非依存
  Job・Grant 失効後の追加保護操作拒否・Job Completed と
  Media Verified 分離・Verified Media≠Project Commit・
  Provenance の Model/Rights Unknown の写像が存在する。
- [ ] **GW-AC-06** MCP-01〜06: 信頼できる Principal Binding・
  Tool/Invocation 権限分離・3 Mode E2E・異種 Stable ID/未知
  Schema 拒否・Disconnect/Retry で二重 Commit 防止・大容量
  Media Read/Disclosure 制約の写像が存在する。
- [ ] **GW-AC-07** UX-01〜06: AI Runtime vs External MCP 分離・
  固定/動的 Grant Scope 表示・特定 ChangeSet/Revision/Effects
  の承認表示・Grant Revoke と Job Cancel 区別・Proposal/
  Committed/Applied/Saved 表示・AI 無効時の通常 DAW 維持の
  写像が存在する。
- [ ] **GW-AC-08** GW-01〜10 の型/状態遷移/Failure Matrix・
  G0〜G6 の実装 Owner が #44/#46 と相互照合され、手動 UI と
  外部 MCP Caller の権威が分離される。
- [ ] **GW-AC-09** SEC-01〜14・DATA-01〜10・REC-01〜12・
  JOB-01〜10・SYS-01〜10 の failure matrix が case ledger に
  fixture・oracle・Owner・status・証拠付きで写像される。
- [ ] **GW-AC-10** AUTX（#259〜#267）・CLI local issuer・
  MCP/CLI/GPUI adapter 独立性・Ruviform #553 評価専用範囲が
  境界として固定される。
- [ ] **GW-AC-11** RT boundary と Cross-consumer 回帰の適用
  条件が固定され、未接続 Provider/実機を PASS としない。
- [ ] **GW-AC-12** 7 文書鎖（spec/design/plan/task/test-spec/
  impl-ir/impl-exp）+ `evidence/sv45/`（case ledger・
  verification）が canonical 配置され、index 再生成・docs
  検証・review-protocol が実行・記録される。

## Deferred Design Concerns

- MCP Transport/SDK/Host 互換性・認証 Local IPC・MCP Tasks
  連携方式・Job Handle/Operation Result 永続化・Generation
  Provenance Schema の物理保存場所・External Provider 許可/
  費用 Budget・Retention/Privacy・具体 UX/Default Limit・
  完全な Security/Crash 試験結果 | Issue #45 未確定事項・
  #46/#59/#62/#114 | 実装・PoC・実機 lane | 未確定を確定済み
  と誤読する誤り。
- Journal 方式 vs 独立 Store+永続 Commit Fence の最終選定・
  Lock/Epoch/Transaction Boundary・ChangeSet
  Canonicalization/Hash/Version・Approval/Idempotency/Audit
  の永続化・Retention | ACG-004 未決・PoC | #58・#62 | 回復
  保証を過大表示する誤り。
- 実外部 MCP Host/Provider の Authenticated Session・実
  GUI-L2/EXT-L3 受入・OS 別 RT 数値 Budget・電源断保証範囲
  | #46 G4 拡張・#83・#114・実機 lane | 後続受入 Issue |
  headless/PoC transport を実環境 PASS と誤認する誤り。
- AUTX Workflow/CLI/Atomic Macro の具体 API・Default Budget/
  Dialog 操作性・第三者 Plugin AI の制御保証限界 | #259〜
  #267・使用感 PoC | AUTX 系 Issue | Gateway 契約を
  再実装する二重化。
- 具体的 Rust API/Crate placement の最終確定・全 DAW Plugin
  Effect の網羅 | v0.8 非対象・実装 Issue | #46 配下 |
  設計契約を実装済み API と誤認する誤り。

## Sources and Relations

- Issue #45 本文（v0.3 提案・v0.4 ACG-001〜005・v0.5
  GTC-001〜008・v0.6 GIA-001〜006・v0.7 GTV-001〜008・
  v0.8 最終整合性監査・GT/JM/MCP/UX 受入条件・検証方法・
  変更範囲）。
- Issue #44（AIB-001〜007・AHG-001〜011 の AI 製品原則と
  Execution Grant/Approval 不変条件）。
- Issue #46（GW-01〜10 実装計画・G0〜G6 Gate・実装/受入索引）。
- 確定済み縦断契約: `docs/specs/vs-a-*`（stable ID/save/
  reload）・`project-model`・`project-persistence`（#17
  ProjectController/Store）・`playback-graph`（#18）・
  `transport-integration`・`musical-time`・`media-render`
  （#21 Job/Media/Provenance）・`workspace-model`・
  `vcs-02-semantic-diff-changeset`（#7 Semantic Diff）・
  `recording-take-comping`（#116 系）・`midi-source-note-
  editing` design（#52/#56）。
- GW 個別設計: `docs/designs/gw-01-gateway-contract-design.md`・
  `docs/designs/gw-02-authorization-policy-design.md`。
- 上流 architecture: `docs/designs/target-architecture-design.md`・
  `synvelis-driven-ruviform-target-architecture-v07.md`・
  `time-transport-midi-automation-plugin-integration-
  architecture.md`・`media-import-export-offline-render-ui-
  diagnostics-architecture.md`。
- 実装 Issue: #53（GW-01）・#54（GW-02）・#55（GW-03）・
  #56（GW-04）・#57（GW-05）・#58（GW-06）・#59（GW-08）・
  #62（GW-10 E2E/security/concurrency/recovery）・#83
  （独立 CI 再現性）・#114（GPUI 承認 UI）・#52（MIDI Note
  domain）・#56（Source/Clip Effect）・#57（semantic diff）・
  #58（revoke/commit）・#59（MCP binding）。
- AUTX 系: synvelis #259〜#267、Ruviform #553（評価専用）。
- Cross-consumer: synvelis #28・Ruviform #219/#294・
  AudioLens #127。
- 関連 Issue: synvelis #2/#7/#8/#10/#11/#13/#17/#20/#21/#22/
  #23/#28/#44/#45/#46/#52/#53〜#59/#62/#83/#86/#107系/#114/
  #127/#128/#145〜#167/#176/#219/#230/#251/#253/#259〜#267/
  #277〜#297/#307/#308/#309/#313/#314/#330/#365/#368・
  #388/#389。
