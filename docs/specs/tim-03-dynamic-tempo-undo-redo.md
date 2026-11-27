---
id: "SPEC-tm03Sr127UndoRedo"
type: "spec"
status: "approved"
title: "TIM-03 tempo undo/redo identity"
created: "2026-11-28"
updated: "2026-11-28"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/127"]
---

# TIM-03 tempo undo/redo identity

テンポイベントの undo は古い tempo revision を復元する。redo は編集後の内容を復元する。undo 後に同じ tick へ別の BPM を追加すると、破棄した履歴と同じ tempo revision に到達し得る。再生中のプラグインには、revision が等しいだけの古いテンポマップではなく、現在のプロジェクトのテンポマップを渡す。

## 受入条件

- 非遡及の `AddTempoEvent`、undo、redo、undo 後の分岐追加を検証する。project revision は毎回進み、tempo revision と内容は対象の履歴に従う。
- プレイヤーの既存プランを更新しても MIDI と future parameter event の位置、順序、ペイロード、source sequence、任意の transport generation binding を保つ。更新のための `Play` は発行しない。
- `ProjectSession` から実 VST3/CLAP までを通し、計測前の undo、再生中の redo、分岐追加を独立 PCM oracle で判別する。編集時の位置は編集 tick より前、計測位置は編集 tick より後とする。
- 未対応、未実行、環境起因の失敗を Pass と区別する。欠落・無効なテンポ情報に既定値を補わない。

## 対象外

remove/update、遡及編集、競合編集、ループ中の編集、キュー満杯時の再試行は対象外。automation の残行列、fault/state/crash、GUI/EXT-L3、macOS/Linux 実行、3OS CI、AudioLens および Issue 全体の受入は別途扱う。

## 検証境界

Runtime Verification は `mise run test-tim03-dynamic-musical-transport` と `mise run test synvelis-player` を正規 lane とする。JSONL の ABI 行は最終成功した app lane、下位行は最終成功した player lane の stdout と同期する。GUI runtime drive は対象外。Formal Verification は NotApplicable。本 slice は有限の実行例を確認し、Issue `#601` の D-layer 証明を転用しない。

各ケースの保証は[テスト仕様](../test-specs/tim-03-dynamic-tempo-undo-redo.md)、実測結果は[検証記録](../../evidence/sv127/dynamic-tempo-undo-redo-verification.md)、静的指摘と実行結果の照合は[レビュー記録](../../evidence/sv127/dynamic-tempo-undo-redo-review.md)を参照。
