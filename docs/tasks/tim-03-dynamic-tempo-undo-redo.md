---
id: "TASK-tm03Ta127UndoRedo"
type: "task"
status: "approved"
title: "TIM-03 tempo undo/redo identity"
created: "2026-11-28"
updated: "2026-11-28"
owners: ["scarecrow173"]
relations:
  references: ["docs/plans/tim-03-dynamic-tempo-undo-redo.md"]
  verified-by: ["docs/test-specs/tim-03-dynamic-tempo-undo-redo.md"]
---

# TIM-03 tempo undo/redo identity タスク

slice: `tim03-dynamic-tempo-undo-redo-s1`。本番の履歴操作を変更せず、下位 control matrix と実 ABI matrix を既存テストに追加する。docs index は変更しない。

## Verification

`mise run test synvelis-player` は下位 matrix を含む。`mise run test-tim03-dynamic-musical-transport` は runner のビルド後、既存と今回の実 ABI テストを同じバイナリで直列実行する。両コマンドの exit 0 と、本 slice に対応するテスト名・正例を確認する。check、lint、回帰 lane、RT、headless-deps、prek の記録は[検証記録](../../evidence/sv127/dynamic-tempo-undo-redo-verification.md)に集約する。
