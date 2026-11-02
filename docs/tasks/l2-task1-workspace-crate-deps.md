---
id: "TASK-034QmkyhbqBh4mLIVhr6wy"
type: "task"
status: "done"
title: "L2 task1 workspace crate deps"
created: "2026-11-02"
updated: "2026-11-02"
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
    - "docs/plans/l2-master-sequential-offline-wav-export.md"
  implemented-by: []
  depends-on:
    - "docs/plans/l2-master-sequential-offline-wav-export.md"
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
    - "docs/test-specs/l2-master-sequential-offline-wav-export.md"
  references: []
  defers: []
  deferred-by: []
---

# L2 task1 workspace crate deps

## Work

Bump ruviform git dependencies to the revision containing rf#198
(`322a661ac0177fc98f799b07ae591c98eb2ec845`), add the `synvelis-render`
workspace crate with `ruviform-io-wav`/`ruviform-exec` dependencies, and
verify the workspace still compiles on the new pin.

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/l2-master-sequential-offline-wav-export.md

## Dependencies

- none (root task)

## Affected Files

- `Cargo.toml` (workspace members + dep revisions)
- `Cargo.lock` (regenerated pin)
- `crates/synvelis-render/Cargo.toml` (new crate manifest)

## Done When

- [x] ruviform deps pinned to `322a661` including rf#198 render APIs
- [x] `synvelis-render` crate skeleton exists as workspace member
- [x] `cargo check --workspace --all-targets` clean on the new pin

## Verification

- [x] `cargo check --workspace --all-targets` — exit 0
- [x] `cargo test --workspace --locked` — 121 pass / 0 fail
