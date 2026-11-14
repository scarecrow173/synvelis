---
id: "EXP-7kQm2wRxNp4sVb8tYc1dFh"
title: "PC-C6 acceptance ledger 監査実験メモ (synvelis#103)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: PC-C6 acceptance ledger 監査（synvelis#103）

## E1 fixture 欠落判定の方法

Issue #103 固定 Test Matrix 全 53 行を「headless で実施可能か」
「既存 fixture が被覆するか」の 2 軸で棚卸しした。手順:

1. Issue 本文の固定 Matrix（PC-C1-R・PC-C2-REL・PC-C3-TR・
   PC-C4-PL・PC-C5-UI・PC-C6-COMP）+ 2026-11-04 追記の PC-X1〜X6
   + GUI-PC-01〜04 を列挙（計 53 行）。
2. 各行の受入内容を test-spec の Rule semantic に写像
   （`docs/test-specs/pc-c6-*.md`・先行 slice の spec/design）。
3. `crates/*/tests/` の `pc_*`・`aut_*`・`uic5*`・`rel_c5`・
   `trace_c3*`・`tim0*`・`sv47_*`・`sf_05*`・`gw10_*` の test 関数
   名を列挙し、行ごとに被覆 fixture を対応付け。
4. 被覆が見つからない行について「headless で原理的に実施可能か」
   を判定 — 実 OS/GUI/plugin/device/RT/consumer を要する部分は
   fixture 追加ではなく `external-gates.md` の NotRun/external
   owner 記録が正しい処理と判断。

観察: 全 53 行の headless 実施可能部分は既存 fixture で被覆済み。
新規 test 追加は 0 件。「欠落 fixture 補完」のタスク名に対し、
監査の結論は「補完不要」— 台帳側にその判定根拠を明記した。

## E2 発見した留意点 1: `pc_c6_oracle_pl.rs` の行コメント番号差異

`crates/synvelis-player/tests/pc_c6_oracle_pl.rs` 内の test 関数
コメント（PL01〜PL08 のラベル）が test-spec
（`docs/test-specs/pc-c6-e2e-oracle-rel-tr.md`）の Rule(PLxx) の
semantic と一部入れ替わっていることを確認:

- コメント `PL03`（QueueFull/atomic batch）は Rule(PL04)
  （queue bounded/atomic reject）に相当。
- コメント `PL04`（GraphSwap/Reload fail closed）は Rule(PL06)
  （generation drift/reload）に相当。
- コメント `PL06`（plan staleness）は Rule(PL03)
  （plan staleness fail closed）に相当。

fixture の test 内容自体は PL01〜PL08 全行を被覆しているため
機能的な欠落ではないが、行番号だけで写像すると誤対応になる。
本 ledger は Issue 受入条件（test-spec semantic）で写像し、
差異を `acceptance-ledger.md` に明記した。fixture 本体の
コメント修正は本 slice の編集範囲（既存 `pc_*` fixture への
「欠落 test の追加のみ」）を超えるため行わず、記録に留めた。

## E3 発見した留意点 2: 部分被覆箇所

完全被覆と見せかけないため、以下を ◐/部分被覆として台帳に
明示した:

- PC-C1-R08: Note/Clip/Tempo の元データ/RT 不変は project/eval
  層（`ctl_01.rs` scope 行列）+ record transaction 層の単一経路
  （`synvelis-workspace/tests/pc_b.rs::pc_b_02_note_scope_records_
  note_relative` — `record_start`/`record_write`/commit で
  note-relative 座標記録を検証）までの被覆。R08 の
  Note/Clip/Tempo×drift 全組合せ fixture は無し。
  （当初「record transaction 層の note-scope fixture は無し」と
  判定しかけたが、file 名 `pc_b.rs` が player/workspace で
  重複しており workspace 側の `pc_b_02_*` 系を見落としていた
  — ambiguous filename の確認漏れとして記録。）
- PC-C5-UI01: Automation/Inspector/Signal Flow の 3 view を
  単一 fixture で同時照合する専用 test は無く、view 別の
  declared-truth 照合の複合証拠。
- PC-C3-TR07: 実 2 Inspector/2 window の二重起動防止は
  workspace/UI 責務（#102/#114）— headless は 2 trace の
  独立性まで。
- PC-X2: mode×transport/seek/loop/focus の全直積 fixture は無く
  次元別被覆。
- PC-C6-COMP04: Synvelis 側の lockfile 単一性 + `*_rt.rs`
  noalloc のみ。両 Consumer 独立判定は ruviform#219/#294 +
  synvelis#28。

## E4 判定語彙の設計

`Pass-measured (headless)` を「`mise run test <package>` が当該
SHA で green だった」ことのみに限定し、case 全体の受入 Pass・
実機 Pass と区別する語彙を採用。sv114
`acceptance-ledger.md` の先例（merged-evidence pointer /
NotRun / external Owner、台帳作成自体は Pass にしない）に倣いつつ、
本 slice は fixture 実測 green を確認したため headless 層の
`Pass-measured` を導入した。`Unsupported` は 0 行 —
技術的非対応の証拠が無い行を捏造しない。

## E5 検証の範囲判断

ledger が `synvelis-workspace/tests/` の fixture
（`pc_c6_oracle_rec`/`pc_c6_oracle_ui`/`pc_c1`/`aut_*`/`uic5*`/
`rel_c5`/`sv47_controls`/`sf_05*`）を引用するため、引用の
正直性のため `mise run test synvelis-workspace` を補助証拠として
実行した（task の正規検証一覧は project/player のみだが、
POLICIES.md §5「変更範囲に比例」に照らし引用先の実測確認は
正当）。`check`/`lint` の workspace は本 slice が同 crate を
変更していないため未実行と `verification.md` に明記した。
