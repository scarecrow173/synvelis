# Issue #1 スコープ調査 — 既存 docs との重複・矛盾

Date: 2026-11-14. 目的: synvelis#1「製品価値と中核体験を定義する」
を `docs/` canonical 文書へ正本化するにあたり、既存文書との
重複・矛盾を調査し、解消方針を記録する。

## 調査対象

- Issue #1 本文（製品価値三層構造・中核体験・Gate A/B/C/AI・
  UX 原則・シナリオ A/B/C・検証観点 6 項目・Creative Branch
  追加 2026-11-11）。
- `docs/` 既存文書: `docs/designs/workspace-docking-state-
  architecture.md`、`docs/specs/ws-a-*`、`docs/specs/sv-gui-*`、
  `docs/specs/sf-*`、`docs/specs/pc-c*-*`、`docs/mockups/
  synvelis-daw.html`、`docs/discovery/`。
- 関連 Issue: #2/#3/#4/#5/#6/#8/#23/#44/#46/#102/#103/#106/
  #108/#109/#114/#277〜#288/#28。

## 調査結果

### 重複の評価

| 既存文書 | 内容 | 判定 |
| --- | --- | --- |
| `docs/designs/workspace-docking-state-architecture.md` | Workspace/Docking/Selection/Focus/Command Routing の統合設計。冒頭で Issue #1 を前提と明記 | **重複ではなく下流設計**。AM-3（文脈維持）と AM-5（情報の正確性）の実装側詳細を担う。矛盾なし |
| `docs/specs/ws-a-workspace-foundation-*.md` | Workspace/Selection/LinkGroup/Command/Dock の headless 契約 | AM-3 の補助証拠。製品価値の定義は持たない |
| `docs/specs/sv-gui-01〜04-*.md`、`sv-gpui-03-*.md`、`svt-product-e2e.md` | Desktop bootstrap・view model・GPUI scenario・製品 State/Receipt | AM-1/AM-3 の headless 証拠。Gate 定義の正本ではない |
| `docs/specs/sf-01〜06-*.md` | Signal Flow semantic model・map・probe・cross-gate oracle | AM-2/AM-5 の実装側。製品価値の定義は持たない |
| `docs/specs/pc-c2/c3/c5/c6-*.md` | Control relation index・trace slot・inspector・oracle | AM-4/AM-5 の実装側。状態語彙の対応は design §5 で明示 |
| `docs/mockups/synvelis-daw.html` | UI イメージのモックアップ | Issue #1 明示どおり**製品仕様ではない**。最終デザインに昇格させない |

### 矛盾の評価

- **製品価値・Gate 定義の矛盾:** 既存 docs に Gate A/B/C/AI の
  別定義や三層構造の別バージョンは確認されなかった。
  `sf-06-cross-gate-oracle-case-ledger.md` 等は Gate を**参照**
  するが再定義していない。
- **用語の差異（矛盾ではないが対応が必要）:** Issue #1 は
  「Configured/Active/Observed/Unknown」を製品語彙として使い、
  実装側は PC-C3 の `Configured/Prepared/Applied/Reported/
  Observed` slot + `Missing/Unsupported/Dropped/Expired` を使う。
  `Active` に相当する製品語彙は実装側の `Applied`/`Prepared`
  系に分解される。→ `docs/designs/product-value-core-design.md`
  §5 で対応表を明示して解消。
- **Creative Branch:** Issue #1 2026-11-11 追加分（#277〜#288）
  は既存 Gate 定義を変更しないと明示。本書 §10/EV-3 で独立
  Feature Acceptance として記録。

### 解消方針

1. 製品価値・中核体験・Gate A/B/C/AI・Acceptance Matrix・UX
   比較計画の**正本を本書チェーン
   （`docs/specs|designs|plans|tasks|test-specs|impl/
   product-value-core*`）に一本化**する。
2. 下流 docs（workspace-docking・ws-a・sv-gui・sf・pc-c 系）は
   本書を `related`/`source` として参照する運用とし、個別 docs
   の更新は本タスクの編集範囲外（各 Owner の別途対応）。
3. 用語差異は design §5 の対応表で吸収し、既存 docs の語彙を
   書き換えない。

## 残存する未決定事項（Issue #1 由来・本タスクで確定しない）

- UX 比較の数値閾値・比較対象 DAW 最終リスト・参加者条件
  （Baseline 計測後に別途合意）。
- 初期リリースで必須とする Gate の範囲（A 必須・B 優先・
  C/AI 独立のみ合意済み。残りは製品 Owner の決裁）。
- Creative Branch の Feature Acceptance 詳細（#277/#288 が正本）。
