---
id: "TASK-QL2QzcJ2SdDIQZN7NDxvaK"
type: "task"
status: "done"
title: "SF-03 slice4 T3 — 受入テスト sf03_l4・既存 test 追従"
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
    - "docs/tasks/sf-03-slice4-processor-control-vending.md"
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

# SF-03 slice4 T3 — 受入テスト sf03_l4・既存 test 追従

## Work

`crates/synvelis-player/tests/sf03_l4.rs` を新規作成し、SF-ACT-05〜09
系の headless 受入項目を実測する。既存 `sf03_l1.rs`/`sf03_l2.rs`/
`sf03_l3.rs` は T1/T2 の署名変更へ追従済みであることを確認する。

- equivalence: Project 名変更・ViewState 相当の変更・undo/redo で
  semantic 同値の新 revision に対し `RoutingPrepare::Equivalent`
  が返り、generation 消費・host connect・compile が発生しないこと、
  receipt が live plan の実状態を維持すること。
- 非 equivalence: 同一 revision 再 prepare・engine/source-format/
  capability generation 差・build mode/epoch/instance 差は通常
  build されること。
- reconcile: revision のみ drift は fault なし、semantic drift は
  `StaleProjectSnapshot`。
- control 払い出し: `take_processor_control` の one-shot・
  unknown/stale `None`・`StaticProcessorHost` 接続 `None`・実
  `PluginHost` 接続で param event が正しい世代の plugin instance
  にのみ配送され、世代交代後の旧 control が新世代へ届かない
  （wrong-plugin 0）。
- SF-ACT-05 headless 固定: rapid R1/R2/R3 の receipt 系列・
  `Superseded` publish 拒否・`PublishQueueFull`・stale counter→
  `EventStale` reconcile・`DeviceEpochChanged`・fresh retry。
- SF-ACT-07/08: `skipped_fault` の両 mode fail-closed を再確認し、
  評価記録を test-spec/PR 本文へ記録。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sf-03-slice4-act-acceptance.md
- docs/designs/sf-03-slice4-act-acceptance-design.md
- docs/test-specs/sf-03-slice4-act-acceptance.md

## Dependencies

- docs/tasks/sf-03-slice4-processor-control-vending.md

## Affected Files

- crates/synvelis-player/tests/sf03_l4.rs（新規）
- crates/synvelis-player/tests/sf03_l1.rs
- crates/synvelis-player/tests/sf03_l2.rs
- crates/synvelis-player/tests/sf03_l3.rs
- docs/test-specs/sf-03-slice4-act-acceptance.md

## Done When

- [x] `sf03_l4.rs` が上記受入項目を網羅し green。
- [x] 既存 `sf03_l1`/`sf03_l2`/`sf03_l3` が新署名で green
      （回帰なし）。
- [x] test-spec が実測結果と対応づけられる。

## Verification

- [x] `cargo test -p synvelis-player --test sf03_l4` green。
- [x] `mise run test` が workspace 全体で green。
