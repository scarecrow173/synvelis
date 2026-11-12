---
id: "TASK-034VWsCOcQUHrW7PISo66X"
type: "task"
status: "done"
title: "SF-03 slice9 T1 — host capability generation live firing"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/plans/sf-03-slice9-capability-firing-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-03-slice9-capability-firing-plan.md"
    - "docs/designs/sf-03-slice9-capability-firing-design.md"
    - "docs/specs/sf-03-slice9-capability-firing.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-03-slice9-capability-firing-plan.md"
    - "docs/designs/sf-03-slice9-capability-firing-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice9-capability-firing.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice9 T1 — host capability generation live firing

## Work

host 側 capability generation を `ProcessorHost` に公開し、
`RoutingEngine` の prepare/publish/reconcile/equivalence に
fold して `PluginCapabilityChanged` と reconcile
`CapabilityGeneration` を headless で live 発火させる。

- `crates/synvelis-player/src/host.rs`:
  `ProcessorHost::capability_generation`（default `0`）、
  `PluginHostProcessors.registration_epoch` +
  `register_boundary`/`register_params` の epoch bump、
  `Scan` descriptor + epoch の FNV-1a 畳み込み。
- `crates/synvelis-player/src/routing_engine.rs`:
  `PreparedRouting.host_capability_generation`、prepare capture
  （connect 前）、publish の project 側 sum + host generation
  2半分再検証、publish 成功時の ledger pin、equivalence の
  `host_current` 条件、reconcile の `CapabilityGeneration`
  比較、module/fn doc 更新。
- `crates/synvelis-project/src/routing_build.rs`・
  `routing_tx.rs`: `capability_generation` が project 側半分
  であることの key 契約文書化（構造不変）。
- `crates/synvelis-player/tests/sf03_l9.rs`（新規・5 件）:
  - `sf03_l9_01_host_generation_tracks_registrations`:
    generation が boundary/catalog/factory 全登録軸で決定的に
    変化し mutation 無しで安定。
  - `sf03_l9_02_publish_rejects_factory_drift`:
    prepare→publish 間の factory drift で
    `PluginCapabilityChanged`、last-good 保持、drift 後の
    再 build 成功。
  - `sf03_l9_03_reconcile_reports_host_capability_drift`:
    publish/activate 後の drift で `CapabilityGeneration`
    mismatch + `PluginCapabilityChanged` fault、plan は live。
  - `sf03_l9_04_host_drift_defeats_equivalence`:
    drift 下の semantic 同一 revision が `Built`、drift 無し
    では `Equivalent`。
  - `sf03_l9_05_host_swap_is_capability_drift`:
    prepare→publish 間の host 差し替えで
    `PluginCapabilityChanged`。
- docs チェーン（spec/design/plan/task/test-spec/impl）。

## Classification

- 種別: feature（runtime fault の live firing + host 境界の
  capability 証拠追加）
- リスク: 中（publish/reconcile/equivalence の gate 条件追加。
  fail-closed 方向のみ、既存経路は default `0` で不変）

## Upstream Documents

- `docs/plans/sf-03-slice9-capability-firing-plan.md`
- `docs/designs/sf-03-slice9-capability-firing-design.md`
- `docs/specs/sf-03-slice9-capability-firing.md`
- `docs/test-specs/sf-03-slice9-capability-firing.md`

## Dependencies

- slice2 の `RoutingFault`/`RoutingReconcileMismatch` taxonomy、
  slice3 の `PluginHostProcessors`/`ProcessorHost` 境界、
  slice8 の fixture パターンが main に存在すること。
- `ruviform-plugin-host` の `HostCommand::Scan` /
  `PluginHost::register_factory` API（git 依存・変更なし）。

## Affected Files

- `crates/synvelis-player/src/host.rs`
- `crates/synvelis-player/src/routing_engine.rs`
- `crates/synvelis-player/tests/sf03_l9.rs`（新規）
- `crates/synvelis-project/src/routing_build.rs`
- `crates/synvelis-project/src/routing_tx.rs`
- `docs/specs/sf-03-slice9-capability-firing.md`（新規）
- `docs/designs/sf-03-slice9-capability-firing-design.md`（新規）
- `docs/plans/sf-03-slice9-capability-firing-plan.md`（新規）
- `docs/tasks/sf-03-slice9-capability-firing-task.md`（新規）
- `docs/test-specs/sf-03-slice9-capability-firing.md`（新規）
- `docs/impl/ir/sf-03-slice9-capability-firing.md`（新規）

## Done When

- `sf03_l9.rs` 5 fixture が green。
- `PluginCapabilityChanged` publish live 発火・reconcile
  `CapabilityGeneration` mismatch が headless で実測される。
- `ActivationNotObserved`/`RetireOverflow`/`DeviceEpoch`/
  外部 gate の可否判定が IR/matrix に記録される。
- canonical verification lanes が全て exit 0。
- `docs/*/README.md` index・占有外ファイルに変更がない。

## Verification

- `cargo test -p synvelis-player --test sf03_l9`（補助証拠）。
- `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`check-app-native`/`lint-app-native`/`build`
  （canonical）。
