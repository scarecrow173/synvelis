---
id: "TASK-pFWmIPcnM0SNWSnPnDf7OQ"
type: "task"
status: "todo"
title: "SV-GUI-01 task5 webview bin・実 Windows GUI 起動検証"
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
    - "docs/tasks/sv-gui-01-task2-headless-service-asset-tests.md"
    - "docs/tasks/sv-gui-01-task3-frontend-pipeline-vendor.md"
    - "docs/tasks/sv-gui-01-task4-frontend-headless-tests.md"
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

- `src/bin/synvelis.rs`: `--project <path>`/`--data-dir <path>` parse →
  `detect_backend` → `AppAssets` + `check_bundle` → `UiRuntime` + `AppService`
  → `host::run` (HostConfig title "Synvelis"、`Ready` で initial notify、
  `syn.app.quit` 応答 → `host.request_close`)。
- `mise run check-app-webview`/`lint-app-webview`/`test-app-webview` で
  feature lane を検証。
- `mise run build-ui` の dist を伴う dev build で実 Windows 起動 → Window
  Shell 表示・File menu (New/Open/Save/Quit)・Capabilities 表示・Quit 終了を
  手動観測。production (`--release`) build でも同確認。
- backend 不在時・asset 不整合時の説明+非ゼロ終了を確認 (可能な範囲)。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sv-gui-01-desktop-app-bootstrap.md
- docs/test-specs/sv-gui-01-e2e-desktop-bootstrap.md

## Dependencies

- docs/tasks/sv-gui-01-task2-headless-service-asset-tests.md
- docs/tasks/sv-gui-01-task3-frontend-pipeline-vendor.md
- docs/tasks/sv-gui-01-task4-frontend-headless-tests.md

## Verification

- `mise run check-app-webview`/`lint-app-webview`/`test-app-webview`
- 実 Windows で `mise run run-app` (dev) + `cargo build --release --features
  webview` 起動・終了 (実測記録)
