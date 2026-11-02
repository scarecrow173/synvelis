---
id: "TASK-034QmkzTccUOQtocumKKXN"
type: "task"
status: "done"
title: "L2 task4 verify publish job"
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
    - "docs/tasks/l2-task3-render-core.md"
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

# L2 task4 verify publish job

## Work

Implement `export.rs` orchestration (preflight → prepare → lowering →
`PreparedRender` + sequential `render()` + `StagedOutput` →
verify-before-publish), `verify.rs` (independent symphonia demux +
`SourceReader` canonical decode; rate/channel/frame checks + optional
PCM compare), and `job.rs` (`ExportJob` progress/cancel, typed
`ExportOutcome`, receipt with capability + verified-output evidence).

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/l2-master-sequential-offline-wav-export.md
- docs/designs/l2-master-sequential-offline-wav-export-design.md

## Dependencies

- docs/tasks/l2-task3-render-core.md

## Affected Files

- `crates/synvelis-render/src/export.rs`
- `crates/synvelis-render/src/verify.rs`
- `crates/synvelis-render/src/job.rs`
- `crates/synvelis-render/src/lib.rs`

## Done When

- [x] Completed outcome only constructed after verify + publish succeed
- [x] Failed/Cancelled/Partial never publish; staged temp removed on all
      failure paths; existing destination preserved unless
      `replace_existing`
- [x] Receipt carries render receipt, snapshot ids, capability evidence,
      and `VerifiedOutput` flags
- [x] Cancel honored in prepare and process phases

## Verification

- [x] `l2_05_*` fault-injection tests — pass
- [x] `cargo test --workspace --locked` — 121 pass / 0 fail
