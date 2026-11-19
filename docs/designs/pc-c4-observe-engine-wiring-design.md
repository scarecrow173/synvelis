---
id: "DESIGN-c4e8b1a2f5d30947e6c1a8b2d5"
type: "design"
status: "approved"
title: "PC-C4 observe engine wiring design — observe_block の engine-driver 配線"
created: "2026-11-15"
updated: "2026-11-15"
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
    - "docs/specs/pc-c4-observe-engine-wiring.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-observe-engine-wiring.md"
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/designs/pc-c4-07-transport-mirror-observe-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c4-drain-composition-design.md"
  refines:
    - "docs/designs/pc-c4-07-transport-mirror-observe-design.md"
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-observe-engine-wiring.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-observe-engine-wiring.md"
  verified-by:
    - "docs/test-specs/pc-c4-observe-engine-wiring.md"
  references:
    - "docs/designs/pc-c4-07-transport-mirror-observe-design.md"
    - "docs/designs/pc-c4-drain-composition-design.md"
  defers: []
  deferred-by: []
---

# PC-C4 observe engine wiring design

## 概要

`observe_block` が要求する物理 call window を、caller の捏造では
なく **engine が処理した事実**から供給する。`RoutingEngine` は全ての
`rt.process` 呼出し（`run` の各 block と `seek` の quiesce pass）を
見る唯一の場所なので、node cursor domain の head 追跡と generation
判定をここに置き、処理済み block ごとに observer callback で
`(generation, call_start, call_frames)` を報告する。
`SessionRoutingEngine::run` がその報告を受けて attach 済み
`RoutingControlDispatch::observe_block` を駆動し、merged
`DispatchCycle` を `Inner` に buffer する。app は `ui_pump` 周期で
buffer を drain し `feed_control_cycle` へ流す。

## cursor domain の根拠（設計の中核）

`observe_block` の `call_start` は **node cursor domain** でなければ
ならない:

- `PluginNode.cursor` は attach 時に `EngineFrame(ClockEpochId(0), 0)`
  で始まり（ruviform `attach.rs`/`node.rs`）、その plan が
  `active_generation` になった後の `process` 呼出し毎に `frames`
  ずつ進む。generation が切り替わると新 plan の node は別 instance
  で cursor 0 から数え直す。
- engine の `cursor()` は seek で `engine_frame` へ再 anchor される
  engine domain であり、rt が実際に処理した frame 数とも node
  cursor とも一致しない（publish 前の処理 block・seek・activation
  タイミングの全てで乖離し得る）。
- node cursor の epoch は routing path では常に `ClockEpochId(0)` —
  `boundary_stage_window`/`ParamDispatch` は `request`/
  `install_plan` のみを送り、`SetClock`/`DeviceRestart`/
  `set_clock_anchor` を routing-path node へ送る経路は存在しない。

したがって engine は「最後に処理した block がどの generation の
node cursor をいくつ進めたか」だけを追跡すればよい:

- `observe_gen`: 直近 block を走らせた generation（`0` = 未活性）。
- `observe_head`: 次の `rt.process` が始まる node frame
  （= 直近 block の開始 head + その frames）。
- block 完了後に `ran = active_generation`（process 後 snapshot）を
  読み、`ran == observe_gen` なら `call_start = observe_head`、
  異なればその block が activation block なので `call_start = 0`
  （node cursor は活性化 block で 0 から数え始める）。その後
  `observe_gen = ran`、`observe_head = call_start + frames`。

この規則は seek の quiesce pass を含む全 `rt.process` に適用する —
観測の有無に関わらず head は進む（観測しない block の report は
次の観測 window に drain される既知の近似であり、receipt の
`applied_frame` が generation 補正の権威として残る）。

## API 形状

- `RoutingEngine::run_observed(blocks, policy, observe:
  &mut dyn FnMut(ObservedBlock))` — `run` と同一の pump/process/
  capture loop に、block 完了ごとの observer 呼出しを足したもの。
  `run` は no-op observer へ delegate し既存 caller の挙動は不変。
