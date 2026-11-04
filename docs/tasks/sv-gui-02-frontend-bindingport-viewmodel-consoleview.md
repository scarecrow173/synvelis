---
id: "TASK-034RADc5Q2mNYPcXkM8fR"
type: "task"
status: "todo"
title: "SV-GUI-02 frontend BindingPort/ViewModel/ConsoleView"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/111"
  changes:
    added:
      - "ui/src/binding/"
      - "ui/src/viewmodel/"
      - "ui/src/views/console.tsx"
      - "ui/test/"
    modified:
      - "ui/src/ipc.ts"
      - "ui/src/adapters/ruviform.ts"
      - "ui/src/app.tsx"
      - "ui/src/store.ts"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv-gui-02-view-model-binding-ipc.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
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

# SV-GUI-02 frontend BindingPort/ViewModel/ConsoleView

## Work

1. `ui/src/binding/`(port/router/wire)— `__rvf` 上の BindingPort +
   notify router + wire 型。
2. `ui/src/viewmodel/`(console-view-model/viz-source)— ViewLink +
   CommandBinding + viz source 束ねる owner(DOM 非依存)。
3. `adapters/ruviform.ts` 拡張(全要素登録・object prop 経路・
   owner-tied cleanup)、`views/console.tsx`(knob/fader/transport/
   seekbar/waveform/meter)、app 統合。
4. vitest + happy-dom: DOM/binding/lifecycle/failure 注入テスト。

## Classification

- `normal-plan-task`
