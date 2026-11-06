---
id: "IMPL-SVGui02FrontendViewModelsRuviformCustomElementsTypedIpc"
type: "impl"
status: "in-progress"
title: "SV-GUI-02 Frontend View Models・Ruviform Custom Elements・Rust IPC/Receipt統合"
created: "2026-11-25"
updated: "2026-11-25"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/111"
    - "https://github.com/scarecrow173/ruviform/issues/376"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/ui_ops.rs"
      - type: "file"
        path: "ui/src/binding/router.ts"
      - type: "file"
        path: "ui/src/binding/port.ts"
      - type: "file"
        path: "ui/src/viewmodel/console-view-model.ts"
      - type: "file"
        path: "ui/src/viewmodel/viz-source.ts"
      - type: "file"
        path: "ui/src/views/console.tsx"
      - type: "file"
        path: "ui/test/binding.test.ts"
      - type: "file"
        path: "ui/test/conformance.test.ts"
      - type: "file"
        path: "docs/specs/sv-gui-02-frontend-view-models-ruviform-custom-elements-typed-ipc-receipt.md"
      - type: "file"
        path: "docs/adr/sv-gui-02-syn-binding-ipc-surface-notify-outbox-bounded-wire-dto.md"
      - type: "file"
        path: "docs/designs/sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md"
      - type: "file"
        path: "docs/plans/sv-gui-02-view-model-binding-ipc.md"
      - type: "file"
        path: "docs/test-specs/sv-gui-02.md"
      - type: "file"
        path: "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
      - type: "file"
        path: "docs/tasks/sv-gui-02-frontend-bindingport-viewmodel-consoleview.md"
      - type: "file"
        path: "docs/tasks/sv-gui-02-headless-vitest-guitest.md"
      - type: "file"
        path: "ui/test/console.test.tsx"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/service_ops.rs"
      - type: "file"
        path: "ui/src/ipc.ts"
      - type: "file"
        path: "ui/src/adapters/ruviform.ts"
      - type: "file"
        path: "ui/src/app.tsx"
      - type: "file"
        path: "ui/tsconfig.json"
      - type: "file"
        path: "tools/sync-ruviform-ui.mjs"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/plans/sv-gui-02-view-model-binding-ipc.md"
    - "docs/designs/sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md"
    - "docs/specs/sv-gui-02-frontend-view-models-ruviform-custom-elements-typed-ipc-receipt.md"
    - "docs/adr/sv-gui-02-syn-binding-ipc-surface-notify-outbox-bounded-wire-dto.md"
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
    - "docs/tasks/sv-gui-02-frontend-bindingport-viewmodel-consoleview.md"
    - "docs/tasks/sv-gui-02-headless-vitest-guitest.md"
  implemented-by: []
  depends-on: []
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
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-GUI-02 Frontend View Models・Ruviform Custom Elements・Rust IPC/Receipt統合

## Summary

全製品 View が共有する薄い View Model / Typed IPC / Binding /
Receipt / Lifecycle 基盤を実装した。Rust 側に `syn.view.*`・
`syn.control.*`・`syn.viz.*` op 面と bounded notify outbox を、
frontend 側に upstream `BindingPort` 実装・notify channel router・
DOM 非依存 `ConsoleViewModel`・IPC-backed viz source・参照
`ConsoleView` を追加し、host tick が outbox を webview へ drain
する。共有 RF-GT fixture を replay + wire conformance の両面で
再利用した（独自 fixture 再実装なし）。

## Context

Issue #111 / SV-GUI-02。前提: SV-GUI-01（#110）の composition
root・`UiRuntime`・asset pipeline と upstream `ui/binding`・
`ui/components`・`ui/viz`・`ui/guitest`（Ruviform #310–#312、
rev `8fb665f`）が揃っている。本 Issue が所有するのは共有基盤
のみ — SF-05 Signal Flow・PC-C5 Control Relations・#81
Observability Snapshot は各 owner の範囲。

## Implementation

### Rust（`crates/synvelis-app`）

