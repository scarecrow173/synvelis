---
id: "IMPL-SVGui02FrontendViewModelsRuviformCustomElementsTypedIpc"
type: "impl"
status: "completed"
title: "SV-GUI-02 Frontend View Models・Ruviform Custom Elements・Rust IPC/Receipt統合"
created: "2026-11-25"
updated: "2026-11-25"
owners: []
relations:
  source: []
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
