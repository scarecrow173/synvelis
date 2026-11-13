---
id: "EXP-Pnfqhcl6Bm2MHZGdZhdyun"
title: "Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価計画 実験記録"
created: "2026-11-14"
updated: "2026-11-14"
---

# Synvelis 製品価値と中核体験 — 実験記録

対象: synvelis#1。実装記録は `docs/impl/ir/product-value-core.md`
を参照。

## 試行と結果

1. **graph router**: `route_graph.js --cwd .` は repo 全体の既存
   broken relation（`docs/designs/rcd-04-*` の `defers`、
   `docs/specs/vcs-*` の `source` 等、本タスクと無関係）で
   fail-closed `blocked`（`focus-required`）。→ 宣言済み delegate
   順（spec→design→plan→test-spec→task→impl）を手動で踏襲。
   `docs/impl/exp/svt-product-e2e.md` と同じ既知の制約。
2. **製品語彙 vs 実装語彙**: Issue #1 の「Configured/Active/
   Observed/Unknown」と PC-C3 ControlTrace の
   `Configured/Prepared/Applied/Reported/Observed` +
   `Missing/Unsupported/Dropped/Expired` は 1:1 で対応しない。
   → design §5 に対応表を置き、「Active」は実装側で
   Prepared/Applied 系に分解されること、製品語彙は断定を
   禁じる側の語彙であることを明記して解消。既存 docs の語彙は
   変更しない。
3. **exp doc の形式**: `docs/impl/exp/` の正規 experiment log は
   `.jsonl`（CLI 管理）だが、本タスクは JSONL の実験イベントを
   持たない文書作業。→ 先行例 `docs/impl/exp/svt-product-e2e.md`
   に倣い front matter 付き Markdown の検討記録とした。exp
   index は `.jsonl` のみを追跡する契約のため影響なし。
4. **Acceptance Matrix の「現状の達成度」**: 実装途中の headless
   証拠（`evidence/sv114/` の SVT-01〜09、SF-06/PC-C6 oracle）を
   `Headless verified` の途中証拠として記録し、実 GUI/実機器の
   受入は一律 `NotRun` とした。設計合意のみで OS/RT/実 GUI
   受入を済ませない Issue 明示の禁止に従う。
