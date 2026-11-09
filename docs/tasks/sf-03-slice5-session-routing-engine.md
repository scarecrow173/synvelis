---
id: "TASK-034Uj826EB6a7TEsfLwDmZ"
type: "task"
status: "done"
title: "SF-03 slice5 T2 — synvelis-app SessionRoutingEngine・canonical probe"
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
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
    - "docs/tasks/sf-03-slice5-session-engine-port.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice5 T2 — synvelis-app SessionRoutingEngine・canonical probe

## Work

player に canonical format probe を追加し、app に
`SessionRoutingEngine`（`EnginePort` 実装の cloneable handle）を
実装する。

- `crates/synvelis-player/src/resolver.rs`: verified-open を
  `prepare` と共有する `pub fn canonical_format(project,
  source_id, media_root) -> Result<AudioFormat, ResolveError>` を
  追加。`lib.rs` で re-export。
- `crates/synvelis-app/src/engine.rs`（新規）:
  - `SessionEngineConfig`（media_root・`EngineConfig`・
    `PrefetchConfig`・`BuildBudget`・`BuildMode`・
    `RuntimeInstanceId`・`RuntimeClockEpochRef`・
    `engine_config_generation`）。
  - `SessionEngineError`（clip 解決失敗の証拠型）。
  - `SessionRoutingEngine`（`Rc<RefCell<Inner>>` handle、Clone）。
    `EnginePort` impl で revision 通知 → `plan_playbacks` →
    `resolver::prepare` → `RoutingDeps`/`RoutingCompileRequest` →
    `RoutingEngine::prepare`→`publish`。
  - 公開面: `set_processor_host`・`run`・`receipt`・`reconcile`・
    `rt_status`・`source_map`・`take_processor_control`・
    `last_prepare_error`・`graph_builds`/`last_built_revision`。
- `crates/synvelis-app/src/app.rs`: `attach_routing_engine` で
  現在 session に engine を接続。
- `crates/synvelis-app/src/lib.rs`: `pub mod engine` + re-export。
- `crates/synvelis-app/Cargo.toml`: `synvelis-player`・
  `ruviform-media-reader`・`ruviform-audio-core` を追加
  （`Cargo.lock` は依存追加に伴い更新）。

## Classification

- 種別: feature（session→routing 接続 adapter）
- リスク: 中（`deps.clips` 順序・generation・失敗境界）

## Upstream Documents

- `docs/plans/sf-03-slice5-session-engine-connect.md`
- `docs/designs/sf-03-slice5-session-engine-connect-design.md`
- `docs/specs/sf-03-slice5-session-engine-connect.md`
- `docs/tasks/sf-03-slice5-session-engine-port.md`

## Dependencies

- task 1（`EnginePort` trait が必要）

## Affected Files

- `crates/synvelis-player/src/resolver.rs`
- `crates/synvelis-player/src/lib.rs`
- `crates/synvelis-app/src/engine.rs`（新規）
- `crates/synvelis-app/src/app.rs`
- `crates/synvelis-app/src/lib.rs`
- `crates/synvelis-app/Cargo.toml`
- `Cargo.lock`（依存解決の反映）

## Done When

- `SessionRoutingEngine` が `EnginePort` を実装し、revision 通知で
  `RoutingEngine::prepare`→`publish` を駆動する。
- `set_processor_host` で `ProcessorHost` を注入できる。
- `canonical_format` probe が `prepare` と同一 verified-open を
  共有する。
- app default feature が headless deps を維持する。

## Verification

- `mise run check` / `mise run check-headless-deps`。
