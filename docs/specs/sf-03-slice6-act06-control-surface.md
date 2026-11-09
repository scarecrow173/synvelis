---
id: "SPEC-034UJpxSHTVIxHKdrntkyR"
type: "spec"
status: "approved"
title: "SF-03 slice6 — SF-ACT-06 control surface headless slice（automation→TimedParamEvent・Fast Control gate・有界 Queue・catalog refresh）"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-plugin-param-binding.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  derived-by:
    - "docs/designs/sf-03-slice6-act06-control-surface-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice6-act06-control-surface.md"
  references:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  defers: []
  deferred-by: []
---

# SF-03 slice6 — SF-ACT-06 control surface headless slice（automation→TimedParamEvent・Fast Control gate・有界 Queue・catalog refresh）

## What Should Be Built

`RoutingControlDispatch` — `RoutingEngine` が publish した世代の
processor control（`take_processor_control` の one-shot vend）を消費し、
semantic Control model（automation lane・parameter target）を
`PluginBindingSession`/`ParamDispatch` 経由で plugin param へ lowering
する headless control surface を `synvelis-player` に公開する。

- automation 評価（`EvalPlan`）→ `TimedParamEvent` → 世代 pin 済み
  binding 検証 → node plan install までの engine 配線。
- Fast Control write の active-generation gate: RT が観測した世代が
  surface 世代と一致するときのみ即時 dispatch、それ以外は型付き
  Block または有界 Queue。
- 世代 swap を跨ぐ Queue: drain 時に *現在* surface の binding で
  再解決し、同一 semantic intent を新世代 instance へ届ける。
- catalog（plugin param 列挙）refresh 時の既存 binding revalidate:
  param 消失・drift で defunct 化し、以後の write を fail closed。
- `PluginParameterBinding` 証拠への公開 accessor（generation/
  capability/instance pin を receipt・SourceMap の世代と照合可能）。

## Why Now

map design `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`
§8.2-1/2 が残存 headless 作業として consumer 横断 same-Revision
fixture と SF-ACT-06 残件（automation→`TimedParamEvent` 配線・
Fast Control active-generation gate・有界 Queue）を指定している。
#104/#121/#105 の型付き契約と PC-C4 の binding/dispatch 基盤は
main 統合済みで、本 slice はそれらを engine へ接続する公開契約面のみ。
SF-04（#107）が本契約を消費するため headless 契約の確定を優先する。

## Users and Value

- SF-04 以降の consumer: `(project_revision, generation)` で照合可能な
  control surface 契約と、世代 pin・frame-base pin・capability pin を
  持つ binding 証拠。
- headless fixture 作者: realtime/offline 独立に駆動できる
  deterministic な param delivery 経路。

## Scope

### In Scope

- `crates/synvelis-player/src/routing_control.rs`（新規公開 module）:
  `RoutingControlDispatch`・`FastControlWrite`/`FastControlOutcome`/
  `FastControlBlock`・`TickReport`/`SurfaceReport`・`SyncOutcome`。
- `RoutingControlDispatch::binding(target)` — session 記録の
  `PluginParameterBinding` を clone で公開。
- same-generation surface refresh での `ParamCatalog` revalidate。
- `crates/synvelis-player/tests/sf03_l6.rs` acceptance fixture（6 件）。
- `crates/synvelis-player/src/lib.rs` の公開 export。

### Out of Scope

- GUI fast-control・実 OS/実 plugin/device 検証（#114 gate・NotRun）。
- SF-ACT-07 Partial Preview vs Export policy（未規定・別 Issue）。
- SF-ACT-08 Ruviform RT/render/AudioLens 受入証拠（upstream gate）。
- SF-ACT-09 device config-only compile 判定・性能 baseline。
- Ruviform generic graph validator/compiler/RT scheduler/plugin host
  の複製・改変。Project/Control Parameter 正本の重複。
- `synvelis-render` の旧 `lower_with_inserts` 経路からの移行
  （consumer 接続は別 slice）。
- AudioLens f64/RT 非回帰（Ruviform #294 の責務・NotRun）。

## Requirements

- automation→`TimedParamEvent` は `lower_event` の capability/
  write-means gate を通り、`TimedEvent` 以外の means は fail closed。
- Fast Control は active-generation 不一致で deliver しない:
  未活性は Queue（既定容量・溢れは `QueueFull` で新規 write を
  拒否・追い出しなし）、surface なし/stale/unbound/lowering 失敗は
  型付き `Blocked`。
- Queue drain は現在 surface の binding で再解決し、binding が
  無ければ typed drop（`queue_dropped`）。node channel `QueueFull`
  時は fast-control write を queue へ戻す。
- 1 node 1 tick につき automation + drained queue を単一 atomic
  `PluginBindingSession::dispatch` batch に merge する。
- `sync` は指定世代の control を one-shot vend で surface に
  適用し、同一世代は `Current`（refresh のみ・control 再 vend
  なし）、別世代は `Swapped`（新 session/instance・queue 保持）
  を型で返す。RT 未観測世代への先行 sync は caller の責務で、
  gate は `active_generation` 引数が担保する。
- `binding(target)` は attach 済み session の記録を返し、
  `graph_generation`/`capability_generation`/`instance_id` pin を
  外部照合に供する。
- catalog refresh は既存 binding を現行 `ParamInfo` 列挙で
  revalidate してから rebind し、param 消失/drift を defunct 化する。

## Acceptance Criteria

- [x] `sf03_l6.rs` 6 fixture が green: automation delivery・
  activation gate・queue/gate/typed block・世代 swap replay・
  Equivalent refresh・offline attach。
- [x] `RoutingControlDispatch::binding` が generation pin を返し、
  receipt/SourceMap の世代と一致することを fixture で確認。
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt` exit 0。
- [x] 実 OS/plugin/consumer・AudioLens f64/RT は NotRun として記録。

## Deferred Design Concerns

- SF-ACT-07 Partial Preview vs Export の mode 別 fail-closed policy —
  現行両 mode fail-closed を維持、policy 決定は後続 Issue。
- SF-ACT-08/09・実機 RT latency/underrun・実 device epoch restart —
  実機タスクとして別 slice。
- GUI fast-control・`synvelis-render` の semantic lowering 消費 —
  consumer/GUI slice。
- plan-replace を跨ぐ dispatched event の delivery 観測強化
  （`ParamDispatch::observe` 連携）は consumer 側 orchestration の
  後続検討事項。

## Sources and Relations

- Issue #106（SF-03）・map design §8.2-1/2・§8.3/8.4。
- `docs/specs/sf-03-map-semantic-runtime-contract.md`（親 contract）。
- PC-C4 `PluginBindingSession`/`ParamDispatch`/`TargetCapability`
  契約（`docs/specs/pc-c4-plugin-param-binding.md`）。
- 実装: `crates/synvelis-player/src/routing_control.rs`、
  `crates/synvelis-player/tests/sf03_l6.rs`。
