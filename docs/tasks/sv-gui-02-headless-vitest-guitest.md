---
id: "TASK-034RADeL6sQ8vFzJ3xTmN"
type: "task"
status: "completed"
title: "SV-GUI-02 検証レーン統合 (headless/vitest/guitest)"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/111"
  changes:
    added: []
    modified:
      - "crates/synvelis-app/src/host.rs"
      - "ui/package.json"
      - "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv-gui-02-view-model-binding-ipc.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/sv-gui-02-frontend-bindingport-viewmodel-consoleview.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sv-gui-02-view-model-binding-ipc.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-02 検証レーン統合 (headless/vitest/guitest)

## Work

1. webview `host.rs`: tick で `ui_pump` + outbox drain → `host.eval`。
2. `ui/guitest` fixture を vendored 共有物として vitest replay lane に
   組込(補助証拠、独立 ContractModel oracle 判定)。
3. `check-app-webview`/`test-app-webview` 経路確認、impl record 作成、
   最終検証(fetch+merge origin/main → 再生成 → 全 mise 検証)。

## Classification

- `normal-plan-task`
