---
id: "TSPEC-zAhBUfpnF97afaGCvStJpY"
type: "test-spec"
status: "approved"
title: "AI-Native Hybrid DAW Baseline — Grant/Principal/Scope/Approval/ChangeSet・AI Off 既定・3 Mode 責務設計 テスト仕様 (synvelis#44)"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/ai-native-baseline-design.md"
    - "docs/specs/ai-native-baseline.md"
    - "docs/specs/product-value-core.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/workspace-model.md"
    - "docs/specs/media-render.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/transport-integration.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/ai-native-baseline-design.md"
  derived-by: []
  verifies:
    - "docs/specs/ai-native-baseline.md"
    - "docs/designs/ai-native-baseline-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  defers: []
  deferred-by: []
---

# AI-Native Hybrid DAW Baseline — テスト仕様 (synvelis#44)

## Purpose

Issue #44 の AIB-001〜007・AHG-001〜011 と受入条件
AIB-AC-01〜08 について、「何を・どの fixture/oracle で・
どの status で検証するか」を固定する。本 slice は docs/
設計契約 + evidence 台帳のみ。実装・GUI・実 device/plugin/OS・
実外部 Provider/MCP Client 受入は対象外。

## Feature

AI-Native Hybrid DAW Baseline の設計契約が、AIB-001〜007 と
AHG-001〜011 を満たし、全要件が既存 `synvelis-gateway`/
`mcp-poc` の headless fixture・#45/#46・#23 配下の実装
Issue・実機 lane へ一貫して写像されること。

## Rules

### AIB-T-R-01 要件網羅

- spec の AIB-001〜007・AHG-001〜011 全てが design・plan・
  test-spec・evidence で参照される。
- AIB-AC-01〜08 が acceptance criteria として spec に存在し、
  evidence ledger で被覆状況が記録される。

### AIB-T-R-02 ケース写像

- AIB-001〜007・AHG-001〜011・AIB-AC-01〜08 の全要件が case
  ledger に存在し、fixture・oracle・Owner・status・証拠を持つ。
- 各ケースの status は `Pass`/`Fail`/`Unsupported`/`NotRun`/
  `Pass(部分)` のいずれかで、実行した検証と一致する。

### AIB-T-R-03 Owner 境界

- Synvelis（DAW 意味論・AI ポリシー・Grant/Approval 権威・
  ProjectController・MCP Adapter）と Ruviform（generic
  Graph/Time/Audio/Plugin/Media/Offline/Job/Observability）・
  Agent Reasoning Runtime（交換可能）の Owner 境界が design・
  plan・evidence で一貫する。
- #45（型契約）・#46（実装/製品受入）・#114（実機 lane）への
  委譲が明示され、Command Gateway の型・実装を本 slice が
  吸収しない。

### AIB-T-R-04 Evidence Honesty

- headless fixture の Pass は `Pass(部分)` として記録され、
  実機/GUI/OS/外部 Provider/実 MCP Client/ユーザー承認 UI
  受入に繰り上げない。
- 未実装・未検証（実外部 Provider・実 GUI・実機 RT・実
  plugin・実 OS lane）は `NotRun`/`Unsupported`。
- 設計レビュー Pass と実測 Pass を区別する。
- `Pass`/`Fail`/`Unsupported`/`NotRun` は実際に実行した
  検証のみを反映する。

### AIB-T-R-05 合意済み/未決定の分離

- AIB-001〜007・AHG-001〜011・受入 8 件（合意済み）と、
  「後続議論で決めること」「未決定の詳細設計論点」（未合意）
  が spec/design/evidence で区別される。
- 未決定事項が独立した受入 Gate として無条件に追加されない。

### AIB-T-R-06 文書鎖整合

- spec→design→plan→task→test-spec→impl-ir/exp→evidence の
  依存が front matter relations で一貫する。
- index が再生成され、lint/audit/route が実行・記録される。

## Case Map（概要 — 詳細は evidence/sv44/ai-native-baseline-case-ledger.md）

Fixture 表記は `file::test_fn`。`sec*`/`data*`/`ahg*`/`rec*`/
`gt*`/`sys*`/`gw10f_*`/`gw10c_*`/`e2e1_*` は
`crates/synvelis-gateway/tests/` 内の test 関数名、
`mcp_adapter.rs` 系は同 crate の MCP adapter test。

