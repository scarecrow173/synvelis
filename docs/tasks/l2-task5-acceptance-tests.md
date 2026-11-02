---
id: "TASK-034QmkApgInfzfYY2D8TrH"
type: "task"
status: "done"
title: "L2 task5 acceptance tests verification"
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
    - "docs/tasks/l2-task4-verify-publish-job.md"
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

# L2 task5 acceptance tests verification

## Work

Write the L2 acceptance suite `crates/synvelis-render/tests/l2_e2e.rs`
covering L2-01..L2-08: deterministic WAV fixtures, independent reopen
PCM compare, clip boundary/gap/mute/anchor cases, block-size invariance
of stateful inserts, PDC + tail policy refusals, fault-injection
partial/cancel, snapshot immutability, and receipt capability evidence —
plus the implementation record with toolchain/hash/exit-code evidence.

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/l2-master-sequential-offline-wav-export.md
- docs/test-specs/l2-master-sequential-offline-wav-export.md

## Dependencies

- docs/tasks/l2-task4-verify-publish-job.md

## Affected Files

- `crates/synvelis-render/tests/l2_e2e.rs` (15 tests)
- `docs/impl/ir/l2-master-sequential-offline-wav-export.md`

## Done When

- [x] L2-01..L2-08 each covered by at least one measured test or an
      explicit record in the implementation record
- [x] All lanes green: fmt-check, check, clippy -D warnings,
      `cargo test --workspace --locked` (121/0), build
- [x] Fixture/output sha256 + toolchain + ruviform pin recorded in IR

## Verification

- [x] `cargo test -p synvelis-render --test l2_e2e` — 15 pass / 0 fail
- [x] `cargo fmt --all -- --check` / `cargo check` / `cargo clippy` / `cargo build` — exit 0
