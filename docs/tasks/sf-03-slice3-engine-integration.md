---
id: "TASK-034UfGn2GRUGJOpEhbrW9K"
type: "task"
status: "done"
title: "SF-03 slice3 T2 — RoutingEngine 接続統合・世代連動 teardown"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
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
    - "docs/plans/sf-03-slice3-plugin-host-teardown.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-03-slice3-plugin-host-teardown.md"
    - "docs/tasks/sf-03-slice3-host-boundary.md"
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
    - "docs/test-specs/sf-03-slice3-plugin-host-connect.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice3 T2 — RoutingEngine 接続統合・世代連動 teardown

## Work

`routing_engine.rs`（+ `routing_lower.rs` の `engine_format` 公開）を
改修し、processor provisioning を `ProcessorHost` boundary 経由に置き換える。

- `RoutingDeps` から `processors` field を撤去。
- `RoutingEngine` に `host: Box<dyn ProcessorHost>` を追加し、
  `RoutingEngine::new` / `RoutingDeps` 経路で注入可能にする
  （`set_processor_host` で差し替えも許容、設計どおり）。
- `stage_build`: `node_roles` の `ChainProcessor` ごとに
  `ProcessorConnectRequest` を構成して `host.connect` を呼び、
  `(NodeId -> Box<dyn GraphProcessor>)` の pending map と
  `Vec<ProcessorLease>` を `StagedBuild` に保持。connect 失敗時は
  それまでの lease を drop（= teardown）して typed fault を返す。
- offline capability gate: `deps.processors` 参照をやめ、接続済み
  processor の `offline_support()` を `staged.pending` 経由で検査。
  `OfflineRenderSupport::Unknown` は fail-closed で
  `OfflineCapabilityMismatch`。
- `PreparedRouting` に `connections: BTreeMap<u64, Vec<ProcessorLease>>`
  を追加し、`publish` 成功時に generation key で格納。
- `publish`: 既存 pending generation が `last_good` でなければその
  lease を即時解放してから新世代を格納（supersede teardown）。
- `observe_activation`: RT 観測された generation の lease のみ残し、
  他世代を解放（activation teardown）。`seek` の drain 経路でも
  `observe_activation` を呼び、stopped 中の観測欠落を防ぐ。
- `RoutingEngine::drop`/`PreparedRouting` drop で残 lease が
  自然に teardown されることを確認。
- 既存 test（`sf03_l1.rs`/`sf03_l2.rs`/その他 `RoutingDeps` 構築箇所）を
  `StaticProcessorHost` へ機械的に移行。

## Acceptance

- `cargo check --workspace --all-targets` が通る。
- `RoutingDeps.processors` が存在せず、全構築箇所が host boundary
  経由。
- prepare→publish→activation の既存 slice1/2 test が全て green
  （last-good・fault 分類の回帰なし）。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sf-03-slice3-plugin-host-teardown.md
- docs/designs/sf-03-slice3-plugin-host-connect-design.md
- docs/tasks/sf-03-slice3-host-boundary.md

## Dependencies

- docs/tasks/sf-03-slice3-host-boundary.md

## Affected Files

- crates/synvelis-player/src/routing_engine.rs
- crates/synvelis-player/src/routing_lower.rs（`engine_format` 公開）
- crates/synvelis-player/tests/sf03_l1.rs
- crates/synvelis-player/tests/sf03_l2.rs

## Done When

- [x] `RoutingDeps.processors` が撤去され `ProcessorHost` boundary
  経由のみで chain processor が接続される。
- [x] `stage_build`/`publish`/`observe_activation`/`seek` が
  generation 連動 lease teardown を実装する。
- [x] `RoutingEngine::collect` が retire drain を公開する。
- [x] 既存 slice1/2 test が `StaticProcessorHost` へ移行し green。

## Verification

- [x] `cargo test -p synvelis-player` 全 target green
  （sf03_l1/l2/l3・pc_b 等、回帰なし）。
- [x] `sf03_l3_09` で pending 置換・active 置換・engine drop の
  teardown タイミングを実測。
