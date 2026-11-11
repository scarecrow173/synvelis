---
id: "TASK-zHlj8Os7dQ66GPeta6Sxew"
type: "task"
status: "done"
title: "TIM-03 task1 — session→driver/routing wiring（BoundarySessionSpec vend・driver session/mirror・routing scoped API）"
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
  implements:
    - "docs/plans/tim-03-session-driver-wiring-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/tim-03-session-driver-wiring-plan.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/designs/tim-03-session-driver-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim-03-session-driver-wiring.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-03-session-driver-wiring-plan.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-03-session-driver-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-03 task1 — session→driver/routing wiring

## Work

- `process_boundary.rs`: `BoundarySessionSpec`（capability/mode/
  consent/budget の Copy spec）追加。
- `host.rs`: `ProcessorControl::boundary` field、
  `ProcessorHost::boundary_attach`（default `None`）、
  `PluginHostProcessors::boundary_spec` への解決集約 +
  `boundary_attach` override。
- `routing_engine.rs`: `RetainedConnection::boundary`、
  `stage_build` で `boundary_attach` 呼出、
  `take_processor_control` で vend。
- `routing_control.rs`: `NodeSurface::boundary`、`vend_nodes` で
  provenance mint + session open、`boundary_session`/
  `boundary_provenance`/`boundary_plan_window`/
  `boundary_stage_window` + `BoundaryStageError`。
- `driver.rs`: `AttachedNode::boundary`、`AttachedDispatch`
  （dispatch/session/`TransportMirror`/last_span）、
  activation/seek/loop/panic/resume の sessioned staging、
  `observe_attached_block` の `observe_cycle` + receipt 反映、
  `attached_boundary`/`attached_span` accessor、
  `ActivateError::BoundarySession`、fallible signature 変更。
- tests: `AttachedNode`/`seek`/transport method 追従 +
  `tim03_session_wiring.rs` 新規。

## Classification

- consumer policy/provenance layer の composition wiring slice。
  oracle・scheduler・binding 層の責務は再実装しない
  （`stage_window`/`observe_cycle`/receipt を消費するのみ）。

## Upstream Documents

- `docs/specs/tim-03-session-driver-wiring.md`
- `docs/designs/tim-03-session-driver-wiring-design.md`
- `docs/plans/tim-03-session-driver-wiring-plan.md`
- `docs/specs/tim03-process-boundary.md`・task3
- `docs/specs/pc-c4-plugin-param-binding.md`

## Dependencies

- task3 `ProcessBoundarySession` API、task2 `AttachDelivery`
  投影、PC-C4 `ParamDispatch`/`PluginBindingSession`。
- Ruviform pin `b02942c2`（merge 時点）: `AppliedReceipt`、
  `TransportScheduler::{apply,do_wrap,pos_at}`、
  `PluginNodeControl`、`SpanContext`/`TransportSpan`。

## Affected Files

- `crates/synvelis-player/src/{process_boundary,host,routing_engine,routing_control,driver}.rs`
- `crates/synvelis-player/tests/{pc_b,tim02_midi_playback,l1_e2e,tim03_session_wiring}.rs`
- docs 7 file（spec/design/plan/task/test-spec/ir）

## Done When

- spec 受入条件 4 項が headless test で固定される。
- `tim03_request_plan.rs` 回帰 + 新規 wiring test が pass。
- fmt/check/lint/test/test-rt/test-msrv が pass。

## Verification

- `mise run fmt-check` `check` `lint` `test` `test-rt` `test-msrv`
- `docs/test-specs/tim-03-session-driver-wiring.md` の rule 証拠を
  IR record に記録。
