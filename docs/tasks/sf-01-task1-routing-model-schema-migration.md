---
id: "TASK-034SF01Pr2uVw7xY4zAbCdFgH"
type: "task"
status: "done"
title: "SF-01: RoutingModel 型・oracle・Schema v6 Migration・ops・検証"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/104"
  changes:
    added:
      - "crates/synvelis-project/src/routing.rs"
      - "crates/synvelis-project/src/migration.rs"
      - "crates/synvelis-project/tests/routing_sf01.rs"
    modified:
      - "crates/synvelis-project/src/id.rs"
      - "crates/synvelis-project/src/lib.rs"
      - "crates/synvelis-project/src/project.rs"
      - "crates/synvelis-project/src/ops.rs"
      - "crates/synvelis-project/src/store.rs"
      - "crates/synvelis-project/src/controller.rs"
      - "crates/synvelis-gateway/src/prepare.rs"
      - "crates/synvelis-project/tests/md_a.rs"
      - "crates/synvelis-project/tests/midi_source.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-01-semantic-routing-model.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/designs/sf-01-semantic-routing-model-design.md"
    - "docs/adr/sf-01-routing-model-schema-v6-migration.md"
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
    - "docs/test-specs/sf-01-e2e-semantic-routing-model.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-01 実装タスク

## T1 — Stable ID + RoutingModel 型(`id.rs` / 新規 `routing.rs`)

- `define_id!` で `BusId`/`StripId`/`ProcessorId`/`ChainId`/
  `SemanticPortId`/`RoutingConnectionId`/`SendId`。
- `RoutingModel` 全 entity/binding 型・serde・`Default` bootstrap
  (well-known master)、`WELL_KNOWN_BASE`。
- `lib.rs` exports。

## T2 — oracle + `Project` 組込(`routing.rs` / `project.rs`)

- `RoutingModel::validate(&Project)`: Map key↔id、Master 厳密 1 件、
  Bus/Track↔Strip、Strip↔Chain/Port、Chain↔Processor、Connection
  方向/domain/owner/kind/dup、Send 1:1、gain_target 存在、DAG。
- `Project.routing`(serde default)、`validate_references` 末尾に
  oracle、`max_issued_id_raw` fold 追加(reserved 除外)、
  `check_id_watermark` に `< WELL_KNOWN_BASE` 条件。

## T3 — Schema v6 + Migration(`store.rs` / `migration.rs` / `controller.rs`)

- `SCHEMA_VERSION = 6`、`migration.rs` ordered step registry・
  `migrate_v5_routing`(read-only 投影・決定 ID 割当・watermark 前進・
  crafted v5+routing 拒否)。
- `controller.load`: version dispatch・migration で
  `live_revision = saved_rev + 1`。

## T4 — `ProjectOp` 拡張(`ops.rs`)

- 12 routing variants + `RemoveTrack` cascade 拡張 +
  `RemoveParameterTarget` の send gain_target 保護。

## T5 — Acceptance test + 検証(`tests/routing_sf01.rs` 等)

- test-spec の Feature/Rule を headless test 化。
- `md_a.rs`/`midi_source.rs` の schema version 断言を 6 へ。
- `mise run fmt-check`/`check`/`lint`/`test`/`build`。
- `git fetch origin && git merge origin/main` → `Cargo.lock` 再生成 →
  再検証 → Conventional Commit(日本語)→ impl record。
