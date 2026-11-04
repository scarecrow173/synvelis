---
id: "TASK-034RBFVsXUmxHPEa6zbZRx"
type: "task"
status: "done"
title: "SYN-OBS-01 task2 ObservabilityOwner Source catalog と syn.app.info logging セクション"
created: "2026-11-04"
updated: "2026-11-04"
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
    - "docs/plans/syn-obs-01-host-logging-source-catalog.md"
  implemented-by: []
  depends-on:
    - "docs/plans/syn-obs-01-host-logging-source-catalog.md"
    - "docs/tasks/syn-obs-01-task1-deps-logging-module.md"
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
    - "docs/test-specs/syn-obs-01-logging-source-catalog.md"
  references: []
  defers: []
  deferred-by: []
---

## Work

- `src/app.rs` `ObservabilityOwner`:
  - フィールド追加: `sources: Vec<MetricSource>`、
    `logging_health: Option<LoggingHealth>`、`logging_state: InstallState`。
  - `new()` で `counter()` の reader を `CounterSource`{descriptor:
    `CounterDescriptor{key:"synvelis.app.intents"/"synvelis.app.intent_errors",
    unit: ObservationUnit::Count, reporting: ReportingHint::SnapshotOnly}`、
    `scope: ObservationScope::Process`}として `MetricSource::from` で
    catalog 化。
  - `attach_logging(state: InstallState, health: Option<LoggingHealth>)`、
    `sources() -> &[MetricSource]`、
    `logging_snapshot() -> Option<HealthSnapshot>`。
  - `SynvelisApp::attach_logging(state, health)` — owner へ委譲。
- `app_info()` に `logging` セクション追加: `state`/`sinks[]`(kind・
  state・各 counter・shutdown_abandoned)/`degraded[]`。
  `SinkKind`/`SinkState` は Display 実装を利用し文字列化。
- 既存 `counters` JSON 形状は維持(後方互換)。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/syn-obs-01-host-logging-source-catalog.md`

## Dependencies

- `docs/tasks/syn-obs-01-task1-deps-logging-module.md`

## Affected Files

- `crates/synvelis-app/src/app.rs`

## Done When

- [ ] `syn.app.info` が `logging.state`/`sinks`/`degraded` を返し、
  未接続時は `off` になる。
- [ ] `sources()` の各 `MetricSource` が descriptor/scope/reader を
  canonical 型で保持する。
- [ ] Logging OFF でも counters が従来通り返る。

## Verification

- [ ] `mise run check` pass、`cargo test -p synvelis-app` pass
