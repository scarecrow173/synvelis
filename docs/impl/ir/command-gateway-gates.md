---
id: "IMPL-034X9hYuHSZsRHijApPhOw"
type: "impl"
status: "completed"
title: "Capability / Command Gateway G0-G6 受入照合・証拠台帳 実装記録 (synvelis#46)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/46"
  changes:
    added:
      - type: "file"
        path: "docs/test-specs/command-gateway-gates.md"
      - type: "file"
        path: "docs/impl/ir/command-gateway-gates.md"
      - type: "file"
        path: "docs/impl/exp/command-gateway-gates.md"
      - type: "file"
        path: "evidence/sv46/command-gateway-gates-ledger.md"
      - type: "file"
        path: "evidence/sv46/command-gateway-gates-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
    - "docs/test-specs/command-gateway.md"
    - "docs/test-specs/command-gateway-gates.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/command-gateway.md"
    - "docs/impl/ir/gw-10-gateway-e2e-test-harness-evidence-gates.md"
    - "evidence/sv45/command-gateway-case-ledger.md"
    - "evidence/sv45/command-gateway-verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/test-specs/command-gateway-gates.md"
  derived-by: []
  verifies:
    - "docs/test-specs/command-gateway-gates.md"
  verified-by: []
  references:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
    - "docs/test-specs/command-gateway.md"
    - "docs/test-specs/command-gateway-gates.md"
    - "docs/impl/ir/command-gateway.md"
    - "evidence/sv45/command-gateway-case-ledger.md"
    - "evidence/sv45/command-gateway-verification.md"
    - "evidence/sv46/command-gateway-gates-ledger.md"
    - "evidence/sv46/command-gateway-gates-verification.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/command-gateway-gates.md"
    rejected: []
---

# Capability / Command Gateway G0-G6 受入照合・証拠台帳 — Issue #46

## Summary

Issue #46「Capability / Command Gateway — GW-01〜10・G0〜G6・
実装依存と受入Gate」の G0〜G6 各 Gate 受入条件を、main 上の
既存実装（`crates/synvelis-gateway`・`synvelis-project`・
`synvelis-workspace`/`synvelis-app`・`mcp-poc`）・既存
fixture（`tests/gw10_*`・`tests/{contract,authorization,
context,prepare,prepare_regressions,proposal,proposal_
regressions,commit_gate,commit,schema_compat}.rs`・
`mcp-poc/tests/{gw10_transport,stdio_e2e}.rs`）・#44/#45
設計正本へ一意に照合し、`evidence/sv46/` の受入台帳と検証
記録として固定した。`mise run gw10`/`gw10-report`/`test
synvelis-gateway` を実測し、G0〜G6・GTV は headless/mock/
harness/PoC real transport レベルで全 PASS、`Product
Accepted`（実 GUI・第三者 MCP Host・実 Provider・実 OS
lane・電源断・大規模負荷・Consumer 回帰）は全 Gate で
NotRun として分離した。**受入照合・台帳のみ**であり、
実装コード・Fixture 追加・別 Gate の PASS 宣言は対象外。

## Context

- 対象 Issue: synvelis #46（OPEN・docs slice）
- ブランチ: worktree `scarecrow173/sv46-gateway-gates`・
  push/PR branch `docs/46-gateway-gates-audit`（派生元
  `origin/main` HEAD `86715f9`・その後 `7d3fe5b` を merge）
- 対象 Revision: `86715f9ac2761f9f275e885e0bbd7ee775d9633e`
  （監査実施時 `origin/main` HEAD）・その後 `origin/main`
  `7d3fe5b` を merge（docs/evidence のみ・コード差分なし）
  ・Ruviform resolved `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`
- 上流正本: Issue #45 docs chain（spec/design/plan/task/
  test-spec/impl-ir/exp）・`evidence/sv45/`（case ledger・
  verification）・GW-01〜10 個別 IR（merge 済み）・
  Issue #46 本文（2026-11-10 AUTX 追記を含む）
