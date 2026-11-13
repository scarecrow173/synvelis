---
id: "EXP-034X8sVqk2mNpLw8dYh4cJ"
type: "experiment"
status: "completed"
title: "Capability / Command Gateway v0.3 — Issue #45 設計契約文書化と GW case 写像の試行"
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
  implemented-by:
    - "docs/impl/ir/command-gateway.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/command-gateway-design.md"
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
    - "docs/designs/target-architecture-design.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/command-gateway-task.md"
    - "docs/test-specs/command-gateway.md"
    - "evidence/sv45/command-gateway-case-ledger.md"
    - "evidence/sv45/command-gateway-verification.md"
  defers: []
  deferred-by: []
---

# Capability / Command Gateway v0.3 — Issue #45 設計契約文書化と GW case 写像の試行

## 問題設定

Issue #45 の Capability / Command Gateway v0.3〜v0.8 横断整合
性・型付き契約・責務境界を canonical 文書鎖として確立する
必要があった。上流 baseline（v0.4 ACG・v0.5 GTC・v0.6 GIA・
v0.7 GTV・v0.8 監査）と #44 AIB/AHG・#46 G0〜G6、および
#53〜#59・#62・#83・#114 の merge 済み実装実体は存在するが、
GT-001〜012・JM-01〜05・MCP-01〜06・UX-01〜06・SEC/DATA/
REC/JOB/SYS・AC-01〜10・E2E-1/2/3・G0〜G6・GTV-001〜008 を
一つの正本 + 台帳として固定する記述がなかった。本 slice は
docs のみで crate を変更しないため、(a) 7 文書鎖の生成、
(b) GW-01〜10 Owner/型/状態/Failure の写像、(c) 全受入 case
の fixture/oracle/mode/layer/status 分類、(d) doc graph/
index/review の整合確認が試行対象となった。

## 試行と結果

### E1: 上流契約と実装の照合

Issue #45 本文（v0.3〜v0.8）を正本として読み、GTC-001〜008・
ACG-001〜005・GIA-001〜006・GTV-001〜008 を GW-R-01〜15・
GW-AC-01〜12 へ整理した。`crates/synvelis-gateway/src/` の
module 構成（capability/registry/request/wire/schema/id・
authorization/grant/policy/durable_policy・context/context_
engine/snapshot・prepare/prepare_engine/prepare_policy・
proposal/proposal_engine/proposal_policy・commit/journal/
store・job/job_engine/media/provenance・mcp*/gateway/outcome/
evidence/oracle/schedule/trace・bin/gw10_report）と
`crates/synvelis-gateway/tests/`・`crates/mcp-poc/tests/`・
`crates/synvelis-workspace/src/gateway_ui.rs`・
`crates/synvelis-project/src/controller.rs` を GW-01〜10 の
実体として照合した。`src/evidence.rs` の MATRIX（case→gate/
owner/required/real_only）を正本の case 一覧として採用した。

- GT-001〜010 が `contract.rs`/`gw10_contract.rs`/
  `gw10_authorization.rs`/`gw10_project.rs` の test で被覆
  されることを確認。GT-011/012 は SYS-01/E2E-1 の
  requirements 経由で被覆。
- SEC-01〜14・DATA-01〜10 が `gw10_authorization.rs`、
  REC-01〜12 が `gw10_concurrency.rs`/`gw10_persistence.rs`/
  `commit_gate.rs`、JOB-01〜10 が `gw10_job.rs`/`job.rs`、
  SYS-01〜10 が `gw10_system.rs`/`gw10_persistence.rs` で
  被覆されることを確認。
- E2E-2-suggest/approve/delegated/restart が
  `mcp-poc/tests/gw10_transport.rs` の `gw10f_*_real` と
  `stdio_e2e.rs` の real transport で被覆されることを確認
  （第三者 MCP Host ではない）。
- SYS-07-rt は budget 未実測で `Unsupported`、SYS-02-ux/
  03-ux/05-powerloss・CONSUMER-RUVIFORM/AUDIOLENS は
  required=false で `NotRun`/`Unsupported` として strict
  report が報告することを確認。

