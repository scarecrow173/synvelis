---
id: "IMPL-034Qeee1v46cNscWJeF2Fc"
type: "impl"
status: "completed"
title: "VS-A task3 controller ops store"
created: "2026-11-01"
updated: "2026-11-01"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/controller.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/controller_store.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vs-a-task3-controller-ops-store.md"
  implemented-by: []
  depends-on: []
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
metadata:
  experiments:
    adopted: []
    rejected: []
---


# VS-A task3 controller ops store

## Summary

Implemented the single-writer `ProjectController` (ExpectedRevision atomic
commands, undo/redo, Live/Saved revision separation), the generic
`Operation`/`Command<O>` seam, `ProjectOp` project operations, and the
`DirectoryBundleStore` persistence port with canonical JSON snapshots and
last-write HEAD.

## Context

Task3 of docs/plans/vs-a-project-recording-foundation.md; controller and
store contract per the approved design doc sections 5-7.

## Implementation

### Changes

- `ops.rs`: `trait Operation { validate; apply }`, `Command<O>` with
  `ExpectedRevision`, `ProjectOp` (project meta, tempo/time-signature
  events, PPQN, track/lane/clip CRUD, source state, asset/clock-epoch
  registration).
- `controller.rs`: apply = expected-revision check -> full validation ->
  clone-mutate-once -> swap live -> push prior snapshot to undo -> clear
  redo -> Live revision +1. Undo/redo restore snapshots but always mint a
  new Live revision and never touch Saved state. `mark_saved` records
  HEAD name + revision.
- `store.rs`: `ProjectStore` port (`save_snapshot`/`head`/
  `read_snapshot`/`save_project`/`load_project`);
  `DirectoryBundleStore` writes `snapshots/r<N>.json` via
  tmp+fsync+rename, then publishes `HEAD` last by the same atomic rename;
  load reads HEAD, parses, rejects unsupported schema version, validates
  references before replacing anything.
- Snapshot envelope: `schema_version = 1`, `deny_unknown_fields` on all
  persisted structs, canonical `serde_json` output.

### Decisions During Implementation

- `ProjectController::apply` is generic over `O: Operation` (controller
  itself non-generic) so `synvelis-recording` can supply its own op type
  without a circular crate dependency — the design review's P1 resolution.
- Undo strategy is snapshot-based (per ADR): simple, correct for PoC
  scale; inverse ops deferred.

## Related Experiments

### Adopted

None — approach was clear from the approved design; no experiment logs.

### Rejected

None.

## Validation

- `controller_store.rs`: stale-revision rejection with zero mutation,
  invalid op atomicity, undo/redo revision minting + Saved isolation,
  save/reload round-trip, malformed JSON / unknown field / unsupported
  schema / broken reference rejection, failed save keeps prior HEAD,
  failed load keeps Live state.

## Risks

- Undo depth is unbounded (PoC scope; policy cap deferred).
- Directory Bundle is a PoC format, not the final persistence format.

## Follow-ups

- Journal/recovery and VCS integration are explicitly out of scope (VS-B+).