- 実装 Issue（merge 済み）: #53〜#59・#62・#83・#114（後続）
- AUTX 系: #259〜#267（接合点の記録のみ・Gate 判定対象外）
- 並行占有: `docs/specs/command-gateway.md` 等の #45 正本・
  実装 crate は他 Worker 占有のため参照のみ

## Implementation

### Changes

- `docs/test-specs/command-gateway-gates.md`: GWGT-T-R-01〜06
  rules・Gate 別 Case Map・4 examples・guarantees・
  non-goals・risk。
- `docs/impl/ir/command-gateway-gates.md`: 本記録。
- `docs/impl/exp/command-gateway-gates.md`: experiment
  record（E1〜E5）。
- `evidence/sv46/command-gateway-gates-ledger.md`: G0〜G6・
  GTV の Gate 別受入照合台帳（環境/Revision/コマンド・
  判定語彙・Gate 別照合表・Issue #46 全 case family 写像・
  横断記録・AUTX 接合点・未達残件・集計）。
- `evidence/sv46/command-gateway-gates-verification.md`:
  実行した検証コマンド・exit code・結果の記録。
- `docs/test-specs/README.md`・`docs/impl/ir/README.md`:
  index 再生成（生成物。`docs/impl/exp/README.md` は jsonl
  event log のみを index する規約のため .md 追加では不変）。

### Decisions During Implementation

- **D-1 受入照合・台帳のみ**: Issue #46 の slice 性質に従い
  新規実装・Fixture 追加・コード変更を行わず、既存実装・
  fixture・#44/#45 正本への照合結果のみを文書化した。
  `crates/` 配下・`Cargo.toml`/`Cargo.lock`/`mise.toml`・
  CI・#45 正本 docs は未変更。
- **D-2 Headless Verified / Product Accepted の 2 軸**:
  Issue #46「`Headless Verified`と`Product Accepted`を別
  状態で示す」に従い、各 Gate を 2 軸で記録。headless/
  mock/harness/PoC real transport の Pass を製品受入に
  繰り上げず、実 GUI・第三者 MCP Host・実 Provider・実 OS
  lane・電源断・大規模負荷・Consumer 回帰を NotRun/
  Unsupported/Deferred として後続 Owner（#114・#83・
  #46 G4/G6 拡張・実機 lane・rf#294 条件付き）へ送った。
- **D-3 case ledger は sv45 を正本として参照**: case 単位の
  fixture/oracle/Owner/mode/layer/status の詳細写像は
  `evidence/sv45/command-gateway-case-ledger.md` が正本。
  本台帳は Gate 受入条件への照合に限定し複写しないことで
  陳腐化と二重管理を防ぐ。
- **D-4 strict report verdict を Gate 判定の正本**:
  `gw10_report -- --strict` の集約 verdict（required row の
  み gate を決定・optional NotRun/Unsupported は報告のみ）
  を Gate 判定の正本とし、G0〜G6・GTV 全 PASS を記録。
  required=false の 5 件（SYS-02-ux/03-ux/05-powerloss・
  CONSUMER-RUVIFORM/AUDIOLENS）は NotRun/Unsupported の
  まま残した。
- **D-5 impl canonical 名**: `docs/impl/ir/command-gateway-
  gates.md`（task 指定 path）。scaffold が生成した slug 名
  `capability-command-gateway-g0-g6-synvelis-46.md` は
  canonical 名へ rename。exp は `docs/impl/exp/command-
  gateway-gates.md`。
- **D-6 index 再生成**: `doc_maintenance.js index apply`・
  `sync_impl_index.js` で生成、手編集しない。
- **D-7 実測記録の区別**: `mise run fmt-check`・`gw10`・
  `gw10-report`・`test synvelis-gateway` を実測し
  verification に記録。`test synvelis-gateway` は 406
  passed/0 failed/25 binaries（sv45 記録の 487 とは別観測・
  混同しない）。`mcp-poc/tests/stdio_e2e.rs` は本 slice の
  正規コマンド対象外のため未再実行と明記し、同一 wire を
  駆動する `gw10_transport.rs`（`mise run gw10` 内で実行・
  4 tests pass）を実測証拠とした。
