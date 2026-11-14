---
id: "IMPL-aqYVtVu6shEVwq4hQuJ7jk"
type: "impl"
status: "completed"
title: "AI Native Baseline — Issue #44 設計契約文書化 + AIB/AHG evidence 台帳"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/44"
  changes:
    added:
      - type: "file"
        path: "docs/specs/ai-native-baseline.md"
      - type: "file"
        path: "docs/designs/ai-native-baseline-design.md"
      - type: "file"
        path: "docs/plans/ai-native-baseline.md"
      - type: "file"
        path: "docs/tasks/ai-native-baseline-task.md"
      - type: "file"
        path: "docs/test-specs/ai-native-baseline.md"
      - type: "file"
        path: "docs/impl/ir/ai-native-baseline.md"
      - type: "file"
        path: "docs/impl/exp/ai-native-baseline.md"
      - type: "file"
        path: "evidence/sv44/ai-native-baseline-case-ledger.md"
      - type: "file"
        path: "evidence/sv44/ai-native-baseline-verification.md"
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
    - "docs/tasks/ai-native-baseline-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/ai-native-baseline-task.md"
    - "docs/plans/ai-native-baseline.md"
    - "docs/designs/ai-native-baseline-design.md"
    - "docs/specs/ai-native-baseline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
    - "docs/designs/midi-source-note-editing-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/ai-native-baseline-task.md"
  derived-by: []
  verifies:
    - "docs/test-specs/ai-native-baseline.md"
  verified-by: []
  references:
    - "docs/specs/ai-native-baseline.md"
    - "docs/designs/ai-native-baseline-design.md"
    - "docs/plans/ai-native-baseline.md"
    - "docs/tasks/ai-native-baseline-task.md"
    - "docs/test-specs/ai-native-baseline.md"
    - "evidence/sv44/ai-native-baseline-case-ledger.md"
    - "evidence/sv44/ai-native-baseline-verification.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/ai-native-baseline.md"
    rejected: []
---

# AI Native Baseline — Issue #44 設計契約文書化 + AIB/AHG evidence 台帳

## Summary

Issue #44「AI-Native Hybrid DAW Baseline v0.1 — Intentional
Usage・Rights・Shared Project・Agent Direction」の canonical
設計契約を docs/ 7 文書鎖（spec/design/plan/task/test-spec/
impl-ir/impl-exp）+ `evidence/sv44/`（case ledger・
verification）として作成した。AIB-001〜007・AHG-001〜011 と
受入条件 AIB-AC-01〜08 を既存 `synvelis-gateway`/`mcp-poc`
の headless fixture・#45/#46・#23 配下の実装 Issue・実機
lane へ写像し、Grant/Principal/Scope/Invocation Target/
Approval Receipt/ChangeSet・3 Mode/Proactive・AI Off の
Owner 境界と evidence 語彙を固定した。**docs/ 設計契約
+ evidence のみ**であり、Command Gateway の型・実装・GUI・
実 device/plugin/OS・実外部 Provider/MCP Client 受入は対象外。

## Context

- 対象 Issue: synvelis #44
- ブランチ: `scarecrow173/sv44-ai-native-baseline`（task spec
  の `docs/44-ai-native-baseline` ではなく Orca worktree の
  実ブランチ名を使用）
- 上流正本: Issue #44 本文（AIB-001〜007・AHG-001〜011・
  受入 8 件・未決定論点）
- 実装側先行設計: `docs/designs/gw-01-gateway-contract-design.md`・
  `docs/designs/gw-02-authorization-policy-design.md`
- headless 実装記録: `docs/impl/ir/gw-01`〜`gw-10`
- 確定済み縦断契約: product-value-core/workspace-model/
  context-inspector/timeline-editing/project-model/
  project-persistence/playback-graph/musical-time/
  transport-integration/media-render/vcs-02-semantic-diff/
  ed-a-editing-model
- 後続 Owner: #45（型契約）・#46（実装/製品受入）・#114（実機）

## Implementation

### Changes

- `docs/specs/ai-native-baseline.md`: AIB-001〜007・AHG-001〜011
  要件・AIB-AC-01〜08 acceptance criteria・Deferred Design
  Concerns・Sources。
- `docs/designs/ai-native-baseline-design.md`: Owner 分離表・
  canonical concepts（Principal/Caller/Grant/Scope/Invocation
  Target/Approval Receipt/ChangeSet/Apply Modes）・state
  machines（Grant/Proposal/Invocation）・failure taxonomy・
  data access/rights/provenance/cost・RT/nonRT boundary・
  alternatives・requirement 写像・risks・deferred・verification
  notes。
- `docs/plans/ai-native-baseline.md`: goal/scope/upstream/file
  map/5 implementation streams/dependency graph/risk register/
  verification plan/rollback/task breakdown。
- `docs/tasks/ai-native-baseline-task.md`: work/classification/
  upstream/dependencies/affected files/done-when/verification/
  out-of-scope。
- `docs/test-specs/ai-native-baseline.md`: AIB-T-R-01〜06
  rules・全要件 case map・3 examples・guarantees・non-goals・
  risk。
- `docs/impl/ir/ai-native-baseline.md`: 本記録。
- `docs/impl/exp/ai-native-baseline.md`: experiment record
  （E1〜E4）。
