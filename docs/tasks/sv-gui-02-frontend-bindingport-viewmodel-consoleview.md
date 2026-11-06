---
id: "TASK-034RADc5Q2mNYPcXkM8fR"
type: "task"
status: "blocked"
title: "SV-GUI-02 frontend BindingPort/ViewModel/ConsoleView"
created: "2026-11-04"
updated: "2026-11-06"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/111"
    - "https://github.com/scarecrow173/ruviform/issues/376"
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

## 2026-11-06 是正結果と受入ブロッカー

上の再開節は開始時点の記録である。製品未変更の RED で追加8件の assertion 失敗と既存9件の成功を確認し、
ConsoleView の同期 cleanup／await 後 disposed gate と ViewModel の attach token／receipt lifetime を修復した。
focused 17件、全 UI 45件、check-ui、build-ui、Rust workspace 全 suite、対象 hooks は成功した。
実 Chromium DOM＋fake IPC でも unmount と旧応答／replacement の二シナリオを確認した。
これは実 Rust IPC／native WebView／実 OS 描画の受入ではない。

独立レビューの10候補を別 Validator と lead で照合した。一時観測 probe 6件には現状欠陥を assert するものがあり、
canonical suite の成功数には合算しない。実 model の receipt callback は attach/reattach の両方で機能した。

**状態は `blocked` のままとする。**
現在 app の lifetime 修復に起因する P0/P1 は確認していないが、
attach 後作成の strip が epoch=0 のまま dispatch する共有 SDK の既存不整合が frontend 正式受入を阻む。
producer の所有側で [Ruviform #376](https://github.com/scarecrow173/ruviform/issues/376) として追跡し、
Synvelis 回避策や backend stale guard の緩和では代替しない。
RF 修復の consumer 取り込みと mounted control-op の positive control を経るまで本 Task と #112 を完了扱いにしない。
今回差分は Draft PR で保存し、merge-ready / 統合済みとは扱わない。

詳細な是正・全検証・残る API/coverage 境界と IR 再開同期漏れは
[既存 Implementation Record](../impl/ir/sv-gui-02-frontend-view-models-ruviform-custom-elements-typed-ipc.md) の追補を参照。
共有 SDK 修復・実 OS／Docking 受入・CI notification gap はこの小さな frontend 修復の成功で消したことにしない。

## Verification

以下は今回実行した修復スライスの検証であり、上記の正式受入ブロッカーを解除するものではない。

- [x] `mise exec -- npm --prefix ui test -- test/console.test.tsx test/binding.test.ts` exits 0（17件成功）。
- [x] `mise run check-ui` exits 0（vendor / TypeScript）。
- [x] `mise run test-ui` exits 0（全45件成功）。
- [x] `mise run build-ui` exits 0。
- [x] `mise run test` exits 0（Rust workspace 全 suite）。
