---
id: "TSPEC-lQ0qYg1as24XtlZ1sHNNWN"
type: "test-spec"
status: "approved"
title: "TIM-03 session→driver/routing wiring test spec — TIM03-SW01..NN"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
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
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/designs/tim-03-session-driver-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/tim-03-task1-session-driver-wiring.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/designs/tim-03-session-driver-wiring-design.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/designs/tim-03-session-driver-wiring-design.md"
  verified-by: []
  references:
    - "docs/tasks/tim-03-task1-session-driver-wiring.md"
  defers: []
  deferred-by: []
---

# TIM-03 session→driver/routing wiring test spec

## Purpose

`ProcessBoundarySession` の driver/routing 配線が、provenance・
frame-base・policy/budget・generation・fail-closed の各不変条件を
headless で固定することを保証する。実 plugin/OS/DAW/consumer は
対象外（`NotRun`/`Unsupported`）。

## Feature

`TIM03-SW` — session→driver/routing wiring（Issue #127 slice）。

## Rules

- **SW-01 session open（driver）**: `AttachedNode.boundary` に
  spec を与えた activation で `attached_boundary` が session +
  dispatch を返し、`session.provenance()` が activation の
  instance/node/graph generation を pin する。spec なし node は
  `None`。
- **SW-02 session open（routing）**: `BoundaryDeclaration` 登録済み
  processor の surface で `boundary_session`/`boundary_provenance`
  が session と pin（instance/node/capability/graph generation）
  を返す。未宣言 processor は `NoSession`。
- **SW-03 stage_window 合成**: driver の `set_loop`/`panic`/
  `seek`/resume、routing の `boundary_stage_window` が
  `ProcessBoundaryPlan` を返し node の control queue に command
  が届く（receipt/動作で確認）。window legality 違反・
  provenance 不整合は staging 前に typed refusal。
- **SW-04 dispatch 合成**: driver の `session.dispatch_params`/
  `dispatch_midi` が既存 `ParamDispatch` seam を通り、ledger の
  `Dispatched`/event 配送が確認できる。誤 target・foreign
  dispatch・stale frame base は typed refusal で何も staging
  されない。
- **SW-05 transport metadata honesty**: `attached_span` が返す
  `SpanContext` が driver-commanded 状態を反映する —
  `set_loop` 後 `loop_ticks=Some`、wrap 通過後 `occurrence>=1`
  かつ `position_ticks` が wrap 済み（loop end 未満）、
  `panic` 後 `status=Stopped`・position 0、`transport_generation`
  が receipt 由来で `0` 固定でない。
- **SW-06 stale/foreign 拒否**: activation/generation 交代後に
  旧 provenance を提示すると `InstanceMismatch`/
  `GraphGeneration` 等の typed refusal で何も staging されない
  （driver・routing 両経路）。
- **SW-07 QueueFull**: node の bounded control queue を溢れさせた
  状態での transport staging が `BoundarySessionError::QueueFull`
  （driver は `ActivateError::BoundarySession`/Err、routing は
  `BoundaryStageError::Session`）として観測できる。
- **SW-08 capability 分岐**: `Unsupported` spec の activation は
  `ActivateError::BoundarySession(Capability)` で publish 前に
  fail、routing の `OfflineUncertified` declaration は connect が
  既存どおり拒否（task2 回帰）。
- **SW-09 非回帰**: `tim03_request_plan.rs`・既存 driver/routing
  test（`pc_b`/`tim02`/`l1_e2e`/`sf03_*`）が pass。`flush_batches`
  の param 配送経路は不変。
- **SW-10 min/max legality**: session が返す `ProcessBoundaryPlan`
  の全 segment が `min_frames <= frames <= max_frames`（0 長
  call なし）。

## Examples

- loop `(100,200)` + 再生で wrap を跨いだ block の span が
  `occurrence=1`, `position_ticks∈[100,200)` を示し、in-loop
  event が occurrence ごとに再投影される。
- `panic()` → `resume_attached_transport()` で `transport_generation`
  が receipt 経由で増加し status が Stopped→Playing に遷移する。
- gen1 surface の provenance を gen2 sync 後に提示 →
  `GraphGeneration`/`InstanceMismatch` typed refusal。

## Guarantees

- session は node ごと・activation/generation ごとに開き、旧
  provenance は typed refusal。
- `observe_attached_block` の span context に捏造固定値
  （`TransportGeneration::new(0)`・`LoopOccurrence::new(0)`・
  `loop_ticks: None`・`position==cursor`）が残らない。
- param 配送権限は `PluginBindingSession`/`ParamDispatch` が唯一
  — boundary session は transport/plan window gate に限定。
- 非宣言 node の legacy 挙動は維持。

## Non-goals

- Tempo/Loop 自動導出、tempo_revision 実値、Degraded deferral の
  mirror 追従、caller out-of-band staging の mirror 反映、
  routing への param/MIDI/`stage_plan` session API 公開、
  実 plugin/OS/DAW/consumer 検証。

## Risk

- signature 変更（`seek`/`resume`/`set_loop`/`clear_loop`/`panic`）
  の波及は player test のみに限定されること。
- `boundary_attach` と `connect` の spec 一致は `boundary_spec`
  集約で構造的に保証されること。
