# Capability / Command Gateway — G0〜G6 受入照合台帳 (synvelis#46)

Issue #46「Capability / Command Gateway — GW-01〜10・G0〜G6・
実装依存と受入Gate」の受入照合・証拠台帳。GW-01〜10 の個別
実装 Issue は完了・実装 PR は main 統合済みであり、本台帳は
**G0〜G6 各 Gate の受入条件を、既存実装・既存 fixture・設計
正本（#44/#45 docs）へ一意に照合し、対象 Revision・実行
コマンド・Pass/Fail/NotRun/Unsupported を記録する**。

**本 slice は受入照合・台帳のみ**であり、新規実装・Fixture
追加・別 Gate の PASS 宣言は行わない。AI Gateway は Gate
A/B/C と独立した受入軸であり、`Headless Verified` と
`Product Accepted` を別状態で記録する。Ruviform 汎用
Gateway 変更時のみ #294 両 Consumer 回帰が条件となる
（本 slice で Ruviform 変更なし）。

## 環境・対象 Revision・実行コマンド

| 項目 | 値 |
|---|---|
| 対象 Repository | scarecrow173/synvelis |
| 対象 Revision | `86715f9ac2761f9f275e885e0bbd7ee775d9633e`（`origin/main`・PR #408 merge commit と同一） |
| Ruviform resolved rev | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock` git 依存） |
| OS / Arch | Windows / x86_64（Git Bash 経由・evidence record の `platform` = `windows-x86_64`） |
| Rust toolchain | cargo/rustc `1.99.1`（環境導入済み rust・`mise.toml` は Rust を pin しない。CI pin は `mise.ci.toml`） |
| Node | `26.12.1`（doc 検証 script 用） |
| 実行日 | 2026-11-14 |
| 作業 worktree | `C:\Users\akiyoshi\orca\workspaces\synvelis\sv46-gateway-gates` |
| ブランチ | `docs/46-gateway-gates-audit`（派生元 `scarecrow173/sv46-gateway-gates` = `origin/main` HEAD） |

### 実行した正規検証コマンド（実測）

| コマンド | exit | 結果 |
|---|---|---|
| `mise run fmt-check` | 0 | `cargo fmt --all -- --check` 成功 |
| `mise run gw10` | 0 | `gw10_report -- --clean` → 9 つの `gw10_*` integration suite（`gw10_{harness,contract,authorization,project,concurrency,persistence,mcp,job,system}`）→ `cargo test -p mcp-poc --test gw10_transport` → `gw10_report -- --strict`。G0〜G6・GTV 全 PASS。strict report が required=false の 5 件（SYS-02-ux・SYS-03-ux・SYS-05-powerloss・SYS-07-rt・CONSUMER-RUVIFORM/AUDIOLENS）を NotRun/Unsupported として正直に報告 |
| `mise run gw10-report` | 0 | `target/gw10-evidence/*.jsonl` を集約。gate verdict: G0〜G6・GTV 全 PASS。missing evidence: SYS-02-ux, SYS-03-ux, SYS-05-powerloss, CONSUMER-RUVIFORM, CONSUMER-AUDIOLENS |
| `mise run test synvelis-gateway` | 0 | `cargo test --locked -p synvelis-gateway`。25 test binary 全成功・**406 passed / 0 failed**（lib 7・bin 0・integration 22 file・doc-tests 0） |

補足: `mise run gw10` 内の `cargo test -p mcp-poc --test
gw10_transport` も成功（4 tests: `gw10f_three_modes_over_
real_transports`・`gw10f_delegated_apply_commits_and_
survives_restart_real`・`gw10f_boundary_attacks_fail_
closed_real`・`gw10f_permit_cli_lifecycle_real`）。
`mcp-poc/tests/stdio_e2e.rs` は `mise run test synvelis-
gateway` の対象外（別 package）だが、GW-08 実装時の real
transport E2E として `gw10_transport.rs` と同一 wire を駆動
する既存 fixture であり、本台帳では参照証拠として扱う
（今回の正規コマンドでは未再実行 — 後述 V 記録参照）。

## 判定語彙

Issue #46 の「`Headless Verified`と`Product Accepted`を別
状態で示す」に従い、各 Gate を 2 軸で記録する。

| 状態 | 意味 |
|---|---|
| `Headless Verified` | headless/mock/harness/PoC real transport の既存証拠で Gate の契約レベル受入条件が実測 Pass。`mode`/`layer` を明記し、実機・実 GUI・実 Provider・第三者 Host 受入に繰り上げない |
| `Product Accepted` | 実製品経路（実 Window/GUI 承認 UX・第三者 MCP Client・実 Provider・実 OS lane・実機 RT）での受入。現時点で未実施のものは `NotRun`/`Deferred` とし Owner Issue を記す |
| `NotRun` | 未実装または未検証（実機・実 GUI・実 Provider・第三者 MCP Host・電源断・大規模負荷・Consumer 回帰） |
| `Unsupported` | harness/環境が技術的に非対応（根拠を伴う。例: `fault::POWER_LOSS`・SYS-07-rt budget 未実測） |
| `Deferred` | 利用条件成立まで保留（GUI-L2/EXT-L3 の Deferred 方針に一致） |

要求状態（Required/Deferred/NotApplicable、POLICIES.md §3）
と実行結果は別軸で管理する。Gate 判定の証拠正本は
`target/gw10-evidence/*.jsonl` を `gw10_report -- --strict`
で集約した verdict とし、case 単位の fixture/oracle/Owner/
mode/layer/status の詳細写像は `evidence/sv45/
command-gateway-case-ledger.md` を正本として参照する
（本台帳は Gate 受入条件への照合に限定し、case ledger を
複写しない）。

## Gate 別照合

### G0 / Contract Gate

**Issue #46 受入条件**: 「#45のDomain/Type/Error/Ownerを合意。
既存Commandと同じ意味的操作をUI非依存・明示Target/Revisionで
表現できる。」Gate 判定条件: 「GW10-A。Typed ID、Schema/
Version、Operation Domain、State Machine、Error」（#46
Gate判定節）。

| 受入項目 | 照合先（実装・fixture・設計正本） | 実行証拠 | 判定 |
|---|---|---|---|
| Domain/Type/Error/Owner の合意 | `docs/specs/command-gateway.md`（GW-R-01/07）・`docs/designs/command-gateway-design.md`・`docs/adr/gw-01-gateway-contract-crate-boundary.md`・`docs/impl/ir/gw-01-gateway-contract.md` | 文書鎖は PR #408 で main 統合済み | Headless Verified: Pass（設計正本として確立） |
| Typed Stable ID / 明示 Target・Revision・UI 非依存 | `crates/synvelis-gateway/src/{capability,registry,request,id,schema,wire,outcome}.rs`・`tests/contract.rs::gt001_*`/`gt002_*`/`gt003_*`・`tests/gw10_contract.rs::*`・`tests/schema_compat.rs::*` | GW10-A: GT-001/002/003 PASS・SYS-08 PASS。`mise run gw10` G0=PASS、`test synvelis-gateway` contract 40 + schema_compat 7 + gw10_contract 3 pass | Headless Verified: Pass（mock/L1-L2） |
| 既存 Command と同じ意味的操作を UI 非依存で表現 | `synvelis.operation.*` capability 群（`prepare_engine.rs`）・`ProjectOp` lower 層（`prepare_engine.rs::lower_*`）・`ProjectController`（`synvelis-project`） | GT-001〜003・GT-010（prepare/approve/preview が Revision を変更しない）PASS | Headless Verified: Pass |
| 製品経路での利用 | `synvelis-workspace`/`synvelis-app` が `synvelis-gateway` に依存（`gateway_ui.rs`・`ui_ops.rs`）— headless contract 経由 | 実 Window での Capability 呼出し・製品統合受入は merge 後の対象 SHA の必須 CI 証拠（Coordinator 確認）と実機 lane | Product Accepted: NotRun（統合検証証拠は merge 後 CI・Coordinator） |

**G0 判定**: Headless Verified = **Pass**（G0=PASS・
required case 全 Pass）。Product Accepted = **NotRun**
（契約の製品経路統合受入は merge 後の対象 SHA の必須 CI
証拠・実機 lane の後続。Contract 自体は headless が意味の
ある検証層であり、G0 の契約性質上 headless Pass が契約
受入の実質的証拠）。

### G1 / Read & Policy Gate

**Issue #46 受入条件**: 「モックまたは実Project Snapshotで
Principal/Grant/Activation、Fixed/Dynamic Scope、Data
Disclosureを評価。Grantなし/Scope外/Media内容の未許可取得を
拒否。」Gate 判定条件: 「GW10-B+F。SEC/DATAのRead・Grant・
Scope・Disclosure部分」。

| 受入項目 | 照合先 | 実行証拠 | 判定 |
|---|---|---|---|
| Principal/Grant/Activation 評価 | `src/{authorization,grant,policy,durable_policy}.rs`・`tests/authorization.rs`（SEC/GT/AHG/ACG 系 74 tests）・`tests/gw10_authorization.rs` | SEC-01〜03 PASS・`test synvelis-gateway` authorization 74 pass | Headless Verified: Pass（mock/L1-L3） |
| Fixed/Dynamic Scope | `src/policy.rs`・`tests/context.rs`（dynamic_scope_filters_by_kind 等）・`gw10_authorization.rs::sec06_*`/`sec07_*` | SEC-06/07 PASS | Headless Verified: Pass |
| Data Disclosure 分離 | `src/context.rs`/`context_engine.rs`/`snapshot.rs`・`tests/context.rs`（43 tests）・`gw10_authorization.rs::data01`〜`data10` | DATA-01〜10 PASS・GT-004 PASS・context 43 pass | Headless Verified: Pass（mock+real/L2-L4: DATA-08 は real record 含む） |
| Grant なし/Scope 外/Media 未許可取得の拒否 | SEC-01/03/04/05/13/14・DATA-01/02/05/07/10・`prepare.rs::unverifiable_effects_fail_closed_for_every_caller` | 全件 PASS | Headless Verified: Pass |
| 実 Project Snapshot での評価 | `MemorySnapshotSource`（in-process）+ `DirectoryBundleStore`（PoC real backend） | GW10-B は mock/L2 中心。実 Project bundle での Read/Disclosure E2E は G4 PoC 経由で部分的に実測（`mcp-poc` の `project.inspect`/`midi.notes.query`） | Headless Verified: Pass（mock+PoC real backend 部分）。Product Accepted: NotRun（実製品 Project での Disclosure 受入は後続） |

**G1 判定**: Headless Verified = **Pass**（G1=PASS）。
Product Accepted = **NotRun**（実製品/実 UI での権限提示・
Disclosure 体験は #114・#46 G6 残件）。

### G2 / Proposal Gate

**Issue #46 受入条件**: 「MIDIを最小対象にPrepared
ChangeSet、Effect Set、Semantic Diff、承認内容Digestと
Revision一致を検証。既存UI Gesture Previewの正本を重複
させない。」Gate 判定条件: 「GW10-C。Semantic Prepared
Effects、Linked Source、Immutable ChangeSet/Approval
Digest」。

| 受入項目 | 照合先 | 実行証拠 | 判定 |
|---|---|---|---|
| Prepared ChangeSet（MIDI 最小対象） | `src/{prepare,prepare_engine,prepare_policy}.rs`・`tests/prepare.rs`（18）・`prepare_regressions.rs`（7）・`tests/gw10_project.rs::gt005_*`/`gt006_*` | GT-005/006 PASS・prepare 系 25 pass | Headless Verified: Pass（mock/L2） |
| Effect Set を Synvelis 側が導出 | `prepare_engine.rs` の effect 導出・`derived_effects_flow_through_grant_scope`・`linked_midi_write_names_source_and_linked_clips`・SEC-05/AC-09 | GT-006 PASS・SEC-05 PASS・AC-09 PASS | Headless Verified: Pass |
| Semantic Diff / Preview | `src/proposal.rs`/`proposal_engine.rs`/`proposal_policy.rs`・`tests/proposal.rs`（30）・`proposal_regressions.rs`（16）・`gw10_project.rs::gt007_*`/`gt008_*` | GT-007/008 PASS・preview/semantic drift/digest 系 pass | Headless Verified: Pass |
| 承認内容 Digest と Revision 一致 | `proposal.rs` の ContentDigest/ExpectedRevision 束縛・`receipt_binding_rejects_wrong_grant`・`receipt_binding_rejects_wrong_task`・`revision_conflict_rejects_apply`・SEC-09/10/11 | GT-008 PASS・SEC-09/10/11 PASS | Headless Verified: Pass |
| 既存 UI Gesture Preview 正本の非重複 | `docs/designs/command-gateway-design.md`（GW-R-05/13）・Semantic Diff 正本は `docs/specs/vcs-02-semantic-diff-changeset.md`（#7） | 設計正本で第二 Preview 正本を持たないことを固定。コード上 `preview_*` は proposal store の読み出し面 | Headless Verified: Pass（設計照合） |
| Proposal を Project Mutation として扱わない | GT-010（prepare/approve/preview で Revision 不変）・`gt010_prepare_approve_preview_leave_revision_untouched` | GT-010 PASS | Headless Verified: Pass |

**G2 判定**: Headless Verified = **Pass**（G2=PASS）。
Product Accepted = **NotRun**（実 GUI での Proposal/Diff
レビュー体験は #114。headless `gateway_ui.rs` の
ApprovalSurface 契約は存在するが実画面未検証）。

### G3 / Authorized Commit Gate

**Issue #46 受入条件**: 「Suggest（Agent Apply不可）／
Approve-to-Apply（個別承認）／Delegated Apply（制限付き
自動Apply）の3モードが実動作。ProjectController Single
Writer、Undo、Grant失効競合、Approval再利用防止、
Idempotency/Crash Recoveryを実証。」Gate 判定条件:
「GW10-C/D/E。初期MIDIの3 Mode、Revoke/Commit順序、
Revision/Retry/Undo、実PersistenceへのCrash/Restart/
Recovery証拠」。

#### 3 Mode の実証箇所

| Mode | headless 実証 | real PoC transport 実証 |
|---|---|---|
| Suggest（Agent Apply 不可） | `proposal.rs::suggest_mode_never_applies`・`gw10_authorization.rs::sec08_suggest_mode_apply_denies_without_revision_change`(L3)・`mcp_adapter.rs::suggest_mode_prepares_but_denies_apply`・`authorization.rs::ahg009_*` | `mcp-poc/tests/gw10_transport.rs::gw10f_three_modes_over_real_transports`（E2E-2-suggest: prepare 許可・apply=`denied`） |
| Approve-to-Apply（個別承認） | `proposal.rs::approve_to_apply_flows_through_request_grant_consume`・`approve_is_a_trusted_user_only_act`・`receipt_binding_rejects_wrong_grant/task`・`receipt_for_another_proposal_denies`・`double_consumption_can_never_succeed` | 同上（E2E-2-approve: control channel で receipt 発行→client channel で 1 回消費→`committed`） |
| Delegated Apply（制限付き自動 Apply） | `proposal.rs::delegated_apply_needs_no_receipt`・`authorization.rs::ahg009_delegated_apply_permits_inside_budget`・`ahg009_apply_mode_comes_from_grant_not_request` | `gw10f_delegated_apply_commits_and_survives_restart_real`（E2E-2-delegated: real CommitGate commit→`committed`） |

Mode は Invocation 引数ではなく Grant の Approval Policy から
Synvelis が導出（`ahg009_apply_mode_comes_from_grant_not_
request`・GW-R-02）。外部 Host の自己申告で Human
Authority/Approval へ昇格しない（GT-002・SEC-02）。

#### 単一 Writer・Undo・Grant 失効競合・Approval 再利用防止・Idempotency/Crash Recovery の実証箇所

| 要求 | 実装・fixture 照合 | 実行証拠 | 判定 |
|---|---|---|---|
| ProjectController Single Writer | `src/commit.rs`（CommitGate/CommitFence 直列化）・`synvelis-project/src/controller.rs`・`tests/commit_gate.rs::concurrent_commits_single_writer_conflict`・`manual_commit_is_human_only_and_journaled`・`gw10_project.rs::gw10c_*`・SYS-01/GT-011 | SYS-01 PASS・GT-011 PASS・commit_gate 22 pass・`rec03_two_commits_one_base_one_conflicts` PASS | Headless Verified: Pass（mock/L2-L3 + real journal/L4） |
| Undo | `gw10_project.rs::gw10c_commit_advances_revision_and_undo_rewinds`・`prepare.rs::prepared_op_commits_through_controller_and_undoes`・E2E-1（commit→undo→recover） | GT-011/E2E-1 PASS | Headless Verified: Pass |
| Grant 失効競合（Revoke vs Commit 線形化） | `src/{commit,journal,durable_policy}.rs`・`tests/gw10_concurrency.rs::rec01`/`rec02`/`gtv005_*`・`commit_gate.rs::revoke_confirmed_before_commit_denies`/`commit_confirmed_before_revoke_stands`・`durable_revoke_survives_restart`・`gw10_authorization.rs::sec12_*`・GT-009 | REC-01/02 PASS・GT-009 PASS・SEC-12 PASS・GTV-005 PASS（deterministic ordering: Controlled Scheduler で両順序を列挙） | Headless Verified: Pass（mock/L2 + file journal real durability） |
| Approval 再利用防止 | `proposal.rs` の Approval Consumed 状態・`double_consumption_can_never_succeed`・`consume_verify_reconfirms_only_the_spending_attempt`・`seam_consume_requires_the_accepted_attempt`・REC-07（crash during consume） | GT-008/009 PASS・REC-07 PASS・proposal_regressions 16 pass | Headless Verified: Pass |
| Idempotency | `commit.rs` の idempotency key・`rec04`/`rec05`/`rec06`・`keyed_commit_retry_replays_the_receipt`・`wire_level_keyed_conflict_rejects_different_payload`・`contract.rs::idempotency_*`・PoC `proposal.apply` retry→同一 `journal_seq` | REC-04/05/06 PASS・MCP-05 PASS（real transport retry で二重 Commit なし） | Headless Verified: Pass（mock/L2 + real/L4） |
| Crash Recovery | `src/journal.rs`（FileJournal・torn tail・CRC）・`commit.rs` の reconcile/Indeterminate・`tests/gw10_persistence.rs::rec07`〜`rec12`/`sys04_*`・`commit_gate.rs::crash_*`/`torn_*`/`durable_*`/`restart_*`・`mcp-poc` serve restart→`operation.get` reconcile（E2E-2-restart/SYS-09） | REC-07〜12 PASS・SYS-04/09 PASS・E2E-2-restart PASS（real process kill+restart/L4） | Headless Verified: Pass（mock/L2 + real process/L4。電源断は Unsupported） |
| 実 Persistence への Crash/Restart/Recovery | `DirectoryBundleStore`+`commit.journal`+`FilePermitStore`（mcp-poc real backend）・`FilePolicyStore`（durable grant/policy replay: `file_policy_store_replays_lifecycle`・`durable_policy_admin_never_double_counts_consumption`） | E2E-2-restart・SYS-09 real/L4 PASS。Grant/Policy の durable replay は FilePolicyStore 単体実測（mock/L2・real file）— PoC `serve` の policy store は in-memory であり、Grant の process 越え永続化は store 層証拠と product 配線の接続が残件 | Headless Verified: Pass（commit/persist は real/L4、policy durable は component/L2） |

**G3 判定**: Headless Verified = **Pass**（G3=PASS・3 Mode
は in-process mock と PoC real transport の両方で実動作）。
Product Accepted = **NotRun**（実 Window での human 承認
UX・実 DAW 内 Commit 体験は #114。Undo は headless
ProjectController 経由で実証済みだが実 GUI Undo 連携は
未検証）。

### G4 / MCP E2E Gate

**Issue #46 受入条件**: 「明示有効化した外部MCP Clientから
G1〜G3の3 Modeが機能する。Caller詐称、接続・Grant取消、
別Project、UI Selection変更、外部情報開示境界を試験。
通常DAWとAI Offは独立。」+「MCP Tool Discovery/Schemaは
権限の代替にならず、各Invocationで再認可。外部Hostの自己
申告でHuman Authority/Approvalへ昇格できない。」「AI
RuntimeとExternal MCP Accessを分離し、MCP/AI Disabledでも
通常DAW操作に影響しない。」「切断→再接続→Operation照合／
Idempotent Retryが安全に機能し、Projectの二重Commitがない。」
+「Transport選択: Local stdio Helper + 認証Local IPCを第一
候補としてPoC。他方式（例: Localhost Streamable HTTP）と
接続Host互換・認証・攻撃面を比較。まだ最終決定しない。」

#### Transport の現状（未確定の明記）

- 実測 transport: `mcp-poc serve --dir <harness>
  [--control-port <port>]` — **Local stdio Helper**
  （newline-delimited JSON-RPC over stdio・MCP host が spawn
  する local helper）+ **認証付き Local IPC 候補**
  （`127.0.0.1` loopback line socket・operator 専用 control
  channel・agent 到達面ではない）。
- 比較対象: Localhost Streamable HTTP は設計上の比較候補
  として残るが **未実装・未比較実測**。Transport の最終
  決定は **未確定**（Issue #46 の「まだ最終決定しない」を
  維持）。
- 実装: `crates/mcp-poc/src/main.rs`（serve/permit CLI・
  stdio+loopback の 2 surface・1 `McpHost`）、
  `crates/synvelis-gateway/src/{mcp,mcp_server,mcp_tools,
  mcp_permit}.rs`。IR: `docs/impl/ir/gw-08-mcp-server-
  adapter-trusted-local-connection-midi-3mode-e2e.md`・
  ADR `gw-01-gateway-contract-crate-boundary` 系。

#### 受入項目照合

| 受入項目 | 照合先・実行証拠 | 判定 |
|---|---|---|
| 明示有効化（opt-in） | `McpConfig{enabled}`・`mcp_adapter.rs::disabled_adapter_denies_the_surface`・`issue_permit`/`FilePermitStore`（offline provisioning） | Headless Verified: Pass。Product: 実製品での有効化 UX は #114/#46 G6 残件 NotRun |
| 外部 MCP Client から G1〜G3 の 3 Mode | **重要な限定**: 実測 client は本 repo の `mcp-poc` test harness（`tests/common` の `Server`/`stdio`/`control`）であり、**第三者 MCP Host（実 Claude/Cursor 等）ではない**。real transport（実 process・実 stdio pipe・実 loopback socket・real `DirectoryBundleStore`+`commit.journal`+`FilePermitStore`）で E2E-2-suggest/approve/delegated/restart が PASS | Headless Verified: Pass（PoC real transport/L4 — 「外部 Client」の実体は自前 harness）。Product Accepted: **NotRun**（第三者 MCP Host での受入は #46 G4 拡張・#114 系後続） |
| Caller 詐称の拒否 | `mcp_adapter.rs::connect_binds_record_identity_not_client_claims`・`wire_authority_flows_from_the_permit_record`・`connect_rejects_invalid_secrets_with_one_blind_verdict`・`tampered_secret_hash_fails_closed`・`issuance_rejects_channel_kind_mismatch`・GT-002/SEC-02・`stdio_e2e.rs`（clientInfo 名偽装を試行） | Headless Verified: Pass（mock/L2 + real/L4）。Product: 実 Host 認証は NotRun |
| 接続・Grant 取消 | `mcp_adapter.rs::revoked_grant_denies_the_very_next_call`・`gw10_transport.rs`（control channel `synvelis.revoke-grant`→直後の client call が `denied`）・`permit_cli revoke`→`PERMIT_INVALID` | Headless Verified: Pass（real/L4 含む）。Product: 実 UI からの取消体験 NotRun |
| 別 Project | SEC-04（foreign project boundary・existence-blind）・`operation_handles_are_scoped_and_blind`・`foreign_and_unknown_operations_deny_identically` | Headless Verified: Pass（mock/L2 + real/L4 の JOB-08）。Product: NotRun |
| UI Selection 変更 | SEC-07・`sys03_approval_binds_proposal_not_ambient_selection`・`sec07_selection_changes_do_not_retarget_the_grant` | Headless Verified: Pass（mock/L2）。Product: 実 GUI Selection 変更との相互作用 NotRun（#114） |
| 外部情報開示境界 | DATA-01〜10・MCP-06（bounded media/range/page）・`rejects_oversized_ranges_pages_and_payloads`・`result_disclosure_requires_the_disclosure_class`・DATA-08 real/L4（300KB inbound refuse） | Headless Verified: Pass（mock+real/L2-L4）。実大容量 media 転送の製品受入 NotRun |
| MCP Tool Discovery/Schema は権限の代替でない | `data09_tool_discovery_does_not_extend_the_grant`・`discovery_requires_a_bound_session`・`catalog_is_channel_scoped`・GT-003（Capability 存在≠利用権限）・`control_methods_are_blind_on_the_client_channel`・`tools/list` 未 bind で `PERMIT_REQUIRED`（real/L4） | Headless Verified: Pass（mock/L2 + real/L4）。**各 Invocation で GW-02 へ再照会する設計を実測で確認** |
| AI Runtime と External MCP Access の分離 | `gateway_ui.rs::AiSwitches`/`AiFeature`（AI/MCP 既定 OFF の headless contract）・`McpConfig.enabled`・`disabled_adapter_denies_the_surface`・E2E-1/SYS-01（AI Off で通常 DAW pipeline 成立）・UX-01 | Headless Verified: Pass（mock/L2-L3）。製品設定面の分離 UX は #114 NotRun |
| 切断→再接続→Operation 照合/Idempotent Retry | `mcp-poc` serve kill→fresh serve→`operation.get` が durable journal から `committed` を reconcile（E2E-2-restart・SYS-09 real/L4）・`proposal.apply` retry が同一 `journal_seq`（REC-06/MCP-05）・`settle_op_reports_committed_after_restart_scoped_reconcile`・`commit_failure_reports_the_reconciled_verdict_not_success` | Headless Verified: Pass（real process/L4 + mock/L2）。二重 Commit なしを実測 |
| 通常 DAW と AI Off の独立 | E2E-1（`e2e1_ai_off_human_pipeline_commits_undoes_and_recovers` mock/L3）・SYS-01・AC-10・GT-012 | Headless Verified: Pass（headless のみ。実 GUI DAW 操作の非干渉は #114/実機 lane NotRun） |

**G4 判定**: Headless Verified = **Pass**（G4=PASS —
real_only row の E2E-2-* 4 件が `mode=real`/L4 で PASS。
ただし real transport の client は自前 `mcp-poc` harness で
あり第三者 MCP Host ではない）。Product Accepted =
**NotRun**（第三者 MCP Client・実 Host 互換・認証 IPC の
製品化・Transport 最終決定は #46 G4 拡張・#114・別設計
待ち）。Transport は **Local stdio Helper + 認証 Local IPC
第一候補・未確定** を維持。

### G5 / Async & Generated Media Gate

**Issue #46 受入条件**: 「必要な生成・分析Job経路を、非RT
Job Owner、Stage/Verified Media、Asset Provenance、Project
Commitの区別で検証。具体モデル／Provider／生成方式は別設計
で決定。G4の初期3 Modeを不必要にブロックしない。」+「Job
OwnerとWindow/MCP接続の寿命分離、開始/Read/External
Transfer/Result/Commitの別認可、Grant Revoke時Cancel
Requestedと停止確認の区別。」「Job Completed≠Media
Verified≠Project Committed。Verified Media/Asset/Source・
Hash/Source Revision・不変Rawを既存#17/#21基盤で検証。」
+「AI Operation/JobのProvenanceと既存Asset/Source来歴の参照、
Provider/Model/Rightsの根拠とUnknown、Privacy/Retention境界
を検証。」「高度Audio生成Providerの完成を初期MIDI/MCP三Mode
の合格条件にしない。」

| 受入項目 | 照合先・実行証拠 | 判定 |
|---|---|---|
| 非 RT Job Owner との分離 | `src/{job,job_engine,media,provenance}.rs`・GW-07 は既存 Job Coordinator への Adapter（第二 Job Executor 非新設・IR `gw-07-*`）・`synvelis-render`/既存 #21 基盤 | Headless Verified: Pass（mock/L2-L3）。実 Window Close での Job 存続は #114 NotRun |
| Window/MCP 接続寿命との分離 | `gw10_job.rs::job02_disconnect_leaves_the_job_book_intact`・`job.rs::ids_stay_distinct_and_jobs_survive_disconnect`・`pending_cancel_upgrades_to_confirmed_on_poll` | Headless Verified: Pass（mock/L2-L3）。実 Window/MCP 同時切断の製品検証 NotRun |
| 開始/Read/External Transfer/Result/Commit の別認可 | `job.rs::submit_denies_out_of_scope_inputs_and_missing_transfer_class`・`result_disclosure_requires_the_disclosure_class`・`data10_external_transfer_needs_its_class`・`unknown_or_foreign_media_denies_identically`・JOB-01/08 | Headless Verified: Pass（mock/L2 + JOB-08 real/L4）。Product: NotRun |
| Grant Revoke 時 Cancel Requested と停止確認の区別 | `job03_revocation_denies_calls_and_requests_cancel`・`job04_unreachable_provider_reports_unconfirmed_never_stopped`・`job09_cancel_verdicts_reflect_actual_ordering`・`cancel_verdicts_are_honest`・JM-02・UX-04 | Headless Verified: Pass（mock/L2）。「Cancelled/Unconfirmed」の実 Provider 応答は Stub/Provider 未接続のため NotRun |
| Job Completed≠Media Verified≠Project Committed | `job05_unverified_output_never_registers`・`gtv002_observed_states_stay_distinct_across_the_lifecycle`・`sys05_sys06_state_nonconflation_is_observable`・`verified_media_commits_through_gate`・JM-03/04・GTV-002 | Headless Verified: Pass（mock/L2-L3） |
| Verified Media/Asset/Source・Hash/Source Revision・不変 Raw（#17/#21 基盤） | `media.rs`/`provenance.rs`・`verified_media_lowers_to_register_source_with_gate_digest`・`batch_media_register_uses_the_gate`・`repeated_get_materializes_once`・REC-11（crash after apply→verified source 残存・clip なし）・E2E-3・`job10_verified_output_then_drifted_base_conflicts`（Apply 拒否→Project 未変更） | Headless Verified: Pass（mock/L2-L3）。実 Media ファイル/実 Provider 出力の Verified Media 昇格は NotRun |
| AI Operation/Job Provenance・Model/Rights Unknown・Privacy/Retention | `job07_absent_claims_stay_unknown_in_provenance`・`provenance_levels_and_secret_bound`（Grant 資格情報/Token/Prompt 全文を素材 Provenance に埋め込まない）・JM-05・DATA-06（prompt injection）・SYS-10（error に機密を流さない） | Headless Verified: Pass（mock/L2）。実 Provider の Rights/Retention 契約は別設計・NotRun |
| 高度 Audio 生成 Provider を初期 3 Mode の合格条件にしない | E2E-3 は Job→Staging/Verification/Provenance→Proposal→Commit を mock/L3 で実証。Provider は Stub/既存 render 系で契約検証可能 | Headless Verified: Pass（G5=PASS・E2E-3）。実生成 Provider 接続は別設計で決定・NotRun（G4 を Block していないことを確認） |

**G5 判定**: Headless Verified = **Pass**（G5=PASS —
JOB-01〜10・E2E-3・JM-01〜05 全 Pass、JOB-08 は real/L4
record 含む）。Product Accepted = **NotRun**（実 Provider・
実生成 Media・実 Window/接続寿命の製品検証は後続。
Provider/Model/Rights の実契約は別設計待ち）。

### G6 / Integration & Hardening Gate

**Issue #46 受入条件**: 「再起動/永続Grant/Policy Recovery、
Scope大量オブジェクト負荷、Request Retry、Cancel、
Transport/RT非干渉、互換Regression、監査・プライバシー
動作を検証。」+ G6 詳細: 「固定/動的Grant Scope、Task/
ActivationとApplyモード分離、Approval Digest/Effect/
Revisionの提示、複数Windowでの対象固定。」「Revoke
Requested/Confirmed、Job Cancel Requested/Cancelled/
Unconfirmed、Committed/Engine Applied/Savedの状態誤表示
がない。」「Prompt Injection混入Metadata、Resource内容漏
えい、Toolの権限外実行、ID偽装、別Project、未許可Media/
External Transferを拒否。」「Concurrent Revoke/Commit、
Approval改竄/再利用、Retry/Restart/Crash Recovery、
Transport/RT非干渉、通常DAW回帰、Ruviform他Consumer互換を
実測・記録。」

| 受入項目 | 照合先・実行証拠 | 判定 |
|---|---|---|
| 再起動 | `mcp-poc` serve kill→fresh serve→durable journal reconcile（E2E-2-restart・SYS-09 real/L4）・`commit_gate.rs::restart_*`・`settle_op_reports_committed_after_restart_scoped_reconcile` | Headless Verified: Pass（real process/L4 + mock/L2）。OS 再起動/電源断は Unsupported（`fault::POWER_LOSS`） |
| 永続 Grant / Policy Recovery | `src/durable_policy.rs::FilePolicyStore`（log replay で policy 復元）・`commit_gate.rs::file_policy_store_replays_lifecycle`・`durable_revoke_survives_restart`・`durable_policy_admin_never_double_counts_consumption`・REC-08（confirmed revoke は torn tail を越えて残る）・REC-12（restart 後 stale proposal 不 commit） | Headless Verified: Pass（component/L2・real file。FilePolicyStore の replay を実測）。**限定**: PoC `serve` の policy store は in-memory であり、製品配線での Grant 永続化は store 層証拠止まり — product 接続は残件 |
| Policy Recovery | REC-08/09・`crash_after_durable_record_recovers_committed`・journal torn-tail recovery | Headless Verified: Pass（mock/L2 + real journal） |
| Scope 大量オブジェクト負荷 | `context.rs` の page/range/selector/total bounded（`page_count_is_bounded`・`page_limit_is_bounded`・`total_items_are_bounded`・`range_window_is_bounded`・`selector_id_count_is_bounded`）・DATA-08（oversized 拒否・real/L4 300KB）・`McpBounds`/`ContextBudget`/`PrepareBudget` | Headless Verified: Pass（bounded 契約の実測）。**大規模 Scope の負荷試験（大量オブジェクトでの性能・耐久）は専用 harness なし — NotRun** |
| Request Retry | REC-04/05/06・AC-06/07・MCP-05・`keyed_commit_retry_replays_the_receipt`・PoC retry→同一 journal_seq | Headless Verified: Pass（mock/L2 + real/L4） |
| Cancel | JOB-03/04/09・`operation.cancel`・`cancel_verdicts_are_honest`・`GatePorts::cancel`→`gate.cancel_apply`（fenced） | Headless Verified: Pass（mock/L2）。実 Provider cancel 応答 NotRun |
| Transport/RT 非干渉 | `sys07_rt_boundary_is_recorded_not_claimed`（RT boundary を記録・budget 未実測）・SYS-07 row・GW-R-14（Gateway/MCP 層で RT work 禁止） | Headless Verified: **Unsupported**（SYS-07-rt は budget 未実測・required=false — 記録のみで PASS 宣言しない）。RT callback への Policy/Network/Mutation 非導入は設計境界として固定。実機 RT budget 実測は NotRun（実機 lane） |
| 互換 Regression（通常 DAW） | E2E-1・SYS-01・AC-10・GT-012（AI Off で編集/Undo/再生/Save/Reload 成立・headless） | Headless Verified: Pass（headless/L3）。実 GUI DAW 回帰は #114/実機 lane NotRun |
| Ruviform 他 Consumer 互換 | CONSUMER-RUVIFORM/CONSUMER-AUDIOLENS（required=false）・本 slice で Ruviform 変更なし・rf#294/#553 は条件付き | NotRun（条件未成立 — Ruviform 共通変更時のみ #294 両 Consumer 回帰が発動） |
| 監査・プライバシー動作 | `src/{journal,evidence,trace}.rs`（causal trace・policy lifecycle journal・EvidenceRecord schema）・DATA-06（prompt injection）・SYS-10（error 秘匿）・`provenance_levels_and_secret_bound`・GTV-006 | Headless Verified: Pass（mock/L1-L4）。実監査 UI/Privacy 設定の製品受入 NotRun |
| 固定/動的 Grant Scope・Task/Activation と Apply モード分離 | SEC-06/07・`ahg009_apply_mode_comes_from_grant_not_request`・`ahg007_*`（permit/activation 系）・UX-01/02（headless contract） | Headless Verified: Pass（mock/L2）。実 UI 表示 NotRun（#114） |
| Approval Digest/Effect/Revision の提示・複数 Window 対象固定 | `gateway_ui.rs::ApprovalSurface`/`explain_grant`/`capture_scope`（headless contract）・`sys03_approval_binds_proposal_not_ambient_selection`・SEC-07 | Headless Verified: Pass(部分)（headless contract のみ）。実 Window 複数表示の対象固定は SYS-02-ux/03-ux = **NotRun**（#114） |
| Revoke/Cancel/状態の誤表示なし | `sys05_sys06_state_nonconflation_is_observable`・`gtv002_*`・`job04_*`/`job09_*`・UX-04/05（headless） | Headless Verified: Pass（mock/L2-L3）。実 UI 誤表示は NotRun |
| Prompt Injection/Resource 漏えい/Tool 権限外/ID 偽装/別 Project/未許可 Media・External Transfer 拒否 | DATA-06/07/10・SEC-01/02/04/13・JOB-08・MCP-02/04・`boundary_attacks_fail_closed_real`（real/L4） | Headless Verified: Pass（mock+real/L2-L4）。実 adversarial 環境での再検証は後続 |
| Concurrent Revoke/Commit・Approval 改竄/再利用・Retry/Restart/Crash Recovery | REC-01〜12・AC-01〜10・SEC-09〜12・GTV-005（deterministic ordering） | Headless Verified: Pass（mock/L2 + real/L4）。実 OS レベルの同時実行ストレスは NotRun |

**G6 判定**: Headless Verified = **Pass**（G6=PASS —
required case 全 Pass。SYS-07-rt=Unsupported・SYS-02-ux/
03-ux/05-powerloss/CONSUMER-*=NotRun は全て required=false
で gate を保持しない）。Product Accepted = **NotRun**
（実 GUI UX・実 OS 再起動/電源断・実機 RT budget・大規模
負荷・実監査 UI・Consumer 回帰は #114/#83/実機 lane/
rf#294 条件付きの後続）。

## Family 写像（Issue #46 case family 一覧）

Issue #46 が要求する全 case family の Gate 帰属と 2 軸
status の一覧。case 単位の fixture/oracle/Owner/mode/layer/
status の詳細正本は `evidence/sv45/command-gateway-case-
ledger.md`（本表は family→Gate→status の照合に限定）。

| Family | Case | Gate | 主な fixture/canon | Headless Verified | Product Accepted |
|---|---|---|---|---|---|
| GT | GT-001〜012 | G0/G1/G2/G3/G6 | `contract.rs`・`gw10_{contract,authorization,project}.rs`・`prepare*.rs`/`proposal*.rs`・GT-011/012 は SYS-01/E2E-1 requirements 経由 | Pass（mock/L1-L3） | NotRun（#114/実機 lane・merge 後 CI 証拠） |
| JM | JM-01〜05 | G5 | `gw10_job.rs::job02`〜`job10`・`job.rs`/`media.rs`/`provenance.rs` | Pass（mock/L2-L3） | NotRun（実 Provider・実 Window Close・#114） |
| MCP | MCP-01〜06 | G4（+G1/G3 部分） | `mcp_adapter.rs`・`gw10_mcp.rs::gw10f_*`・`mcp-poc/tests/{gw10_transport,stdio_e2e}.rs` | Pass（mock/L2 + PoC real/L4） | NotRun（第三者 MCP Host・実認証） |
| UX | UX-01〜06 | G6 | `synvelis-workspace/src/gateway_ui.rs`（headless contract）・`gw10_system.rs::sys02`/`sys03`/`sys05_sys06` | Pass（部分 headless） | NotRun（実 GPUI・#114） |
| SEC | SEC-01〜14 | G1（+G2/G3 部分） | `authorization.rs`（SEC 系）・`gw10_authorization.rs::sec01`〜`sec14`・`contract.rs::gt002_*` | Pass（mock/L1-L3） | NotRun |
| DATA | DATA-01〜10 | G1 | `gw10_authorization.rs::data01`〜`data10`・`context.rs` bounded/pin/cursor 系 | Pass（mock+real/L2-L4） | NotRun（実製品 Disclosure 体験） |
| REC | REC-01〜12 | G3（+GTV-005） | `gw10_concurrency.rs::rec01`〜`rec06`・`gw10_persistence.rs::rec07`〜`rec12`・`commit_gate.rs` crash/torn/durable/restart 系 | Pass（mock/L2 + real journal/L4） | NotRun（電源断・実機 lane） |
| JOB | JOB-01〜10 | G5 | `gw10_job.rs::job02`〜`job10`・`job.rs`（submit/cancel/disclosure/provenance） | Pass（mock/L2-L3 + JOB-08 real/L4） | NotRun（実 Provider） |
| SYS | SYS-01〜10 | G6 | `gw10_system.rs::*`・`gw10_persistence.rs::sys04_*`・SYS-09 は real-only | Pass（mock+real/L1-L4） | NotRun（実 GUI/OS lane） |
| SYS-02-ux/03-ux/05-powerloss/07-rt | （optional row） | G6 | `sys07_rt_boundary_is_recorded_not_claimed`（記録のみ）・SYS-05 は `fault::POWER_LOSS` 非対応 | NotRun/Unsupported（required=false） | NotRun（#114・実機 lane・#83） |
| CONSUMER | CONSUMER-RUVIFORM/AUDIOLENS | G6 | （harness row・条件付き） | NotRun（required=false・条件未成立） | NotRun（Ruviform 変更時のみ #294・#553） |
| AC | AC-01〜10 | G3 | `gw10_{project,concurrency,persistence}.rs`・`commit_gate.rs`（commit stage evaluation/revalidation 系） | Pass（mock/L2 + real journal） | NotRun |
| E2E | E2E-1 | G6 | `gw10_system.rs::e2e1_ai_off_human_pipeline_commits_undoes_and_recovers` | Pass（mock/L3） | NotRun（実 GUI DAW・#114） |
| E2E | E2E-2-suggest/approve/delegated/restart | G4 | `mcp-poc/tests/gw10_transport.rs::gw10f_{three_modes_over_real_transports,permit_cli_lifecycle_real,delegated_apply_commits_and_survives_restart_real,boundary_attacks_fail_closed_real}` | Pass（PoC real/L4） | NotRun（第三者 MCP Host） |
| E2E | E2E-3 | G5 | `gw10_job.rs::gtv004_e2e3_verified_media_commits_with_provenance` | Pass（mock/L3） | NotRun（実 Provider） |
| GTV | GTV-001〜008 | GTV | `gw10_harness.rs`・`src/{evidence,oracle,schedule,trace}.rs` | Pass（mock/L1-L4） | — |

## 横断記録事項（Issue #46 個別要求）

| 要求 | 台帳記録 |
|---|---|
| 切断→再接続→Operation 照合/Idempotent Retry | **実測済み（PoC real/L4 + mock/L2）**: `mcp-poc` serve kill→fresh serve→`operation.get` が `commit.journal` から `committed` を reconcile（E2E-2-restart・SYS-09）。`proposal.apply` retry は同一 `journal_seq` を返し二重 Commit なし（REC-06・MCP-05・AC-07）。`settle_op_reports_committed_after_restart_scoped_reconcile`・`commit_failure_reports_the_reconciled_verdict_not_success` が照合 verdict を偽装しないことを実測。実 Host 切断（ネットワーク断・第三者 Client 再接続）は NotRun |
| MCP Tool Discovery/Schema が権限の代替でない | **設計・実測で確認**: `tools/list` は bound session 必須（`discovery_requires_a_bound_session`・real/L4 で `PERMIT_REQUIRED`）、catalog は channel scoped、各 Invocation は GW-02 へ再照会（DATA-09・GT-003・SEC-13）。Tool/Resource 公開は権限証明ではない（GW-R-10） |
| AI Runtime と External MCP Access の分離 | **headless contract で固定**: `gateway_ui.rs::AiSwitches`/`AiFeature` が AI/MCP を独立 feature として既定 OFF 管理、`McpConfig.enabled` が adapter surface を deny-by-default で閉じ、E2E-1/SYS-01/AC-10 が AI Off での通常 DAW pipeline（commit/undo/recover）成立を実測。Synvelis 管理 AI Runtime と外部 MCP 接続は別設定・別状態（GW-R-11/UX-01）。実製品設定 UI は #114 NotRun |
| Transport 選択 | **Local stdio Helper + 認証 Local IPC が第一候補・未確定**。実測: stdio（newline-delimited JSON-RPC・MCP host spawn helper）+ `127.0.0.1` loopback 認証 control channel（operator 専用・agent 非到達）。Localhost Streamable HTTP は比較候補として残るが未実装・未比較。接続 Host 互換・認証・攻撃面の比較 PoC は未完 — Issue #46 の「まだ最終決定しない」を維持し、台帳でも確定扱いしない |

## AUTX Consumer（#259〜267）接合点

Issue #46 の 2026-11-10 追記に従い、AUTX を GW への新
Consumer 機能として記録する。**独立 Macro Engine・別
Commit Gate は Gate 判定に含めない**。

| AUTX Issue | GW 接合点 | 現状 |
|---|---|---|
| #259 AUTX-00 設計 | 自動化層の最新 Owner/DAG | OPEN（設計） |
| #260 AUTX-01 Action Catalog | `CapabilityRef`/`CapabilityDescriptor` の Presentation 投影（別 Action Registry/Executor にしない・GW-R-13） | OPEN |
| #261 AUTX-02 CLI Trusted Local Adapter | `AdapterSeal` 付き信頼境界・`PrincipalKind::Human` 偽造禁止（CLI local issuer は Adapter） | OPEN |
| #262 AUTX-03 独立 CLI | Native/WebView 非依存の独立 Adapter | OPEN |
| #263 AUTX-04 Atomic Macro | 既存 GW-04 `synvelis.operation.batch`→`ProjectOp::Composite`（`ops.rs`/`prepare_engine.rs` に実装済み）→GW-05/06 Commit で一度だけ CAS/Undo。**ID Binding 不足のみ GW-04 の既存変更として特定済み** | OPEN・batch/Composite は main に存在 |
| #264 AUTX-05 Workflow DSL | Gateway 外側の非 RT Step・Plan/Resume cursor。GW-02 Grant・GW-05 Approval・GW-06 ProjectWriter/Journal・GW-07 Provider Job Truth を再実装しない | OPEN |
| #265 AUTX-06 Workflow Coordinator | #264 Versioned Plan 確定まで Start Block | OPEN |
| #266 AUTX-07 GPUI UX | #114 系 GUI lane | OPEN |
| #267 AUTX-08 全経路 E2E | MCP/CLI/GPUI の同一 Capability と独立 Policy/Commit oracle。#267 と GW-10 Security/Recovery は Case ID・対象 SHA/実 OS/NotRun を独立判定 | OPEN |

- `synvelis.operation.batch`（`prepare_engine.rs`）と
  `ProjectOp::Composite`（`synvelis-project/src/ops.rs`・
  nested Composite 拒否・CAS/Undo は既存 Commit 経路）は
  **既に main に存在する GW-04 の一部**であり、AUTX の
  Atomic Macro が再利用する接合点。新しい Macro Engine・
  別 Commit Gate・第二 Writer を AUTX 側に要求しない。
- CLI の mutating invocation は #261 Auth/Host の安全 Gate
  が満たされるまで公開しない（Issue #46 明記）。
- Ruviform #553 は Generic 抽出の評価のみで、Synvelis
  Gateway 型の移植を要求しない。

## 未達残件と後続 Owner

| 残件 | 状態 | Owner/後続 |
|---|---|---|
| 実 GUI 承認 UX（SYS-02-ux/03-ux・UX-01〜06 実画面） | NotRun | #114（GPUI Native Product E2E） |
| 第三者 MCP Host での G4 受入・実 Host 互換 | NotRun | #46 G4 拡張・#114 系・Transport 比較 PoC |
| Transport 最終決定（Streamable HTTP 比較） | 未確定 | #46・別設計 |
| 実 Provider・実生成 Media・Provider/Model/Rights 実契約 | NotRun | 別設計（#46 G5 注記・#21 系） |
| 電源断 durability（SYS-05-powerloss） | Unsupported/NotRun | 実機 lane・#83 |
| OS 別 RT budget 実測（SYS-07-rt） | Unsupported | 実機 lane（budget 未実測のまま Pass にしない） |
| 大規模 Scope 負荷試験 | NotRun | #46 G6 残件・専用 harness 未整備 |
| Grant の product 配線での永続化（FilePolicyStore→serve 統合） | 部分証拠 | #46 G6 残件（store 層は実測済み） |
| Ruviform/AudioLens Consumer 回帰 | NotRun（条件付き） | Ruviform 変更時のみ #294・#553 評価 |
| AUTX #259〜267 の GW 接合 | 未着手（設計 OPEN） | AUTX 系 Issue（本台帳の Gate 判定対象外） |
| 独立 CI での gw10 再現 | main に `mise run gw10` 系が存在（#83 系）。merge 後 main SHA の必須 CI 結果が統合検証証拠 | Coordinator/CI |

## 集計

| Gate | Headless Verified | Product Accepted | 主な証拠 |
|---|---|---|---|
| G0 Contract | **Pass** | NotRun（#114/実機 lane） | GT-001〜003・SYS-08・G0=PASS |
| G1 Read & Policy | **Pass** | NotRun | SEC-01〜14・DATA-01〜10・GT-004・G1=PASS |
| G2 Proposal | **Pass** | NotRun（#114） | GT-005〜009・proposal/prepare 系・G2=PASS |
| G3 Authorized Commit | **Pass** | NotRun（#114） | 3 Mode（mock+PoC real/L4）・REC-01〜12・AC-01〜10・G3=PASS |
| G4 MCP E2E | **Pass**（PoC real/L4・第三者 Host ではない） | **NotRun** | E2E-2-*（real）・MCP-01〜06・DATA-09・G4=PASS |
| G5 Job/Media/Provenance | **Pass** | NotRun（実 Provider 別設計） | JOB-01〜10・JM-01〜05・E2E-3・G5=PASS |
| G6 Integration/Hardening | **Pass**（required case） | **NotRun** | E2E-1・SYS-01〜10・REC/AC・G6=PASS（SYS-07-rt Unsupported・5 件 NotRun は required=false） |
| GTV | **Pass** | — | GTV-001〜008・harness self-verification |

**総合**: G0〜G6 の headless/mock/harness/PoC real
transport レベルの受入は全 Gate で **Pass**（strict
report・required case）。`Product Accepted` は全 Gate で
**NotRun** — 実 GUI・第三者 MCP Host・実 Provider・実 OS
lane・電源断・大規模負荷・Consumer 回帰は後続 Owner
（#114・#83・#46 G4/G6 拡張・実機 lane・rf#294 条件付き）
に残る。本台帳は Issue #46 を Close するものではなく、
G0〜G6 の headless 受入照合結果と Product Accepted 未達
残件を固定する証拠である。
