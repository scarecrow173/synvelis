---
id: "TASK-xum593l0Fok2XTCWotqIwp"
type: "task"
status: "todo"
title: "SV-GUI-01 task4 frontend headless test (ipc/store/components)"
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
    - "docs/tasks/sv-gui-01-task3-frontend-pipeline-vendor.md"
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

- `ui/vitest.config.ts` + `ui/test/env.ts`: vitest + happy-dom bootstrap
  (`node --test` は `solid-js/web` が node condition で SSR build に
  解決され client render 不能のため vitest を採用)。
- `ui/test/ipc.test.ts`: `call` の intent 形状・result resolve/reject・
  `onEvent` の result 非配送。
- `ui/test/store.test.ts`: `sys.capabilities`+`syn.*` 応答→store 投影・
  notify・error 記録。
- `ui/test/app.test.tsx`: `<App/>` mount・`rvf-transport` typed property・
  CustomEvent→`syn.transport.set` intent・error banner・cleanup。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sv-gui-01-desktop-app-bootstrap.md
- docs/test-specs/sv-gui-01-e2e-desktop-bootstrap.md

## Dependencies

- docs/tasks/sv-gui-01-task3-frontend-pipeline-vendor.md

## Verification

- `mise run test-ui` (vitest + happy-dom)
