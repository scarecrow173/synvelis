---
id: "TASK-034Uj7V9p3khaqx8qH7C2Z"
type: "task"
status: "done"
title: "SF-03 slice5 T1 — workspace EnginePort・ProjectSession 接続"
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

# SF-03 slice5 T1 — workspace EnginePort・ProjectSession 接続

## Work

`crates/synvelis-workspace/src/session.rs` に `EnginePort` trait を
追加し、`ProjectSession` の engine 保持を `Box<dyn EnginePort>` に
変更する。`EngineStub` は trait の既定実装として残す。

- `EnginePort`: `on_project_revision(&mut self, project: &Project,
  revision: ProjectRevision)`・`graph_builds(&self) -> u64`・
  `last_built_revision(&self) -> ProjectRevision`。
- `ProjectSession.engine` を `Box<dyn EnginePort>` 化、`new` は
  `EngineStub` を既定装着、`set_engine`/`with_engine` で注入可能に。
- `apply_internal`/`undo`/`redo` は commit 成功後に
  `self.controller.live()` と revision を渡して通知。
- `lib.rs` で `EnginePort` を re-export。
- 既存 test（`ed_a.rs`/`sv48_viz.rs`/`ws_a.rs`）の
  `.graph_builds` field アクセスを `.graph_builds()` へ追従。

## Classification

- 種別: refactor + API 追加（session engine 境界の trait 化）
- リスク: 低（既定実装が EngineStub で挙動不変）

## Upstream Documents

- `docs/plans/sf-03-slice5-session-engine-connect.md`
- `docs/designs/sf-03-slice5-session-engine-connect-design.md`
- `docs/specs/sf-03-slice5-session-engine-connect.md`

## Dependencies

- なし（player/app より先行可能）

## Affected Files

- `crates/synvelis-workspace/src/session.rs`
- `crates/synvelis-workspace/src/lib.rs`
- `crates/synvelis-workspace/tests/ed_a.rs`
- `crates/synvelis-workspace/tests/sv48_viz.rs`
- `crates/synvelis-workspace/tests/ws_a.rs`

## Done When

- `EnginePort` trait が object-safe で `EngineStub`/将来の
  `SessionRoutingEngine` が実装できる。
- `ProjectSession` が `set_engine` で engine を差し替えられ、
  commit 成功 revision のみ `(&Project, revision)` で通知する。
- 既存 workspace test が `.graph_builds()` で green。

## Verification

- `mise run check`（workspace）/ `mise run test`（workspace 関連）。
