---
id: "TASK-alib02-t1"
type: "task"
status: "done"
title: "ALIB-02 Task 1+2 — import pipeline module・namespace・tests"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/alib-02-streaming-verified-import.md"
  depends-on:
    - "docs/designs/alib-02-streaming-verified-import-design.md"
  verifies:
    - "docs/test-specs/alib-02-streaming-verified-import.md"
---

# ALIB-02 Tasks

## Task 1 — `src/import.rs`

- [x] `ImportIntent`/`ImportTarget`/`ImportStore`/`ImportManifest`
      (serde, format_version, atomic manifest writes via
      `StagedFile`+`PublishMode::Replace`).
- [x] Stage machine `Started→Staged→Verified→Published→Committed`
      persisted per transition.
- [x] `stage_source`: bounded 64 KiB chunks, SHA-256 + byte-length
      measurement, `check_interrupts` per chunk, byte budget,
      pin digest/size expectation checks, provider completion-slot
      cross-check.
- [x] `verify_staged`: readback re-measure vs manifest.
- [x] `BlobPublisher` port + `StagedBlobPublisher`
      (`PublishMode::CreateNew`, `FileAndDirectorySync` with honest
      `Unsupported` fallback, publish-temp readback verify,
      integrity gate before seal).
- [x] Publish trichotomy: `Published` post-failure verify,
      `Indeterminate` stat-reconcile + single retry +
      `PublishIndeterminate`, `NotPublished` typed failure;
      `DirectorySync::{Supported,Unsupported,NotRequested,Failed,Unknown}`
      recorded from observed evidence only.
- [x] `CommitVerifiedImport` `Operation`: atomic asset+source+
      applied-import(+clip) at manifest `expected_project_revision`;
      reseed-past-reserved-ids; lane/track/clip-contract validation.
- [x] `rebase_import` (only stale-resolution), `inspect_imports`,
      `gc_orphans` (project+manifest reachability, foreign/legacy
      untouched).

## Task 2 — namespace + tests

- [x] `LocalBlobDir::object_rel` additive `sha256:<64hex>` →
      `sha256/<hex>` `RelativePath`; legacy path unchanged.
- [x] `tests/import.rs`: 15 tests covering bounded streaming (1 MiB
      @ 7-byte chunks), opaque bytes, replay idempotency, conflict,
      digest/size/budget integrity, stale+rebase, missing/wrong-track
      lane rejection, publish failure resume, indeterminate reconcile,
      dirsync-failure recording, orphan GC protect/delete, namespace
      separation, live-vs-saved distinction.

## Notes

- `Source` commits at `Imported` — byte integrity verified; media
  decode verification remains a consumer `SetSourceState` stage.
- `CommittedLive` ≠ `SavedRevision`/`HEAD`; save stays caller-owned.
- Scripted publishers prove fault *classification* only — never real
  durability evidence (spec constraint).
