---
id: "TSPEC-034UJrwI2ZQcJUrzIaZFZE"
type: "test-spec"
status: "approved"
title: "SF-03 slice6 test-spec — sf03_l6 control surface acceptance"
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
    - "docs/designs/sf-03-slice6-act06-control-surface-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-slice6-act06-control-surface-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-03-slice6-act06-control-surface.md"
  verified-by: []
  references:
    - "docs/test-specs/sf-03-slice4-act-acceptance.md"
  defers: []
  deferred-by: []
---

# SF-03 slice6 test-spec — sf03_l6 control surface acceptance

## Purpose

SF-03 slice6 の受入契約を固定する。spec
`sf-03-slice6-act06-control-surface.md` の要件を
`crates/synvelis-player/tests/sf03_l6.rs`（6 件）として、
`RoutingEngine` + `PluginHostProcessors`（決定的 recorder plugin）
による headless realtime 経路で実測する。offline attach は
static host で独立に固定する。GUI・実 OS/実 plugin/device・
SF-ACT-07/08/09 は対象外（NotRun）。

## Feature

`RoutingControlDispatch` が published 世代の one-shot processor
control を世代 surface として vend し、automation lane の評価値を
`lower_event` 経由で `TimedParamEvent` 化して世代 pin 済み
`PluginBindingSession`/`ParamDispatch` へ staging する。RT が
観測した active generation と surface 世代が一致するときのみ
deliver し、不一致では型付き Block または有界 FIFO Queue に留める。
Queue は世代 swap を跨いで現在 surface の binding で再解決され、
catalog refresh は既存 binding を現行 `ParamInfo` で revalidate
する。

## Rules

- Rule 1（automation delivery）: active 世代で `tick` が automation
  lane を `TimedEvent` として評価・dispatch し、plugin instance が
  stamp された `abs_frame`・`param_id`・評価値を受け取る。
- Rule 2（activation gate）: publish 済みでも RT 未観測
  （`active_generation = 0`）では `tick` が `gated`、`fast_control`
  が `Queued`。観測後は同一呼出しが deliver する。
- Rule 3（queue 契約）: inactive 中の write は FIFO queue に入り、
  capacity 溢れは `Blocked(QueueFull)` で新規 write を拒否し
  既存を追い出さない。unbound target・unsupported means
  （`Immediate` 等）は queue に入らず型付き `Blocked`。
- Rule 4（世代 swap replay）: `sync` が新世代 surface に
  `Swapped` し、queue の write は drain 時に新世代 binding で
  再解決されて新 plugin instance にのみ届く — 退役世代 instance
  には届かない。`binding(target)` の `graph_generation`/
  `instance_id` pin が世代ごとに新しい。
- Rule 5（Equivalent refresh）: control model のみの変更は
  `RoutingPrepare::Equivalent`（rebuild なし）で、同一世代の
  `sync` が `EvalPlan` を refresh して新 lane 内容を評価する。
- Rule 6（offline attach）: offline build の世代でも同一
  `RoutingControlDispatch` 契約で attach でき、static host では
  `without_control` に typed 記録され fast control は決して
  deliver しない。

## Examples

- `sf03_l6_01_automation_tick_delivers_on_active_generation`:
  ramp 0.2→0.6 の beat-2 評価値 0.4 が param 7・abs_frame 300 で
  recorder に 1 件配送される。
- `sf03_l6_02_gate_holds_until_rt_observes_activation`: publish
  直後は `gated`/`Queued`、`run` 後の `tick` で queue drain +
  automation が 1 batch で配送される。
- `sf03_l6_03_fast_control_gate_queue_and_typed_blocks`: active
  中の write は即時 `Delivered`、ghost target は
  `UnboundTarget`、`Immediate` means は `Lowering`、capacity 2 の
  queue 溢れは `QueueFull` で 0.33 は配送されない。
- `sf03_l6_04_queue_replays_through_new_generation_bindings`:
  gen-1 active 中に queue した write が gen-2 publish+RT 観測後の
  drain で instance[1] にのみ 0.7 として届く。
- `sf03_l6_05_equivalent_revision_refreshes_control_surface`:
  lane 書換で `Equivalent`、同一世代 `sync`→`Current` で refresh
  された plan が beat-2 の新値 0.9 を配送する。
- `sf03_l6_06_offline_build_attaches_without_plugin_controls`:
  `BuildMode::Offline` 世代で attach すると `controlled_processors`
  が空・`without_control` に processor・`bound_targets` 0、
  fast control は `Blocked`/`Queued` のみで deliver しない。

## Guarantees

- 全 write は `PluginBindingSession` の instance/node/
  capability/graph/frame-base pin と `ParamDispatch` の binding・
  param_id 照合を通る — pin 不一致は atomic に拒否される。
- 世代 pin・receipt・SourceMap の世代照合は `binding(target)` の
  証拠で外部から検証可能（cross-consumer same-Revision 契約）。
- catalog drift は `revalidate` で defunct 化され、静黙の誤配送は
  起きない。

## Non-goals

- GUI fast-control・実 OS/実 plugin/device/実 GUI scene。
- SF-ACT-07 preview/export policy・SF-ACT-08/09。
- `synvelis-render` の semantic lowering 消費・Ruviform 内部変更。
- `ParamDispatch::observe` を用いた delivery ledger の consumer
  側統合（span/call window は driver 責務）。

## Risk

- `ParamDispatch::dispatch` の plan-replace semantics: 呼出しを
  跨ぐと未配送 event が `PlanReplaced` になり得る — fixture は
  control 呼出し間に `eng.run` を挟む実利用形で固定し、
  module docs に契約を明記済み。
- recorder plugin は headless fake — 実 plugin format
  （VST3/AU/CLAP）の param 配送意味論は NotRun。
