---
id: "TASK-LJ3dobeIa1bFvvZxBJivEy"
type: "task"
status: "done"
title: "SF-03 slice4 T2 — 世代連動 processor control 払い出し"
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
    - "docs/plans/sf-03-slice4-act-acceptance.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-03-slice4-act-acceptance.md"
    - "docs/designs/sf-03-slice4-act-acceptance-design.md"
    - "docs/tasks/sf-03-slice4-norebuild-equivalence.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-03-slice4-act-acceptance.md"
    - "docs/designs/sf-03-slice4-act-acceptance-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice4-act-acceptance.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice4 T2 — 世代連動 processor control 払い出し

## Work

chain processor 接続の非 RT 制御半体を世代スコープで engine が保持し、
公開済み世代に限り払い出す。

- `ProcessorConnection` に `control: Option<PluginNodeControl>` を追加。
  `PluginHostProcessors` は `attach` の返す control をここへ移し、
  lease の teardown closure は host lifecycle のみを担う。
  `StaticProcessorHost` は `None`。
- `RetainedConnection { processor, node, lease, control }` を導入し、
  `StagedBuild.leases`/`PreparedRouting.leases`/engine `connections`
  map の値型を更新。既存の世代連動 teardown 規則は不変。
- `RoutingEngine::take_processor_control(generation,
  processor_id) -> Option<ProcessorControl>` を追加。
  `ProcessorControl { node, control }` を `host.rs` に定義。
  one-shot・未知世代/processor・non-plugin 接続は `None`。
- `sf03_l3.rs` の `RecordingHost` 等 `ProcessorConnection` 構築箇所に
  `control: None` を追従。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sf-03-slice4-act-acceptance.md
- docs/designs/sf-03-slice4-act-acceptance-design.md
- docs/specs/sf-03-slice4-act-acceptance-norebuild.md

## Dependencies

- docs/tasks/sf-03-slice4-norebuild-equivalence.md
- docs/tasks/sf-03-slice4-acceptance-tests.md

## Affected Files

- crates/synvelis-player/src/host.rs
- crates/synvelis-player/src/routing_engine.rs
- crates/synvelis-player/src/lib.rs
- crates/synvelis-player/tests/sf03_l3.rs

## Done When

- [x] `ProcessorConnection.control` が存在し `PluginHostProcessors` が
      attach の control を保持、lease teardown は lifecycle のみ。
- [x] `RetainedConnection` が世代 ledger の値型として機能し、
      pending 置換・Active 観測・engine drop で lease+control が
      既存規則どおり解放される。
- [x] `take_processor_control` が公開済み世代の chain processor
      control を one-shot で返し、unknown/stale/non-plugin は `None`。

## Verification

- [x] `cargo check -p synvelis-player --all-targets` 通過。
- [x] `cargo test -p synvelis-player --test sf03_l3` green
      （既存 teardown 契約の回帰なし）。
