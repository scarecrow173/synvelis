---
id: "TSPEC-034VWwjeNu5OL7N0dTcTSF"
type: "test-spec"
status: "approved"
title: "SF-03 slice9 — host capability generation live firing test-spec"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/sf-03-slice9-capability-firing.md"
    - "docs/designs/sf-03-slice9-capability-firing-design.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice9-capability-firing.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-slice9-capability-firing-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-03-slice9-capability-firing.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice9 — host capability generation live firing test-spec

## Purpose

`PluginCapabilityChanged` と reconcile
`RoutingReconcileMismatch::CapabilityGeneration` が host 側
登録 drift で live 発火することを headless に固定する。
slice2 で定義された fault が defensive-only であった残件を、
`ProcessorHost::capability_generation` の新設により実測へ
格上げする証拠。`ActivationNotObserved`/`RetireOverflow`/
`DeviceEpoch`/外部 gate は発火経路が無いため本 spec の対象外
（IR/matrix で NotRun 記録）。

## Feature: host capability generation の fold

### Rule TS-9-01: generation は登録軸全てを反映し決定的

`crates/synvelis-player/tests/sf03_l9.rs::
sf03_l9_01_host_generation_tracks_registrations`

- `PluginHostProcessors::capability_generation` は mutation
  無しで連続 2 回同一値を返す。
- `register_params`（param catalog 登録）で値が変化し、
  さらに `register_boundary`（boundary declaration 登録）で
  再び変化する。
- 共有 `PluginHost` handle 経由の `register_factory`
  （factory drift — `Scan` descriptor 軸）で再び変化し、
  その後は再び安定する。

### Rule TS-9-02: publish は host drift を
   `PluginCapabilityChanged` で拒否する

`sf03_l9_02_publish_rejects_factory_drift`

- gen1 を prepare→publish→run で `Active` 化
  （`last_good_generation == Some(1)`）。
- gen2 を prepare（host generation を pin）した後、project
  fingerprint が不変のまま共有 handle で factory を追加登録。
- `publish` は `RoutingFault::PluginCapabilityChanged` を返し、
  receipt は `Blocked { fault, last_good_generation: Some(1) }`、
  `rt_status().active_generation == 1`（last-good が live）。
- drift 後の新規 prepare→publish は成功し新 generation が
  active になる（fault は drift であり host の汚染ではない）。

### Rule TS-9-03: reconcile は host drift を
   `CapabilityGeneration` として報告する

`sf03_l9_03_reconcile_reports_host_capability_drift`

- publish/activate 後の clean reconcile は
  `mismatches` 空・`fault` None。
- host drift 後の reconcile は
  `mismatches == [CapabilityGeneration]`、
  `fault == Some(PluginCapabilityChanged)`、
  `plan_key.project_revision` は publish 時 revision。
- receipt は `Blocked { PluginCapabilityChanged, last_good: 1 }`、
  `active_generation == 1` のまま run が音声を生成する
  （drift は報告されるが適用されない）。

### Rule TS-9-04: host drift は NoRebuild equivalence を敗退させる

`sf03_l9_04_host_drift_defeats_equivalence`

- publish 済み plan がある状態で semantic 同一 revision
  （`SetProjectName` のみ、fingerprint 不変）を prepare:
  - host drift 有り → `RoutingPrepare::Built`（`Equivalent`
    ではない）。publish 成功・`Active` 化。
  - host drift 無し（対照 engine）→ `RoutingPrepare::
    Equivalent` — 既存 NoRebuild 経路は維持される。

### Rule TS-9-05: host 差し替えは capability drift

`sf03_l9_05_host_swap_is_capability_drift`

- plugin host で prepare した後 `set_processor_host` で
  `StaticProcessorHost`（default generation `0`）に差し替え、
  publish は `PluginCapabilityChanged` を返す。

## Feature: 非回帰

### Rule TS-9-06: 既存 fixture 非破壊

- `sf03_l2.rs`・`sf03_l3.rs` を含む `mise run test` 全件
  PASS。`StaticProcessorHost` の default `0` により既存
  fixture の capability 経路は不変。

## Guarantees

- host 登録状態の drift は publish/reconcile/equivalence の
  全てで `PluginCapabilityChanged` 系として表面化し、
  `SpeculativeMismatch` に shadow されない。
- connect による instance 消費は capability drift と見なさない
  （generation は connect 前に capture され、登録状態のみを
  畳み込む）。
- `RoutingBuildKey` の構造・値・`semantic_eq` は不変 —
  workspace/render 側の key 構築・比較を壊さない。

## Non-goals

- `ActivationNotObserved`・`RetireOverflow`・reconcile
  `DeviceEpoch`・`EventStale` の live firing — 発火経路が
  存在しないか headless 決定的再現不可。NotRun。
- 実 VST3・実機・GUI・RT 実測の外部 gate。
- `EnginePort`/session 経路の変更、SF-ACT-07 policy。

## Risk

- host generation が drift を見逃す（hash 衝突・`Scan` が
  覆わない登録軸）と `PluginCapabilityChanged` が再び
  発火しない。対策: epoch + descriptor の 2 軸 fold、
  fixture が全登録軸を個別に mutate して変化を assert。
