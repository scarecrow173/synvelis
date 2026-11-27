---
id: "DESIGN-tm03De127UndoRedo"
type: "design"
status: "approved"
title: "TIM-03 tempo undo/redo identity"
created: "2026-11-28"
updated: "2026-11-28"
owners: ["scarecrow173"]
relations:
  realizes: ["docs/specs/tim-03-dynamic-tempo-undo-redo.md"]
---

# TIM-03 tempo undo/redo identity 設計

## 通知とプランの境界

`ProjectSession::undo/redo` は成功時に `on_project_revision` と `note_project_changed` を呼ぶ。ルーティングの構成が等価でも、engine は `RoutingControlDispatch::sync_musical_transport` を呼ぶ。`compile_project_musical_transport` は永続テンポスナップショットと project PPQN を検証する。`same_project_basis` はスナップショット全体を比較し、`plan_carries` は PPQN、tempo revision、コンパイル済みマップの共有割り当てを照合する。

`ParamDispatch::refresh_musical_plan` は MIDI と制御イベントを保持してプランを置換する。`Play` は追加しない。下位テストは明示 sync の後にこの契約を確認し、別途 A から B の直接比較で revision だけを比較する実装を検出する。

## 実 ABI の観測順序

セッションの通知を確認するには、新しい transport command を発行する前にプランと RT install receipt を取得する必要がある。`control_apply_transport` 自体にも再同期処理があるため、計測を先にすると通知漏れを見逃し得る。再生中の操作前は `Seek` のみで編集 tick より前に戻し、適用 receipt と観測 span で状態・位置を確認する。

計測では同じ編集 tick に異なる BPM を置き、後続の四分音符境界から 2 project ticks 手前を再生する。project PPQN は 480、adapter の投影先は 960。fixture の impulse を 48 kHz の PCM で照合し、L1 の 2 block lookahead を分離する。入力試料のコピーや初期無音を正例に数えない。
