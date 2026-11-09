---
id: "IMPL-034UJqjrnNLIXxfkFE25tv"
type: "impl"
status: "completed"
title: "SF-03 slice6 実装記録 — RoutingControlDispatch・sf03_l6 acceptance"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/routing_control.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/sf03_l6.rs"
      - type: "file"
        path: "docs/specs/sf-03-slice6-act06-control-surface.md"
      - type: "file"
        path: "docs/designs/sf-03-slice6-act06-control-surface-design.md"
      - type: "file"
        path: "docs/test-specs/sf-03-slice6-act06-control-surface.md"
      - type: "file"
        path: "docs/impl/ir/sf-03-slice6-routingcontroldispatch-sf03-l6-acceptance.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice6-act06-control-surface.md"
    - "docs/designs/sf-03-slice6-act06-control-surface-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-03-slice6-act06-control-surface-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-slice6-act06-control-surface-design.md"
    - "docs/specs/sf-03-slice6-act06-control-surface.md"
  derived-by: []
  verifies:
    - "docs/test-specs/sf-03-slice6-act06-control-surface.md"
  verified-by: []
  references:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-03 slice6 実装記録 — RoutingControlDispatch・sf03_l6 acceptance

## Summary

`crates/synvelis-player/src/routing_control.rs` に
`RoutingControlDispatch` を新設し、`RoutingEngine` の published
世代に対する headless control surface を公開した。automation lane
の `EvalPlan` 評価を `lower_event` 経由で `TimedParamEvent` 化し、
`PluginBindingSession`/`ParamDispatch` の世代・instance・
capability・frame-base pin を通して plugin param へ配送する。
Fast Control は RT 観測 active-generation gate + 有界 FIFO
Queue（既定容量・溢れは `QueueFull` で追い出しなし）、世代 swap
を跨ぐ drain は現在 surface の binding で再解決する。
`RoutingControlDispatch::binding(target)` で記録済み
`PluginParameterBinding` 証拠を公開し、same-generation refresh
では `ParamCatalog` の `ParamInfo` 列挙で既存 binding を
`revalidate` してから rebind する。`sf03_l6.rs` 6 fixture で
realtime/offline 独立経路を固定した。

## Context

Issue #106 SF-03。map design
`docs/designs/sf-03-map-semantic-runtime-mapping-design.md` §8.2-1/2
が残存 headless 作業として consumer 横断 same-Revision fixture と
SF-ACT-06 残件（automation→`TimedParamEvent` 配線・Fast Control
active-generation gate・有界 Queue）を指定。slice1〜5
（PR #191/#192/#195/#197/#198）で lowering/SourceMap/activation/
fault/plugin host/NoRebuild/session 接続は main 統合済み。
spec `sf-03-slice6-act06-control-surface.md`・design・test-spec
に従う。

## Implementation

### Changes

- `crates/synvelis-player/src/routing_control.rs`（新規）:
  - `RoutingControlDispatch`: `attach`/`sync`/`tick`/`fast_control`/
    `binding`/`report`/`queue_depth`/`last_block`/`last_tick`/
    `with_queue_capacity`/`generation`。
  - `GenerationSurface`: `EvalPlan`+`EvalScratch`・
    `BTreeMap<ProcessorId, NodeSurface{session, dispatch}>`・
    `ParamCatalog`・`SurfaceReport`。
  - `tick_inner`: stale 検出→`refresh_surface`→gate→
    `drain_queue`+`collect_automation` を node 別 batch に merge→
    `flush_batches`（`session.dispatch` 1 batch/node、QueueFull は
    fast-control write を queue へ戻す）。
  - `fast_control_inner`: surface current/pin mismatch 検査→gate→
    `lower_for_node`（`lower_event` の capability/write-means/
    mapping 検証込み）→`session.dispatch` 即時、または queue。
  - `refresh_surface`: `ControlGraph::compile` 再生成 +
    session pin が現行の node のみ `session.revalidate`（現行
    `ParamInfo` 列挙で defunct 化）→`bind_node`。
  - `binding(target)`: surface 内全 node の session を走査し
    記録済み `PluginParameterBinding` を clone で返す。
- `crates/synvelis-player/src/lib.rs`: `pub mod routing_control`
  + public export（`RoutingControlDispatch`・`FastControl*`・
  `TickReport`・`SurfaceReport`・`SyncOutcome`・`ParamCatalog`・
  `ControlDispatchError`）。
