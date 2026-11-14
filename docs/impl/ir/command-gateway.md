---
id: "IMPL-034X8sM6gVaKFLBghHMqW3"
type: "impl"
status: "completed"
title: "Capability / Command Gateway 設計契約 実装記録 (synvelis#45)"
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
    - "docs/tasks/command-gateway-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/command-gateway-task.md"
    - "docs/plans/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
    - "docs/specs/command-gateway.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
    - "docs/designs/target-architecture-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/command-gateway-task.md"
  derived-by: []
  verifies:
    - "docs/test-specs/command-gateway.md"
  verified-by: []
  references:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
    - "docs/plans/command-gateway.md"
    - "docs/tasks/command-gateway-task.md"
    - "docs/test-specs/command-gateway.md"
    - "evidence/sv45/command-gateway-case-ledger.md"
    - "evidence/sv45/command-gateway-verification.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/command-gateway.md"
    rejected: []
---

# Capability / Command Gateway — Issue #45 設計契約文書化 + GW evidence 台帳

## Summary

Issue #45「Capability / Command Gateway v0.3 — 横断整合性・
型付き契約・責務境界」の canonical 設計契約を docs/ 7 文書鎖
（spec/design/plan/task/test-spec/impl-ir/impl-exp）+
`evidence/sv45/`（case ledger・verification）として作成した。
v0.3〜v0.8 で合意された GW-01〜10 の Owner・型・状態遷移・
Failure/Recovery Matrix・G0〜G6 Gate・GT-001〜012/JM-01〜05/
MCP-01〜06/UX-01〜06 を既存 fixture・Issue・Receipt・
`crates/synvelis-gateway`/`crates/mcp-poc`/`gateway_ui.rs` の
実体へ写像し、AUTX/CLI/adapter 境界と evidence 語彙を固定
した。**docs/ 設計契約 + evidence のみ**であり、実装コード・
GUI・実 device/plugin/OS/Provider 受入は対象外。

## Context

- 対象 Issue: synvelis #45
- ブランチ: `docs/45-command-gateway`
- 上流正本: Issue #45 本文（v0.3〜v0.8）・#44（AIB/AHG）・
  #46（G0〜G6 実装受入）
- 確定済み縦断契約: vs-a/project-model/project-persistence/
  playback-graph/transport-integration/musical-time/media-
  render/workspace-model/vcs-02/recording-take-comping/
  midi-source-note-editing・gw-01/gw-02 design・
  target-architecture・time-transport・media-import-export
- 実装 Issue: #53（GW-01）・#54（GW-02）・#55（GW-03）・
  #56（GW-04）・#57（GW-05）・#58（GW-06）・#59（GW-08）・
  #62（GW-10）・#83（CI 再現）・#114（GPUI UX）・#52/#56/
  #57/#58/#59（MIDI/Effect/diff/revoke/binding）
- AUTX 系: #259〜#267・ruviform #553/#294（参照のみ）

## Implementation

### Changes

- `docs/specs/command-gateway.md`: GW-R-01〜15 要件・
  GW-AC-01〜12 acceptance criteria・Deferred Design Concerns・
  Sources。
- `docs/designs/command-gateway-design.md`: GW-01〜10 Owner
  表・10 contract・data/control flow・3 Mode・state machine・
  failure taxonomy（SEC/DATA/REC/JOB/SYS）・GT/JM/MCP/UX/Gate
  写像・risks・deferred・verification notes。
- `docs/plans/command-gateway.md`: goal/scope/upstream/file
  map/5 implementation streams/dependency graph/risk
  register/verification plan/rollback/task breakdown。
- `docs/tasks/command-gateway-task.md`: work/classification/
  upstream/dependencies/affected files/done-when/verification/
  out-of-scope。
- `docs/test-specs/command-gateway.md`: GW-T-R-01〜06 rules・
  case map・4 examples・guarantees・non-goals・risk。
- `docs/impl/ir/command-gateway.md`: 本記録。
- `docs/impl/exp/command-gateway.md`: experiment record
  （E1〜E4）。
- `evidence/sv45/command-gateway-case-ledger.md`: 全受入
  case の fixture・oracle・Owner・mode/layer・status・証拠の
  台帳。
