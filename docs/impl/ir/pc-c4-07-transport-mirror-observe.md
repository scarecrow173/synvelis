---
id: "IMPL-7c4e9a1b2d3f4e5a6b7c8d9e0f1a2b3c4d5e"
type: "impl"
status: "completed"
title: "PC-C4-07 transport mirror + observe_block + capability rebind"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added:
      - type: "file"
        path: "docs/specs/pc-c4-07-transport-mirror-observe.md"
      - type: "file"
        path: "docs/designs/pc-c4-07-transport-mirror-observe-design.md"
      - type: "file"
        path: "docs/plans/pc-c4-07-transport-mirror-observe-plan.md"
      - type: "file"
        path: "docs/tasks/pc-c4-07-transport-mirror-observe-task.md"
      - type: "file"
        path: "docs/test-specs/pc-c4-07-transport-mirror-observe.md"
      - type: "file"
        path: "docs/impl/ir/pc-c4-07-transport-mirror-observe.md"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c4_07_transport_observe.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c4_07_capability_rebind.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/driver.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_control.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/designs/pc-c4-07-transport-mirror-observe-design.md"
    - "docs/plans/pc-c4-07-transport-mirror-observe-plan.md"
    - "docs/tasks/pc-c4-07-transport-mirror-observe-task.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-07-transport-mirror-observe.md"
  verified-by: []
  references: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C4-07 implementation record

## Summary

Issue #101（PLUG-C4-04 残件）slice 7 — routing path の
`RoutingControlDispatch::observe_cycle` が caller 供給の
`TransportSpan` を要求し `syn.control.observe` が呼出ごとに
虚構 span を生成していた問題を、`driver.rs` の
`TransportMirror` を `pub(crate)` 化して `NodeSurface` ごとに
保持させ、`observe_block` で runtime 所有の transport 状態から
span を内部導出するようにした。capability-drift → republish →
rebind の結合テスト（test-only）も追加。

## Context

`observe_cycle` は caller が `TransportSpan` を捏造して渡す
契約で、`ui_ops.rs` の `syn.control.observe` は呼出ごとに虚構
span を生成していた — transport 位置・status・generation が
runtime の実状態と無関係に「それらしい」値になっていた。
driver path は `TransportMirror`（`driver.rs`）で commanded
transport → `SpanContext` 投影を解決済みだが、routing control
surface には同等物がなかった。本 slice は routing path に同じ
mirror を持たせ、span を runtime 所有の transport 状態から
内部導出する（design 問題節）。あわせて capability rebind の
junction を headless で一気通貫証明する（test-only、src 変更は
原則不要）。

上流文書: spec `docs/specs/pc-c4-07-transport-mirror-observe.md`、
design `docs/designs/pc-c4-07-transport-mirror-observe-design.md`、
plan `docs/plans/pc-c4-07-transport-mirror-observe-plan.md`、
task `docs/tasks/pc-c4-07-transport-mirror-observe-task.md`、
test-spec `docs/test-specs/pc-c4-07-transport-mirror-observe.md`。

## Implementation

### `driver.rs`

- `TransportMirror` を `pub(crate)` 化 (constructor・
  `effective_generation`/`raw_position_at`/`position_at`/
  `span_context`/`note_receipts`/`anchor_at`/`occurrence_before`/
  `note_play`/`note_stop`/`note_seek`/`note_set_loop`/`note_clear_loop`/
  `note_tg_bump` を同様に)。routing control から再利用するための
  visibility 変更のみで、driver path の動作は不変。
- `note_pause(frame)` を追加 — Pause が status `Paused` で
  position を凍結し anchor を取り直し tg を bump する完全な
  command coverage (scheduler の Pause apply と同じ意味論)。

### `routing_control.rs`

- `NodeSurface` に `transport: TransportMirror`、
  `staged_transport: Vec<(TransportCommand, u64)>`、
  `scheduled_transport: Vec<(u64, TransportCommand)>`、
  `last_span: Option<TransportSpan>` を追加。
- command 記録点 (push 成功後のみ):
  - `fast_control_inner` / `flush_batches` の
    `session.dispatch` 成功後に `(Play, 0)` を staged へ —
    `ParamDispatch::dispatch` が内部で push する Play を同じ
    due で mirror に反映 (`install_plan` は transport command
    ではないので記録しない)。
  - `boundary_stage_window` の `stage_window` 成功後に commands
    を staged へ — TIM-03 ingress 点と同一。
- `observe_block(generation, call_start, call_frames)`:
  surface generation 不一致は `None` (fail-closed)。node ごとに
  `observe_node_block` が staged→scheduled 解決
  (`abs_due = call_start.frame + due_frames`)、apply frame での
  window 分割、segment 毎の `span_context` 導出 +
  `observe_cycle` 供給、observation merge (後 segment 優先)、
  receipt merge → `note_receipts`、window 末尾以降の apply 保持、
  `last_span` 記録を行う。
- `note_transport_command`: `Play`/`Pause`/`Stop`/valid `Seek`/
  valid 非空 `SetLoop`/`ClearLoop` を mirror の `note_*` へ
  写像。`media_time_ticks` で node の frame base へ変換し、
  scheduler が reject する command は note しない
  (投影不能な apply を mirror が捏造しない)。
- `last_observed_span(processor)` — 診断 accessor
  (driver の `attached_span` 相当)。

### `lib.rs`

変更なし — `observe_block`/`last_observed_span` は
`RoutingControlDispatch` の method、`TransportMirror` は
`pub(crate)` で公開面は不変。

## Related Experiments

### Adopted

なし — `metadata.experiments.adopted` は空。

### Rejected

なし — `metadata.experiments.rejected` は空。

## Validation

- `cargo test -p synvelis-player --test pc_c4_07_transport_observe`
  — 10/10 pass。
- `cargo test -p synvelis-player --test pc_c4_07_capability_rebind`
  — 2/2 pass。
- canonical `mise` 検証 — PR 本文参照。

## Risks

- mirror は commanded-state 投影 — push と callback head の
  race、scheduler 側の reject は mirror に届かない。generation
  軸（transport/plan generation）は receipt が訂正するが、
  status/position は commanded 近似のまま（driver mirror が
  文書化するのと同じ honesty contract、design 正直さの境界）。
- caller が window を飛ばして観測した場合、staged command の
  解決は最初に観測した `call_start` 基準になる — run loop の
  「毎 block 連続観測」契約が前提。
- capability rebind junction は test-only（`SharedAdapter`
  経由の drift 注入）— production 経路の rebind 配線・
  `register_params`/`register_boundary` の epoch bump を
  production で起こす経路は本 slice の範囲外。

## Follow-ups

- app 層の `drain_output_reports` 周期配線、engine-driver 配線、
  `syn.control.status` `plan_generation` 露出 — 後続 slice。
- production composition root `PluginHost`、wire schema L1、
  real VST3/OS/GUI/AudioLens — 外部 gate、未実行。
