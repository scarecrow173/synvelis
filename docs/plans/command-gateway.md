---
id: "PLAN-034X8sL2niGTZok1BMGuzN"
type: "plan"
status: "approved"
title: "Capability / Command Gateway 設計契約 実施計画 (synvelis#45)"
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
  implements:
    - "docs/specs/command-gateway.md"
  implemented-by: []
  depends-on:
    - "docs/designs/command-gateway-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/command-gateway-design.md"
  derived-by:
    - "docs/tasks/command-gateway-task.md"
  verifies: []
  verified-by:
    - "docs/test-specs/command-gateway.md"
  references: []
  defers: []
  deferred-by: []
---

# Capability / Command Gateway — 実施計画 (synvelis#45)

本計画は `docs/specs/command-gateway.md` + `docs/designs/
command-gateway-design.md` の実施分解。**本 slice は docs/
設計契約 + evidence 台帳のみ**であり、実装は #46 配下の各実装
Issue が Owner。本 plan は「何をどの Issue/Fixture/Gate で
満たすか」の対応表と、未検証領域の後続 Issue への委譲を固定
する。

## Goal

Issue #45 の受入条件（GT-001〜012・JM-01〜05・MCP-01〜06・
UX-01〜06）と GW-01〜10 の型/状態遷移/Failure Matrix・
G0〜G6 Gate を、既存 Synvelis fixture・Issue・Receipt・
`crates/synvelis-gateway`/`crates/mcp-poc`/`gateway_ui.rs`
の実体へ写像し、Owner 境界と evidence 語彙を固定する。
設計完了を実装完了・実機/GUI/Provider 受入とみなさない。

## Scope

### In Scope（本 slice）

- 7 文書鎖（spec/design/plan/task/test-spec/impl-ir/impl-exp）
  + `evidence/sv45/`（case ledger + verification）
- GT-001〜012・JM-01〜05・MCP-01〜06・UX-01〜06 の fixture/
  Issue/Owner/status 写像
- SEC-01〜14・DATA-01〜10・REC-01〜12・JOB-01〜10・SYS-01〜10・
  AC-01〜10・E2E-1/2/3・G0〜G6・GTV-001〜008 の写像
- GW-01〜10 Owner 表・状態機械・Failure Matrix・AUTX/CLI/
  adapter 境界・RT boundary の固定
- index 再生成・docs 検証・review-protocol・headless 回帰
  実測（`mise run gw10`/`gw10-report`/`check`/`test
  synvelis-gateway`）・PR

### Out of Scope

- 実装コード（`crates/`・`Cargo.toml`・`Cargo.lock`・`mise.toml`・
  CI・Ruviform）
- GUI 実装・実 device/plugin・実 OS 受入（#114・#83）
- 実外部 MCP Host/Provider の Authenticated Session（#46 G4
  拡張・後続試験）
- AUTX 文書編集（#259〜#267）・sv#44 文書編集（並行 Worker）
- Caller 自己申告の Trusted Authority 昇格
- 実装・実機・実 Provider の PASS 宣言

## Upstream

- `docs/specs/command-gateway.md`（GW-R-01〜15・GW-AC-01〜12）
- `docs/designs/command-gateway-design.md`（Owner 表・contract・
  state machine・failure taxonomy・写像・risks）
- Issue #45 本文（v0.3〜v0.8・GT/JM/MCP/UX・検証方法・範囲）
- Issue #44（AIB/AHG 不変条件）・Issue #46（G0〜G6 実装受入）
- 確定済み縦断契約: vs-a/project-model/project-persistence/
  playback-graph/transport-integration/musical-time/media-
  render/workspace-model/vcs-02/recording-take-comping/
  midi-source-note-editing・gw-01/gw-02 design・
  target-architecture(-design)・time-transport architecture・
  media-import-export architecture

## File Map

| File | 種別 | 状態 |
|---|---|---|
| `docs/specs/command-gateway.md` | spec | 本 slice で作成 |
| `docs/designs/command-gateway-design.md` | design | 本 slice で作成 |
| `docs/plans/command-gateway.md` | plan | 本 slice で作成 |
| `docs/tasks/command-gateway-task.md` | task | 本 slice で作成 |
| `docs/test-specs/command-gateway.md` | test-spec | 本 slice で作成 |
| `docs/impl/ir/command-gateway.md` | impl record | 本 slice で作成 |
| `docs/impl/exp/command-gateway.md` | experiment record | 本 slice で作成 |
| `evidence/sv45/command-gateway-case-ledger.md` | evidence | 本 slice で作成 |
| `evidence/sv45/command-gateway-verification.md` | evidence | 本 slice で作成 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | index | 再生成のみ |

## Implementation Streams（Owner 別）

