---
id: "PLAN-IEMv5jVeXCZss5Jrrf2uC9"
type: "plan"
status: "approved"
title: "AI-Native Hybrid DAW Baseline — Grant/Principal/Scope/Approval/ChangeSet・AI Off 既定・3 Mode 責務設計 実装計画 (synvelis#44)"
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
    - "https://github.com/scarecrow173/synvelis/issues/44"
  implemented-by: []
  depends-on:
    - "docs/designs/ai-native-baseline-design.md"
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
    - "docs/designs/ai-native-baseline-design.md"
  derived-by:
    - "docs/tasks/ai-native-baseline-task.md"
  verifies: []
  verified-by:
    - "docs/test-specs/ai-native-baseline.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  defers: []
  deferred-by: []
---

# AI-Native Hybrid DAW Baseline — 実装計画 (synvelis#44)

本計画は `docs/specs/ai-native-baseline.md` + `docs/designs/
ai-native-baseline-design.md` の実装分解。**本 slice は docs/
設計契約 + evidence 台帳のみ**であり、Command Gateway の型・
実装は #45、MCP/Permission UX/RT/AI OFF 製品受入は #46/#114
が Owner。本 plan は「AIB-001〜007/AHG-001〜011 と受入条件
8 件をどの fixture/Issue/Owner で満たすか」の対応表と、未実装・
未検証領域の後続 Issue への委譲を固定する。

## Goal

Issue #44 の合意済み不変条件（AIB-001〜007・AHG-001〜011）と
受入条件 8 件（AIB-AC-01〜08）を、既存 `synvelis-gateway`/
`mcp-poc` の headless fixture・#45/#46・#23 配下の実装 Issue・
実機 lane へ写像し、Owner 境界と evidence 語彙を固定する。
設計完了を実装完了・実機受入とみなさない。

## Scope

### In Scope（本 slice）

- 7 文書鎖（spec/design/plan/task/test-spec/impl-ir/impl-exp）
  + `evidence/sv44/`（case ledger + verification）
- AIB-001〜007・AHG-001〜011・AIB-AC-01〜08 の fixture/Issue/
  Owner/status 写像
- Grant/Principal/Scope/Invocation Target/Approval Receipt/
  ChangeSet・3 Mode/Proactive・AI Off の Owner 境界表
- #45（型契約）・#46（実装計画/製品受入）・#114（GUI/OS/
  Privacy）への委譲境界
- index 再生成・docs 検証・review-protocol・PR

### Out of Scope

- 実装コード（`crates/`・`Cargo.toml`・`Cargo.lock`・
  `mise.toml`・CI・Ruviform）
- GUI 実装・実 device/plugin・実 OS 受入・実外部 MCP
  Client/Provider・ユーザー承認 UI
- 通常 DAW 利用への AI Grant/Provider 接続の強制
- Ruviform 汎用 Gateway/認証 Provider の Synvelis 専用としての
  重複実装
- 未決定事項（Grant UI・Context 取得対象・Privacy/Retention・
  Provider 契約・Agent 自律性・具体 Budget・MCP Tool
  Schema/Transport）の確定

## Upstream

- `docs/specs/ai-native-baseline.md`（AIB-001〜007・AHG-001〜011・
  AIB-AC-01〜08）
- `docs/designs/ai-native-baseline-design.md`（canonical
  concepts・state machines・failure taxonomy・Owner 表・
  requirement 写像・alternatives）
- `docs/designs/gw-01-gateway-contract-design.md`・
  `docs/designs/gw-02-authorization-policy-design.md`（#44 の
  不変条件を継承する実装側先行設計）
- `docs/designs/midi-source-note-editing-design.md`（#52 —
  限定 MIDI 編集の対象 domain）
- `docs/impl/ir/gw-01`〜`gw-10`（GW-01〜10 headless 実装記録）
- 確定済み縦断契約: product-value-core/workspace-model/
  context-inspector/timeline-editing/project-model/
  project-persistence/playback-graph/musical-time/
  transport-integration/media-render/vcs-02-semantic-diff/
  ed-a-editing-model

## File Map

