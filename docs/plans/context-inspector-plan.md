---
id: "PLAN-0KH6KhO6yo49yc5HP6kMSN"
type: "plan"
status: "approved"
title: "Context Inspector 設計定義 実施計画 — spec/design/test-spec 文書チェーン + UX Fixture + 被覆台帳（synvelis#5）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/5"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/context-inspector.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/context-inspector-design.md"
    - "docs/tasks/context-inspector-task.md"
    - "docs/test-specs/context-inspector.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/context-inspector.md"
    - "docs/designs/context-inspector-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/context-inspector.md"
  references: []
  defers: []
  deferred-by: []
---

# Context Inspector 設計定義 実施計画

## スコープ

synvelis#5 は設計 Issue であり、本 slice の成果物は **設計契約の
文書化**である。対象は `docs/` の spec→design→plan→task→
test-spec→impl(ir/exp) チェーンと `evidence/sv5/` の受入条件
被覆台帳。**Inspector View・Provider・状態機械・一括編集・Probe
UI の製品実装は含まない**（#112/#108/#107 が Owner）。
編集範囲は task spec の「編集してよい範囲」に限定する。

## ファイルマップ

| ファイル | 内容 |
| --- | --- |
| `docs/specs/context-inspector.md` | What/Why/CI-01〜13 受入条件/Non-goals |
| `docs/designs/context-inspector-design.md` | Owner 境界・既存実装対応表・TargetKind union・状態機械・情報階層・一括編集・Probe・永続化・Decision Log D1〜D19・Scenario/Oracle |
| `docs/plans/context-inspector-plan.md` | 本計画（スコープ・ファイルマップ・リスク・検証マトリクス・ロールバック） |
| `docs/tasks/context-inspector-task.md` | 本 slice の作業分解 |
| `docs/test-specs/context-inspector.md` | CI-01〜13 に対応する UX Fixture/Rule/Example 定義（将来実装の検証契約） |
| `docs/impl/ir/context-inspector.md` | 本 slice の実施記録 |
| `docs/impl/exp/context-inspector.md` | 実験メモ（graph router 状態・ID 採番・設計上の判断記録） |
| `evidence/sv5/coverage.md` | 受入条件 × 既存 Fixture/実装被覆の台帳（設計のみは NotRun） |
| `docs/<type>/README.md` | doc-maintenance `index apply` による生成物（手編集しない） |

## 後続実装への引き継ぎ（本 slice の対象外）

| 領域 | Owner | 本設計が固定した契約 |
| --- | --- | --- |
| Inspector View/Provider/描画 | #112（SV-GUI-03）・#108（SF-05） | `ContextInspectionTarget` union・Section/Provider DTO・状態機械・wire 規約 |
| Signal Flow 投影・Port Inspector 連携 | #108 | `SignalFlowEntityRef`/`SemanticProbePoint` への写像、boundary 提示 |
| RT Probe/Probe Broker/Capture | #107 + Ruviform #185/#308/#309 | ProbeMode/lease/観測分類の表示契約（実装は再利用） |
| 実 Window/IME/DPI/Plugin | #114/#176 + Ruviform #313/#314 | GUI-L2/EXT-L3 Gate（headless と分離記録） |

## リスクと対策

| リスク | 対策 |
| --- | --- |
| 既存 `deliver()` が Explore→Follow 復帰を実装していない | 契約と実装のギャップを design D5 と evidence 台帳に明示。link.rs の共有 semantics を変更せず、Inspector 側消費規則として定義。実装は #112/#108 slice |
| `SemanticTarget` が Processor/Parameter/Connection/Probe を運べない | D4 で fail-closed に限定し、Explore/明示 inspect 導線を契約化。「link で全 kind に到達できる」と偽らない |
| `ContextInspectionTarget` が既存 3 focus（inspection_focus/signal_flow/control_relation）と重複 | union は参照の同格な同居であり、各 surface の focus state を統合・置換しない。実装時の state 配置は #112/#108 が既存 `Option` + tolerant decode 規則で決める |
| 設計のみの項目を Pass と誤記 | evidence 台帳で `NotRun`/`Deferred`/`Unsupported` を POLICIES §3 の語彙で固定。headless 既存証拠と設計契約を列で分離 |
| docs index の手編集 | `doc-maintenance` の `index plan`/`index apply` のみ使用 |
| graph router の blocked | 既存 broken relations は本 slice 起因でない（先行 slice と同状態）。文書チェーンは規約どおり作成し、無関係 docs を修復しない |

## 検証マトリクス

| 受入条件群 | 検証 |
| --- | --- |
| CI-01〜13（製品契約） | `docs/test-specs/context-inspector.md` の Rule/Example に写像。現時点の被覆は `evidence/sv5/coverage.md` で台帳化し、設計のみの項目は `NotRun` |
| 文書整合 | `doc_maintenance.js index plan/apply` で index 再生成、`doc-status` 系 audit で新規 finding が無いこと |
| 形式 | `mise run fmt-check`（docs-only slice。Rust 変更なし） |
| レビュー | `review-protocol` による仕様整合・反例レビュー。Confirmed P0/P1 が残らないことを承認条件とする |

## ロールバック

全変更は additive（新規 docs/evidence/index 行のみ）。revert で
文書チェーンと台帳が除去され、既存 spec/design/実装・index の
他行は無変更のまま残る。
