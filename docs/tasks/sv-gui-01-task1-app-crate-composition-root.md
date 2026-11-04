---
id: "TASK-MlLPS12iG90WZhRs4S9pub"
type: "task"
status: "todo"
title: "SV-GUI-01 task1 synvelis-app crate skeleton・Composition Root・AppService"
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
    - "docs/plans/sv-gui-01-desktop-app-bootstrap.md"
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
---

## Work

- ルート `Cargo.toml`: member に `crates/synvelis-app` 追加、workspace deps に
  `ruviform-ui-webview` (git)・`rust-embed = "8"` 追加。`Cargo.lock` 再生成。
- `crates/synvelis-app/Cargo.toml`: `synvelis-project`/`synvelis-workspace`/
  `synvelis-recording`/`ruviform-ui`/`serde`/`serde_json`/`sha2` +
  optional `ruviform-ui-webview`/`rust-embed`、feature `webview`、
  `[[bin]] name="synvelis" required-features=["webview"]`。
- `src/error.rs`: `AppError`・`AssetError` (manifest missing/invalid・schema
  mismatch・asset missing/corrupt)。
- `src/app.rs`: `SynvelisApp` (唯一の `ProjectSession` + store + workspace +
  `AudioRuntimeSlot`/`RecorderSlot`/`ObservabilityOwner`)。
- `src/host.rs`: `ShellWindowHost: WindowHandle` (単一 window 写像、
  multi-window op は `BackendUnavailable`)。
- `src/assets.rs`: `AppAssets` (rust-embed `frontend/dist`, webview feature)。
- `src/revision.rs`: `AssetManifest` 型・`check_bundle`。
- `src/service.rs`: `AppService` — `syn.app.info`/`syn.app.quit`/
  `syn.project.{new,open,save,info}`/`syn.transport.{set,state}`/
  `syn.workspace.{mount,state}`。
- `src/lib.rs`: module/公開面・doc。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sv-gui-01-desktop-app-bootstrap.md
- docs/designs/sv-gui-01-desktop-app-bootstrap-design.md

## Dependencies

- なし (最初の task)

## Verification

- `mise run check` (feature 無し headless で型チェックが通る)