本 slice は実装を行わない。以下は「どの実装 Issue/Gate が
どの case family を満たすか」の対応表。実装状態は
`evidence/sv45/command-gateway-case-ledger.md` を正本とする。

### Stream A: GW-01〜05 Contract/Policy/Context/Prepare/Proposal（#53〜#57・G0〜G2）

| Family | Owner Issue | 現状 | 後続 |
|---|---|---|---|
| GT-001〜003・SEC-01〜14・DATA-01〜10 | #53/#54/#55 | headless Pass（`contract.rs`/`authorization.rs`/`context.rs`/`gw10_*`） | 実環境・#83 CI |
| GT-004〜009・AC-05 系 | #55/#56/#57 | headless Pass（`prepare*.rs`/`proposal*.rs`/`gw10_project.rs`） | 実機・#62 |
| GT-010 | #53〜#57 | headless Pass（`gt010_*`） | — |

### Stream B: GW-06 Commit/Recovery（#58・G3）

| Family | Owner Issue | 現状 | 後続 |
|---|---|---|---|
| GT-009・REC-01〜12・AC-01〜10 | #58・#62 | headless Pass（`commit_gate.rs`/`gw10_concurrency.rs`/`gw10_persistence.rs`） | 実 Store/電源断・#62・#83 |
| SYS-04・SYS-09 | #58・#62 | headless+process-restart Pass | 実機 fault・#83 |

### Stream C: GW-07〜09 Job/Media/MCP/UX（#59・#114・G4〜G6）

| Family | Owner Issue | 現状 | 後続 |
|---|---|---|---|
| JM-01〜05・JOB-01〜10・E2E-3 | #21 接続・#62 | headless Pass+一部 real（`job*.rs`/`gw10_job.rs`） | 実 Provider・#46 G5 |
| MCP-01〜06・E2E-2-* | #59・#62 | PoC real transport Pass（`mcp_adapter.rs`/`gw10_mcp.rs`/`mcp-poc`） | 第三者 MCP Host・#114・#46 G4 拡張 |
| UX-01〜06・SYS-02-ux/03-ux | #114 | headless 契約 Pass(部分)（`gateway_ui.rs`/`gw10_system.rs`） | 実 GPUI UI・#114 |
| GT-011/012・E2E-1・SYS-01〜10 | #62・#83 | headless+process Pass（`gw10_system.rs`/`gw10_harness.rs`） | 実機 lane・#83 |

### Stream D: GW-10 Harness/Evidence（#62・#83・GTV）

| Family | Owner Issue | 現状 | 後続 |
|---|---|---|---|
| GTV-001〜008・G0〜G6 | #62・#83 | `mise run gw10`/`gw10-report` で PASS（headless+PoC real） | 独立 CI 再現・#83・実機 lane |
| SYS-07-rt・SYS-05-powerloss・CONSUMER-* | #62・#83・rf#294/#553 | Unsupported/NotRun（required=false） | 実機 budget・電源断・両 Consumer 回帰（条件付き） |

### Stream E: 境界・非重複（AUTX・CLI・adapter）

| 責務 | Owner Issue | 状態 |
|---|---|---|
| AUTX-00 最新 Owner/DAG | synvelis #259 | 別 Issue（参照のみ） |
| Action=CapabilityRef 投影 | synvelis #260 | 別 Issue（参照のみ） |
| CLI local issuer（AdapterSeal 維持） | synvelis #261 | 別 Issue（参照のみ） |
| CLI 独立 Adapter | synvelis #262 | 別 Issue（参照のみ） |
| Atomic Macro=GW-04 batch→Composite→GW-05/06 | synvelis #263 | 別 Issue（参照のみ） |
| Workflow Definition/Coordinator（Gateway 外側） | synvelis #264/#265 | 別 Issue（参照のみ） |
| MCP/CLI/GPUI 同一 Capability・独立 oracle | synvelis #267 | 別 Issue（参照のみ） |
| Ruviform 共通化評価（Gateway/DAW 型移植なし） | ruviform #553 | 評価専用（参照のみ） |
| 両 Consumer 回帰（共通変更時のみ） | ruviform #294・synvelis #28・audio-lens #127 | 条件付き（本 slice で Ruviform 変更なし） |

## Dependency Graph

```
spec (command-gateway.md)
  └─ design (command-gateway-design.md)
       └─ plan (command-gateway.md) ── task (command-gateway-task.md)
            └─ test-spec (command-gateway.md) ── impl-ir/exp ── evidence/sv45/
```

実装依存（本 slice 外・既に merge 済みの実体を参照）:

```
#53〜#57 (GW-01〜05 contract/policy/context/prepare/proposal)
  → #58 (GW-06 commit/revoke/idempotency/recovery)
  → #59 (GW-08 MCP adapter/binding)
  → #62 (GW-10 E2E/security/concurrency/recovery harness)
  → #83 (independent CI reproducibility)
  → #114 (GPUI approval UI・real external MCP/Provider session)
  → 実機 lane (GUI-L2/EXT-L3・実 Provider・OS 別 RT budget)
```

