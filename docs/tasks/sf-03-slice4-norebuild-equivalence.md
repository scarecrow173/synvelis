---
id: "TASK-034UgPv6iDxfZLWWpr3Dnz"
type: "task"
status: "done"
title: "SF-03 slice4 T1 — NoRebuild Equivalence・reconcile revision-drift 抑制"
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

# SF-03 slice4 T1 — NoRebuild Equivalence・reconcile revision-drift 抑制

## Work

`crates/synvelis-project/src/routing_build.rs` に `ExecutionEquivalence`
を追加し、`RoutingEngine::prepare` を `RoutingPrepare` enum 化して
equivalence 経路を実装する。

- `ExecutionEquivalence`: `configured_project_revision`・
  `active_built_from_revision`・`matching_semantic_build_key`・
  `plan_generation` を保持する契約型。`synvelis-project` の
  `lib.rs` で re-export。
- `RoutingPrepare` enum（player 側、`routing_engine.rs`）:
  `Built(PreparedRouting)` / `Equivalent(ExecutionEquivalence)`。
  `into_built()`/`expect_built()` helper を提供。
- `prepare` 戻り値を `Result<RoutingPrepare, RoutingFault>` に変更。
  `checked_key` 通過後・generation 予約/lowering 前に
  `source_map.build_key.semantic_eq(&new_key)` ∧
  `project_revision` 差ありで equivalence を返す。同一 revision の
  再 prepare は常に build。
- `reconcile` で live state から key を再構成し
  `published_key.semantic_eq(&live_key)` が真なら
  `ProjectRevision` mismatch を push しない（semantic drift 時は
  従来どおり報告）。
- 既存 test の `prepare` 呼出しを新戻り値型へ追従。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sf-03-slice4-act-acceptance.md
- docs/designs/sf-03-slice4-act-acceptance-design.md
- docs/specs/sf-03-slice4-act-acceptance-norebuild.md

## Dependencies

- docs/tasks/sf-03-slice4-processor-control-vending.md（本 task が先行）
- docs/tasks/sf-03-slice4-acceptance-tests.md

## Affected Files

- crates/synvelis-project/src/routing_build.rs
- crates/synvelis-project/src/lib.rs
- crates/synvelis-player/src/routing_engine.rs
- crates/synvelis-player/src/lib.rs
- crates/synvelis-player/tests/sf03_l1.rs
- crates/synvelis-player/tests/sf03_l2.rs

## Done When

- [x] `ExecutionEquivalence` が project 側契約型として存在し
      re-export される。
- [x] `prepare` が `RoutingPrepare::{Built, Equivalent}` を返し、
      semantic 同値の新 revision で generation 消費・host connect・
      compile が発生しない。
- [x] 同一 revision・generation/epoch/mode 差の request は通常 build。
- [x] `reconcile` が revision のみ drift を fault とせず semantic
      drift は従来どおり報告する。

## Verification

- [x] `cargo check -p synvelis-project -p synvelis-player --all-targets`
      通過。
- [x] `cargo test -p synvelis-player --test sf03_l1 --test sf03_l2`
      green（既存契約の回帰なし）。
