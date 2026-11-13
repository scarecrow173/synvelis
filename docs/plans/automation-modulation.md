---
id: "PLAN-41PTXXqDP41Hx7iG3WXLBl"
type: "plan"
status: "approved"
title: "Automation / Modulation 統合 Parameter Control — 計画（synvelis#6）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/automation-modulation-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/automation-modulation-design.md"
    - "docs/specs/automation-modulation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/automation-modulation-task.md"
    - "docs/test-specs/automation-modulation.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Automation / Modulation 統合 Parameter Control — 計画

## スコープ

**docs-only slice**。synvelis#6 の製品 UX・設計仕様・受入 Fixture・
設計文書チェーン 7 文書と `evidence/sv6/` 受入台帳を作成する。
Rust crate・UI・CI・Cargo/mise 設定の変更は **行わない**。
実装は下流 Issue（#98〜103・#121/#122・#131〜134・#102/#114）が
担当する。

### File Map

| ファイル | 内容 | 状態 |
| --- | --- | --- |
| `docs/specs/automation-modulation.md` | What/Why/AC(AM-01〜13)/Non-goals/Sources | 作成 |
| `docs/designs/automation-modulation-design.md` | Owner 境界・Scope/Target/Time Mapping・画面操作 Fixture・Decision Log(D-01〜20)・Scenario/Oracle・検証戦略 | 作成 |
| `docs/plans/automation-modulation.md` | 本計画 | 作成 |
| `docs/tasks/automation-modulation-task.md` | タスク分解・完了条件 | 作成 |
| `docs/test-specs/automation-modulation.md` | Feature/Rule/Example/Guarantee・Fixture 契約 | 作成 |
| `docs/impl/ir/automation-modulation.md` | 実装記録（docs slice のみ） | 作成 |
| `docs/impl/exp/automation-modulation.md` | 実験記録（router 状態・ID 生成・設計判断） | 作成 |
| `evidence/sv6/coverage.md` | AM-01〜13 × 既存被覆の受入台帳 | 作成 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | 生成 index（doc-maintenance で再生成） | 再生成 |

### スコープ外（触らない）

- `crates/**`、`Cargo.toml`/`Cargo.lock`/`mise.toml`/CI — 実装 Owner
  の領域。本 slice は docs のみ。
- Ruviform 共通 Control/Graph/Plugin 正本 — DAW 固有 Scope を
  持ち込まない。
- `docs/*/README.md` の手編集 — index は `doc-maintenance` の
  `index apply` のみ。
- Issue #6 のクローズ・PR マージ — Coordinator の責務。

## 依存関係

### 上流（消費する既存契約）

- CTL-01 #121 / CTL-02 #122: `ControlOwner × AutomationTimeScope`・
  `ParameterTargetId` 正本。
- PC-A #40 / PC-B #43: Control Model・評価・Dispatch・Note scope。
- PC-C1 #98 / PC-C2 #99 / PC-C3 #100 / PC-C4 #101 / PC-C5 #102 /
  PC-C6 #103: Recording Transaction・Relation・Trace・Plugin
  Binding・Inspector・Oracle。
- AUT-01〜04 #131〜134: Mode/Region/Loop/LOD。
- SF-04 #107 / #5 Context Inspector / #114 SV-GUI-05 / #3/#4:
  Probe・Inspector・共通 UX・E2E。

### 下流（本設計が供給する先）

- #98〜103・#121/#122・#131〜134: Scope/Target/Time Mapping・
  状態機械・Fixture 定義から実装。
- #102/#114/#103: 製品 GUI・Cross Oracle の受入 Fixture。

## リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 旧 Scope 表を新型の代用品にしてしまう | AM-01 違反・二重正本 | design D-02・Scope/Target/Time Mapping 表で 2 軸を明示。spec/design で旧表=表示名と明記 |
| headless 証拠を GUI Pass と誤記 | POLICIES.md 違反・虚偽受入 | coverage.md で `Pass (headless)`/`Partial`/`NotRun`/`Deferred`/`Unsupported` を分離。design D-19 |
| 第二 Control Engine の誘発 | 実装で二重正本 | spec/design で `ControlModel` 参照のみ・新 schema 禁止を明記 |
| 数値閾値の捏造（Touch Return・LOD・精度） | 根拠なき契約 | design「未決定・PoC 後」節・D-16/D-20 で既存契約参照か委譲に限定 |
| 既存 broken relation の巻き込み修復 | 範囲外変更 | router blocked を exp に記録し、無関係な relation は修復しない（context-inspector 先例と同じ） |

## 検証マトリクス

| 検証 | コマンド | 対象 | 期待 |
| --- | --- | --- | --- |
| Markdown/構成の軽量チェック | `mise run fmt-check` | docs のみ（crate 変更なし） | exit 0 |
| index 整合 | `doc_maintenance.js index plan` → `index apply --expect <hash>` → 再 `index plan` | 触れた 7 dir | `in-sync` |
| doc-status 監査 | `doc_maintenance.js`/`doc-status` の確認手順 | docs 全体 | 新規 doc の front matter/関係が valid（既存 broken relation は範囲外として記録） |
| review-protocol | 仕様整合 + 反例/敵対的レビュー | spec→design→test-spec→coverage | Confirmed P0/P1 = 0 |
| Rust build/test | **対象外**（crate 変更なし） | — | NotRun/NotApplicable として記録 |

## Downstream Handoff

- 実装 slice は `docs/designs/automation-modulation-design.md` の
  Scope/Target/Time Mapping・画面操作 Fixture・Decision Log と
  `docs/test-specs/automation-modulation.md` の Fixture 契約を
  消費する。
- `evidence/sv6/coverage.md` が AC ごとの既存被覆と残 Gate を
  示し、#102/#114/#103 が GUI/実機受入の対象を特定する。

## Rollback

- docs-only のため、問題があれば当該ファイルを revert するだけで
  復旧可能。生成 index は `doc-maintenance` で再生成して戻す。
- 実装・永続データ・schema への影響はない。
