---
id: "EXP-tim-01-ppqn-config"
type: "experiment"
status: "completed"
title: "TIM-01 PPQN config — site 列挙完全性監査（punch marks 欠落の発見）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/tim-01-ppqn-config.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-01-ppqn-config-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/tim-01-ppqn-config-task.md"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN config — site 列挙完全性監査

## 仮説

先行 slice（`tim-01-configurable-ppqn`）の `SetProjectPpqn` /
`preview_ppqn` は「Project aggregate が永続化する全 musical
tick を exact rational beat 経由で再投影する」契約を掲げるが、
`reproject_ppqn_with` の site 列挙が実際に全 site を網羅して
いるかは未検証。列挙漏れがあれば PPQN 変更後に旧 grid の値が
残り、silent retime になる。

## 方法

`Project` の全 persisted field を型で走査し、musical tick
domain（`MusicalTick`/`MusicalDuration`/生 `i64` tick）を持つ
値を `reproject_ppqn_with` の walk と突合した。

## 観測結果

- **確認された漏れ**: `monitoring.punch.punch_in`/`punch_out`
  （`Option<MusicalTick>`）。walk は tempo/signature events・
  clip anchor/extent・comp ranges のみを訪問しており、punch
  marks は PPQN 変更後も旧 grid の値を保持し続ける —
  Issue #125 の「silent retime 禁止」契約違反。
- **対象外と確認した domain**:
  - clip `Note.start`/`len`・automation `BeatPoint` 系 —
    rational beat 正本で PPQN 非依存（設計通り）。
  - `MidiSource` notes/`MidiTick`/`division`、
    `smf_documents` — source-native、保存対象外。
  - recording session/pass/stream/segment/take の frame
    domain — 保存対象外。
  - `plugin_instances`/`routing`/`control` authority —
    persisted tick を持たない。
  - `MidiPlanEvent`/`MidiPlaybackPlan`（`MusicalTick` を持つ）
    — compile 出力の非永続型で `Project` 非保持。
- **下流影響の確認**: `preview_ppqn` は同一
  `reproject_ppqn_with` walk を共有するため site 追加のみで
  dry-run/command の一致が保たれる。`PpqnPreviewSite.kind` は
  `&'static str` の wire 透過で、app 層
  （`synvelis-app`/`synvelis-workspace`）に exhaustive match
  は無く、新 kind 追加は JSON 投影を壊さない。punch を持た
  ない既存 fixture の `site_count` は不変。

## 結論

site 列挙の欠落は `monitoring.punch` の 2 site のみ。
`PpqnSite::single("punch_in"|"punch_out", "project")` で既存
`convert` コールバックへ載せる修復を採用（adopted —
`docs/impl/ir/tim-01-ppqn-config.md`）。代替案（punch marks
を musical tick domain から外す・app 層で補正）は、永続化
 tick の grid 不整合を下流へ押し下げるため棄却。