- `ui_ops.rs`（新規）: `UiOpsState`（`ControlsUi`・`VizUi`・bounded
  `VecDeque` outbox `Rc<RefCell>`・view/subscription/surface
  registry・frame dedup・manual viz source port・注入 clock・
  counter）。全 op 実装: view attach/detach/close/list、control
  dispatch/subscribe/unsubscribe/targets、viz bind/meta/query/
  viewport/select/snapshot + test-only register/push_frame/
  source_changed。
- `ui_pump()`: 両 gateway pump → `drain_wire` receipts →
  subscription rows（upstream `StateProjection` 形
  `{revision, value, provenance?}` — observe() がそのまま食える）→
  latest-wins meter/spectrum frames。全 op の尾部で呼び、op の
  帰結が必ず outbox へ載る。
- `app_state_notify` とは別に `push_notify(channel, data)` が
  `{channel, data}` 形で `syn.control.receipt` /
  `syn.control.projection` / `syn.viz.frame` / `syn.viz.event` /
  `syn.view.state` を emit。outbox は cap 超過で最古を drop し
  drop counter を記録（黙って失われない）。
- `bin/synvelis.rs`: Ready 後の毎 event-loop turn で共有 outbox
  handle（`event_outbox()`、service へ move する前に clone）を
  drain → `dispatch_script` push。
- `capabilities` は views=available・multi-window=deferred を正直に
  報告。

### Frontend（`ui/`）

- `binding/router.ts`: `onEvent` 単一購読の裏で `data.channel`
  別 fan-out。listener 数を観測可能（leak 検証用）。lazy
  singleton + test reset。
- `binding/port.ts`: upstream `BindingPort` 実装。
  `dispatch`→`syn.control.dispatch`（`expectReceipt` は wire から
  剥がす — backend `DispatchContext` が unknown field 拒否）、
  `subscribe`→`syn.control.subscribe` + projection route（route 先
  に登録して hydration push の race を防ぐ）、`query`→
  `syn.viz.query`（AbortSignal は client-side honoring — op は
  in-process 同期のため wire cancel 不要）。
- `viewmodel/console-view-model.ts`: DOM 非依存。`ViewLink` +
  `CommandBinding`（transport/position + param strip）所有、
  `syn.control.receipt` → `binding.ack` route、
  attach/detach/close を `syn.view.*` op + link lifecycle へ写像。
- `viewmodel/viz-source.ts`: `IpcWaveformSource`/`IpcMeterSource`/
  `IpcSpectrumSource` — typed `source` property 経由で要素へ接続、
  meta/query/snapshot op と `syn.viz.frame`/`syn.viz.event` push を
  消費。
- `adapters/ruviform.ts`: controls + viz 両方の register、全参照
  要素の JSX typing。object 値は `setProp` 経由の real property
  assignment のみ（JSON attribute 化しない）。
- `views/console.tsx`: 参照構成（transport/seekbar/knob/fader/
  waveform/meter）。`bindControlElement`/`bindTransportElement` が
  gesture→intent→receipt→reflection を担い、unbind は onCleanup
  紐付け — remount/dock で listener/subscription 二重化なし。

## Verification

実行環境: Windows 11 x64、Git Bash、Rust 1.99.0 pinned、mise
管理 toolchain。

| 検証 | 結果 |
| --- | --- |
| `mise run fmt-check` | Passed |
| `mise run check` | Passed |
| `mise run lint` | Passed（-D warnings 0件） |
| `mise run test` | Passed（workspace 全 suite、ui_ops 10件含む） |
| `mise run check-ui` / `test-ui` / `build-ui` | Passed（vitest 37件） |
| `mise run check-app-webview` / `lint-app-webview` / `test-app-webview` | Passed |
| `mise run check-headless-deps` / `check-ruviform-rev` | Passed（8fb665f1 REV match） |

vitest 内訳: `binding.test.ts` 9件（dispatch wire shape・projection
routing・stale drop・rejected/unsupported reflection・transport
intent・idempotent rebind・view model lifecycle・subscription
release）、`conformance.test.ts` 12件（`scenarios/binding` 全
fixture の ContractModel oracle deep-equal + expect pins +
determinism、op name/intent/notify wire conformance）。

NotRun / environment-dependent:

- **RF-GT-01 実ブラウザ lane**（`ui/browser-test` playwright）—
  playwright browser 未導入の環境のため未実施。実 GUI 表示・実機
  WebView2 挙動は本 lane では担保していない。
- **実 WebView 手動 E2E**（`synvelis` bin 実窓起動 → console 操作）
  — headless/mock のみ検証済み。
- 二論理 window の同 target 更新は Rust gateway test で実測済み
  （`two_views_same_target_both_observe`）だが、frontend 側 2 view
  実 DOM シナリオは未実施。

## Notes / follow-ups

- `syn.control.dispatch` の op result は queue ack のみ — receipt
  と projection は notify 経由（ADR 決定節）。`provenance` は
  optional field として absent で送る（null は upstream validator
  拒否 — conformance lane で検出・修正済み）。
- `syn.viz.query` は in-process 同期解決のため `cancelQuery` は
  client-side AbortSignal honoring のみ（wire cancel op なし）。
- View close は Project/audio/render job を暗黙キャンセルしない
  （`detach_cancels_gesture_but_not_transport` で実測）。
- RF-GT-02 IPC fixture corpus の残り suite（`ipc/intents`・
  `ipc/events`・`ipc/data`）は upstream Rust conformance test が
  同一 corpus を検証済み。Synvelis 側では `syn.*` wire shape を
  同 mirror で追加検証したが、corpus case 全量の再実行はしない
  （重複回避）。

## 2026-11-06 lifecycle 是正（追補・frontend 正式受入 BLOCKED）

対象は `TASK-034RADc5Q2mNYPcXkM8fR` の既存 lifetime 契約の回復であり、
SV-GUI-03 / Issue #112 全体、新しい CI trigger、永続化設計、実 OS 受入ではない。
上記既存本文・検証表は履歴として残すが、今回の是正の実証として読み替えない。
元の「onCleanup 紐付け・二重化なし」という記述だけでは、await を跨ぐ実 mount の
cleanup や late attach を保証できていなかったことを追加回帰テストで確認した。

### 観測と修復

- 製品コード未変更で console 5件 + model 3件が assertion で失敗し、既存9件は成功した。
  setup/type/transform error を RED の根拠にしていない。
- `ConsoleView` の cleanup 登録は有効な Solid owner の同期区間へ移動し、
  DOM unbind・両 viz source・`vm.detach("unmount")` を所有させた。
  attach / wave bind / meter bind の各 await 後は disposed lifetime を再開しない。
- `ConsoleViewModel` は attach attempt を失効可能にし、detach/close が receipt route と
  local link を同期解放する。遅延応答から link/購読を復活させず、
  新しい mount を detach する補償 RPC は追加しない。reattach は receipt route を一つ復元する。
- 修復 source は `ui/src/views/console.tsx` と `ui/src/viewmodel/console-view-model.ts`。
  回帰追加は `ui/test/console.test.tsx` と `ui/test/binding.test.ts`。
  backend、Job 取消契約、wire DTO、公開シグネチャ、dependency/lock、CI workflow は変更しない。

### 今回実行した検証

検証対象は origin/main `7a3a9cb55ae271cf96b65c04e25ad10ec5b1d8ec` を基にする
worktree HEAD `98fcb49c19ac45e670a155a18ef533c84e8b3c93` と本是正の UI 四ファイル差分。
最新 main を fetch して drift がないことを確認し、既存 vendor を再生成した。

| 検証 | 今回の結果 |
| --- | --- |
| 修正前 focused vitest / review 反映後の revised RED | 8 failed / 9 passed、意味のある assertion |
| 修正後 focused vitest | 17/17 成功、stderr 空（owner 警告を抑制せず解消） |
| `mise run check-ui` | 成功（vendor check + TypeScript） |
| `mise exec -- prek run --files`（対象四ファイル） | 成功 |
| `mise run sync-ui-deps` | 成功 |
| `mise run test-ui` | 全45件成功 |
| `mise run build-ui` | production bundle / assets manifest 生成成功 |
| `mise run test` | Rust workspace 全 suite 成功、ui_ops 10件含む |
| 実 Chromium client DOM + fake IPC | live/unmount/旧 DOM、pending old attach→replacement→late response の二シナリオ成功 |