- `crates/synvelis-player/tests/sf03_l6.rs`（新規・6 件）:
  deterministic recorder plugin（`PluginHostProcessors` 経由）+
  `StaticProcessorHost`/`GainProcessor`（`VerifiedGain` wrapper
  で offline gate 通過）を使う headless fixture。
- docs: spec/design/test-spec/impl record 新設 + 各 README index。

### Decisions During Implementation

- **batch merge**: `ParamDispatch::dispatch` は node plan を
  置き換えるため、tick 内で queue drain と automation を node
  ごとに 1 `session.dispatch` batch に merge。呼出しを跨ぐ
  plan-replace は upstream 契約（`PlanReplaced` ledger）に従い、
  module docs に「≈現在位置 stamp・適切な service 周期」を
  consumer 前提として明記。
- **catalog refresh**: one-shot control は再 vend せず、session/
  dispatch を世代内で存続させたまま `session.revalidate`+
  `bind_node` で再 bind — param 消失/drift は defunct 化され
  fail closed。
- **binding 証拠**: `PluginBindingSession::binding` は既に記録を
  持つが `RoutingControlDispatch` から到達不能だったため、
  surface nodes を走査する public accessor を追加（clone 返却、
  pin を外部照合可能に）。
- **queue drain の再解決**: queue は `FastControlWrite`（semantic
  intent）のみ保持し、drain 時に現在 surface の `lower_for_node`
  で再 lower — stale `(node, param)` pair を保持しない。
- **offline attach**: `BuildMode::Offline` 世代でも同一
  `RoutingControlDispatch` 契約で attach 可能。static host は
  control を vend しないため `without_control` に typed 記録、
  `Verified` 宣言 processor でも plugin host 経路が無ければ
  param surface は存在しない（fixture 06 で固定）。

## Related Experiments

なし。

## Validation

- `cargo test -p synvelis-player --test sf03_l6`: 6/6 pass。
- `mise run fmt` / `mise run fmt-check` / `mise run check` /
  `mise run lint`: exit 0（Windows、toolchain 1.99.0）。
- `mise run test`（`cargo test --workspace --locked`）: exit 0、
  全 workspace test green（`sf03_l6` 含む）。
- 補助証拠: `sf03_l6.rs` 単体実行で automation delivery・
  activation gate・queue/typed block・世代 swap replay・
  Equivalent refresh・offline attach を実測。

## NotRun（正直な記録）

- 実 OS/実 plugin format（VST3/AU/CLAP）/実 GUI scene: NotRun
  （#114 gate・headless slice の範囲外）。
- SF-ACT-07 Partial Preview vs Export policy: 未規定・別 Issue。
- SF-ACT-08 Ruviform RT/render/AudioLens 受入証拠: upstream gate。
- SF-ACT-09 device config-only compile 判定・性能 baseline: 別 slice。
- AudioLens f64/RT 非回帰: Ruviform #294 の責務・NotRun。
- `synvelis-render` の semantic lowering 消費（旧
  `lower_with_inserts` 経路の移行）: consumer slice・本 slice 範囲外。
- `ParamDispatch::observe` を用いた delivery ledger の consumer
  側統合: span/call window は driver 責務・後続検討。

## Risks

- plan-replace semantics: control 呼出しを跨ぐと未配送 event が
  `PlanReplaced` になり得る。fixture は `eng.run` を挟む実利用形で
  固定し、module docs に契約を明記。consumer が厳密な delivery
  保証を要する場合は `observe` 統合が必要（follow-up）。
- `active_generation` は caller 供給 — 誤った値を渡す consumer は
  gate を迂回し得る。`rt_status()` 由来を前提とする契約を docs 化。
- queue capacity は caller 設定 — 実機 latency との整合は
  SF-ACT-05/09 の実機 slice で再検証。

## Follow-ups

- SF-04（#107）consumer が `binding(target)` 証拠で
  receipt/SourceMap との世代照合を行う接続。
- `synvelis-render` の semantic lowering 消費・GUI fast-control。
- SF-ACT-07 policy 決定・SF-ACT-08/09・実機検証（別 slice/Issue）。
- `ParamDispatch::observe` 連携による delivery 観測の consumer 統合。
