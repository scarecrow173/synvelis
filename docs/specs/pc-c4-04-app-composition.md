---
id: "SPEC-PCc4040bs3vN1pQ8wXyZ2aB"
type: "spec"
status: "approved"
title: "PC-C4-04 workspace/app composition — catalog vend・dispatch 所有・tick 駆動・fast_control"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
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
    - "docs/specs/pc-c4-03-param-observation.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
  refines:
    - "docs/specs/pc-c4-plugin-param-binding.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  defers: []
  deferred-by: []
---

# PC-C4-04 workspace/app composition

## 目的

PC-C4 の下部機構（`PluginBindingSession`/`ParamDispatch`、
`RoutingControlDispatch` の generation-scoped tick/flush/gate と
pc-c4-03 report drain、`ProcessBoundarySession` の session→driver
配線、workspace `ControlSession::plugin_reported` feed）は全て
merge 済みだが、production 経路では誰も `attach`/`sync`/`tick`/
`drain` を駆動していない。本 spec は app/workspace composition
root への配線を定義する — catalog vend・dispatch 所有・tick 駆動・
`fast_control`・reported observation の feed 橋渡し。

## 要件

1. **catalog vend**: `PluginHostProcessors`（`synvelis-player`）が
   `ParamInfo` capture 機構を持ち、production 経路の attach が
   caller-vended 以外の catalog で動作する。pinned ruviform
   （b02942c2）には attached（`Shared`/L1）instance の公開
   parameter 列挙 API が存在しないため、composition root が
   plugin descriptor id ごとに登録する静的 `ParamInfo` table を
   host adapter が connect 時に processor 単位へ vend する。
   未登録 plugin は空 table を vend し、bind は typed に失敗する
   — 列挙を捏造しない。
2. **dispatch 所有**: `SessionRoutingEngine`
   （`synvelis-app`）が `RoutingControlDispatch` を所有し、
   generation publish ごとに `attach`/`sync` を駆動する。既存の
   `on_project_revision → rebuild`・`take_processor_control`・
   `rt_status` の意味論は維持する。
3. **tick 駆動**: control 周期（`ui_pump`）で
   `tick(pos, position, rt_status().active_generation)` を駆動する。
   `pos`/`position` は session playback の正直な `(beat, media)`
   instant 対。`fast_control` と observation surface を
   op/面として露出する。
4. **observation 橋渡し**: `Reported` stage の dispatch observation
   のみを workspace `ControlSession::feed_plugin_report` へ橋渡し
   する。host 解決値（`Scheduled`/`Dispatched`）を plugin report
   として再ラベルしない。stamp は `Estimated` provenance —
   測定時刻は backend 推定であり `Live`/`Captured` を名乗らない。
5. **generation gate**: generation swap で旧 generation の
   dispatch/report が誤配達されないことを end-to-end で証明する
   — gate 消費が本 slice の本体。

## 非目標

- 実 plugin・実 GUI・audio thread 上の RT 駆動（NotRun 継続）。
- pc-c4-05（persisted instance id、別 Worker 進行中）との統合 —
  main に入っていれば hydration 接続を消費してよいが必須ではない。
- pinned ruviform に無い wire 能力（L1 report verb、instance
  parameter 列挙）の捏造 — pin bump 待ち NotRun として記録する。
- `service.rs`/`app.rs`/`screens.rs`/`binding.rs`/`routing_control.rs`/
  `dispatch.rs` 等の別 Worker 占有ファイルの変更。

## 受入条件

- `SessionRoutingEngine` が `RoutingControlDispatch` を所有し、
  `attach`/`sync`/`tick`/`fast_control`/observation が production
  経路で駆動される（headless fixture で証明）。
- catalog vend（`ParamInfo` capture）が host 経路で供給され、
  手組み test 以外の production attach が動作する。
- generation swap で旧 generation の dispatch/report が誤配達
  されない end-to-end 証明、および reported observation が
  workspace `ControlSession` feed に到達する橋渡し。実 plugin/GUI
  行は NotRun 継続。
