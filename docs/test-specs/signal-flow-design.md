---
id: "TSPEC-034X9GPU9Og63h7UIzLf1r"
type: "test-spec"
status: "approved"
title: "Signal Flow 設計正本 — 受入仕様と検証"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/4"
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
    - "docs/specs/signal-flow-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/test-specs/sf-06-cross-gate-oracle-case-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/signal-flow-design-design.md"
  derived-by: []
  verifies:
    - "docs/designs/signal-flow-design-design.md"
    - "docs/specs/signal-flow-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/109"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  defers: []
  deferred-by: []
---

# Signal Flow 設計正本 — 受入仕様と検証

## Purpose

`docs/designs/signal-flow-design-design.md` が定義する Signal Flow
View の階層・Typed Port/Connection・Semantic Routing 編集・Probe
表示・世代証拠契約が、文書として一貫し追跡可能であることを検証
する。本 test-spec は設計正本の**文書受入**を対象とし、実装
コードの動作検証は #109（SF-06 oracle）が担う。実機/実 GUI
部分は本書の検証範囲外であり、NotRun として扱う。

## Feature

Signal Flow 設計正本 — 文書受入と scenario 対応

## Rules

- SFD-T1 文書チェーン完全性: spec/design/plan/task/test-spec/ir/
  exp の 7 文書が存在し、front matter の `type`/`status`/`id` が
  一意で、相互 `relations` が既存ファイルまたは外部 URL に解決
  する。
- SFD-T2 要件カバレッジ: spec の SFD-R1〜R8 が design の
  Canonical Design（§A〜E）・Decision Log・Deferred Design
  Concerns のいずれかに対応付いている。
- SFD-T3 受入追跡可能性: Issue #4 の受入項目 6 件それぞれに
  design §E.4 の scenario ID（SFD-HIER-01/02・SFD-ROUTE-01/02・
  SFD-VALID-01・SFD-EDIT-01・SFD-PROBE-01）が 1 つ以上割り当て
  られ、各 ID に既存 Case family 参照と lane 状態がある。
- SFD-T4 語彙整合: design が参照する enum/型名
  （`ProbeResolveState`・`RoutingEditError`・`SignalFlowEdgeRole`・
  `SignalFlowScope`・`SignalFlowNodeRuntime` 等）が
  `crates/synvelis-project`・`crates/synvelis-workspace` の
  実装語彙と一致する。
- SFD-T5 未決分離: Visual Group 保存粒度・Channel Map UI 既定・
  特殊 Feedback・GUI 数値閾値が「提案/決定待ち」として採用決定
  と分離されている。
- SFD-T6 証拠誠実性: lane 状態が headless Pass と実機 NotRun を
  区別し、実機/実 plugin/実 device の受入を Pass と記載しない。
- SFD-T7 Owner 境界: Synvelis（意味論・View）と Ruviform
  （Generic Graph/RT/Probe）の責務分界と #104〜109 実装依存が
  明記され、本タスクが文書のみであることが固定されている。

## Examples

- Given Issue #4 受入項目「ノード移動は DSP 順序に影響せず」
  When design を読む
  Then SFD-EDIT-01 が `visual_move_node` vs `ReorderProcessor`
  の境界を定義し SF-UX-02/03・SF-MODEL-06 を参照する。
- Given `ProbeResolveState` enum
  When design §D.3 を読む
  Then `Requested`/`QueuedForBinding`/`PendingRtInstall`/
  `BoundActive`/`PartiallyRebound`/`Lost`/`Stopped`/
  `Unavailable`/`Unsupported`/`Unresolved`/`StaleGeneration`
  の実装語彙が使われ、Issue 本文の概念名との対応が注記される。
- Given 実機 GUI 未実施
  When scenario 表の lane 状態を読む
  Then SF-UX-08/#114 は NotRun と明記され headless Pass に
  昇格していない。

## Guarantees

- 本 test-spec の検証は文書の存在・整合・追跡可能性に限定され、
  コード変更を要求しない。
- scenario ID は #109 の正式 Case registry を再定義せず、
  設計レベルの追跡子として機能する。
- 未実施の実機検証を Pass と偽らない。

## Non-goals

- `crates/` の実装検証・テスト実行。
- 実 OS GUI・実 Plugin・実 Device・AudioLens consumer の受入
  （#114/#109/#219/#294 の外部 gate）。
- SF-01〜06 各 slice 文書の契約詳細の再検証。
- Ruviform Generic Graph/Probe/Clock の検証。

## Risk

- 文書受入のみでは実装との乖離を検出できない → 後続 slice が
  本書を authoritative 参照として実装し、#109 oracle が実装
  契約を検証する。
- scenario ID と #109 Case ID の混同 → SFD-* 接頭辞と「設計
  追跡子」明記で回避。
- 未決事項が暗黙決定される → Decision Log の「保留」区分と
  Deferred Design Concerns で固定。
