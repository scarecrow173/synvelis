---
id: "TASK-034Um6UmzaMloEGrFyYAS2"
type: "task"
status: "in-progress"
title: "SF-04 slice1 T1 — project/player Semantic Probe Adapter + ruviform pin bump"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-04-slice1-semantic-probe-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-04-slice1-semantic-probe-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-04-slice1-semantic-probe-adapter.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice1 T1 — project/player Semantic Probe Adapter + ruviform pin bump

## Work

- ruviform pin `cc7d5296` → `0f5b5737`（#308 RebindReceipt/CapturePolicy、
  #309 ProbeClockMark/ProbeTimeContext/ObservationCoverage を含む main HEAD）。
- `synvelis-project/src/probe.rs`: Issue #107 §2 の semantic contract 型
  （`SignalBoundary`/`SemanticSignalEntityRef`/`SemanticProbePoint`/
  `ProbeSubscriptionKey`/`ProbeResolveState`/`SemanticProbeBinding`/
  `ProbeCaptureSegment` 他）と `resolve_probe_point`（fail-closed、
  Unavailable/Unsupported を正直に返し PCM を捏造しない）。
- `synvelis-player/src/probe.rs`: `ProbeSession`（`ProbeBroker`/
  `ProbeCollector`/binding ledger/segment ledger）— request/set_mode/
  capture/stop_capture/remove/on_publish/drain。requested/queued と
  RT-installed を区別し、generation 不一致を stale として拒否。
- `routing_engine.rs`: `rt.set_probe_deck` install、`publish` 成功時の
  `broker.on_publish` rebind、probe public passthrough。
- headless テスト `sf04_project_probe.rs` / `sf04_probe_adapter.rs`。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sf-04-slice1-semantic-probe-adapter.md
- docs/designs/sf-04-slice1-semantic-probe-adapter-design.md
- docs/specs/sf-04-slice1-semantic-probe-adapter.md
- https://github.com/scarecrow173/synvelis/issues/107

## Dependencies

- SF-03 slice1..5（`RoutingSourceMap`/`RoutingBuildKey`/`RoutingEngine`/
  `source_map()`）— merge 済み。
- ruviform `0f5b5737` pin — commit `10a1bea` で bump 済み。

## Affected Files

- `crates/synvelis-project/src/probe.rs`（新規）
- `crates/synvelis-project/src/lib.rs`（export）
- `crates/synvelis-player/src/probe.rs`（新規）
- `crates/synvelis-player/src/routing_engine.rs`（probe 配線のみ）
- `crates/synvelis-player/src/lib.rs`（export）
- `crates/synvelis-project/tests/sf04_project_probe.rs`（新規）
- `crates/synvelis-player/tests/sf04_probe_adapter.rs`（新規）
- `Cargo.lock`（pin bump、commit 済み）
- `docs/`（spec/design/plan/task/test-spec/impl record）

## Done When

- [ ] pin が `0f5b5737` で `check-ruviform-rev` green
- [ ] project contract 型が定義・export され、`resolve_probe_point` が
  fail-closed で Unavailable/Unsupported を返す
- [ ] player `ProbeSession` + engine 配線で lifecycle（Requested→
  QueuedForBinding→BoundActive）と generation-guarded stale rejection、
  `RebindReceipt` 経由の capture segment cut が動作
- [ ] `sf04_*.rs` が green、`mise run fmt-check`/`check`/`lint`/`test`/
  `test-rt` green

## Verification

- [ ] `mise run fmt-check` / `mise run check` / `mise run lint`
- [ ] `mise run test` / `mise run test-rt`
- [ ] `mise run check-ruviform-rev`
- [ ] `sf04_project_probe.rs`: strip/processor/send × boundary の解決、
  Unavailable/Unsupported/Unresolved、明示 port 検証
- [ ] `sf04_probe_adapter.rs`: install→drain で BoundActive、
  generation 不一致 status → StaleGeneration、publish で segment cut・
  auto-recapture 無し、remove → Stopped
