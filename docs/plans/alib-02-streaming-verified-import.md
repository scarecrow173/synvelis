---
id: "PLAN-alib02-streaming-import"
type: "plan"
status: "completed"
title: "ALIB-02 実装計画 — Streaming Verified Import・SHA-256 識別子（synvelis-library slice）"
created: "2026-11-09"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-library/src/import.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-library/src/project.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-02-streaming-verified-import.md"
  implemented-by:
    - "docs/tasks/alib-02-streaming-verified-import.md"
    - "docs/impl/ir/alib-02-streaming-verified-import.md"
  depends-on:
    - "docs/designs/alib-02-streaming-verified-import-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/alib-02-streaming-verified-import.md"
---

# ALIB-02 Plan

## Task decomposition

- `docs/tasks/alib-02-task1-import-pipeline.md` — `src/import.rs`:
  `ImportStore`/`ImportManifest`/`ImportIntent`, staged streaming +
  SHA-256 measurement, staged readback verify, `BlobPublisher` port +
  `StagedBlobPublisher` (ruviform `StagedFile`/`SealedFile`),
  publish-effect trichotomy reconciliation, `CommitVerifiedImport`
  operation, `rebase_import`/`inspect_imports`/`gc_orphans`.
- `docs/tasks/alib-02-task2-namespace-and-tests.md` — `project.rs`
  `object_rel` additive `sha256:<hex>` → `sha256/<hex>` mapping;
  `tests/import.rs` acceptance coverage.

## File map

- `crates/synvelis-library/src/import.rs` (new) — pipeline + recovery.
- `crates/synvelis-library/src/lib.rs` — module + exports.
- `crates/synvelis-library/src/project.rs` — additive namespaced
  `object_rel` (legacy single-component path unchanged).
- `crates/synvelis-library/tests/import.rs` (new) — 15 tests.

## Verification matrix

| Check | Command |
| --- | --- |
| fmt | `mise run fmt-check` |
| typecheck | `mise run check` |
| lint | `mise run lint` |
| tests | `mise run test` |
| build | `mise run build` |
| MSRV | `mise run test-msrv` |
| RT | `mise run test-rt` |