| 要件 | 要求 | Fixture/Oracle | Owner | Status |
|---|---|---|---|---|
| AIB-001 | AI なしで完結・同一 Project 正本 | `gw10_system.rs::e2e1_ai_off_human_pipeline_commits_undoes_and_recovers`・`authorization.rs::sec01_human_needs_no_grant` | #44/#17 | Pass(部分) |
| AIB-002 | AI 既定 OFF・意図的利用 | `mcp_adapter.rs::disabled_adapter_denies_the_surface`・`e2e1_*`・AI Off 系 | #44/#46 | Pass(部分) |
| AIB-003 | 明示指示・Proactive 分離 | `authorization.rs::ahg007_*` 系 | #44/#46 | Pass(部分) |
| AIB-004 | 共通 Command/履歴・状態分離 | `gw10_project.rs::gt005`〜`gt010`・`gw10c_*`・`gw10_system.rs::sys05_sys06`・`gtv002` | #44/#17 | Pass(部分) |
| AIB-005 | Rights/来歴/透明性・Unknown | `gw10_authorization.rs::data01`〜`data10`・`data_effect_*`・`file_store_*`・`tampered_secret_hash_fails_closed` | #44/#46 | Pass(部分) |
| AIB-006 | 技術的強制・RT 安全・保証境界 | `gw10_system.rs::sys07_rt_boundary_is_recorded_not_claimed`・`gw10_persistence.rs::rec11_*`・`sec14_*` | #44/#46 | Pass(部分) |
| AIB-007 | モデル非依存・Provider 交換可能 | （設計上の非対象 — Provider 契約は未決定） | #45/#46 | NotRun |
| AHG-001 | Pluggable Hybrid・Synvelis 権威 | `gw10_*` 全体・`e2e1_*` | #44/#45/#46 | Pass(部分) |
| AHG-002 | MCP 初期連携・Adapter 位置づけ | `gw10_mcp.rs::gw10f_*`・`mcp_adapter.rs` 系 | #44/#46 | Pass(部分) |
| AHG-003 | Gateway 論理契約 6 要素 | `gw10_contract.rs`・`gw10_authorization.rs` | #45/#46 | Pass(部分) |
| AHG-004/AHG-009 | 3 Apply Mode・Approval 評価 | `authorization.rs::ahg009_*`・`sec08_*`・`gw10_mcp.rs::gw10f_*`・`mcp_adapter.rs` 3 mode 系・`gw10_transport.rs` | #44/#46 | Pass(部分) |
| AHG-005 | 既存設計整合・Single Writer | `e2e1_*`・`gw10_project.rs::gw10c_*`・`gw10_persistence.rs::rec10_*` | #44/#17 | Pass(部分) |
| AHG-006 | Explicit Target・UI 状態非依存 | `gw10_authorization.rs::sec07_selection_changes_do_not_retarget_the_grant`・`gw10_system.rs::sys03_*`・`gt004_*` | #44/#2/#8 | Pass(部分) |
| AHG-007 | Grant/Policy 論理型・Default Deny | `authorization.rs::sec01`〜`sec04`・`sec12_*`・`sec13_*`・`ahg007_*`・`ahg010_*` | #44/#45 | Pass(部分) |
| AHG-008 | Scope/効果評価・継承禁止 | `authorization.rs::sec05_*`〜`sec11_*`・`sec14_*`・`gw10_concurrency.rs::ac05_*` | #44/#45 | Pass(部分) |
| AHG-010 | Validity/Revoke/Commit 順序 | `gw10_concurrency.rs::rec01`〜`rec03`・`gtv005_*`・`ac05_*`・`gw10_persistence.rs::rec07`〜`rec12`・`gw10_system.rs::sys02`/`sys04` | #44/#46 | Pass(部分) |
| AHG-011 | 受入 8 件・変更管理 | `evidence/sv44/` 台帳・AIB-AC-01〜08 写像 | #44 | Pass(部分) |
| AI-GATE | 独立第四軸・AI Off でも A を壊さない | `e2e1_*`・`gw10_system.rs::sys01`〜`sys10` 系 | #44/#23 | Pass(部分) |
| AIB-AC-01 | 未認証/Grant なし/Scope 外/未許可 Read・送信拒否 | `gw10_authorization.rs::sec01`〜`sec05`・`sec13`/`sec14`・`data01`〜`data10`・`gw10_mcp.rs::gw10f_*` | #44/#45/#46 | Pass(部分) |
| AIB-AC-02 | UI Selection 変更で対象不変 | `sec07_selection_changes_do_not_retarget_the_grant`・`sys03_*` | #44/#2 | Pass(部分) |
| AIB-AC-03 | Track 権限の無条件継承なし・固定/動的 Scope | `sec05_*`・`sec09_*`・`sec10_dynamic_*` | #44/#45 | Pass(部分) |
| AIB-AC-04 | 範囲外 Note 移動/新規 Clip/所属変更/Unknown 安全判定 | `sec06_move_*`・`sec11_create_*`・`sec14_*` | #44/#45 | Pass(部分) |
| AIB-AC-05 | 3 Mode の適用権限分離 | `ahg009_*`・`sec08_*`・`gw10f_*`・`mcp_adapter` 3 mode | #44/#46 | Pass(部分) |
| AIB-AC-06 | Delegated Apply のみで Proactive 不可 | `ahg007_delegated_grant_is_not_proactive_authority`・`ahg007_*` | #44/#46 | Pass(部分) |
| AIB-AC-07 | Revoke/Revision/Tamper/Replay の安全処理 | `rec01`〜`rec12`・`sec09`/`sec10`/`sec11`/`sec12`・`gtv005_*`・`rec05_rec06_*` | #44/#46 | Pass(部分) |
| AIB-AC-08 | AI/MCP 無効時の通常 DAW 継続 | `e2e1_ai_off_human_pipeline_*`・`sec01_human_needs_no_grant`・`disabled_adapter_denies_the_surface` | #44/#23 | Pass(部分) |

