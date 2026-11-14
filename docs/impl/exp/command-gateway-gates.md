---
id: "EXP-FlsRK89CrxmSsyDxBKCufu"
type: "experiment"
status: "completed"
title: "Capability / Command Gateway G0-G6 受入照合・証拠台帳の試行 (synvelis#46)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/46"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/command-gateway-gates.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
    - "docs/test-specs/command-gateway.md"
    - "docs/test-specs/command-gateway-gates.md"
    - "evidence/sv45/command-gateway-case-ledger.md"
    - "evidence/sv45/command-gateway-verification.md"
    - "evidence/sv46/command-gateway-gates-ledger.md"
    - "evidence/sv46/command-gateway-gates-verification.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/command-gateway-gates.md"
    - "docs/impl/ir/command-gateway-gates.md"
    - "evidence/sv46/command-gateway-gates-ledger.md"
    - "evidence/sv46/command-gateway-gates-verification.md"
  defers: []
  deferred-by: []
---

# Capability / Command Gateway G0-G6 受入照合・証拠台帳の試行

## 問題設定

Issue #46 の G0〜G6 各 Gate 受入条件を、GW-01〜10 の merge
済み実装・既存 fixture・#44/#45 設計正本・GW-10 harness の
実測 verdict へ一意に照合し、`Headless Verified` と
`Product Accepted` を別状態で記録する受入台帳が必要だった。
本 slice は受入照合・台帳のみでコード変更を伴わないため、
(a) Issue #46 の Gate 定義・受入条件の分解、(b) G3 の 3
Mode・Single Writer・Undo・失効競合・Approval 再利用防止・
Idempotency/Crash Recovery の実証箇所の特定、(c) G4/G5/G6
の headless 実測可能範囲と実機/実 Provider/実 OS の
NotRun 範囲の分離、(d) Transport 未確定・Discovery≠権限・
AI Runtime/External MCP 分離・AUTX 接合点の記録、(e)
実測コマンドの実行と結果記録が試行対象となった。

## 試行と結果

### E1: Gate 定義の分解と正本の特定

Issue #46 本文（Gate 判定 GW10-A/B/C/D/E/F/G/H・G0〜G6
受入条件・2026-11-10 AUTX 追記）を正本として読み、G0
Contract・G1 Read & Policy・G2 Proposal・G3 Authorized
Commit・G4 MCP E2E・G5 Async & Generated Media・G6
Integration & Hardening・GTV の各 Gate を受入項目へ分解した。
case 単位の fixture/oracle/Owner/mode/layer/status の
詳細写像は `evidence/sv45/command-gateway-case-ledger.md`
を正本として採用し、本台帳では Gate 受入条件への照合に
限定して参照する構成を試みた。

- `src/evidence.rs` の MATRIX（case→gate/owner/required/
  real_only）と `gw10_report -- --strict` の集約規則
  （required row のみ gate を決定・optional NotRun/
  Unsupported は報告のみ）を Gate 判定の正本として確認。
- G3 の 3 Mode は `proposal.rs`（suggest_mode_never_
  applies・approve_to_apply_flows_through_request_grant_
  consume・delegated_apply_needs_no_receipt・
  ahg009_apply_mode_comes_from_grant_not_request）と
  `mcp-poc/tests/gw10_transport.rs::gw10f_*_real` の両
  層で実証箇所を特定。Mode が Grant の Approval Policy
  から導出され Invocation 引数・caller 自己申告で昇格
  しないことを確認。
- Single Writer（`commit.rs`/CommitGate/CommitFence・
  `concurrent_commits_single_writer_conflict`）・Undo
  （`gw10c_commit_advances_revision_and_undoes`）・Grant
  失効競合（REC-01/02・GT-009・SEC-12・GTV-005）・
  Approval 再利用防止（double_consumption・REC-07）・
  Idempotency（REC-04/05/06・MCP-05）・Crash Recovery
  （REC-07〜12・SYS-04/09・E2E-2-restart）を file/test
  関数単位で照合。

### E2: headless 実測可能範囲と NotRun 範囲の分離

`mise run fmt-check`・`mise run gw10`・`mise run
gw10-report`・`mise run test synvelis-gateway` を実測した
（全 exit 0・結果は `evidence/sv46/command-gateway-gates-
verification.md`）。G0〜G6・GTV は strict report で全
PASS、required=false の 5 件（SYS-02-ux・SYS-03-ux・
SYS-05-powerloss・SYS-07-rt・CONSUMER-RUVIFORM/AUDIOLENS）
は NotRun/Unsupported のまま報告されることを確認。

- `test synvelis-gateway` は 406 passed/0 failed/25
  binaries。sv45 記録の 487 とは別観測であり混同しない
  ことを台帳に明記。
- `mcp-poc/tests/gw10_transport.rs` の 4 tests は
  `mise run gw10` 内で実行・pass。real stdio + 認証
  loopback transport・real `DirectoryBundleStore`+
  `commit.journal`+`FilePermitStore` を使用するが、
  client は本 repo の `mcp-poc` test harness であり
  第三者 MCP Host ではないことを明記。
