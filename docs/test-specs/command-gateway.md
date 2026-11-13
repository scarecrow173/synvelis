---
id: "TSPEC-034X8sKuWkm8UBzSfXcpb1"
type: "test-spec"
status: "approved"
title: "Capability / Command Gateway 設計契約 検証仕様 (synvelis#45)"
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
    - "docs/designs/command-gateway-design.md"
    - "docs/specs/command-gateway.md"
    - "docs/test-specs/media-render.md"
    - "docs/test-specs/playback-graph.md"
    - "docs/test-specs/workspace-model.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/command-gateway-design.md"
  derived-by: []
  verifies:
    - "docs/designs/command-gateway-design.md"
    - "docs/specs/command-gateway.md"
  verified-by: []
  references:
    - "evidence/sv45/command-gateway-case-ledger.md"
    - "evidence/sv45/command-gateway-verification.md"
  defers: []
  deferred-by: []
---

# Capability / Command Gateway 設計契約 検証仕様 (synvelis#45)

## Purpose

Issue #45 の GT-001〜012・JM-01〜05・MCP-01〜06・UX-01〜06・
SEC/DATA/REC/JOB/SYS・AC-01〜10・E2E-1/2/3・G0〜G6・GTV-001〜008
の受入ケースについて、「何を・どの fixture/oracle で・どの
mode/layer/status で検証するか」を固定する。本 slice は
docs/ 設計契約 + evidence 台帳 + headless 回帰実測のみ。
実装・GUI・実 device/plugin/OS/Provider 受入は対象外。

## Feature

Capability / Command Gateway の設計契約が、GW-R-01〜15 と
GW-AC-01〜12 を満たし、全受入ケースが既存 fixture/Issue/
Owner/evidence へ一貫して写像されること。

## Rules

### GW-T-R-01 要件網羅

- spec の GW-R-01〜15 全てが design・plan・test-spec・
  evidence で参照される。
- GW-AC-01〜12 が acceptance criteria として spec に存在し、
  evidence ledger で被覆状況が記録される。

### GW-T-R-02 ケース写像

- GT-001〜012・JM-01〜05・MCP-01〜06・UX-01〜06・SEC-01〜14・
  DATA-01〜10・REC-01〜12・JOB-01〜10・SYS-01〜10・AC-01〜10・
  E2E-1/2/3・G0〜G6・GTV-001〜008 の全 case が case ledger に
  存在し、fixture・oracle・Owner・mode/layer・status・証拠を
  持つ。
- 各 case の status は `Pass`/`Fail`/`NotRun`/`Unsupported`
  のいずれかで、実行した検証と一致する。要求状態
  （Required/Deferred/NotApplicable）と実行結果は別軸。

### GW-T-R-03 Owner 境界

- GW-01〜10 の Owner（Capability/Authorization/Context/
  Prepare/Proposal/Commit/Job-Media-Provenance/MCP/UX/
  Verification）と実装 Issue（#53〜#59・#62・#83・#114）が
  design・plan・evidence で一貫する。
- Project Single Writer/Undo・Job Owner・Media Store・Semantic
  Diff・Transport Receipt の第二正本を Gateway が持たない。
- AUTX（#259〜#267）・CLI・MCP/CLI/GPUI adapter・Ruviform
  #553/#294 の境界が一貫する。

### GW-T-R-04 Evidence Honesty

- headless/mock/PoC real transport の Pass は mode/layer を
  明記し、実機/GUI/Provider/第三者 MCP Host 受入に繰り上げ
  ない。
- 未実装・未検証（SYS-02-ux/03-ux/05-powerloss・CONSUMER-*・
  実 GPUI・実外部 Provider）は `NotRun`/`Unsupported`。
- SYS-07-rt は budget 未実測として `Unsupported`。
- 設計レビュー Pass と実測 Pass を区別する。
- `Pass`/`Fail`/`NotRun`/`Unsupported` は実際に実行した
  検証のみを反映する。

### GW-T-R-05 横断契約

