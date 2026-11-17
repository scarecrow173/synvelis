---
id: "PLAN-l2esj17AEoBoJdXsY3Q1EE"
type: "plan"
status: "approved"
title: "Issue #514 plan — 判定記録 slice（実装不要分岐）"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/514"
    - "docs/specs/issue-514-session-engine-selection.md"
    - "docs/designs/issue-514-session-engine-selection.md"
    - "docs/adr/issue-514-session-engine-selection.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/engine.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-514-session-engine-selection.md"
    - "docs/designs/issue-514-session-engine-selection.md"
    - "docs/adr/issue-514-session-engine-selection.md"
  implemented-by:
    - "docs/tasks/issue-514-session-engine-selection.md"
  depends-on:
    - "docs/specs/issue-514-session-engine-selection.md"
    - "docs/designs/issue-514-session-engine-selection.md"
    - "docs/adr/issue-514-session-engine-selection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-514-session-engine-selection.md"
    - "evidence/sv514/decision-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-514-session-engine-selection.md"
  derived-by:
    - "docs/tasks/issue-514-session-engine-selection.md"
    - "docs/test-specs/issue-514-session-engine-selection.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #514 plan — 判定記録 slice

## スコープ

判定分岐 B（実装不要）を採用。本 slice の作業は判定記録と
「意図的な未配線」の明記であり、rebuild 経路の挙動変更・dep 追加・
テスト追加は行わない。

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `docs/discovery/issue-514-session-engine-selection.md` | 新規 — 調査結果と判定 |
| `docs/specs/issue-514-session-engine-selection.md` | 新規 — 受入条件と判定材料 |
| `docs/designs/issue-514-session-engine-selection.md` | 新規 — 責務境界・trigger 条件・AC4 |
| `docs/adr/issue-514-session-engine-selection.md` | 新規 — 決定記録 |
| `docs/plans/issue-514-session-engine-selection.md` | 新規（本書） |
| `docs/tasks/issue-514-session-engine-selection.md` | 新規 — task 分解 |
| `docs/test-specs/issue-514-session-engine-selection.md` | 新規 — 検証仕様 |
| `docs/impl/ir/issue-514-session-engine-selection.md` | 新規 — 実施記録 |
| `docs/impl/exp/issue-514-session-engine-selection.md` + `-exp.jsonl` | 新規 — 実験ログ |
| `evidence/sv514/decision-ledger.md` | 新規 — 判定根拠 ledger・lane 簿記 |
| `evidence/sv514/verification.md` | 新規 — 検証実測記録 |
| `crates/synvelis-app/src/engine.rs` | module doc に selection 非消費が意図的である旨を追記（挙動変更なし） |

## リスクと rollback

- リスク: 判定が将来覆る可能性 → design 文書に trigger 条件を明記し、
  engine.rs doc で境界を明示。rollback は doc 削除のみでコード影響なし。
- `engine.rs` の変更は doc comment のみで lint/test に影響しない。

## 検証マトリクス

| 検証 | コマンド | 期待 |
| --- | --- | --- |
| fmt | `mise run fmt-check` | pass |
| app check/lint/test | `mise run check/lint/test synvelis-app` | pass（回帰なし） |
| workspace check/lint/test | `mise run check/lint/test synvelis-workspace` | pass（`rcp_selection_integration` 回帰なし） |
| headless deps | 変更なしのため dep 追加なし → `check-headless-deps` は対象外（dep 追加時のみ必須） | — |
| 実機/RT | T3/T4・実 plugin/OS/PCM/RT | NotRun（#109/#296/`scarecrow173/ruviform#630`/`scarecrow173/ruviform#631`） |
