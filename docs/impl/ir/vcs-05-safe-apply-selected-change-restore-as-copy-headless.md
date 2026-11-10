---
id: "IMPL-034V4SMgc7OlchXo2EUCfn"
type: "impl"
status: "completed"
title: "VCS-05 safe apply selected change / restore as copy (headless)"
created: "2026-11-10"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/96"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/apply/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/src/apply/entity.rs"
      - type: "file"
        path: "crates/synvelis-project/src/apply/plan.rs"
      - type: "file"
        path: "crates/synvelis-project/src/apply/journal.rs"
      - type: "file"
        path: "crates/synvelis-project/src/restore/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs05_apply.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs05_restore.rs"
      - type: "file"
        path: "docs/specs/vcs-05-apply-restore-headless.md"
      - type: "file"
        path: "docs/designs/vcs-05-apply-restore-headless-design.md"
      - type: "file"
        path: "docs/plans/vcs-05-apply-restore-headless.md"
      - type: "file"
        path: "docs/tasks/vcs-05-apply-restore-headless.md"
      - type: "file"
        path: "docs/test-specs/vcs-05-apply-restore-fixtures.md"
      - type: "file"
        path: "docs/impl/ir/vcs-05-safe-apply-selected-change-restore-as-copy-headless.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/vcs03_git_provider.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-05-apply-restore-headless.md"
    - "docs/designs/vcs-05-apply-restore-headless-design.md"
    - "docs/plans/vcs-05-apply-restore-headless.md"
    - "docs/tasks/vcs-05-apply-restore-headless.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-05-apply-restore-headless-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-03-git-read-provider.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/vcs-03-git-read-provider.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-05-apply-restore-headless.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-05-apply-restore-fixtures.md"
  verified-by: []
  references: []
  defers:
    - "VCS-06 Windows real FS/provider/plugin E2E・fault acceptance（Issue #97）"
    - "cross-consumer validation（Issue #294）"
    - "VCS-04 History/Visual Diff UI（Issue #95）"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-05 safe apply selected change / restore as copy (headless)

## Summary

`apply_selected` applies an explicitly approved subset of a canonical
VCS-02 `ChangeSet` to the live project through `ProjectController`'s
single-writer path (exactly one undo step), guarded by approval,
`ProjectRevision` CAS, and a recomputed target fingerprint; a
content-keyed journal makes retries, ACK-gap, and crash recovery
idempotent. `restore_as_copy` materializes a fixed `ImmutableSnapshot`
into a sibling staging directory, verifies every staged byte, and
publishes via same-directory rename — the source project, Saved HEAD,
Git index, and working tree are never touched.

## Context

Implements `docs/tasks/vcs-05-apply-restore-headless.md` per
`docs/specs/vcs-05-apply-restore-headless.md` and
`docs/designs/vcs-05-apply-restore-headless-design.md`, verified by
`docs/test-specs/vcs-05-apply-restore-fixtures.md` (synvelis#96,
headless slice; real-OS/provider/plugin E2E deferred to synvelis#97,
cross-consumer to synvelis#294).

## Implementation

### Changes

- `src/apply/` — intent/receipt/error types, `ChangeSetSource` /
  `PluginAvailability` ports, entity extraction + canonical
  fingerprint + whole-entity transplant, typed write plan +
  asset/plugin/port preflight, `ApplySelectedOp` single `Operation`,
  in-memory and directory-backed `ApplyJournal`.
- `src/restore/` — `RestoreRequest`/`RestorePorts` (blob, plugin-state,
  storage probes), staged manifest (`Started → Materialized →
  Validated → Published`), staged publish + readback validation,
  resume/replay for staging-manifest and post-rename crash windows.
- `src/lib.rs` — module wiring and public re-exports.
- `tests/vcs05_apply.rs` (13 tests), `tests/vcs05_restore.rs`
  (7 tests).

### Decisions During Implementation

- Absent entities fingerprint as JSON `null` so `Added` changes are
  fingerprintable on both sides; presence rules stay in the planner.
- Fingerprint object set derives from the declared selection plus the
  target object — computed before planning so ACK-gap reconciliation
  works even when live already equals the after-state.
- Journal `Pending` is armed only after inspecting the prior record;
  `Applied` replays the stored receipt, `Pending` triggers
  live-state reconciliation (`Replayed` vs fresh attempt).
- `Removed` changes are rejected (cascade removal deferred); every
  other unsupported kind/change/field fails closed.
- Restore lineage stays receipt-only — no schema v11 (decision
  recorded in the spec).
- Effects reports group booleans into `untouched`/`source`/`git`/`copy`
  sub-structs to satisfy `struct_excessive_bools` without suppression.

## Related Experiments

### Adopted

<!-- none -->

### Rejected

<!-- none -->

## Validation

- `mise run fmt-check` — pass (Windows, rust 1.99.0 pinned toolchain).
- `mise run check` — pass, workspace `--all-targets`.
- `mise run lint` — pass, `cargo clippy --workspace --all-targets -- -D warnings`.
- `mise run test` — pass, `cargo test --workspace --locked` (incl. 13
  apply + 7 restore tests).
- `mise run test-rt` — pass.
- `mise run test-msrv` — pass under `rust@1.89.0`.

## Risks

- Real filesystem durability (dirsync/fsync semantics), real
  providers/plugins, and GUI wiring are unverified headless — deferred
  to synvelis#97 / synvelis#294.
- `quick_bundle_check` on the post-rename reconcile path verifies the
  envelope but not every blob digest; full readback ran pre-publish.

## Follow-ups

- synvelis#97: Windows real FS/provider/plugin E2E and fault acceptance.
- synvelis#294: cross-consumer validation.
- synvelis#95 (VCS-04): history/visual-diff UI consumes these receipts.
