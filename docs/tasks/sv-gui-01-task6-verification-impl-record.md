---
id: "TASK-WhMYtZA3TKzN6d0ELzgJ3D"
type: "task"
status: "done"
title: "SV-GUI-01 task6 最終検証・merge・impl record"
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
    - "docs/tasks/sv-gui-01-task5-webview-bin-gui-launch.md"
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

- `git fetch origin` → `git merge origin/main` → 生成物 (`Cargo.lock`・
  `ui/package-lock.json`・`ui/vendor`・`frontend/dist`・`mise.lock`) 再生成。
- `mise run fmt`/`fmt-check`/`check`/`lint`/`test`/`check-ui`/`test-ui`/
  `build-ui`/`check-app-webview`/`lint-app-webview`/`test-app-webview` +
  `node tools/check-headless-deps.mjs` を全実行。
- `docs/impl/ir/sv-gui-01-desktop-app-bootstrap.md` に実施検証・実測・未検証
  範囲・残課題 (SV-GUI-02 接続点・RF-GT-03・multi-window) を記録。
- 各 task の status を `done` へ更新、doc index 再生成。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sv-gui-01-desktop-app-bootstrap.md
- docs/test-specs/sv-gui-01-e2e-desktop-bootstrap.md

## Dependencies

- docs/tasks/sv-gui-01-task5-webview-bin-gui-launch.md

## Verification

- 上記全検証の実施済み結果を impl record に記録
