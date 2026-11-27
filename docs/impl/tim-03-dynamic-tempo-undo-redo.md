---
id: "IMPL-tm03Ir127UndoRedo"
type: "impl"
status: "approved"
title: "TIM-03 tempo undo/redo identity"
created: "2026-11-28"
updated: "2026-11-28"
owners: ["scarecrow173"]
relations:
  references: ["docs/tasks/tim-03-dynamic-tempo-undo-redo.md"]
---

# TIM-03 tempo undo/redo identity 実装記録

変更は下位プレイヤーと app の既存 integration test に限定した。下位テストでは混合プランを明示 sync し、同一 revision の A/B 切替では古い割り当てを current としないことを確認する。app テストは新規アプリ・エンジンを形式ごとに作り、セッション通知、プラン置換 receipt、PCM を順に観測する。正規の mise lane、production code、依存 pin は変更していない。

測定値、実行 SHA、環境の制約は[検証記録](../../evidence/sv127/dynamic-tempo-undo-redo-verification.md)に、判定の境界は[レビュー記録](../../evidence/sv127/dynamic-tempo-undo-redo-review.md)に記す。本 slice の実行検証は、履歴同一性の形式的証明ではない。
