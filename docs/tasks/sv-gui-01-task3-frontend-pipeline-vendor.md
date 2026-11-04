---
id: "TASK-5ySbythnz4gMVnQauJQq8a"
type: "task"
status: "todo"
title: "SV-GUI-01 task3 ui/ frontend pipeline・vendored sync・mise tasks"
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
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

## Work

- `tools/sync-ruviform-ui.mjs`: `Cargo.lock` の ruviform git revision を正本に
  `ui/{core,components,binding,viz}` を `ui/vendor/ruviform/` へ同期
  (audio-lens 版の rev 読み取りのみ変更)。
- `ui/package.json` (`solid-js`/`vite`/`vite-plugin-solid`/`typescript`/
  `happy-dom`/`@types/node`)・`package-lock.json` (`npm install` で生成・追跡)。
- `ui/tsconfig.json` (solid-js jsx・strict・`allowImportingTsExtensions`)。
- `ui/vite.config.ts` (`outDir: ../crates/synvelis-app/frontend/dist`・base `./`
  ・`synvelis-assets.json` manifest 生成 plugin)。
- `ui/index.html` (CSP meta・`<div id="app">`)・`ui/src/**`
  (main.tsx・ipc.ts・types.ts・store.ts・App.tsx・components/・adapters/ruviform.ts)。
- `mise.toml`: `[tools] node = "26.12.1"` 追加 + `sync-ui-deps`/`setup-ui`/
  `check-ui`/`test-ui`/`build-ui`/`check-app-webview`/`lint-app-webview`/
  `test-app-webview`/`run-app` タスク。`mise.lock` 再生成。
- `.gitignore`: `ui/node_modules/`・`ui/vendor/`・`crates/synvelis-app/frontend/dist/`。
- `.worktreeinclude`: `ui/vendor/` 追加。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sv-gui-01-desktop-app-bootstrap.md
- docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md

## Dependencies

- docs/tasks/sv-gui-01-task1-app-crate-composition-root.md

## Verification

- `mise run sync-ui-deps` → `mise run setup-ui` → `mise run check-ui` →
  `mise run build-ui` (dist + manifest 生成)