- `mcp-poc/tests/stdio_e2e.rs` は正規コマンド対象外
  package のため未再実行と明記（推測で Pass にしない）。

### E3: 2 軸 status の適用

各 Gate を `Headless Verified`/`Product Accepted` の 2 軸で
記録する規則を適用した。headless/mock/harness/PoC real
transport の Pass は mode/layer を明記し、実 GUI・第三者
MCP Host・実 Provider・実 OS lane・電源断・大規模負荷・
Consumer 回帰を全 Gate で `Product Accepted = NotRun`
として分離した。設計合意・Issue 起票・mock 成功・PoC
transport 成功を `Product Accepted` と記録しない規則を
test-spec GWGT-T-R-03 に固定した。

### E4: Transport 未確定・Discovery≠権限・分離の記録

- Transport: Local stdio Helper + 認証 Local IPC が第一
  候補・**未確定**（Issue #46「まだ最終決定しない」を
  維持）。Localhost Streamable HTTP は比較候補として残る
  が未実装・未比較であることを台帳に明記。
- MCP Tool Discovery/Schema が権限の代替でないこと:
  `tools/list` は bound session 必須・catalog は channel
  scoped・各 Invocation は GW-02 へ再照会（DATA-09・
  GT-003・SEC-13・real/L4 で `PERMIT_REQUIRED`）を実測
  証拠として記録。
- AI Runtime と External MCP Access の分離:
  `gateway_ui.rs::AiSwitches`/`AiFeature`・`McpConfig.
  enabled`・E2E-1/SYS-01/AC-10 の AI Off DAW pipeline
  成立を headless contract として記録。実製品設定 UI は
  #114 NotRun。
- 切断→再接続→Operation 照合/Idempotent Retry:
  serve kill→fresh serve→`operation.get` が durable
  journal から `committed` を reconcile（E2E-2-restart・
  SYS-09 real/L4）・`proposal.apply` retry が同一
  `journal_seq`（REC-06・MCP-05）を記録。実 Host 切断は
  NotRun。

### E5: AUTX 接合点の記録と文書鎖整合

Issue #46 2026-11-10 追記に従い AUTX #259〜267 を GW への
新 Consumer 接合点として記録。`synvelis.operation.batch`
（`prepare_engine.rs`）と `ProjectOp::Composite`
（`synvelis-project/src/ops.rs`）は既存 GW-04 の一部で
あり、独立 Macro Engine・別 Commit Gate を Gate 判定に
含めない。CLI mutating invocation は #261 Auth/Host Gate
まで非公開を維持。

- test-spec→impl-ir→impl-exp→evidence の 3 文書 +
  `evidence/sv46/`（ledger・verification）の鎖を canonical
  front matter で作成。impl-ir の scaffold 生成 slug 名
  `capability-command-gateway-g0-g6-synvelis-46.md` は
  task 指定の canonical 名 `command-gateway-gates.md` へ
  rename。
- `docs/{test-specs,impl/ir,impl/exp}/README.md` は
  `doc_maintenance.js index apply`・`sync_impl_index.js`
  で再生成（手編集しない）。

## 採用した判断

- **A-1 受入照合・台帳のみ**: Issue #46 の slice 性質に
  従い実装・Fixture・コードを変更せず、既存証拠の照合と
  記録のみを行った。
- **A-2 2 軸 status**: `Headless Verified`/`Product
  Accepted` を別状態で記録し、headless/mock/PoC real の
  Pass を製品受入に繰り上げない。
- **A-3 sv45 case ledger を正本として参照**: case 単位の
  詳細は sv45 に集約し、本台帳は Gate 受入条件への照合に
  限定（複写・二重管理を回避）。
- **A-4 strict report verdict を Gate 判定の正本**:
  `gw10_report -- --strict` の集約 verdict を採用し、
  required=false の NotRun/Unsupported を gate に影響
  させない規則を維持。
- **A-5 impl canonical 名**: `docs/impl/ir/command-gateway-
  gates.md`・`docs/impl/exp/command-gateway-gates.md`
  （task 指定 path）。scaffold の slug 名は canonical 名へ
  rename。
- **A-6 実測と推測の区別**: 実行したコマンドのみを Pass
  として記録し、未再実行の `stdio_e2e.rs`・未実行の
  `check` を明記して推測で補わない。

## 結論

Issue #46 の G0〜G6 受入照合は `evidence/sv46/` の台帳・
検証記録 + test-spec/impl-ir/impl-exp の 3 文書として
確立された。G0〜G6・GTV は headless/mock/harness/PoC real
transport レベルで全 Gate **Pass**（strict report・
required case）だが、`Product Accepted` は全 Gate で
**NotRun** — 実 GUI・第三者 MCP Host・実 Provider・実 OS
lane・電源断・大規模負荷・Consumer 回帰は後続 Owner
（#114・#83・#46 G4/G6 拡張・実機 lane・rf#294 条件付き）
に残る。本 slice は Issue #46 を Close するものではなく、
headless 受入照合結果と Product Accepted 未達残件を固定
する証拠である。設計合意・Issue 起票・mock/PoC 成功は
製品受入を意味しない。