- `RoutingEngine::seek_observed(engine_frame, observe)` — quiesce
  pass の `rt.process` 成功時に同じ window を報告する。
  `seek` は no-op observer へ delegate。
- `ObservedBlock { generation: u64, call_start: EngineFrame,
  call_frames: u32 }` — observer へ渡す 1 block 分の証拠。
  `call_frames` は `u32::try_from(frames).unwrap_or(u32::MAX)`
  （`observe_node_block` と同じ飽和規則）。

callback 形状を選んだ理由: `SessionRoutingEngine` は `Inner` 内の
`dispatch: Option<RoutingControlDispatch>` と `bridge.engine` を同時に
借りる必要があり、engine が dispatch を直接知る設計（同一 crate
とはいえ循環参照的）より「engine は window の真実だけを報告し、
消費側が `observe_block` を呼ぶ」分離が境界として正しい。

## app 側の流れ

`SessionRoutingEngine::run`:

1. `inner.borrow_mut()` → `inner.bridge.engine_mut()` と
   `inner.dispatch` を disjoint field borrow で同時に取る。
2. `run_observed` の observer 内で `dispatch.observe_block(
   window.generation, window.call_start, window.call_frames)` を呼ぶ。
   surface generation が ran generation と一致しない window は
   `None` で捨てられる（published 未活性・stale surface・
   pre-activation 全て fail-closed）。
3. 返った `BTreeMap<ProcessorId, DispatchCycle>` を block ごとに
   1 つの `DispatchCycle` へ merge（observations/receipts を連結
   — target は node を跨いで一意）し `Inner.pending_observe_cycles`
   に push する。

`SynvelisApp`:

- `drain_control_observations()`（pub、決定的 1 pass — fixture/
  明示 driver 用）: `engine.control_drain_observations()` で Vec を
  取り出し、各 cycle を `feed_control_cycle` へ流して fed 数を返す。
- `ui_pump` 末尾で `drain_control_reports` の直後に呼ぶ。
  `feed_control_cycle` の `Reported`-only 規則は不変。

### Reported 伝播の時間差（正直な契約として明記）

buffer される cycle は observe 時点の stage snapshot である。
param report が `Reported` に進めるのは `drain_output_reports`
（host→ledger）であり、observe_block の cycle には次の block 以降
で初めて `Reported` が現れる。したって feed は「report が ledger
に載った後の最初の観測 block」で起きる — 1 block 分の遅延は
捏造ではなく ledger の実状態をそのまま反映する。`feed_plugin_report`
は upsert なので同一 Reported の再 feed は冪等。

## `syn.control.status` の `observe` 節

surface の `controlled_processors` ごとに:

- `last_observed_span`: `dispatch.last_observed_span(processor)`
  の `TransportSpan` を verbatim serialize（start/end/status/
  position/loop/occurrence/clock_epoch/transport_generation/
  plan_generation/tempo_revision/deliverable）。未観測は `null`。
- `plan_generation`: その span の `ctx.plan_generation`。
  span 未観測なら `null`。

processor key は tagged `ProcessorId`。`with_control_dispatch`
経由で集めるため engine 側に新規 accessor は不要
（`report().controlled_processors` + `last_observed_span` の既存
API のみ使用）。

## 失敗時の振る舞い

- surface 無し / generation 不一致: `observe_block` が `None` —
  buffer に何も入らず、window は読み捨てられる。`run` 自体の
  `ProcessError` 経路は不変。
- `rt.process` 失敗 block: `run` は `?` で中断し、失敗 block は
  処理されていないので window を報告しない（head も進めない）。
- `seek` の quiesce pass が失敗した場合: `observe_activation` の
  既存挙動（無条件呼出し）は維持し、head 追跡と observer は
  成功時のみ — node cursor が進んでいない block を観測しない。