## Examples

### EX-1: AIB-AC-05 3 Apply Mode（Pass(部分)）

- **Given**: `mode_grant`/`base_grant` で Suggest/Approve-to-
  Apply/Delegated Apply の各 Grant を構成。
- **When**: `authorization.rs::ahg009_suggest_mode_denies_apply_
  but_permits_prestages`・`ahg009_approve_to_apply_requires_
  approval`・`ahg009_delegated_apply_permits_inside_budget`・
  `ahg009_apply_mode_comes_from_grant_not_request` を実行。
- **Then**: Suggest は Apply を拒否し prestage のみ許可、
  Approve-to-Apply は承認 receipt を要求、Delegated Apply は
  Grant の Scope/budget 内のみ自動適用、mode は Request でなく
  Grant から来る。
- **Status**: Pass(部分) — headless。実 MCP Client/承認 UI/
  実 Provider は NotRun。

### EX-2: AIB-AC-07 Revoke/競合/冪等/Crash（Pass(部分)）

- **Given**: `rec01`/`rec02` で revoke-first/commit-first の
  両順序を構成、`rec03` で同一 base への 2 commit、`rec05_06`
  で keyed retry、`rec07`〜`rec12` で crash point を構成。
- **When**: `gw10_concurrency.rs`・`gw10_persistence.rs` を実行。
- **Then**: revoke-first は commit を拒否、commit-first は
  失効だけで自動 Undo しない、one-winner・keyed retry は
  replay/conflict、crash は committed/indeterminate に回復し
  torn state を残さない。
- **Status**: Pass(部分) — headless。実 power-loss/実機
  durability は NotRun。

### EX-3: AIB-AC-08 AI/MCP 無効時の通常 DAW（Pass(部分)）

- **Given**: AI/MCP を無効化した状態で human principal のみ。
- **When**: `gw10_system.rs::e2e1_ai_off_human_pipeline_commits_
  undoes_and_recovers`・`authorization.rs::sec01_human_needs_no_
  grant`・`mcp_adapter.rs::disabled_adapter_denies_the_surface`
  を実行。
- **Then**: Grant なしで human pipeline が commit/undo/recover
  し、disabled adapter は surface を deny するが通常 DAW 経路
  は継続する。
- **Status**: Pass(部分) — headless。実 GUI/OS での Save/
  Playback/Export 継続は #114 の別 Gate。

## Guarantees

- 全要件（AIB-001〜007・AHG-001〜011・AIB-AC-01〜08）が case
  ledger に存在し、status が実行した検証と一致する。
- headless Pass が実機/GUI/OS/外部 Provider/実 MCP Client/
  ユーザー承認 UI 受入に繰り上げられない。
- 合意済み AIB/AHG と未決定事項が区別され、未決定事項が受入
  Gate 化されない。
- #45/#46/#114 への委譲境界が一貫する。
- Grant/Principal/Scope/Approval/ChangeSet・3 Mode/Proactive・
  AI Off の Owner 境界が design・plan・evidence で一貫する。

## Non-goals

- 実装コードの作成・変更。
- GUI・実 device・実 plugin・実 OS・実外部 Provider・実 MCP
  Client・ユーザー承認 UI での受入。
- Command Gateway の型・実装（#45/#46 が Owner）。
- 通常 DAW 利用への AI Grant/Provider 接続の強制。
- Ruviform 汎用 Gateway/認証 Provider の重複実装。
- 未決定事項（Grant UI・Context・Privacy・Provider・Agent
  自律性・Budget・MCP Schema/Transport）の確定。

## Risk

- **R-TS-1 status の誤分類**: headless Pass を `Pass` と
  記録すると実機/製品受入と混同される。→ `Pass(部分)` を使用し、
  実機/GUI/OS/Provider/MCP Client/承認 UI は別 status
  （`NotRun`/`Unsupported`）として明示。
- **R-TS-2 fixture の陳腐化**: 参照する fixture 名が実装と
  乖離する。→ case ledger に fixture path を明記し、lint/
  audit で link を検証。
- **R-TS-3 合意済み/未決定の混同**: 未決定事項を受入 Gate 化
  すると実装を過剰拘束する。→ AIB-T-R-05 で分離を必須化。
- **R-TS-4 Owner 境界の崩壊**: #45/#46/#114 への委譲が漏れると
  Command Gateway の型・実装を本 slice が吸収してしまう。
  → AIB-T-R-03 で委譲を必須化。
