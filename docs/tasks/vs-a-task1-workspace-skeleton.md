---
id: "TASK-034Qe0bZIka1POLkub4sqO"
type: "task"
status: "done"
title: "VS-A task1 workspace skeleton"
created: "2026-11-01"
updated: "2026-11-01"
owners: []
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
    - "docs/plans/vs-a-project-recording-foundation.md"
  implemented-by: []
  depends-on:
    - "docs/plans/vs-a-project-recording-foundation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VS-A task1 workspace skeleton

## Work

Cargo workspace 骨格: ルート Cargo.toml（resolver=2/edition2024/rust-version=1.89/workspace.lints/publish=false/members 3クレート）、crates/*/Cargo.toml+src stub、mise.toml に rust=1.99.0 + fmt/fmt-check/check/lint/test/build/test-msrv タスク、ruviform git 依存追加、cargo fetch で Cargo.lock 生成。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-a-project-recording-foundation.md
- docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md

## Dependencies

- 直列依存（plan Task Breakdown 参照）

## Affected Files

- Cargo.toml
- mise.toml
- crates/*/Cargo.toml
- crates/*/src/lib.rs
- Cargo.lock

## Done When

- [ ] mise run check が stub で通り Cargo.lock に ruviform-media-core の git resolved revision が記録される

## Verification

- [ ] 対応 test-spec 項目が緑（mise run test）