- `evidence/sv44/ai-native-baseline-case-ledger.md`: 全要件の
  fixture・oracle・Owner・status・証拠の台帳。
- `evidence/sv44/ai-native-baseline-verification.md`: 検証記録。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/
  README.md`: index 再生成（生成物）。

### Decisions During Implementation

- **D-1 7 文書鎖 + evidence**: Issue #21（media-render）の
  先例に従い、spec→design→plan→task→test-spec→impl-ir/exp
  の 7 文書鎖と `evidence/sv44/` を canonical 配置。
- **D-2 AIB/AHG 要件番号**: Issue #44 の AIB-001〜007（製品・
  権利・安全性）と AHG-001〜011（Pluggable Hybrid/Gateway/
  MCP/3 Mode/Explicit Target/Grant/Scope/Apply/Validity/
  受入）をそのまま要件 ID として採用。受入 8 件は AIB-AC-01〜08
  に対応付け。
- **D-3 合意済み/未決定の分離**: Issue #44 が「後続議論で
  決めること」「未決定の詳細設計論点」として保持する事項を
  Deferred Design Concerns に集約し、独立した受入 Gate に
  しないことを明示。
- **D-4 Owner 境界表**: Synvelis（DAW 意味論・AI ポリシー・
  Grant/Approval 権威・ProjectController・MCP Adapter）と
  Ruviform（generic primitive）・Agent Reasoning Runtime
  （交換可能）を責務別に固定。#45/#46/#114 への委譲を明示。
- **D-5 status 語彙**: `Pass`/`Fail`/`Unsupported`/`NotRun`/
  `Pass(部分)` を使用。headless Pass は `Pass(部分)`、未実装・
  未検証は `NotRun`/`Unsupported`、実機/GUI/OS/Provider/MCP
  Client/承認 UI は別 status。
- **D-6 fixture 表記**: `file::test_fn` 形式で `sec*`/`data*`/
  `ahg*`/`rec*`/`gt*`/`sys*`/`gw10f_*`/`gw10c_*`/`e2e1_*`/
  `mcp_adapter.rs` 系を記述。実在 test 関数名を grep で確認。
- **D-7 index 再生成**: `doc_maintenance.js index apply`・
  `sync_impl_index.js` で生成、手編集しない。

## Related Experiments

### Adopted

- `docs/impl/exp/ai-native-baseline.md`（E1〜E4: 文書鎖構造・
  AIB/AHG 写像・Owner 境界・evidence 語彙の設計判断）

### Rejected

- なし

## Validation

`evidence/sv44/ai-native-baseline-verification.md` を正本とする。
実行した検証:

- `mise run fmt-check` — exit 0
- `mise run gw10` — exit 0（gw10 86 tests pass、strict report
  G0〜G6/GTV PASS、missing evidence: SYS-02-ux/SYS-03-ux/
  SYS-05-powerloss/CONSUMER-RUVIFORM/CONSUMER-AUDIOLENS、
  UNSUPPORTED: SYS-07-rt）
- `mise run test synvelis-gateway` — exit 0（406 tests pass）
- `doc_status.js lint --type {spec,design,plan,task,test-spec}`
- `doc_maintenance.js index apply`
- `audit_impl_record.js --json`・`audit_experiment_log.js --json`
- `sync_impl_index.js --kind ir|exp --apply`
- `audit_docs.js --type all --json`
- `route_graph.js --focus ai-native-baseline --json`
- `review-protocol`

## Risks

- **R-IR-1 headless Pass の過剰主張**: `Pass(部分)` を `Pass`
  に繰り上げると実機/製品受入と混同される。→ evidence 語彙で
  区別、未実装・未検証は `NotRun`/`Unsupported`。
- **R-IR-2 #45/#46 との境界崩壊**: Command Gateway の型・
  実装・製品受入を本 slice が吸収すると責務が重複する。→
  Owner 境界表と relations で #45/#46/#114 を固定。
- **R-IR-3 未決定事項の Gate 化**: 「後続議論で決めること」を
  受入条件にすると実装を過剰拘束する。→ Deferred Design
  Concerns で「独立した受入 Gate ではない」と明示。
- **R-IR-4 fixture 陳腐化**: 参照する fixture 名が実装と
  乖離する。→ case ledger に fixture path を明記、lint で検証。
- **R-IR-5 baseline blocker**: `route_graph.js` が報告する
  `artifact-graph` + `broken-relation` は pre-existing repository
  baseline であり本 slice 範囲外。修復せず baseline として記録。

## Follow-ups

- #45 Capability/Command Gateway 詳細型契約（Grant/Principal/
  Scope/Approval/ChangeSet の具体 Rust 型・Schema・wire・
  永続化）
- #46 MCP/Permission UX/RT/AI OFF 製品受入（実外部 MCP
  Client/Provider・承認 UI・GPUI/OS/Privacy）
- #114 GUI/OS/実機 lane（実 device/plugin・実 OS 受入）
- #23 配下の最小 AI E2E PoC/失敗系テスト/他 Consumer 互換
  テストと既存実行計画への接続
- 未決定事項（Grant UI・Context・Privacy/Retention・Provider
  契約・Agent 自律性・具体 Budget・MCP Tool Schema/Transport）
  の確定
