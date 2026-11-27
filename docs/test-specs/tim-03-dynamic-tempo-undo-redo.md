---
id: "TESTSPEC-tm03Ts127UndoRedo"
type: "test-spec"
status: "approved"
title: "TIM-03 tempo undo/redo identity"
created: "2026-11-28"
updated: "2026-11-28"
owners: ["scarecrow173"]
relations:
  verifies: ["docs/specs/tim-03-dynamic-tempo-undo-redo.md"]
---

# TIM-03 tempo undo/redo identity テスト仕様

## Purpose

履歴を戻した後も古いテンポをプラグインが使い続ける回帰と、同じ revision の分岐を変更なしと判定する回帰を検出する。

## Feature / Rules

`Add A → undo → redo → undo → add B` を実行する。本番テストでは `ProjectSession` を通し、テンポ以外のプロジェクト内容が変わらないことを比較する。計測操作より前に新しいプランの RT 適用を確認する。

## Examples

| 操作 | 実 ABI の操作時状態 | 復元・追加後の BPM | 独立判定の click frame |
| --- | --- | ---: | ---: |
| 最初の undo | Pause | 93.750 | 640 |
| redo | Playing | 75.000 | 672 |
| 分岐追加 | Playing | 110.000 | 622 |

追加場所は tick 9,600。開始する位置は 24 四分音符目の 2 project ticks 手前。測定窓で click は stereo 同一かつ amplitude 0.5、それ以外は無音とする。lookahead 512 frames より前のブロックは正例に数えない。

## Guarantees

下位の混合プランでは全イベントと generation binding が残る。分岐追加では A と B の tempo revision が同じでも、内容と `same_project_basis` が一致しない。無効な PPQN では既定値ではなく「musical source なし」を報告する。

## Non-Goals / Risk

再生位置を編集 tick より前に戻せない場合、非遡及の正例には含めない。実 ABI の正例が成立しても、全編集操作・RT 隔離・GUI・実機の受入には転用しない。
