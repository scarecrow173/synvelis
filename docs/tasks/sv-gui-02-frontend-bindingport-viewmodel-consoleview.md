---
id: "TASK-034RADc5Q2mNYPcXkM8fR"
type: "task"
status: "in-progress"
title: "SV-GUI-02 frontend BindingPort/ViewModel/ConsoleView"
created: "2026-11-04"
updated: "2026-11-06"
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

## 2026-11-06 lifecycle 是正による再開

既存の owner-tied cleanup / mount–unmount–remount 契約を復元する
是正作業として本タスクを再開する。変更前の `ConsoleView` は
`await vm.attach()` 後に cleanup を登録し、`ConsoleViewModel.detach()`
は receipt listener の解放と pending attach の無効化を行っていない。
製品再現テストと修復コードは、再開時点では未実施である。

### 是正範囲

- Solid owner が有効な同期区間で cleanup を登録し、unmount 時に
  DOM binding と viz source を解放する。
- detach/close 後に遅延 attach 応答が local binding を復活させない。
- receipt 購読を attach/detach の lifetime に合わせ、再 attach 時に
  一つだけ復元する。
- unmount を backend close や Project/Audio/Render Job の取消で代用しない。
- normal mount/unmount、attach/viz bind 待機中の unmount、旧応答が
  新 mount より遅れる場合、detach/re-attach の回帰テストを先に作成する。

ここでは既存契約の是正だけを扱う。SV-GUI-03 (#112) 全体、実 OS、
新しい CI trigger、権限、永続化の設計変更を完了したとは扱わない。
