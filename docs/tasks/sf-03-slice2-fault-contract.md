---
id: "TASK-106SF03Tz2aBc3dEf4gHi5jKl6"
type: "task"
title: "SF-03 slice2 T1 — RoutingFault 18分類・engine 改修・reconcile 実装"
status: "in-progress"
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
  implements:
    - "docs/plans/sf-03-slice2-fault-reconcile.md"
  implemented-by: []
  depends-on: []
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
    - "docs/test-specs/sf-03-slice2-fault-reconcile.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice2 T1 — fault 契約・engine 実装

- `routing_build.rs`: `RoutingFault` を Issue #106 §5 の 18 区分に
  再構成（設計 §1 の対応表どおり）。`RoutingReconcileMismatch`/
  `RoutingReconcileReport` を追加。
- `routing_lower.rs`: `SemanticLowerError::PortUnresolved` 追加、
  `fault()` を新 taxonomy に写像。
- `routing_engine.rs`:
  - `checked_key`: instance/epoch → `DeviceEpochChanged`、
    budget → `CompileResourceExceeded`。
  - `prepare`: offline capability gate、skipped 昇格（spec R4）、
    cycle/overflow/invalid の分類、`InvalidRouting` 統一。
  - `publish`: fingerprint → `SpeculativeMismatch`、capability →
    `PluginCapabilityChanged`、内部整合 → `BadSourceMap`、
    世代順序 → `Superseded`。
  - `reconcile`: 全 drift 側面の検出と fault 記録（設計 §5）。
  - Ledger: `published_request_id`/`observed_frames`/counter
    baseline 追加、`frames_observed` 修正。
- 受入: `cargo check`/`lint` で variant 網羅、設計の mint 経路表と
  実装の一致。
