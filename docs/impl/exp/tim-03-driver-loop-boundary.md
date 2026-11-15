---
id: "EXP-tim03-driver-loopbound-impl"
type: "experiment"
status: "completed"
title: "TIM-03 driver-side loop-wrap boundary derivation — AttachedDispatch WrapSim 供給の試行"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added:
      - type: "file"
        path: "docs/impl/exp/tim-03-driver-loop-boundary.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/tim-03-driver-loop-boundary.md"
  depends-on:
    - "docs/designs/tim-03-driver-loop-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-03-driver-loop-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-03-driver-loop-boundary.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/tim-03-driver-loop-boundary.md"
  defers: []
  deferred-by: []
---

# TIM-03 driver-side loop-wrap boundary 導出 — 実装と検証の記録

## 問題設定

routing 側 slice（`docs/impl/exp/tim-03-loop-boundary.md`）で確立
した `WrapSim` + read-only preview 契約を `HeadlessDriver` 経路へ
適用する。driver には `NodeSurface` のような scheduled/staged
queue が無く、transport command は全て `due = 0` の head apply —
この差分をどう pending に写像するかが設計上の焦点だった。

## E1 node cursor ドメインの分離

seek の quiesce pass は driver `cursor` を更新する前に RT block を
2 回処理し、各 pass で `observe_attached_block` が呼ばれる。node
側 cursor は process ごとに `block_frames` 進み `set_clock` で
re-anchor するため、quiesce 中は node cursor が driver cursor を
2 block 先行する。driver cursor をそのまま walk domain に使うと
wrap frame がずれる — `sim_window` を node cursor ドメインとして
独立に持ち、`set_clock` 成功時に即時更新する方式を採用した。
DB-02/DB-05 の seek test がこの経路を実測で固定する。

## E2 oracle applied set における offset 0 の扱い

初版 test は `plan.boundaries == [0, wrap_offset]` を期待したが、
実測では oracle の applied set が offset 0（head boundary）を
除外して `[wrap_offset]` のみを返した — head boundary は span を
分割しないため applied set に残らない既存正規化。driver は
`BoundaryPoint` を供給するだけで、正規化は oracle に委ねる設計が
そのまま確認できた。test は interior offset のみを assert する
形に修正した。

## 結果

`cargo test --test tim03_driver_loop_boundary`（mise 経由）:
6 test 全 pass。導出 `plan.boundaries` と plugin 観測 interior
cut が activation anchor・steady loop・window-end/head wrap・
SetLoop 変更・ClearLoop・Seek cascade・non-playing・sessionless
の全経路で一致。残り lane の結果は `evidence/sv127/` に記録。

## NotRun

実 plugin・実 Host/OS・GUI-L2/EXT-L3・実機・precision
certification・consumer oracle — 別 gate（headless fake plugin の
み、OS-L1）。