- **D-8 AUTX を Gate 判定に含めない**: Issue #46 2026-11-10
  追記に従い AUTX #259〜267 は GW への新 Consumer 接合点
  として記録。`synvelis.operation.batch`/`ProjectOp::
  Composite` は既存 GW-04 の一部、独立 Macro Engine/別
  Commit Gate は要求しない。CLI mutating invocation は
  #261 Auth/Host Gate まで非公開を維持。

## Related Experiments

### Adopted

- `docs/impl/exp/command-gateway-gates.md`（E1〜E5: 照合
  手法・2 軸 status・real_only 規則・Transport 未確定の
  維持・AUTX 境界）
- `docs/impl/exp/command-gateway.md`（#45・参照のみ）
- `evidence/sv45/command-gateway-case-ledger.md`・
  `command-gateway-verification.md`（#45・正本として参照）

### Rejected

- なし

## Validation

`evidence/sv46/command-gateway-gates-verification.md` を
正本とする。実行した検証（実測・exit code 記録）:

- `mise run fmt-check`（exit 0）
- `mise run gw10`（exit 0・G0〜G6・GTV PASS・`mcp-poc
  gw10_transport` 4 tests pass）
- `mise run gw10-report`（exit 0・5 件 NotRun/Unsupported
  報告）
- `mise run test synvelis-gateway`（exit 0・406 passed/
  0 failed/25 binaries）
- `doc_status.js lint`・`audit_docs.js --type all --json`・
  `audit_impl_record.js --json`・`audit_experiment_log.js
  --json`・`sync_impl_index.js --kind ir --apply`・
  `--kind exp --apply`・`doc_maintenance.js index apply`・
  `route_graph.js --focus command-gateway --json`
- `review-protocol`

ローカル検証 = 変更範囲（docs/evidence のみ）の検証 +
当該 crate の正規 test。統合検証は merge 後の対象 SHA の
必須 CI 結果が証拠（Coordinator 確認）。

## Risks

- **R-IR-1 status の誤昇格**: headless/mock/PoC real の
  Pass を `Product Accepted` と記録しない。2 軸 status・
  mode/layer/required/real_only を明記。
- **R-IR-2 sv45 ledger との乖離**: case 単位の正本は sv45
  ledger。Gate 照合のみを本台帳に記録し複写しない。
- **R-IR-3 Transport/Host の確定扱い**: 「第一候補・未確定」
  「PoC real・第三者 Host ではない」を固定語彙で維持。
- **R-IR-4 並行 Worker 衝突**: sv#44・AUTX 系・他 slice が
  並行編集中。本 slice は許可範囲（`evidence/sv46/`・3
  docs・生成 index）のみを変更。

## Follow-ups

- 実 GUI 承認 UX・実 Window 複数対象固定（SYS-02-ux/03-ux・
  UX-01〜06 実画面）→ #114
- 第三者 MCP Host での G4 受入・実 Host 互換・Transport
  最終決定（Localhost Streamable HTTP 比較 PoC）→ #46
  G4 拡張・別設計
- 実 Provider・実生成 Media・Provider/Model/Rights 実契約
  → 別設計（#46 G5 注記・#21 系）
- 電源断 durability・OS 別 RT budget 実測・大規模 Scope
  負荷試験・実監査 UI → 実機 lane・#83・#46 G6 残件
- Grant の product 配線での永続化（FilePolicyStore→serve
  統合）→ #46 G6 残件
- Ruviform/AudioLens Consumer 回帰 → Ruviform 変更時のみ
  #294・#553 評価（条件付き）
- AUTX #259〜267 の GW 接合（設計 OPEN・本台帳対象外）
- merge 後 main SHA の必須 CI 結果（統合検証証拠・
  Coordinator 確認）