- `evidence/sv45/command-gateway-verification.md`: 検証記録。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/
  README.md`: index 再生成（生成物）。

### Decisions During Implementation

- **D-1 7 文書鎖 + evidence/sv45/**: Issue #18/#21 の先例に
  従い canonical 配置。spec/design/plan/task/test-spec/
  impl-ir/impl-exp の 7 文書と case ledger・verification を
  `evidence/sv45/` に配置。
- **D-2 GW-R-01〜15 要件番号**: Issue #45 の v0.3〜v0.8
  合意（GTC/ACG/GIA/GTV）+ AUTX 境界 + RT/Consumer +
  Evidence Honesty を GW-R-01〜15 に整理。
- **D-3 GW-AC-01〜12**: Issue #45 の受入条件（GT-001〜012・
  JM/MCP/UX・GW-01〜10 Owner 照合・failure matrix・AUTX・
  RT/Consumer・文書鎖）を 15 項目に対応付け、case ledger で
  被覆状況を記録。
- **D-4 status 語彙**: 要求状態（Required/Deferred/
  NotApplicable）と実行結果（Pass/Fail/NotRun/Unsupported）
  を別軸で管理。mode（mock/real）・layer（L1〜L5）・
  required/real_only を ledger に明記し、headless/mock/PoC
  real transport の Pass を実機/GUI/Provider/第三者 Host
  受入に繰り上げない。
- **D-5 impl/exp canonical 名**: `docs/impl/ir/command-
  gateway.md`・`docs/impl/exp/command-gateway.md`（media-
  render/playback-graph 先例に一致）。scaffold が生成した
  slug 名は canonical 名へ rename。
- **D-6 index 再生成**: `doc_maintenance.js index apply`・
  `sync_impl_index.js` で生成、手編集しない。
- **D-7 headless 回帰実測の記録**: `mise run gw10`・
  `gw10-report`・`check`・`test synvelis-gateway` を実際に
  実行し、G0〜G6/GTV verdict と 5 件の NotRun/Unsupported
  を verification に記録。実装 crate は変更しない。

## Related Experiments

### Adopted

- `docs/impl/exp/command-gateway.md`（E1〜E4: 文書鎖構造・
  case 写像・Owner 境界・evidence 語彙の設計判断）

### Rejected

- なし

## Validation

`evidence/sv45/command-gateway-verification.md` を正本とする。
実行した検証:

- `mise run fmt-check`
- `doc_status.js lint --type {spec,design,plan,task,test-spec}`
- `doc_maintenance.js index apply`
- `audit_impl_record.js --json`・`audit_experiment_log.js
  --json`・`sync_impl_index.js --kind ir/exp --apply`
- `audit_docs.js --type all --json`
- `route_graph.js --focus command-gateway --json`
- `mise run gw10`・`mise run gw10-report`（headless 回帰）
- `mise run check synvelis-gateway`・`mise run test
  synvelis-gateway`（対象 package）
- `review-protocol`

## Risks

- **R-IR-1 headless/mock Pass の過剰主張**: mode/layer/
  required/real_only を明記し、未実装・未検証は `NotRun`/
  `Unsupported`。
- **R-IR-2 実装との乖離**: fixture/test 名を ledger に pin、
  #46 配下変更時に更新。
- **R-IR-3 Owner 境界の曖昧さ**: GW-01〜10 Owner 表と
  Project/Job/Media/Diff/Transport 第二正本禁止を API/
  Fixture/Issue で照合。
- **R-IR-4 並行 Worker 衝突**: sv#44・AUTX 系・asset-library
  系が並行編集中。本 slice は参照のみで編集しない。

## Follow-ups

- GW-01〜05 contract/policy/context/prepare/proposal の実装
  維持・拡張（#53〜#57・merge 済み）
- GW-06 commit/revoke/idempotency/recovery の実装維持・拡張
  （#58・merge 済み）
- GW-08 MCP adapter・trusted binding の実装維持・拡張
  （#59・merge 済み）
- GW-10 E2E/security/concurrency/recovery harness の維持・
  拡張（#62・merge 済み）
- 独立 CI 再現性（#83）
- GPUI 承認 UI・実外部 MCP/Provider Authenticated Session
  （#114・#46 G4 拡張）
- 実機 lane（GUI-L2/EXT-L3・OS 別 RT budget・電源断・実
  Provider）
- AUTX Action/Workflow/CLI/Macro/Coordinator（#259〜#267）・
  #267 同一 Capability/独立 oracle 受入
- Ruviform 共通変更時の両 Consumer 回帰（rf#294・条件付き）