| File | 種別 | 状態 |
|---|---|---|
| `docs/specs/ai-native-baseline.md` | spec | 本 slice で作成 |
| `docs/designs/ai-native-baseline-design.md` | design | 本 slice で作成 |
| `docs/plans/ai-native-baseline.md` | plan | 本 slice で作成 |
| `docs/tasks/ai-native-baseline-task.md` | task | 本 slice で作成 |
| `docs/test-specs/ai-native-baseline.md` | test-spec | 本 slice で作成 |
| `docs/impl/ir/ai-native-baseline.md` | impl record | 本 slice で作成 |
| `docs/impl/exp/ai-native-baseline.md` | experiment record | 本 slice で作成 |
| `evidence/sv44/ai-native-baseline-case-ledger.md` | evidence | 本 slice で作成 |
| `evidence/sv44/ai-native-baseline-verification.md` | evidence | 本 slice で作成 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | index | 再生成のみ |

## Implementation Streams（Owner 別）

本 slice は実装を行わない。以下は「どの実装 Issue/fixture が
どの AIB/AHG/受入条件を満たすか」の対応表。

### Stream A: Synvelis AI-Native Baseline 契約（本 Issue #44）

| 領域 | Owner | 現状 | 後続 |
|---|---|---|---|
| AIB-001〜007/AHG-001〜011 の製品・権限・状態契約 | #44（本 slice） | docs 設計契約 + headless 写像 | #45 型契約・#46 実装計画 |
| 受入条件 AIB-AC-01〜08 の headless 被覆 | GW-01〜10（`synvelis-gateway`/`mcp-poc`） | `mise run gw10` G0〜G6/GTV PASS（headless） | #46 製品受入・#114 実機 |

### Stream B: #45 Capability/Command Gateway 詳細型契約

| 領域 | Owner | 現状 | 後続 |
|---|---|---|---|
| Grant/Principal/Scope/Approval/ChangeSet の具体 Rust 型・Schema・wire・永続化 | #45 | 未着手（v0.3 設計提案/レビュー待ち） | #45 が本設計の論理契約を継承 |
| Authorization 評価・Commit Gate・線形化手法 | #45/#46 | headless 実装済み（GW-02/06） | #45 で型契約を確定 |

### Stream C: #46 MCP/Permission UX/RT/AI OFF 製品受入

| 領域 | Owner | 現状 | 後続 |
|---|---|---|---|
| MCP Tool Schema/Transport・接続認証・権限 UX・AI Off UI・Provenance/権利 UI | #46 | 未着手 | #46 が製品受入を実施 |
| 実外部 MCP Client/Provider・ユーザー承認 UI・GPUI/OS/Privacy | #46/#114 | NotRun | #46/#114 の別 Gate |

### Stream D: #23 配下 通常 DAW / AI 独立第四軸

| 領域 | Owner | 現状 | 後続 |
|---|---|---|---|
| Gate A/B/C 通常 DAW 受入・AI 独立第四軸の区別 | #1/#8/#23 | 既存 spec/plan で管理 | #23 実行計画・#46 AI Gate |
| ProjectController Single Writer・非破壊 Save/Undo | #17 | 確定済み（`project-persistence`） | #45/#46 が境界を維持 |

### Stream E: Ruviform generic capability（参照のみ）

| 責務 | 状態 |
|---|---|
| Generic Graph/Time/Audio/Plugin/Media/Offline/Job/Observability | Ruviform が所有・Synvelis は利用。本 slice では変更しない |
| 汎用 Gateway/認証 Provider | Ruviform 境界。Synvelis 専用として重複実装しない。共通化は実需検証後に別途判断 |

## Dependency Graph

```
spec (ai-native-baseline.md)
  └─ design (ai-native-baseline-design.md)
       └─ plan (ai-native-baseline.md) ── task (ai-native-baseline-task.md)
            └─ test-spec (ai-native-baseline.md) ── impl-ir/exp ── evidence/sv44/
```

実装依存（本 slice 外）:

```
#44 (本 slice: docs 契約 + headless 写像)
  → #45 (型契約: Grant/Scope/Approval/ChangeSet の具体化)
  → #46 (MCP/Permission UX/RT/AI OFF 製品受入)
  → #114 (GUI/OS/Privacy 実機 lane)
  → #23 (通常 DAW Gate A/B/C・AI 独立第四軸の区別)
```

## Risk Register