- #44（AIB/AHG）・#46（G0〜G6）・#52/#56（MIDI/Effect）・
  #57（semantic diff）・#58（revoke/commit）・#59（MCP
  binding）・#17（ProjectController）・#20（Transport）・
  #21（Job/Media）・#7（Semantic Diff）・#114（GPUI UX）・
  #83（CI 再現）・AUTX #259〜#267・ruviform #553/#294 の
  横断写像が case ledger に存在する。
- Trusted Authority 非自己申告・Capability≠権限・Read≠
  Disclosure・Proposal≠Apply・Synvelis-derived Effects・
  immutable ChangeSet・digest 意味束縛・receipt 束縛・
  revoke 後 commit 拒否・GW-01〜05 revision 非変更・GW-06
  Single Writer・AI-off DAW 維持が一貫して適用される。

### GW-T-R-06 文書鎖整合

- spec→design→plan→task→test-spec→impl-ir/exp→evidence の
  依存が front matter relations で一貫する。
- index が再生成され、lint/audit/route/fmt-check が実行・
  記録される。

## Case Map（概要 — 詳細は evidence/sv45/command-gateway-case-ledger.md）

Fixture 表記は `file::test_fn`（Rust integration test の関数名）
または `file`（module 単位）。gw10 evidence は
`target/gw10-evidence/*.jsonl` の verdict を正本とする。

