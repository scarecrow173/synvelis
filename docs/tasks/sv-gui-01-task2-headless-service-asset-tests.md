---
id: "TASK-jLkK7lARWtOgmAI02PaXQ5"
type: "task"
status: "todo"
title: "SV-GUI-01 task2 headless service op・asset check テスト"
created: "2026-11-04"
updated: "2026-11-04"
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
    - "docs/plans/sv-gui-01-desktop-app-bootstrap.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/sv-gui-01-task1-app-crate-composition-root.md"
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
    - "docs/test-specs/sv-gui-01-e2e-desktop-bootstrap.md"
  references: []
  defers: []
  deferred-by: []
---

## Work

- `crates/synvelis-app/tests/service_ops.rs`:
  - `UiRuntime` + `AppService` の dispatch 契約 (raw intent → Event)。
  - project new → save → open → info round-trip (tempdir
    `DirectoryBundleStore`)。transport set/state。workspace mount 冪等・
    再 open で unmount。capabilities 反映。未 open で `unavailable`。
- `crates/synvelis-app/tests/asset_check.rs`:
  - `check_bundle` 受理・`ManifestMissing`/`ManifestInvalid`/
    `SchemaMismatch`/`AssetMissing`/`AssetCorrupt`/`index.html missing`。
- `tools/check-headless-deps.mjs`: `synvelis-app` 以外の crate が
  `ruviform-ui-webview`/`rust-embed`/`wry`/`tao` を参照しないことの検査
  (Cargo.toml scan) + `ui/` 以外に `solid-js` package.json 依存が無いこと。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sv-gui-01-desktop-app-bootstrap.md
- docs/test-specs/sv-gui-01-e2e-desktop-bootstrap.md

## Dependencies

- docs/tasks/sv-gui-01-task1-app-crate-composition-root.md

## Verification

- `mise run test`・`node tools/check-headless-deps.mjs`