AUTX 系（#259〜#267）は Gateway 契約を参照するが本 slice の
完了を待たない。sv#44（ai-native-baseline）は並行 Worker が
担当し本 slice は参照のみ。

## Risk Register

| Risk | 影響 | 対策 |
|---|---|---|
| headless/mock Pass の過剰主張 | 実環境受入との混同 | evidence schema の mode/layer/required/real_only・ledger で Pass/NotRun/Unsupported を分離 |
| 実装との乖離 | docs 陳腐化 | fixture/test 名を ledger に pin・#46 配下変更時に更新 |
| AUTX/CLI/adapter の二重実装 | 権威混同 | GW-R-13 で非重複固定・#259〜#267/rf#553 を参照のみ |
| wire 自己申告の権威化 | security 破壊 | AdapterSeal/permit record 束縛を設計に明記・SEC-02/DATA-09 fixture |
| Recovery 保証の過大表示 | 偽の durability 主張 | SYS-05-powerloss Unsupported・SIGKILL≠電源断を区別 |
| 並行 Worker 衝突 | merge conflict | 参照のみ・編集範囲を厳守 |

## Verification Plan

本 slice の検証（docs/ 設計契約 + headless 回帰実測）:

1. `mise run fmt-check` — Markdown/構成ファイル整形
2. `node .agents/skills/doc-status/scripts/doc_status.js lint
   --type {spec,design,plan,task,test-spec} --json`
3. `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
   index apply`（触った dir の index 再生成）
4. `node .agents/skills/impl-doc/scripts/audit_impl_record.js
   --json`・`audit_experiment_log.js --json`・
   `sync_impl_index.js --kind ir/exp --apply`
5. `node .agents/skills/doc-status/scripts/audit_docs.js
   --type all --json`
6. `node .agents/skills/doc-driven-dev-graph/scripts/
   route_graph.js --focus command-gateway --json`
7. `mise run gw10`・`mise run gw10-report` — headless 回帰
   入口の実測（G0〜G6/GTV verdict・evidence JSONL）
8. `mise run check synvelis-gateway`・`mise run test
   synvelis-gateway` — 対象 package の check/test 実測
9. `review-protocol` — 設計契約のレビュー（blocker なしを承認）

実装検証は対象外（実装 Issue が Owner）。headless/mock/PoC
real transport の Pass は evidence ledger で mode/layer を
明記し、実機/GUI/Provider/第三者 Host 受入に繰り上げない。

## Rollback

- 本 slice は docs/ + evidence/ のみ。revert で完全に戻る。
- index 再生成は生成物のため、revert 時に再生成で整合。
- 実装への影響なし（コード変更なし）。

## Task Breakdown

| Task | 内容 | 状態 |
|---|---|---|
| T-1 | spec 作成（GW-R-01〜15・GW-AC-01〜12） | 本 slice |
| T-2 | design 作成（Owner 表・contract・state machine・failure・写像） | 本 slice |
| T-3 | plan 作成（file map・stream・dependency・risk・verification） | 本 slice |
| T-4 | task 作成（done-when・verification） | 本 slice |
| T-5 | test-spec 作成（GW-T-R・case map・example） | 本 slice |
| T-6 | impl-ir/exp 作成 | 本 slice |
| T-7 | evidence/sv45/ case ledger + verification | 本 slice |
| T-8 | index 再生成・docs 検証・headless 回帰実測・review-protocol | 本 slice |
| T-9 | commit・push・PR・worker_done | 本 slice |

後続（本 slice 外・実装/受入 Issue）:

- T-10 GW-01〜05 contract/policy/context/prepare/proposal の
  実装維持・拡張（#53〜#57・既に merge 済み）
- T-11 GW-06 commit/revoke/idempotency/recovery の実装維持・
  拡張（#58・既に merge 済み）
- T-12 GW-08 MCP adapter・trusted binding の実装維持・拡張
  （#59・既に merge 済み）
- T-13 GW-10 E2E/security/concurrency/recovery harness の
  維持・拡張（#62・既に merge 済み）
- T-14 独立 CI 再現性（#83）
- T-15 GPUI 承認 UI・実外部 MCP/Provider Authenticated
  Session（#114・#46 G4 拡張）
- T-16 実機 lane（GUI-L2/EXT-L3・OS 別 RT budget・電源断・
  実 Provider）
- T-17 AUTX Action/Workflow/CLI/Macro/Coordinator（#259〜
  #267）・#267 同一 Capability/独立 oracle 受入
- T-18 Ruviform 共通変更時の両 Consumer 回帰（rf#294・
  条件付き）
