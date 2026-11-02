---
id: "TASK-034QmkyMFxwLahrrG4nFN1"
type: "task"
status: "done"
title: "L2 task2 snapshot preflight"
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
    - "docs/tasks/l2-task1-workspace-crate-deps.md"
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

# L2 task2 snapshot preflight

## Work

Implement `RenderSnapshot::freeze` (project clone + live/saved/tempo
revision + source digest capture) and the two-stage preflight
(`preflight_static` for request-geometry/destination/capability/magnitude
checks, `preflight_canonical` for source codec/rate policy) plus the
typed error taxonomy.

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/l2-master-sequential-offline-wav-export.md
- docs/designs/l2-master-sequential-offline-wav-export-design.md

## Dependencies

- docs/tasks/l2-task1-workspace-crate-deps.md

## Affected Files

- `crates/synvelis-render/src/snapshot.rs`
- `crates/synvelis-render/src/spec.rs`
- `crates/synvelis-render/src/preflight.rs`
- `crates/synvelis-render/src/error.rs`

## Done When

- [x] Snapshot freezes revision ids + per-source digests without touching
      controller/undo state
- [x] Preflight rejects invalid range, unknown tail, destination
      conflict, insufficient capability, bad channel geometry,
      zero/oversized magnitudes, unregistered/unverified sources,
      non-PCM codec and rate mismatch with typed errors
- [x] Non-muted clips that fail to lower surface as
      `UnrenderableClip`; all-muted as `NothingToRender`

## Verification

- [x] `cargo test -p synvelis-render --test l2_e2e` refusal tests — pass
- [x] `cargo clippy --workspace --all-targets -- -D warnings` — clean