| Family | Case | Fixture/Oracle | Owner | Status |
|---|---|---|---|---|
| GT | GT-001〜003 | `contract.rs::gt001_*`/`gt002_*`/`gt003_*`・`gw10_contract.rs::*`・oracle | #53・GW10-A/G0 | Pass(mock/L1-L2) |
| GT | GT-004 | `gw10_authorization.rs::gt004_*`・`context.rs::*` | #55・GW10-B/G1 | Pass(mock/L2) |
| GT | GT-005/006 | `gw10_project.rs::gt005_*`/`gt006_*`・`prepare*.rs::*` | #56・GW10-C/G2 | Pass(mock/L2) |
| GT | GT-007/008 | `gw10_project.rs::gt007_*`/`gt008_*`・`proposal*.rs::*` | #57・GW10-C/G2 | Pass(mock/L2) |
| GT | GT-009 | `gw10_authorization.rs::sec12_*`・`gw10_concurrency.rs::rec01_*`・`commit_gate.rs::*` | #58・GW10-D/G2-G3 | Pass(mock/L2) |
| GT | GT-010 | `gw10_project.rs::gt010_*`・`contract.rs::gt010_*` | #53〜#57・GW10-D/G3 | Pass(mock/L2) |
| GT | GT-011 | `gw10_system.rs`(SYS-01)・`gw10_project.rs::gw10c_*`・`commit_gate.rs::*` | #58・GW10-H/G6 | Pass(mock/L3) |
| GT | GT-012 | `gw10_system.rs::e2e1_*`(E2E-1) | #62・GW10-H/G6 | Pass(mock/L3)・実 GUI NotRun |
| JM | JM-01〜05 | `gw10_job.rs::job02`〜`job10`・`job.rs::*`・`media.rs`/`provenance.rs` | #21 接続・GW10-G/G5 | Pass(mock+一部 real/L2-L4)・実 Provider NotRun |
| MCP | MCP-01〜06 | `mcp_adapter.rs::*`・`gw10_mcp.rs::gw10f_*`・`mcp-poc/tests/gw10_transport.rs::gw10f_*_real`・`stdio_e2e.rs` | #59・GW10-F/G4 | Pass(PoC real transport/L4)・第三者 Host NotRun |
| UX | UX-01〜06 | `synvelis-workspace/src/gateway_ui.rs`・`gw10_system.rs::sys02`/`sys03`/`sys05_sys06` | #114・GW10-H/G6 | Pass(部分 headless)・実 GPUI NotRun |
| SEC | SEC-01〜14 | `gw10_authorization.rs::sec01`〜`sec14`・`contract.rs::gt002_*` | #54・GW10-B/G1 | Pass(mock/L1-L3) |
| DATA | DATA-01〜10 | `gw10_authorization.rs::data01`〜`data10`・`context.rs::*` | #55・GW10-B/G1 | Pass(mock+real/L2-L4) |
| REC | REC-01〜12 | `gw10_concurrency.rs::rec01`〜`rec06`・`gw10_persistence.rs::rec07`〜`rec12`・`commit_gate.rs::*` | #58・GW10-D/E・G3 | Pass(mock/L2) |
| JOB | JOB-01〜10 | `gw10_job.rs::job02`〜`job10`・`job.rs::*` | #21 接続・GW10-G/G5 | Pass(mock+real/L2-L4) |
| SYS | SYS-01〜10 | `gw10_system.rs::*`・`gw10_persistence.rs::sys04_*` | #62・GW10-H/G6 | Pass(mock+real/L1-L4) |
| SYS | SYS-07-rt | `sys07_rt_boundary_is_recorded_not_claimed`（記録のみ） | #62・GW10-H/G6 | **Unsupported**（budget 未実測・required=false） |
| SYS | SYS-02-ux/03-ux | （未実装 — #114 GPUI） | #114・GW10-H/G6 | **NotRun**（required=false） |
| SYS | SYS-05-powerloss | （harness 非対応 — `fault::POWER_LOSS`） | #62・GW10-H/G6 | **NotRun/Unsupported**（required=false） |
| CONSUMER | CONSUMER-RUVIFORM/AUDIOLENS | （Ruviform 変更なし・rf#294/#553 条件付き） | #28・rf#294・#553 | **NotRun**（required=false・条件未成立） |
| AC | AC-01〜10 | `gw10_concurrency.rs`/`gw10_persistence.rs`/`commit_gate.rs`（ACG-005 受入候補） | #58・GW10-D・G3 | Pass(mock/L2) |
| E2E | E2E-1 | `gw10_system.rs::e2e1_ai_off_human_pipeline_*` | #62・GW10-H/G6 | Pass(mock/L3) |
| E2E | E2E-2-suggest/approve/delegated/restart | `mcp-poc/tests/gw10_transport.rs::gw10f_*_real` | #59・GW10-F/G4 | Pass(real PoC transport/L4) |
| E2E | E2E-3 | `gw10_job.rs::gtv004_e2e3_*` | #21 接続・GW10-G/G5 | Pass(mock/L3) |
| GTV | GTV-001〜008 | `gw10_harness.rs::*`・`evidence.rs`/`oracle.rs`/`schedule.rs`/`trace.rs` | #62・GW10-*/GTV | Pass(mock/L1-L4) |
| Gate | G0〜G6 | `cargo run -p synvelis-gateway --bin gw10_report -- --strict` | #62・#83 | **PASS**（headless+PoC real・5 件 NotRun/Unsupported） |

## Examples

### EX-1: GT-002 Trusted Authority 非自己申告（Pass）

- **Given**: wire request が caller/authority 相当の field を
  申告しようとする。
- **When**: `contract.rs::gt002_authority_fields_rejected_on_wire`
  ・`gt002_caller_comes_from_adapter_not_wire`・`gw10_authorization.
  rs::sec02_wire_authority_fields_cannot_self_declare` を実行。
- **Then**: wire 型に authority field が存在せず未知 field は
  reject、`CallerIdentity` は adapter 発行 `AdapterSeal` を
  要求する。client claim は permit record ではなく binding
  record から導出される。
- **Status**: Pass — mock/L1-L2。実外部 Host の認証は #59/
  #114 の後続。

### EX-2: GT-009 Grant 失効後の未使用 Approval Commit 拒否
  （Pass）

- **Given**: Approval Receipt 発行済み・未消費の Proposal が
  存在し、その後 Grant が revoke される。
- **When**: `gw10_authorization.rs::sec12_revoked_grant_kills_
  the_unused_approval`・`gw10_concurrency.rs::rec01_revoke_
  first_denies_commit`・`commit_gate.rs::revoke_confirmed_
  before_commit_denies` を実行。