Chromium 150 / Windows の測定では live の route 数は receipt=1 / projection=4 /
viz.event=1 / viz.frame=1、unmount 後はすべて0。
古い attach 応答の前後で replacement の route 数は不変で、viewport/select 転送を維持し、
最後の dispose 後はすべて0。console entries は空、truncated=false。
この測定は fake IPC であり、実 Rust IPC / native WebView / 実 OS 描画 / GPU / Audio の
受入と混同しない。従来表の optional webview 検証を今回再実行したとも扱わない。

原出力は workspace の `coordinator-state.md` から辿る
`Temp/verified-skip-and-tracker-replay/sv-lifecycle-*` に保存した。
ブラウザの wait コマンドは EOF 応答となったが、同一 tab の read-only eval で結果取得し、
製品エラーや成功と推測せず原 envelope を確認した。一時測定二ファイルは commit 対象外とし削除済み。

### 記録上の境界

作業中の決定・RED/GREEN と検証証跡は state / Temp spec / logs に逐次保存した。
既存 IR の再開同期は実装開始時に漏れており、この追補を開始時から存在した記録とは扱わない。
既存の複数 task をまとめる relation と created/updated=2026-11-25 の将来日付は履歴上の
既存異常として残し、根拠のない日付変更や task 分割を行わない。

### 独立 review と判定

独立した cooperative / adversarial review の10候補を別の Validator で確認し、
lead が原コード・原結果・候補ごとの到達性を照合した。
追加の別モデル review はユーザー選択によりスキップした。
観測は9件確認／1件棄却であって、「9個の新規欠陥」ではない。

lead 著の一時観測 probe は6件成功し、保存 copy の hash と実行対象の一致も確認した。
そのうち実 model receipt callback は attach と reattach の双方で配送し、
queue ack が receipt を偽造しないこと、異なる view の receipt を除外することを検証した。
欠陥の現状値を assert する観測も含むため、この6件を canonical 45件に合算したり
契約充足の GREEN と読み替えたりしない。一時 probe は削除し、原本・出力は Temp に保存した。

今回の現在 app / VM API 経路の lifetime 修復に起因する P0/P1 は確認していない。
ただし attach 後に作成した strip が view epoch を採用しない既存の共有 SDK 不整合を確認した。
client probe では先行 binding=1／後発 binding=0／dispatch ctx.generation=0、
backend source の stale guard では確認 receipt に到達しない。
Ruviform current main にも同じ欠落があり、[Ruviform #376](https://github.com/scarecrow173/ruviform/issues/376)
で追跡する。Synvelis 固有の回避策や backend guard の緩和では隠さない。
このため frontend 全体の正式受入は BLOCKED とし、本 Task/IR と #112 を完了扱いにしない。
修復スライスは Draft PR で保存し、merge-ready / 統合済みとは扱わない。

残る P2/P3・検証境界も維持する：

- public な `vm.link` を直接操作すると VM の route 管理を迂回できる。
  raw detach 後の route 残留は constructor route 時代にも存在し、
  raw attach の route 欠落は今回の scope 化によるもの。現在 app は VM API を用い、
  public API の再設計や新しい利用禁止契約を本修復で導入しない。
- 失効した `attach()` は最後の committed generation を返すが local state は復活させない。
  現在の呼出側は返却値を用いない。新しい cancellation sentinel / reject 契約は追加しない。
- attach の拒否時 UI は既存の別問題。disposed meter の in-flight refresh は snapshot を一度
  続行し得るが、listener/subscriber の再生成はなく errors は catch される。
- canonical lifecycle tests の mounted control-op / wire params / receipt 配送には盲点が残る。
  追加 probe の限定的な確認を実 backend 接続の end-to-end 受入と混同しない。
- 現在の単一 `Show` の unmount→remount と遅延応答は確認したが、同じ view ID の
  concurrent owner や実 Dock detach の並行順序までは受け入れていない。
- 明示的な新 attach after close は backend が epoch を上げて許容する既存契約であり、
  古い completion による復活とは区別する。永久 closed seal は加えない。

CI トリガー・権限・Job 契約は変更せず、bot CI 自動通知 gap は別件の ADR 待ちである。
