---
id: "EXP-t2MR3MoQtUvrJ0sMbKh597"
title: "Timeline Editing / Selection / Tool System 設計定義 実験メモ (synvelis#11)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: Timeline Editing / Selection / Tool System（synvelis#11）

## E1 doc-driven-dev-graph router の状態

`route_graph.js --cwd . --json` を実行。`status: blocked`、
`current/next: probe`、`edgeId: null`。blocker は **既存の
repository-wide broken relation**（`docs/designs/automation-
modulation-design.md`・`rcd-*`・`tim-02-*`・`vcs-*`・
`docs/impl/ir/*` の deferred relation が存在しない path を指す）
であり、本 slice が新たに作ったものではない。sv#5/sv#6 先例と
同様、この blocked は slice 起因でないため、router が返す
delegate が無いまま `spec → design → plan → task → test-spec →
impl` の文書チェーンを規約どおり作成した。無関係な broken
relation は **修復しない**（範囲外）。exp に記録して後続の
doc-status 修復 slice に委譲。

## E2 artifact ID の生成

先行 slice（sv#5/sv#6）に倣い、`SPEC-`/`DESIGN-`/`PLAN-`/`TASK-`/
`TSPEC-`/`IMPL-`/`EXP-` + 22 文字 Base62 の artifact ID を生成
（crypto random 22 bytes を base62 化）。生成値:

- `SPEC-BCK7X1Qx5rbH151Bo7ftE7`
- `DESIGN-T4E6PtTJ870Fpg4pFi3aM6`
- `PLAN-uGKWo3QVS9uwBCoBMRyVGT`
- `TASK-Nw48LwleZYov2jWLb0O7Vg`
- `TSPEC-vJFdbqSGXmBCZDnTy2Telx`
- `IMPL-kBHWiP6FFajB356aw7WOHn`
- `EXP-t2MR3MoQtUvrJ0sMbKh597`

## E3 設計上の主要判断（design D-01〜12 の根拠）

- **Selection を 7 概念に分離**: Issue #11 合意①の Object/Time/
  Lane/Primary/Inspection/Focus/Linked を別 state として固定。
  ED-A の `ObjectSelections`/`TimeSelection`/`LaneScope`/
  `edit_binding` 実装と一致させ、単一 Selection+mode 案を棄却
  （D-01）。
- **Time Selection 単独で Ripple しない**: `clip.remove` は object
  selection のみ、range 削除は独立 `clips.delete-range`。暗黙
  Ripple を構造的に排除（D-02・TE-03）。
- **Gesture fixation と 1 Undo**: ED-A `GestureFixation`/
  `ProjectOp::Composite` 契約を製品 UX へ写像。frame ごとの
  Project 書込み・自動 rebase・target すり替えを棄却（D-06）。
- **Grid/Snap 分離・明示 Unit は Zoom 不変**: ED-A
  `SnapUnit::Explicit|Auto` と一致。Grid 密度連動案を棄却（D-04）。
- **混在 Time Domain は定義済み規則のみ**: ED-B の結果表を正本と
  し、未宣言変換は型付き拒否。独自の混合演算契約を発明しない
  （D-05）。
- **数値を発明しない**: px Hit Size・Snap Threshold/Hysteresis・
  OS keybind は Issue #11 §9 の PoC 後決定事項。headless は意味的
  probe のみ（D-11）。
- **#10/#12 は継承境界のみ**: Project Model・Take/Comp の entity/
  UI 詳細は各 Owner の後続設計へ deferred とし、本設計で先取り
  しない。

## E4 証拠分類の方針

`evidence/sv11/coverage.md` で TE-01〜14 を既存 headless テスト
（`ed_a.rs`/`ed_b.rs`/`midi_source.rs`/`md_a.rs`/`ws_a.rs`/
`uic5*`/`vcs04_history.rs`/`synvelis-library import*`/
`workspace_model.rs`）へ写像。`Pass (headless)`/`Partial`/
`NotRun`/`Deferred`/`Unsupported` を分離し、Requirement 状態と
実行結果を混同しない。Pixel/OS 入力の合格と Headless の合格を
混同しない（POLICIES.md）。

## E5 未解決・後続へ委譲

- 製品 Timeline 編集 UX の実装は #112/#114・#10/#12 系実装
  Issue・#9 import 系が担当。本 slice は契約と証拠分類のみ。
- GUI/実機/実 Plugin 受入は #112/#114/EXT-L3 の別 Gate。
- repository-wide broken relation（router blocker）は本 slice
  範囲外。doc-status 修復は別途。
- sv#10（Project Model）の文書チェーンは別 Worker が並行作業中。
  本 slice は #10 の Issue・既存契約（MD-A/MIDI Source/ED-B）を
  参照するに留め、#10 側の新規文書には依存しない。