- **Then**: Commit Gate が最新 Grant/Policy Epoch を再評価し
  Commit を拒否する。Revoke 確定が先なら Commit 拒否、Commit
  確定が先なら既存 Mutation は有効（`rec02_commit_first_
  stands`）。
- **Status**: Pass — mock/L2。実 Store/電源断は #62/#83。

### EX-3: MCP-03 3 Mode real transport E2E（Pass・PoC）

- **Given**: mcp-poc の real transport（stdio helper + 認証
  付き loopback control channel）と permit/session binding。
- **When**: `mcp-poc/tests/gw10_transport.rs::gw10f_permit_
  cli_lifecycle_real`・`gw10f_delegated_apply_commits_and_
  survives_restart_real`・`gw10f_boundary_attacks_fail_closed_
  real`・`gw10f_three_modes_over_real_transports`・
  `stdio_e2e.rs` を実行。
- **Then**: Suggest（Agent Apply 不可）・Approve-to-Apply
  （Receipt 必須・消費）・Delegated Apply（Grant 内・budget
  bind）が real transport で成立し、restart を越えても
  二重 Commit しない。
- **Status**: Pass — real PoC transport/L4。第三者 MCP Host・
  実 Provider・実 GUI は #46 G4 拡張・#114 の後続で NotRun。

### EX-4: SYS-05-powerloss 電源断 durability（Unsupported）

- **Given**: `fault::POWER_LOSS` は harness で常に
  Unsupported として記録される設計。
- **When**: `gw10_report -- --strict` を実行。
- **Then**: SYS-05-powerloss・SYS-02-ux・SYS-03-ux・
  CONSUMER-RUVIFORM・CONSUMER-AUDIOLENS が NotRun/
  Unsupported として報告され、G0〜G6/GTV は PASS のまま
  （required=false row）。
- **Status**: Unsupported/NotRun — required=false。実機
  lane・#83・#114 の後続。

## Guarantees

- 全受入 case が case ledger に存在し、status が実行した
  検証と一致する。
- headless/mock/PoC real transport の Pass が実機/GUI/
  Provider/第三者 Host 受入に繰り上げられない。
- 未実装・未検証領域（SYS-02-ux/03-ux/05-powerloss・CONSUMER-*・
  実 GPUI・実外部 Provider・SYS-07-rt budget）が `NotRun`/
  `Unsupported` として明示される。
- GW-01〜10 Owner・状態機械・Failure Matrix・AUTX/CLI/adapter
  境界・RT boundary が一貫する。
- #44/#46/#52/#56/#57/#58/#59/#17/#20/#21/#7/#114/#83・
  AUTX #259〜#267・ruviform #553/#294 の横断写像が存在する。

## Non-goals

- 実装コードの作成・変更。
- GUI・実 device・実 plugin・実 OS・実 Provider・第三者 MCP
  Host での受入。
- AUTX 文書・sv#44 文書の編集（並行 Worker・参照のみ）。
- Caller 自己申告の Trusted Authority 昇格。
- 実装・実機・実 Provider の PASS 宣言。

## Risk

- **R-TS-1 status の誤分類**: headless/mock Pass を `Pass`
  （実環境）と記録すると実機受入と混同される。→ mode/layer/
  required/real_only を明記し、実機/GUI/Provider は別
  status/後続 Issue として明示。
- **R-TS-2 fixture の陳腐化**: 参照する fixture/test 名が
  実装と乖離する。→ case ledger に fixture `file::test_fn`
  を明記し、lint/audit で link を検証。
- **R-TS-3 横断契約の漏れ**: #44/#46・AUTX・rf#553/#294 の
  写像が漏れる。→ GW-T-R-05 で必須化。
- **R-TS-4 evidence JSONL の取り違え**: `target/gw10-
  evidence/*.jsonl` は実行毎に増え、別 process の record と
  混在しうる。→ verdict は `gw10_report -- --strict` の
  集約を正本とし、ledger には集約結果を写像する。