| Risk | 影響 | 対策 |
|---|---|---|
| headless PASS を製品/実機受入と誤認 | 偽の完了主張・Gate 誤判定 | evidence 台帳で `Pass(部分)`/`NotRun`/`Unsupported` を分離。実 GUI/OS/Provider/実機は #46/#114 の別 Gate |
| #45/#46 が不変条件を継承せず詳細契約が矛盾 | セキュリティ境界の崩壊 | relations で #45/#46 を固定し、AIB/AHG → fixture 写像を台帳化。矛盾時は本 spec/design を更新 |
| 未決定事項を受入 Gate 化し実装を過剰拘束 | 実装遅延・不要な複雑化 | Deferred Design Concerns で「独立した受入 Gate ではない」と明示。合意済み AIB/AHG のみ受入対象 |
| AI Off 既定が通常 DAW 経路へ影響し Gate A を壊す | 通常 DAW 非独立 | `e2e1_ai_off_human_pipeline_*` で human pipeline を headless 確認。製品 Gate A 判定は #23/#1 |
| Grant/Scope/Approval 論理契約が #45 の具体型で実現不能 | 設計と実装の乖離 | 本設計は論理契約に限定。具体 Rust 型・永続化・線形化は #45/#46 の未確定事項 |
| Ruviform 汎用基盤へ DAW 固有意味論を混入 | Consumer 間の責務汚染 | Owner 分離表で Ruviform の責務を固定。Synvelis 固有の Grant/Scope/MCP を Ruviform へ移さない |

## Verification Plan

本 slice の検証（docs/ 設計契約 + evidence のみ）:

1. `mise run fmt-check` — Markdown/構成ファイル整形
2. `node .agents/skills/doc-status/scripts/doc_status.js lint
   --type {spec,design,plan,task,test-spec} --json`
3. `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
   index apply`（触った dir の index 再生成）
4. `node .agents/skills/impl-doc/scripts/audit_impl_record.js
   --json`・`audit_experiment_log.js --json`
   ・`sync_impl_index.js --kind ir|exp --apply`
5. `node .agents/skills/doc-status/scripts/audit_docs.js
   --type all --json`
6. `node .agents/skills/doc-driven-dev-graph/scripts/
   route_graph.js --focus ai-native-baseline --json`
7. `review-protocol` — 設計契約のレビュー（blocker なしを承認）
8. `mise run gw10`・`mise run gw10-report`・`mise run test
   synvelis-gateway` — headless 回帰入口（実測を evidence に記録）

実装検証は対象外（#45/#46/#114 が Owner）。headless fixture
の Pass は evidence ledger で「Pass(部分)」として記録し、実機/
GUI/OS/外部 Provider/実 MCP Client 受入に繰り上げない。

## Rollback

- 本 slice は docs/ + evidence/ のみ。revert で完全に戻る。
- index 再生成は生成物のため、revert 時に再生成で整合。
- 実装への影響なし（コード変更なし）。

## Task Breakdown

| Task | 内容 | 状態 |
|---|---|---|
| T-1 | spec 作成（AIB-001〜007・AHG-001〜011・AIB-AC-01〜08） | 本 slice |
| T-2 | design 作成（canonical concepts・state machines・failure・Owner 表・写像） | 本 slice |
| T-3 | plan 作成（file map・stream・dependency・risk・verification） | 本 slice |
| T-4 | task 作成（done-when・verification） | 本 slice |
| T-5 | test-spec 作成（AIB-T-R・case map・example） | 本 slice |
| T-6 | impl-ir/exp 作成 | 本 slice |
| T-7 | evidence/sv44/ case ledger + verification | 本 slice |
| T-8 | index 再生成・docs 検証・review-protocol | 本 slice |
| T-9 | commit・push・PR・worker_done | 本 slice |

後続（本 slice 外・実装/受入 Issue）:

- T-10 #45 Capability/Command Gateway 詳細型契約（Grant/
  Principal/Scope/Approval/ChangeSet の具体 Rust 型・Schema・
  wire・永続化）
- T-11 #46 MCP/Permission UX/RT/AI OFF 製品受入（実外部 MCP
  Client/Provider・承認 UI・GPUI/OS/Privacy）
- T-12 #114 GUI/OS/実機 lane（実 device/plugin・実 OS 受入）
- T-13 #23 配下の最小 AI E2E PoC/失敗系テスト/他 Consumer
  互換テストと既存実行計画への接続
