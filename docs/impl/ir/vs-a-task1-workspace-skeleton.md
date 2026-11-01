---
id: "IMPL-034QeecIYVT0VX2rTFukXg"
type: "impl"
status: "completed"
title: "VS-A task1 workspace skeleton"
created: "2026-11-01"
updated: "2026-11-01"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-project/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-recording/Cargo.toml"
      - type: "file"
        path: "crates/recording-poc/Cargo.toml"
    modified:
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "mise.lock"
      - type: "file"
        path: ".gitignore"
      - type: "file"
        path: "AGENTS.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vs-a-task1-workspace-skeleton.md"
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


# VS-A task1 workspace skeleton

## Summary

Created the Synvelis Cargo workspace skeleton: root workspace manifest,
three member crates (`synvelis-project`, `synvelis-recording`,
`recording-poc`), ruviform git dependency, and mise verification tasks.

## Context

Task1 of docs/plans/vs-a-project-recording-foundation.md implementing
docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md
and docs/adr/vs-a-workspace-ruviform-git-poc-stable-id.md (Issue #13).

## Implementation

### Changes

- Root `Cargo.toml`: `resolver = "2"`, `edition = "2024"`,
  `rust-version = "1.89"`, `publish = false`, workspace lints
  (`unsafe_code = "forbid"`, clippy `all`/`pedantic` warnings) matching
  the ruviform conventions.
- `ruviform-media-core` as a default-branch-tracking git dependency;
  resolved revision recorded in `Cargo.lock` (`41772a7a`).
- `mise.toml`: `rust = "1.99.0"` toolchain pin plus `fmt`, `fmt-check`,
  `check`, `lint`, `test`, `build`, `test-msrv` tasks.
- `.gitignore`: `target/` and the PoC output bundle directory.
- `AGENTS.md`: Rust workspace conventions section.

### Decisions During Implementation

- Chose the 3-crate layout per ADR D-1 (project data/persistence, recording
  behavior, headless PoC bin) with dependency direction
  `recording-poc -> synvelis-recording -> synvelis-project`.
- Kept lint severity at `warn` and let `lint`'s `-D warnings` gate
  enforce cleanliness, matching ruviform's workspace lint structure.

## Related Experiments

### Adopted

None — approach was clear from the approved design; no experiment logs.

### Rejected

None.

## Validation

- `mise run check` / `build` / `test` / `fmt-check` / `lint`: pass.
- `mise run test-msrv` (rust 1.89.0): full test suite passes.
- `Cargo.lock` records the resolved ruviform git revision.

## Risks

- None identified for the skeleton itself.

## Follow-ups

- `test-msrv` requires the 1.89 toolchain via mise; first run downloads it.
