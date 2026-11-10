---
id: "TSPEC-034VpQe1oQ6xR3wV8vC0xN"
type: "test-spec"
status: "proposed"
title: "RCD-03 Arm/Monitor/Punch Project 永続投影 fixture matrix"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-03-monitor-persistence-design.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-03-monitor-persistence.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 Arm/Monitor/Punch Project 永続投影 fixture matrix

## Purpose

Arm/Monitor/channel_map/lane 宣言と punch marks の Project 永続投影
（schema v12・migration・op/undo・app write-through/read-through）が
headless fixture で正しく証明されることを固定する。実 Device/実
GUI/multi-window の受入は対象外（外部 gate、NotRun）。

## Feature

Monitor persistence projection — `Project.monitoring` POD・
`SetTrackMonitor`/`ClearTrackMonitor`/`SetPunchMarks` op・
`syn.monitoring.set`/`punch` write-through・`syn.monitoring.state`
read-through・project_new/open rehydrate。

## Rules

- **R1 authored 宣言のみ永続**: `project.monitoring.tracks` と
  `punch` が save/load・migration・undo を越えて復元される。
  `input_output_ready`/`hardware_direct`/latency/稼働 session は
  永続化されない。
- **R2 op は ProjectController 経由**: 宣言変更は
  `ProjectOp`+`apply` のみ。validate 失敗時に project/revision/
  undo stack は不変。成功時は revision+1・undo 可能。
- **R3 参照整合**: 宣言 track の不存在・lane の他 track 属属・
  空 channel_map は validate/`validate_references` で拒否。
  `RemoveTrack` は decl を cascade 除去、`RemoveLane` は宣言参照中
  の lane を拒否する。
- **R4 migration は空既定**: v11 snapshot は monitoring 空で
  load され revision が 1 進む。v12 body を v11 schema として
  読むことは拒否される。
- **R5 wire 契約不変**: `syn.monitoring.*` の op/params/state
  JSON 形状は先行 slice から不変。route/gate/latency 投影は
  `decide`/`MonitorProbe`/`preflight`/`LatencyReport` の消費のみで
  app/project 側に意味論を複製しない。
- **R6 rehydrate は正本読み**: project_new/open で session 観測値は
  reset、宣言・punch は load 済み `project.monitoring` がそのまま
  `syn.monitoring.state` に投影される。

## Examples

- **E1 宣言 write-through + undo**: `syn.monitoring.set` で
  arm/monitor/lane/channel_map を設定すると `project.monitoring.
  tracks[track]` が更新され revision が進む。`syn.project.undo`
  で宣言が消え、`redo` で復元する。`clear:true` で decl 除去。
- **E2 punch write-through**: `syn.monitoring.punch` の
  in/out/clear が `project.monitoring.punch` を更新し undo で
  復元。session active 中は従来どおり拒否。
- **E3 save/open rehydrate**: 宣言+punch 設定後に project save→
  `project.open`（または新規 project への load）で
  `syn.monitoring.state` が同じ宣言・punch・route を投影し、
  session 観測値は既定に戻る。
- **E4 観測値非永続**: `input_output_ready`/`hardware_direct`/
  latency を設定しても save/load・undo 対象に含まれず、
  project.open 後は既定（ready=false・Unknown・未注入）に戻る。
- **E5 project 側 op 検証**: `SetTrackMonitor` の未知 track・
  他 track lane・空 channel_map 拒否、`ClearTrackMonitor`、
  `SetPunchMarks`、v11→v12 migration、serde round trip、
  `RemoveTrack` cascade、`RemoveLane` 参照拒否を project 層
  fixture で検証。
- **E6 既存投影不変**: 先行 slice の route 行列・punch 境界・
  Hardware Direct・latency・window/focus 不変 fixture が
  write-through 化後も pass（意味論の非回帰）。

## Guarantees

- `syn.monitoring.state` は宣言を project から、観測値を overlay
  から投影し、いずれも捏造しない（未宣言 track は tracks に
  現れない、観測値未設定は既定投影）。
- monitor 宣言変更のみが revision を消費する。session 観測値・
  transport・latency 注入は revision/undo に影響しない。
- v11 snapshot の load は情報損失なく成功し、monitoring は空。

## Non-goals

- 実 Device/Capture/Software Monitor の可聴計測、実 GPUI GUI・
  multi-window/focus 受入、#114 Scenario、MonitorAdapter の実
  RT graph 搭載 — 外部 gate（NotRun）。
- Loop/PunchInLoop gate・count-in/pre-roll UX。
- VCS semantic diff への monitor 分類追加（follow-up）。
- `synvelis-recording`/`player`/`workspace` の変更。

## Risk

- project POD と recording 型の写像漏れ（enum 値・channel_map
  意味）で route が変わる危険 — E6 の既存行列 fixture が
  decide 契約との一致を継続検証する。
- `RemoveTrack` cascade / `RemoveLane` 拒否の追加で既存の
  track/lane 削除 fixture が壊れる危険 — project/app 双方の
  既存テストで非回帰を確認する。
