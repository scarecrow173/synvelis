---
id: "TASK-034RADc5Q2mNYPcXkM8fR"
type: "task"
status: "done"
title: "SV-GUI-02 frontend BindingPort/ViewModel/ConsoleView"
created: "2026-11-04"
updated: "2026-11-07"
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
  references:
    - "https://github.com/scarecrow173/ruviform/pull/383"
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

## 2026-11-06 RF #376 修復 SDK 取込（consumer 側契約検証）

前節の正式受入ブロッカーだった共有 SDK 不整合は、producer 側で
[Ruviform #379](https://github.com/scarecrow173/ruviform/pull/379)（fix `4f258c2`）として
RF main `513b29b13810900ede0c67b295d702cfc75b7fc4` に統合済み。
本タスクではその consumer 取込と恒久回帰化を行った。

- `Cargo.lock` の ruviform 解決を既定ブランチ追従で `8fb665f` → `513b29b` へ更新。
  `ui/vendor/ruviform` は既存 `tools/sync-ruviform-ui.mjs` で同 revision に再生成し、
  `check-ruviform-rev` が lock / vendor REV / ビルド manifest 三者一致を確認。
- consumer 境界の恒久テストを追加：`binding.test.ts` に attach 済み link 上の
  後発 binding が live epoch で dispatch・receipt 相関・reattach epoch 更新・
  stale receipt 排除・detach/close 後 wire dispatch なしを検証する suite、
  `console.test.tsx` に実 mount した `ConsoleView` の `rvf-gesture` →
  `syn.control.dispatch`（generation=1）→ receipt `applied` →
  `syn.control.projection` で pending 解除・値反映までの positive control。
- 旧 vendor SDK (`8fb665f`) では同テストが epoch=0 dispatch で 3 件 RED
  （意味のある assertion 失敗）、修復 SDK で 23/23 focused・全 UI 51 件 GREEN。
- 取込に伴う upstream API 追従：`ruviform-observability` の `SinkConfig` に
  `span_events`、`SinkDestination::RollingFile` に `filename_suffix` が追加されたため、
  Synvelis 側 initializer とテスト helper に `SpanEvents::NONE` / `None`
  （変更前と同一動作）を補完。`synvelis` bin の `clippy::doc_markdown` /
  `too_many_lines`（webview feature でのみ発現する main 由来の既存 lint）は
  `install_logging()` 抽出で解消。SDK の回避策や backend stale guard 緩和は行っていない。

**本タスクの status を `done` とする。** Task の完了条件（RF 修復の consumer
取込 + mounted control-op の positive control）は充足した。ただしこれは
headless + vitest/happy-dom + fake IPC での契約検証であり、実 OS WebView
GUI・実 Rust IPC・native GPU 描画の正式受入は test-spec「未検証として明示
するもの」のとおり別 lane の NotRun のまま残る。#112（SV-GUI-03 全体）の
完了を意味しない。

## 2026-11-07 attach epoch 採用（RF #383 / P1 修復）

前節 `done` 化後の独立レビュー（Reviewer B、`TEMP/sv171-review-b-2361694.md`
IC-B1）で P1 が確認された：`ConsoleViewModel.attach` は attach 応答の
`res.generation` を getter に閉じ込めたまま `link.attach(windowId)` を呼び、
link は自前の attach 回数しか刻まない。同 viewId の fresh-VM remount や
pending attach 破棄後の再 attach で、frontend の `ctx.generation` が backend
live epoch を下回り、backend stale guard が全 dispatch を receipt なしで
沈黙破棄する（観測: subscription/projection は生きたまま操作だけ効かない
zombie console）。実 headless `UiRuntime` で別 Validator が再現済み。

SDK 側修復は RF [#383](https://github.com/scarecrow173/ruviform/pull/383) として
RF main `49de02cc530d324772d8556225030eb2b9307751` に統合済み
（`ViewLink.attach(windowId, generation?)`：指定 epoch を link・全 binding・
query・subscription context へ採用、省略時は local increment を維持）。
本タスクはその consumer 取込（ユーザー承認済み方針A・設計の Task 2）。

- `Cargo.lock` の ruviform 22 source を `513b29b` → `49de02c` に解決更新
  （既定ブランチ追従 git 依存・恒久 pin なし・vendor 運用変更なし）。
  `mise run sync-ui-deps` で `ui/vendor/ruviform` を同 revision に再生成し、
  `check-ruviform-rev` が lock / vendor REV / manifest 三者一致を確認。
- `console-view-model.ts` は token 検証後に
  `link.attach(this.windowId, res.generation)` を呼ぶ。`attachToken` による
  遅延・逆順応答の失効構造は不変。backend stale guard・receipt・
  revision・Job 取消契約の緩和、SDK で修復済みの P1 を consumer で
  複製する counter、design 非目標（commandId namespace・detach 窓
  receipt・detached binding 契約）の混入は行っていない。
- consumer 境界の恒久回帰を追加：`ui/test/backend-epoch.ts` に backend
  `ControlsUi` の epoch/stale-guard を独立に ledger 化する double を新設し、
  `binding.test.ts` に `authoritative attach epoch` suite を追加。wire
  `ctx.generation` を SDK 送出値ではなく ledger の backend live epoch と
  照合する（fake host が送出値をそのまま受理する blindness を回避）。
  fresh-VM remount（attach reply=2）で pre-attach transport と後発 strip
  の双方の wire generation・receipt 相関・projection 着地、pending attach
  破棄後の再 attach 再同期、逆順旧応答の token 失効、死 epoch を刻んだ
  偽 dispatch が ledger で drop される negative control を検証。
- backend guard 契約の Rust 側原証跡は既存
  `crates/synvelis-app/tests/ui_ops.rs`
  （`unknown_and_detached_dispatch_are_dropped_not_faked`）と
  `crates/synvelis-workspace/tests/sv47_controls.rs`
  （`dock_move_cancels_gesture_and_stale_generation_cannot_land`）で、
  `mise run test` / `test-app-webview` / `test-msrv` で再実測済み。
- 旧 SDK/head（`2361694`、vendor `513b29b`）で新 suite が 3 件 RED
  （wire ctx.generation=1 vs backend live=2 の assertion 失敗）、
  SDK 取込 + consumer 修正で 21/21 focused・全 UI 55 件 GREEN。

引き続き headless + vitest/happy-dom + fake IPC の契約検証であり、
実 OS WebView GUI・実 Rust IPC・native GPU 描画・実 plugin/実 DSP・
実 multi window は別 lane の NotRun のまま。#112 全体完了を意味しない。

## Verification

以下は今回実行した修復スライスの検証であり、上記の正式受入ブロッカーを解除するものではない。

- [x] `mise exec -- npm --prefix ui test -- test/console.test.tsx test/binding.test.ts` exits 0（17件成功）。
- [x] `mise run check-ui` exits 0（vendor / TypeScript）。
- [x] `mise run test-ui` exits 0（全45件成功）。
- [x] `mise run build-ui` exits 0。
- [x] `mise run test` exits 0（Rust workspace 全 suite）。

### 2026-11-06 RF #376 修復 SDK 取込後

- [x] 旧 SDK (`8fb665f`) で新規契約テスト 3 件 RED（後発 binding epoch=0、
      mounted dispatch ctx.generation=0）。
- [x] 修復 SDK (`513b29b`) で focused `binding.test.ts`+`console.test.tsx`
      23/23 成功、`mise run test-ui` 全51件成功。
- [x] `mise run fmt-check` / `check` / `lint` / `test` exits 0。
- [x] `mise run check-ui` / `build-ui` exits 0。
- [x] `mise run check-ruviform-rev` exits 0（`513b29b`、lock/vendor/manifest 一致）。
- [x] `mise run check-headless-deps` / `gw10` exits 0（gw10 は既存 NOT-RUN
      case を除き全 gate PASS）。
- [x] `mise run check-app-webview` / `lint-app-webview` / `test-app-webview` exits 0。

### 2026-11-07 attach epoch 採用（RF #383）

- [x] 旧 SDK (`513b29b`) + head `2361694` で新規 `authoritative attach
      epoch` suite 3 件 RED（remount/pending/逆順の wire ctx.generation=1
      vs backend live=2、意味のある assertion 失敗）。
- [x] `mise exec -- npm --prefix ui test -- test/binding.test.ts` exits 0
      （21/21、修復 SDK + consumer 修正後）。
- [x] `mise run test-ui` exits 0（全 6 ファイル 55 件成功）。
- [x] `mise run check-ui` / `build-ui` exits 0。
- [x] `mise run check-ruviform-rev` exits 0（`49de02c`、22 lockfile
      sources、lock/vendor REV/manifest host 三者一致）。
- [x] `mise run fmt-check` / `check` / `lint` / `test` exits 0。
- [x] `mise run check-headless-deps` / `gw10` exits 0（gw10 は既存
      NOT-RUN/UNSUPPORTED case のみ残存）。
- [x] `mise run check-app-webview` / `lint-app-webview` /
      `test-app-webview` exits 0。
- [x] `mise run test-msrv` exits 0（rust@1.89.0 で workspace 全 suite）。
