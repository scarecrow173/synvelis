---
id: "DESIGN-034UJqgcnnlXJWllu9yUGb"
type: "design"
status: "approved"
title: "SF-03 slice6 design — RoutingControlDispatch control surface（世代 gate・有界 queue・binding 証拠・catalog refresh）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice6-act06-control-surface.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-03-slice6-act06-control-surface.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice6-act06-control-surface.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  defers: []
  deferred-by: []
---

# SF-03 slice6 design — RoutingControlDispatch control surface

## Overview

`RoutingControlDispatch` は `RoutingEngine` の published 世代に対する
headless control surface。`attach`/`sync` で世代ごとの
`GenerationSurface`（`EvalPlan` + scratch + per-processor
`NodeSurface{session, dispatch}` + `ParamCatalog` + `SurfaceReport`）
を構築し、`tick`（automation+queue の merged dispatch）と
`fast_control`（point-in-time write の gate/queue/即時 dispatch）を
提供する。全 write は `PluginBindingSession` の世代・instance・
capability・frame-base pin を通り、plugin へは `ParamDispatch` の
transport plan 経由でのみ届く。

責務境界: semantic 評価・lowering・queue/gate は Synvelis、
graph 検証・compile・RT schedule・plugin host は Ruviform。
`PluginBindingSession`/`ParamDispatch`（PC-C4）の契約を再実装せず
消費する。

## 世代 surface と one-shot vend

- `attach(engine, generation, project, catalogs)`:
  `engine.take_processor_control(generation, processor)` で
  generation-local `PluginNodeControl`+`GraphNodeId`+`TimeBase` を
  one-shot vend。`None`（static host・foreign generation・既取得）は
  `SurfaceReport.without_control` へ typed 記録し surface 自体は
  attach される（制御対象が無いことを偽装しない）。
- `PluginBindingSession::attach` + `ParamDispatch::new` を構築し、
  `bind_node` が capability 保持 target を stored catalog の
  `ParamInfo` 列挙と照合して bind。失敗は `SurfaceReport.unbound`
  に target→error で記録。
- `sync(engine, generation, project, catalogs)`:
  surface と同一世代は refresh のみ（`SyncOutcome::Current`、
  one-shot control は再 vend しない）、別世代は surface を破棄して
  再 vend（`Swapped`、queue 保持で再検証 replay）。sync 対象世代の
  選択は caller 責務 — RT 未観測世代を渡しても deliver gate は
  `active_generation` 不一致で閉じたまま（one-shot control の
  消費のみ先行し得る点は caller orchestration の前提）。

## Active-generation gate と有界 Queue

- `tick`/`fast_control` は caller が `RoutingEngine::rt_status()` 由来の
  `active_generation` を受け取り、`Some(surface.generation)` と
  一致しない限り deliver しない（fail closed）。`None`
  （engine 停止・未観測）は常に gated。
- `fast_control`: active 一致時は `lower_event`（capability・
  write-means・mapping 検証）→ `session.dispatch` で即時 plan
  install。不一致時は `FastControlWrite` を FIFO queue へ
  （`with_queue_capacity`、既定 `DEFAULT_QUEUE_CAPACITY`）。
  溢れは `Blocked(QueueFull)` — 追い出しなし。
  node channel `QueueFull` は write を queue へ戻し `Queued`。
- `tick`: queue を drain し各 write を**現在 surface** の
  `lower_for_node`（target→owner processor→node session→binding）で
  再解決。解決不能は `queue_dropped`（typed drop、silent でない）。
  続いて全 node の bound target を `eval_event`（`EvalPlan`・
  `BeatPoint`・`position`・`TimedEvent`）で評価し、queue entry と
  automation entry を node ごとに 1 batch に merge して
  `session.dispatch` — `ParamDispatch::dispatch` は node plan を
  置き換えるため、1 service call 内での batch 分割は pending event
  を失わせ得る。呼出しを跨ぐ plan-replace は upstream 契約
  （`PlanReplaced` ledger 記録）に従い、consumer は ≈現在位置の
  stamp と適切な service 周期を前提とする（module docs に明記）。

## Catalog refresh と binding 証拠

- `tick`/`fast_control`/`sync` 入口で `surface.plan.is_stale(project)`
  または `surface_is_current` 不成立を検出したら `refresh_surface`:
  `ControlGraph::compile` で `EvalPlan`+scratch を再生成し、
  session pin（capability generation・graph generation）が現行と
  合う node のみ、現行 `ParamCatalog` の `ParamInfo` で
  `session.revalidate`（列挙から消えた/ drift した param の
  binding を defunct 化）→ `bind_node` で再 bind。
  control（one-shot vend）は再取得しない — session/dispatch は
  世代内で存続する。
- `RoutingControlDispatch::binding(target)` は attach 済み
  `NodeSurface.session` の記録済み `PluginParameterBinding` を
  clone で返す。`graph_generation`/`capability_generation`/
  `instance_id`/`attach_clock_context` pin により、activation
  receipt・`RoutingSourceMap` の世代と外部照合できる
  （cross-consumer same-Revision fixture の証拠面）。

## Failure と証拠

- 全失敗は型で返す: `FastControlOutcome::Blocked(FastControlBlock)`
  （`NoSurface`/`StaleSurface`/`UnboundTarget`/`Lowering`/
  `Dispatch`/`QueueFull`）・`TickReport` カウンタ
  （`gated`/`stale`/`evaluated`/`events`/`queue_delivered`/
  `queue_dropped`/`queue_retained`/`nodes_dispatched`/
  `dispatch_errors`/`eval_skipped`）・`SurfaceReport`
  （`controlled_processors`/`without_control`/`unbound`/
  `bound_targets`）。
- `last_block`/`last_tick` で直近結果を観測可能（診断用）。

## Approaches（検討して棄却したもの）

- **B: fast_control も queue 経由のみ** — active 時の即時性を失い、
  GUI の「今書く」意味論に反する。gate+queue の併用を採用。
- **C: dispatch ごとの delivery 観測ループ** — `ParamDispatch::observe`
  は span/call window を必要とし driver 側 orchestration の責務。
  本 slice では plan-replace 契約を docs 化し、観測統合は consumer
  slice へ defer。
- **D: `synvelis-render` も同時移行** — consumer 接続は別 slice
  （scope 外）。offline attach の同一契約は fixture 06 で固定。

## Verification

`crates/synvelis-player/tests/sf03_l6.rs`（6 件）—
spec/test-spec の Feature/Rule/Example に対応。全 lane は
`mise run fmt-check`/`check`/`lint`/`test`/`test-rt`。
