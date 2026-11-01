---
id: "IMPL-034Qeefum0sCLzMQV4B0T5"
type: "impl"
status: "completed"
title: "VS-A task5 poc and acceptance tests"
created: "2026-11-01"
updated: "2026-11-01"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/recording-poc/src/main.rs"
      - type: "file"
        path: "crates/recording-poc/tests/poc_determinism.rs"
    modified:
      []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vs-a-task5-poc-and-acceptance-tests.md"
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


# VS-A task5 poc and acceptance tests

## Summary

Implemented the deterministic headless `recording-poc` binary: builds a
fixed project, registers a recording session graph, saves, reloads, and
verifies canonical-state equality — plus the determinism test proving
identical stdout across two runs.

## Context

Task5 of docs/plans/vs-a-project-recording-foundation.md; PoC acceptance
criteria per the test spec (TSPEC-034QdZPqGguxUOzVihyxjE, AC11).

## Implementation

### Changes

- `main.rs`: deterministic fixed data (fixed ID generator seeds, fixed
  names/digests), drives `ProjectController` + `SynvelisOp` +
  `DirectoryBundleStore` through create -> register -> save -> reload ->
  compare, and prints a stable, ordering-independent summary (BTreeMap-
  ordered output).
- `tests/poc_determinism.rs`: runs the binary twice and asserts byte-
  identical stdout and a successful exit.

### Decisions During Implementation

- The PoC writes its bundle under `crates/recording-poc/poc-out/`
  (gitignored) so runs are hermetic and repeatable.

## Related Experiments

### Adopted

None — approach was clear from the approved design; no experiment logs.

### Rejected

None.

## Validation

- `mise run test`: `poc_determinism` passes (two-run identical stdout);
  the PoC itself exits 0 with the expected canonical summary.
- Manual double-run equivalence covered by the test.

## Risks

- None; output is fully deterministic by construction (no timestamps,
  randomness, or filesystem ordering in the summary).

## Follow-ups

- The PoC is the E2E-R starting point; E2E-L/R/X lanes extend it in later
  phases.
