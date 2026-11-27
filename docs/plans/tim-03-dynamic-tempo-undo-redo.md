---
id: "PLAN-tm03Pl127UndoRedo"
type: "plan"
status: "approved"
title: "TIM-03 tempo undo/redo identity"
created: "2026-11-28"
updated: "2026-11-28"
owners: ["scarecrow173"]
relations:
  references: ["docs/designs/tim-03-dynamic-tempo-undo-redo.md"]
---

# TIM-03 tempo undo/redo identity 実装計画

| 順序 | 対象 | 完了の根拠 |
| --- | --- | --- |
| 1 | `synvelis-player/tests/tim03_musical_transport_plan.rs` | 履歴操作後の明示 sync と混合プランの比較。同一 revision の A から B への切替も単独で検証する。 |
| 2 | `synvelis-app/tests/tim03_l1_production_musical_transport.rs` | 実セッション通知、RT receipt、VST3/CLAP の独立 PCM 判定を既存テストバイナリに追加する。 |
| 3 | `evidence/sv127/` の本 slice の証跡 | 正規コマンド、対象 SHA、成果物ハッシュ、stdout の JSONL を照合する。 |

最初に下位境界で問題を切り分け、次に app の通知と実 ABI を確認する。production の欠陥が判明した場合のみ、責務を持つ層を直す。lease 外の変更が必要なら Coordinator に確認する。

スキーマと公開 API は追加しない。テスト追加だけで済む場合、ロールバックはテストと本 slice の文書の変更を戻す。runner 不在や vendor revision 不一致を検出した場合は provisioning として記録し、fixture SHA や正規 lane の条件は変更しない。
