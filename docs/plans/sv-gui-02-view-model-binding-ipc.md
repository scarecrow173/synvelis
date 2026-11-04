---
id: "PLAN-SVGUI02"
type: "plan"
status: "approved"
title: "SV-GUI-02 View Model・Binding IPC 実装計画"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/111"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gui-02-frontend-view-models-ruviform-custom-elements-typed-ipc-receipt.md"
  implemented-by:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
    - "docs/tasks/sv-gui-02-frontend-bindingport-viewmodel-consoleview.md"
    - "docs/tasks/sv-gui-02-headless-vitest-guitest.md"
  depends-on:
    - "docs/designs/sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md"
    - "docs/adr/sv-gui-02-syn-binding-ipc-surface-notify-outbox-bounded-wire-dto.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-02.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-02 View Model・Binding IPC 実装計画

## 実装スライス

1. **T1 (task: ruviform-rev-rust-syn-ui-op)**: ruviform 依存を RF-GT
   同梱 rev へ更新 (`cargo update -p ruviform-*` → `Cargo.lock` 再生成 →
   `mise run sync-ui-deps`)。`synvelis-app` に `ui_dto.rs`/`ui_outbox.rs`/
   `ui_ops.rs` を追加し、`syn.view.*`/`syn.control.*`/`syn.viz.*` op を
   `AppService` へ配線。capabilities 更新。Rust headless op テスト。
2. **T2 (task: frontend-bindingport-viewmodel-consoleview)**: `ui/src/
   binding/`(port/router/wire)、`ui/src/viewmodel/`、
   `adapters/ruviform.ts` 拡張、`views/console.tsx`、app 統合。
   vitest + happy-dom で DOM/binding/lifecycle テスト。
3. **T3 (task: headless-vitest-guitest)**: `ui/guitest` fixture の
   vendored replay lane(補助証拠)、host.rs outbox drain 配線、
   `check-app-webview`/`test-app-webview` 経路確認、impl record。

依存: T1 → T2(frontend は op 面と vendored binding に依存) → T3。

## リスク

- ruviform rev 追従で vendored API 差異 → T1 で `check-ruviform-rev` +
  vendor diff を確認してから T2。
- generation lockstep → design のリスク節どおり attach で lockstep。

## 検証

`mise run check/lint/test/fmt-check` + `check-ui/test-ui/build-ui` +
`check-app-webview/lint-app-webview/test-app-webview` + `check-headless-deps`
+ `check-ruviform-rev`。guitest lane は補助証拠。実 GUI/macOS/Linux は
未検証として明示。