### E2: Owner 境界の固定

GW-01〜10 を Capability/Authorization/Context/Prepare/
Proposal/Commit/Job-Media-Provenance/MCP/UX/Verification の
責務で固定し、Project Single Writer/Undo・Job Owner・Media
Store・Semantic Diff・Transport Receipt の第二正本を Gateway
が持たない原則を design の Owner 表に固定した。AUTX（#259〜
#267）・CLI local issuer・MCP/CLI/GPUI adapter 独立性・
Ruviform #553 評価専用範囲を GW-R-13 で非重複として固定。

### E3: Evidence honesty の分類規則

要求状態（Required/Deferred/NotApplicable）と実行結果
（Pass/Fail/NotRun/Unsupported）を別軸で管理し、mode
（mock/real）・layer（L1〜L5）・required/real_only を ledger
に明記する規則を確立した。`src/evidence.rs` の MATRIX・
`gw10_report -- --strict` の集約 verdict を正本とし、
headless/mock/PoC real transport の Pass を実機/GUI/
Provider/第三者 Host 受入に繰り上げない。POLICIES.md の
OS-L1/GUI-L2/EXT-L3・Required/Deferred/NotApplicable・
Pass/Fail/NotRun/Unsupported と整合させた。

### E4: 文書鎖生成と index/relation 整合

spec→design→plan→task→test-spec→impl-ir/exp の 7 文書鎖を
canonical front matter（id/type/status/title/created/updated/
owners/relations）で生成し、relations（implements/
derived-by/verifies/references/depends-on 等）を相互に整合
させた。`docs/{specs,designs,plans,tasks,test-specs,impl/ir,
impl/exp}/README.md` は `doc_maintenance.js index apply`・
`sync_impl_index.js` で再生成し、手編集しない。
`evidence/sv45/` の case ledger と verification を正本とし
て配置した。

## 採用した判断

- **A-1 7 文書鎖 + evidence/sv45/**: Issue #18（playback-
  graph）・#21（media-render）の先例に従い canonical 配置。
- **A-2 GW-R-01〜15 + GW-AC-01〜12**: Issue #45 の v0.3〜v0.8
  合意と AUTX 境界・RT/Consumer・Evidence Honesty を要件
  番号へ整理し、受入条件を GW-AC-01〜12 に対応付け。
- **A-3 GW-01〜10 Owner 表**: 各 GW の責務・所有/非所有
  契約・実装 Issue・実体 module を一表で固定し、第二正本
  禁止を明記。
- **A-4 status 語彙**: 要求状態（Required/Deferred/
  NotApplicable）と実行結果（Pass/Fail/NotRun/Unsupported）
  を別軸、mode/layer/required/real_only を ledger に明記。
  headless/mock/PoC real transport の Pass を実環境受入に
  繰り上げない。
- **A-5 impl/exp canonical 名**: `docs/impl/ir/command-
  gateway.md`・`docs/impl/exp/command-gateway.md`（media-
  render 先例一致）。scaffold の slug 名は canonical 名へ
  rename。
- **A-6 evidence JSONL の正本**: `target/gw10-evidence/*.jsonl`
  は実行毎に増えるため、verdict は `gw10_report -- --strict`
  の集約を正本とし ledger へ写像する。

## 結論

Issue #45 の設計契約は docs/ 7 文書鎖 + `evidence/sv45/` と
して確立された。全受入 case（GT/JM/MCP/UX・SEC/DATA/REC/
JOB/SYS・AC・E2E・GTV・G0〜G6）のうち、headless/mock/PoC
real transport で被覆されるものは `Pass`、実機/GUI/Provider/
第三者 Host・電源断・OS 別 RT budget・両 Consumer 回帰は
`NotRun`/`Unsupported`（required=false）として記録された。
設計完了は実装完了・実機/GUI/Provider 受入を意味せず、後続
実装/受入 Issue（#46 配下・#83・#114・実機 lane・AUTX
#259〜#267・rf#294/#553 条件付き）が Owner として残る。
