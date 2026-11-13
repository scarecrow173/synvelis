---
id: "EXP-T8VY4WjhgxpKHUgVhLbtob"
type: "experiment"
status: "completed"
title: "AI-Native Hybrid DAW Baseline — Issue #44 設計契約文書化と AIB/AHG 写像の試行"
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
  implemented-by:
    - "docs/impl/ir/ai-native-baseline.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ai-native-baseline-design.md"
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/ai-native-baseline-task.md"
    - "docs/test-specs/ai-native-baseline.md"
  defers: []
  deferred-by: []
---

# AI-Native Hybrid DAW Baseline — Issue #44 設計契約文書化と AIB/AHG 写像の試行

## 問題設定

Issue #44 は AIB-001〜007（製品・権利・安全性の基盤不変条件）
と AHG-001〜011（Pluggable Hybrid/Gateway/MCP/3 Mode/Explicit
Target/Grant/Scope/Apply/Validity/受入）を Approved baseline
として保持するが、canonical spec/design/plan/task/test-spec/
impl の 7 文書鎖と evidence 台帳が未整備だった。GW-01〜10 の
headless 実装（`crates/synvelis-gateway`・`crates/mcp-poc`）と
`mise run gw10` の G0〜G6/GTV 証拠は存在するが、「合意済み
設計のどの不変条件がどの fixture で被覆され、何が未被覆か」
を一枚の契約表に固定する必要があった。headless PASS を製品
受入・実機受入に繰り上げないための evidence 語彙も必要。

## 試行と結果

### E1: 上流契約と実装の照合

Issue #44 本文の AIB-001〜007/AHG-001〜011 を読み、GW-01〜10
の impl 記録（`docs/impl/ir/gw-01`〜`gw-10`）と `crates/
synvelis-gateway/tests/gw10_*.rs`・`authorization.rs`・
`mcp_adapter.rs` の test 関数を grep で棚卸しした。

- 結果: `sec01`〜`sec14`・`data01`〜`data10`・`ahg007_*`・
  `ahg009_*`・`ahg010_*`・`rec01`〜`rec12`・`gt004`〜`gt010`・
  `sys01`〜`sys10`・`gw10f_*`・`gw10c_*`・`e2e1_*`・
  `mcp_adapter.rs` 系の test 関数が AIB/AHG の各不変条件に
  対応することを確認。`mise run gw10` は G0〜G6/GTV PASS、
  `mise run test synvelis-gateway` は 406 tests pass。
- 判断: headless fixture は `Pass(部分)` として記録し、
  実機/GUI/OS/外部 Provider/実 MCP Client/ユーザー承認 UI
  受入は `NotRun`/`Unsupported` で分離する。

### E2: Owner 境界の固定

Synvelis（DAW 意味論・AI ポリシー・Grant/Approval 権威・
ProjectController・MCP Adapter）と Ruviform（generic
Graph/Time/Audio/Plugin/Media/Offline/Job/Observability）・
Agent Reasoning Runtime（交換可能）・#45/#46/#114（後続
Owner）の境界を design の Owner 分離表に固定した。

- 結果: Command Gateway の型・実装は #45、MCP/Permission UX/
  RT/AI OFF 製品受入は #46/#114 が Owner であることを明示。
  Ruviform 汎用 Gateway/認証 Provider を Synvelis 専用として
  重複実装しないことを確認。
- 判断: 本 slice は docs/ 設計契約 + evidence のみであり、
  実装コード・`Cargo.toml`/`Cargo.lock`/`mise.toml`/CI/
  Ruviform を変更しない。

### E3: 合意済み/未決定の分離

Issue #44 が「後続議論で決めること」「未決定の詳細設計論点」
として保持する事項（Grant UI・Context 取得対象・Privacy/
Retention・Provider 契約・Agent 自律性・具体 Budget・MCP
Tool Schema/Transport・各受入試験の実証結果等）を spec の
Deferred Design Concerns に集約した。

- 結果: 合意済み AIB-001〜007/AHG-001〜011/受入 8 件のみを
  受入条件・証拠写像の対象とし、未決定事項は「独立した受入
  Gate ではない」と明示。
- 判断: 未決定事項を受入 Gate 化すると実装を過剰拘束する
  ため、spec/design/evidence で分離を必須化（AIB-T-R-05）。

### E4: 文書鎖生成と index/relation 整合

spec→design→plan→task→test-spec→impl-ir/exp の 7 文書鎖を
canonical front matter・relations・内容で作成し、
`evidence/sv44/` に case ledger + verification を配置した。

- 結果: `doc_maintenance.js index apply` で index を再生成、
  `doc_status.js lint`/`audit_docs.js`/`route_graph.js`/
  `audit_impl_record.js`/`audit_experiment_log.js`/
  `sync_impl_index.js`/`fmt-check`/`gw10`/`test` を実行。
  `route_graph.js` は pre-existing baseline blocker
  （`artifact-graph` + `broken-relation`）を報告するが、本
  slice の新規文書が起因する blocker ではない。
- 判断: baseline blocker は範囲外として修復せず、verification
  に baseline として記録する。

## 採用した判断

- **7 文書鎖 + evidence**: Issue #21（media-render）の先例に
  従い canonical 配置。
- **AIB/AHG 要件番号**: Issue #44 の AIB-001〜007/AHG-001〜011
  をそのまま要件 ID として採用。受入 8 件は AIB-AC-01〜08 に
  対応付け。
- **合意済み/未決定の分離**: Deferred Design Concerns で
  未決定事項を受入 Gate から除外。
- **Owner 境界表**: Synvelis/Ruviform/Agent Runtime/#45/#46/
  #114 を責務別に固定。
- **status 語彙**: `Pass`/`Fail`/`Unsupported`/`NotRun`/
  `Pass(部分)` を使用。headless Pass は `Pass(部分)`、未実装・
  未検証は `NotRun`/`Unsupported`。
- **fixture 表記**: `file::test_fn` 形式で実在 test 関数名を
  記述。
- **index 再生成**: `doc_maintenance.js index apply`・
  `sync_impl_index.js` で生成、手編集しない。

## 結論

Issue #44 の canonical 設計契約を docs/ 7 文書鎖 + evidence
台帳として作成し、AIB-001〜007/AHG-001〜011 と受入条件
AIB-AC-01〜08 を headless fixture・#45/#46・#23 配下の実装
Issue・実機 lane へ写像した。headless PASS を製品/実機受入に
繰り上げない evidence 語彙を固定し、合意済み事項と未決定事項
を分離した。後続 Owner（#45 型契約・#46 実装/製品受入・
#114 実機 lane・#23 実行計画）が本設計の不変条件を継承する。
