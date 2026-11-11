---
id: "TASK-PCc4040bs3vN1pQ8wXyZ2aE"
type: "task"
status: "done"
title: "PC-C4-04 task1 — workspace/app composition 実装"
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
  implements:
    - "docs/plans/pc-c4-04-app-composition-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-04-app-composition-plan.md"
    - "docs/specs/pc-c4-04-app-composition.md"
    - "docs/designs/pc-c4-04-app-composition-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C4-04 task1 — workspace/app composition 実装

## 実施内容

1. `PluginHostProcessors` catalog vend — `register_params` で
   plugin descriptor id ごとの `ParamInfo` table を登録し、
   `connect` 成功時に `vendored` cell（`ParamCatalog`）へ蓄積。
   `param_catalog_cell()` で composition root へ共有 cell を vend。
2. `SessionRoutingEngine` dispatch 所有 — `Inner.dispatch`、
   publish 後 `sync_control_dispatch`（attach/sync + catalog
   live 読み）、`control_tick`/`control_fast_control`/
   `control_observe_cycle`/`with_control_dispatch` 公開面、
   `set_param_catalog`、`last_control_error` 記録。
3. `ui_pump` tail の `pump_control` — session playback の
   `(BeatPoint, MediaTime)` instant 対で tick 駆動、
   `last_control_tick` 保持。`control_fast_control`・
   `feed_control_cycle`（`Reported` stage のみ
   `feed_plugin_report`、`Estimated` stamp）を面として露出。
4. headless fixture `pc_c4_04_app_composition.rs` —
   publish→attach→tick→fast_control→drain→observe→feed と
   generation swap gate の end-to-end 証明。

## 検証

- `cargo check -p synvelis-player` — Passed（host.rs vend）。
- `mise run fmt-check` / `check` / `lint` / `test` — 実施結果は
  PR 本文に記録。
- 実 plugin・実 GUI・L1 wire 能力は NotRun 継続（pin に未含有）。
