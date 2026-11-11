---
id: "PLAN-Yn3mzcCyD4Dwd0Ga4zylHB"
type: "plan"
status: "approved"
title: "TIM-03 session→driver/routing wiring plan"
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
    - "docs/specs/tim-03-session-driver-wiring.md"
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
  verifies: []
  verified-by:
    - "docs/test-specs/tim-03-session-driver-wiring.md"
  references:
    - "docs/tasks/tim-03-task1-session-driver-wiring.md"
  defers: []
  deferred-by: []
---

# TIM-03 session→driver/routing wiring plan

## Goal

`ProcessBoundarySession` を `HeadlessDriver`（per-node activation
session + honest span context）と `RoutingControlDispatch`
（generation-local `NodeSurface` session + scoped staging API）へ
接続し、Issue #127 の session→driver/routing wiring slice を
headless で完結させる。

## Scope

spec/design の In Scope のみ。`synvelis-project`/`synvelis-app`/
`synvelis-render`/他 crate、Tempo/Loop 自動導出、実 plugin/OS、
Issue Close は対象外。

## Upstream Documents

- `docs/specs/tim-03-session-driver-wiring.md`
- `docs/designs/tim-03-session-driver-wiring-design.md`
- `docs/specs/tim03-process-boundary.md`（task1–3 完成済み oracle）
- `docs/specs/pc-c4-plugin-param-binding.md`（binding 世代 pin）

## Design Inputs

- `ProcessorConnection` は render が exhaustive destructure →
  `ProcessorControl` + `ProcessorHost::boundary_attach` 経路。
- `ParamDispatch::observe_cycle` が receipts を返す →
  `TransportMirror` の generation 補正源。
- `TransportScheduler::apply`/`do_wrap`/`pos_at` の規則を mirror
  が投影（連続モデルと代数的等価）。
- min_frames=1 の activated contract で全非零 span legal →
  Degraded deferral の mirror 乖離は到達不能。

## File Map

| File | 変更 |
| --- | --- |
| `crates/synvelis-player/src/process_boundary.rs` | `BoundarySessionSpec` 追加 |
| `crates/synvelis-player/src/host.rs` | `ProcessorControl::boundary`、`ProcessorHost::boundary_attach`、`PluginHostProcessors::boundary_spec` 集約 |
| `crates/synvelis-player/src/routing_engine.rs` | `RetainedConnection::boundary`、`stage_build`/`retain_connection`/`take_processor_control` |
| `crates/synvelis-player/src/routing_control.rs` | `NodeSurface::boundary`、`vend_nodes` session open、`BoundaryStageError` + scoped API |
| `crates/synvelis-player/src/driver.rs` | `AttachedNode::boundary`、`AttachedDispatch`/`TransportMirror`、session staging、receipt 反映 observe、accessor、`ActivateError::BoundarySession` |
| `crates/synvelis-player/tests/pc_b.rs`, `tim02_midi_playback.rs`, `l1_e2e.rs` | `AttachedNode`/`seek`/transport method 追従 |
| `crates/synvelis-player/tests/tim03_session_wiring.rs` | 新規統合 test |
| docs 6 file | 本 chain |

## Implementation Sequence

1. `BoundarySessionSpec` + host/engine vend（host.rs,
   process_boundary.rs, routing_engine.rs）— routing 側の入力
   経路を先に確定。
2. `RoutingControlDispatch` session open + scoped API
   （routing_control.rs）。
3. `HeadlessDriver` session + `TransportMirror` + staging 経由化 +
   accessor（driver.rs）。
4. 既存 test 追従（AttachedNode field・fallible signature）。
5. `tim03_session_wiring.rs` 新規 test。
6. docs 最終化（IR record）+ 検証。

## Dependencies and Risks

- task3 の `ProcessBoundarySession` API（`open`/`plan_window`/
  `stage_window`/`stage_plan`/`dispatch_params`/`dispatch_midi`）。
- `observe_cycle` receipts（`AppliedReceipt::{transport_generation,
  plan_generation, clock_epoch}`）。
- リスク: signature 変更の波及（player test のみ）、mirror の
  out-of-band 限界（doc 明記）、`boundary_attach` 二重計算
  （`boundary_spec` 集約で構造的に一致）。

## Task Breakdown

- `docs/tasks/tim-03-task1-session-driver-wiring.md`（本 slice
  唯一の task — spec/design/plan を implements する実装単位）。

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `test-msrv`（`check-headless-deps` 存在すれば）。
- `tests/tim03_request_plan.rs` 回帰 + 新規 wiring test + 既存
  driver/routing test。
- 実 plugin・実 OS・実 DAW・AudioLens f64/両 Consumer・実 GUI は
  `NotRun`/`Unsupported`。

## Open Questions / Gaps

- `AttachedNode.boundary` の caller contract（spec==実 attach 入力）
  は検証不能 — doc 明記、`boundary_attach` 経由の composition で
  構造的に保証。
- routing_control への param/MIDI/`stage_plan` session API 公開は
  権限単一化のため本 slice では行わない（design 判断）。

## Self-Review

- spec 受入条件 4 項を file map/sequence が全て覆う。
- `ProcessorConnection` 非変更・`ProcessorControl` 拡張・
  `boundary_attach` default method で render/app 互換を維持。
- driver の非 sessioned 経路は legacy 挙動を温存（test 互換）。

## Implementation Handoff

`docs/tasks/tim-03-task1-session-driver-wiring.md` に実装単位を
記録。実装は本 plan の file map/sequence に従う。

## Review Handoff

`docs/test-specs/tim-03-session-driver-wiring.md` の rule に対して
`docs/impl/ir/tim-03-session-driver-wiring-slice.md` に検証証拠を
記録する。
