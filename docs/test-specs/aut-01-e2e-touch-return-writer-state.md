---
id: "TSPEC-034AUT01Hd5jKm8nRt3wXsYq"
type: "test-spec"
status: "approved"
title: "AUT-01 Touch Return・Writer State 境界 検証仕様"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/131"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
    - "docs/designs/aut-01-touch-return-writer-state-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
    - "docs/designs/aut-01-touch-return-writer-state-design.md"
  derived-by: []
  verifies:
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
    - "docs/designs/aut-01-touch-return-writer-state-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/103"
  defers: []
  deferred-by: []
---

# AUT-01 検証仕様

対象:`crates/synvelis-workspace/tests/aut_01.rs`(headless PoC —
実 Plugin・実 GUI は #101/#102/#103 の受入で別判定)。

## Feature: Touch Return(AUT-01-01)

- Rule: Touch の gesture release で `WriterState::Returning` が始まり、
  `x = clamp(elapsed/duration)`・smoothstep `w=3x²-2x³` の補間値が
  reference に現れる。
  - Example: automation 0.2・release 0.8・150ms で、75ms 時点の
    `reference = 0.5*0.8 + 0.5*automation(t)`(w=0.5)、
    `reference_source == Return`。
- Rule: 150ms 経過後(clock advance)に return は完了し、reference は
  automation へ一致、`writer_state` は `Idle` へ戻る。
- Rule: return 中の `v_automation(t)` は移動する評価位置へ追従する
  (Linear 曲線の傾斜中でも reference が連続して curve へ寄る)。
  - Example: Linear region 上で advance+clock を並行して進めると
    各 tick の reference が release→curve へ単調に近づく。
- Rule: `set_return_duration` で可変期間を設定できる(0 < duration)。
  - Example: 300ms で 75ms 時点の x=0.25 → w=0.15625。
- Rule: duration 0 は即完了だが `instant: true` として
  `WriterState::Returning` で観測可能(click/zipper 影響表示)。
- Rule: return は session runtime のみ — project revision・保存点列・
  undo stack・recording segments を一切変更しない。
- Rule: 新しい gesture begin は active return を置き換え、gesture の
  seed 値は補間中の現在 reference(ジャンプなし)。

## Feature: mode 別 release 意味(AUT-01-02)

- Rule: `Read` の release は補間せず即 `Returned` — writer state は
  `Idle` へ、書込みなし。
- Rule: `Touch` は gates(armed+record+rolling+open txn)成立時のみ
  gesture 区間を書込み、release で return 開始。
- Rule: `Latch` は release 後も `LatchHeld` で最終値を書込継続し、
  `transport_advance` で held 値が txn に着地する。mode 変更または
  stop で閉鎖。
- Rule: `Write` は `record_start` 時に固定した Reference 基準値を
  `WriteHeld` として保持し、gesture なしでも `transport_advance` で
  control-rate write する。非 armed target には書かない。
- Rule: `Off` の gesture_end は `BaseEdit`(`SetParameterBase` =
  1 undo)であり、writer state は残らない。

## Feature: 境界閉鎖 State Matrix(AUT-01-03)

- Rule: `transport_stop` は全 gesture・latch/write held・return を
  `TransportStop` で明示閉鎖し txn を seal する。
- Rule: `transport_seek` は `TransportSeek` で同様に閉鎖 + seal。
- Rule: `transport_advance` の loop wrap は `LoopWrap` で閉鎖 +
  seal(点列上書きなし)。
- Rule: `transport_play` の新規 play edge(非 Playing→Playing)は
  `NewTransportPlay` で閉鎖する。`Paused→Playing` の resume は
  閉鎖しない(同一 playback の継続)。
- Rule: `note_focus_lost` は `FocusLost` で全 target の state を
  閉鎖する(2Window/Plugin GUI/IME focus 移動に相当)。
- Rule: `note_plugin_crash(project, processor)` は当該 processor
  所有 target のみ `PluginCrash` で閉鎖する(他 target は継続)。
- Rule: foreign project apply(`note_project_changed`)は txn を
  seal(ProjectChange)し、消えた target の state を
  `TargetDeleted` で閉鎖する(残存 target は継続)。
- Rule: 全閉鎖は `WriterClosure` として記録される(観測可能性)。
- Rule: Latch/Write の held state は次再生で暗黙再開しない
  (明示 `record_start` のみが新 transaction を開く)。

## Feature: GestureFixation(AUT-01-04)

- Rule: gesture begin で target・project revision・arm・clock epoch・
  frame base が固定され、`gesture_fixation` で検査できる。
- Rule: fixation は Inspector Follow/arm 変更で変わらず、書込は
  `record_start` 固定集合と txn seal が守る(既存契約と整合)。

## Feature: 維持契約(AUT-01-05)

- Rule: `record_start` の gate(record-enabled + rolling + armed
  write-mode + bound lane)は変わらない。
- Rule: `evaluate` の manual 注入優先順は gesture > latch/write-held >
  return — 同時に 1 種のみが reference を決める。
- Rule: Reference/Modulation/Host Value/Plugin Reported を混同しない
  — return 中の `reference_source` は `Return`、contributions は
  変わらず合成後に評価される。
- Rule: pause は gate を閉じるが held/return を維持する(seal しない
  維持契約 — PC-C1 と同じ)。
