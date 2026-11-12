---
id: "IMPL-PCc4040bs3vN1pQ8wXyZ2aG"
type: "impl"
status: "completed"
title: "PC-C4-04 app composition — catalog vend・dispatch 所有・tick 駆動・feed 橋渡し"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added:
      - type: "file"
        path: "docs/specs/pc-c4-04-app-composition.md"
      - type: "file"
        path: "docs/designs/pc-c4-04-app-composition-design.md"
      - type: "file"
        path: "docs/plans/pc-c4-04-app-composition-plan.md"
      - type: "file"
        path: "docs/tasks/pc-c4-04-task1-app-composition.md"
      - type: "file"
        path: "docs/test-specs/pc-c4-04-app-composition.md"
      - type: "file"
        path: "docs/impl/ir/pc-c4-04-app-composition-slice.md"
      - type: "file"
        path: "crates/synvelis-app/tests/pc_c4_04_app_composition.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/host.rs"
      - type: "file"
        path: "crates/synvelis-app/src/engine.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/Cargo.toml"
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
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C4-04 app composition — implementation record

## Summary

PC-C4 の下部機構を production 経路へ配線する app/workspace
composition slice — `PluginHostProcessors` の catalog vend、
`SessionRoutingEngine` の `RoutingControlDispatch` 所有と
publish 連動 attach/sync、`ui_pump` tail の tick 駆動、
`Reported` stage の `feed_plugin_report` 橋渡しを実装した。

## Context

PC-C4 の下部機構（`PluginBindingSession`/`ParamDispatch`、
`RoutingControlDispatch` の generation-scoped tick/flush/gate と
pc-c4-03 report drain、`ProcessBoundarySession` の
session→driver 配線、workspace `ControlSession::plugin_reported`
feed）は全て merge 済みだが、production 経路では誰も
`attach`/`sync`/`tick`/`drain` を駆動していなかった。本 slice は
app/workspace composition root への配線を定義・実装するもので、
catalog vend・dispatch 所有・tick 駆動・`fast_control`・
reported observation の feed 橋渡しが対象。generation swap で
旧 generation の dispatch/report が誤配達されないことの
end-to-end 証明が本体（spec 要件 5）。

上流文書: spec `docs/specs/pc-c4-04-app-composition.md`、
design `docs/designs/pc-c4-04-app-composition-design.md`、
plan `docs/plans/pc-c4-04-app-composition-plan.md`、task
`docs/tasks/pc-c4-04-task1-app-composition.md`、test-spec
`docs/test-specs/pc-c4-04-app-composition.md`。

## Implementation

### Changes

- `synvelis-player/src/host.rs`: `PluginHostProcessors` に
  `catalogs`（plugin descriptor id → `ParamInfo`）・`vendored`
  （`Rc<RefCell<ParamCatalog>>`）を追加。`register_params`・
  `param_catalog`・`param_catalog_cell` を公開し、`connect` 成功時に
  `request.processor.id → params` を蓄積。pinned ruviform に attached
  instance の公開 param 列挙が無いため、vend は caller 登録を host
  が集約する形（design 参照）。`ProcessorConnection` は不変
  （literal 構築・分解する範囲外ファイルを破壊しないため）。
- `synvelis-app/src/engine.rs`: `Inner.dispatch:
  Option<RoutingControlDispatch>` を所有。`rebuild` の publish 後に
  `sync_control_dispatch` が live published generation
  （`source_map().generation`）へ attach/sync。catalogs は共有 cell
  を都度 clone。公開面: `set_param_catalog`、`control_tick`、
  `control_fast_control`、`control_observe_cycle`、
  `with_control_dispatch`、`control_report`/`control_generation`/
  `last_control_error`。`active_generation` は `0 → None` 規約。
- `synvelis-app/src/ui_ops.rs`: `ui_pump` tail に `pump_control`
  （session `playback().position` + `clock_now()` の instant 対で
  tick 駆動、`last_control_tick` 保持）。`control_fast_control`・
  `feed_control_cycle`（`Reported` stage のみ
  `feed_plugin_report`、`MediaTimestamp::estimated` stamp）を面
  として露出。`DispatchStage` 選別のため
  `ruviform-audio-plugin-bridge` を `synvelis-app` に追加
  （coordinator 範囲拡張）。
- `crates/synvelis-app/tests/pc_c4_04_app_composition.rs`: 新規
  headless fixture — catalog vend・attach/sync・tick・
  fast_control・drain→observe→feed・generation swap gate の
  end-to-end 証明。

### Decisions During Implementation

- `SessionEngineConfig` field 追加は 5 つの既存 test literal を
  破壊するため `set_param_catalog` setter 方式を採用。
- `ProcessorConnection.params` field は `export.rs` 分解と
  `sf03_l3.rs`/`sf03_revision.rs` literal を破壊するため不採用 —
  蓄積 `ParamCatalog` が dispatch に必要な evidence を全て持つ。
- `observe_cycle` の span/call window は RT runtime 内の物理 call
  partition であり caller 供給 — app は pass-through surface のみ
  提供し、捏造しない。
- `control.rs`（workspace）は変更なし — `feed_plugin_report` が
  既に橋渡し先として完備しており、接続部の新設は不要と判断。

## Related Experiments

### Adopted

なし — `metadata.experiments.adopted` は空。

### Rejected

なし — `metadata.experiments.rejected` は空。

## Validation

- `crates/synvelis-app/tests/pc_c4_04_app_composition.rs`（新規
  headless fixture、4 test）: catalog vend・attach/sync・tick・
  fast_control・drain→observe→feed・generation swap gate の
  end-to-end 証明（Implementation 節のファイル一覧どおり）。
- task1 の検証記録: `cargo check -p synvelis-player` — Passed
  （host.rs vend）。canonical `mise run fmt-check` / `check` /
  `lint` / `test` の実施結果は PR 本文に記録。

## Risks

- pinned ruviform（`b02942c2`）に attached（`Shared`/L1）instance
  の公開 parameter 列挙 API が存在しないため、catalog は
  composition root の caller 登録を host が集約する形 —
  未登録 plugin は空 table を vend し bind は typed に失敗する
  （列挙を捏造しない設計だが、登録漏れは bind 失敗として現れる）。
- `feed_control_cycle` の dep 拡張（`ruviform-audio-plugin-bridge`
  を `synvelis-app` に追加）は Coordinator 承認済みで実装 —
  拒否時は primitive 列受け取りへの縮退が fallback だった
  （plan risk）。
- 実 plugin instance・実 GUI・audio thread 上の RT 駆動は
  headless fixture の範囲外 — tick 駆動の RT 実値は未検証。

## Follow-ups

- 実 plugin instance・実 GUI・audio thread RT 駆動（pin/範囲外）。
- L1 wire report verb・instance parameter 列挙（pinned ruviform
  `b02942c2` に未含有 — pin bump 待ち）。
- `syn.*` op 登録（`service.rs` は別 Worker 占有 — 面は
  `SynvelisApp`/`SessionRoutingEngine` の pub method として露出。
  後続の pc-c4-06 で `syn.control.*` op として公開）。
