---
id: "DISC-p1cxf6X5hKAAYXCVPyHLTu"
type: "discovery"
status: "resolved"
title: "SV175 Native App Shell 棚卸し — 既存 composition root・headless Binding・利用可能 Ruviform 契約・RF387 依存 gap"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/ruviform/issues/384"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
    - "docs/adr/sv-gui-02-syn-binding-ipc-surface-notify-outbox-bounded-wire-dto.md"
    - "docs/adr/sv177-product-theme-crate-boundary-dtcg-document-envelope-and-slot-based-workspa.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV175 Native App Shell 棚卸し

Issue #175（SV-GPUI-01・2026-11-07 横断レビュー確定仕様 v2）の初期
slice に向けた、既存実装・消費可能な Ruviform 実 API・不足契約の
棚卸し。確認時点の SHAS は Synvelis main `53318e7`・Ruviform main
`e8adcc09`（作業時点 `Cargo.lock` pin は `5c0f16b` で更新が必要）。

## 既存構成（Synvelis側・再利用対象）

- `crates/synvelis-app` — 製品 composition root。
  - `SynvelisApp`: `WorkspaceManager<ShellWindowHost>` → `ProjectSession`
    → `ProjectController` の単一正本チェーン、`DirectoryBundleStore`
    （save/reload）、audio/recorder slot、`ObservabilityOwner`、
    shared quit flag（`Arc<AtomicBool>`）、`UiOpsState`。
  - `AppService`: `syn.*` op surface（`syn.app.*`/`syn.project.*`/
    `syn.transport.set`/`syn.workspace.mount`/`syn.view.*`/
    `syn.control.*`/`syn.viz.*`）を `ruviform_ui::Service` として
    `UiRuntime` へ登録。GUI/headless で同一 dispatch。
  - `UiOpsState`: `ControlsUi`/`VizUi` gateway、共有 notify outbox
    （`Rc<RefCell<VecDeque<Event>>>`、cap 4096・drop 計数）、
    view/subscription/surface registry、`notify_seq`。
  - `webview` feature: `ruviform-ui-webview`/`rust-embed` optional 依存
    + `bin synvelis`（host `run` + `dispatch_script` eval ループ）。
    lib 本体は headless で GUI 型を持たない。
  - `set_ui_now` は `SynvelisApp` 上に存在するが、`AppService` が
    `UiRuntime` へ sealed されると到達不能。webview bin は clock を
    注入していない（`ui_now_ms` は 0 固定のまま）。
- `crates/synvelis-workspace` — headless binding 正本。
  - `controls_ui.rs`: upstream `ui/binding` 契約の Rust 正本 —
    `CommandIntent`（`begin`/`preview`/`commit`/`cancel`/`action`）、
    `DispatchContext`（commandId/viewId/windowId/generation/issuedAt）、
    `CommandReceipt`（accepted/queued/applied/rejected/unsupported）、
    `CommandPhase`、`BindingSnapshot`、generation guard、gesture
    state machine、`ack_timeout`。
  - `viz_ui.rs`: `VizUi` surface registry（meter/spectrum/waveform）。
  - `dock`/`view`/`gesture`: `ruviform_ui::dock` の `DockTree`/
    `ViewInstanceId` を消費。
- `ui/`（SolidJS+TS frontend）・`crates/synvelis-app` webview host —
  旧 GUI 経路。履歴・検証資産として保持、Native 実証までは破壊
  削除しない（#175/RF384 確定方針）。

## 消費可能な Ruviform 実 API（main `e8adcc09`）

- `ruviform-ui::driver`（RF386, 5c0f16b で既に lock 済み）:
  `WindowSpec`/`WindowLifecycle`/`DriverEvent`/`WindowCapabilities`/
  `Capability`/`WindowDriver` port + `LogicalSize`/`LogicalPosition`。
  `Action(Intent)` typed channel は `postMessage` decode の正規後継。
- `ruviform-ui::native_slot`（RF399, e8adcc09 で追加）:
  `SurfaceSlotRegistry`/`NativeSurfaceSlot`/`SlotLease`/`SlotEvent`/
  `SlotCapabilities`。本 slice では消費しない（SV176/RF389 所有）。
- `ruviform-ui::runtime`: `UiRuntime` — `dispatch_window_intent`
  （`WindowLifecycle::Action` 経路）、`drain_jobs_routed`
  （`RoutedEvent{window: Option<WindowId>}`）、`handle_window`
  （CloseRequested→`lifecycle closing`）、`cancel_window_jobs`。
- `ruviform-ui::ipc`: `Intent`/`Event`（`Result`/`Job`/`Notify`/
  `Lifecycle`/`Error`）、`ErrorBody`、stable error codes。
- `ruviform-ui-gpui`（feature `gpui`・非 default）: `GpuiDriver`
  （`WindowDriver` 実装・`run(tick, on_event)` blocking loop）、
  `IntentSink`（view→driver `Action` 送出）、`GpuiSurfaceAdapter`。
  `native_surface_slot` は backend 有無で正直に Supported/Unsupported、
  `ime` は Unsupported 固定（RF389 所有）。
- `ruviform-ui-test`: `FakeDriver`（`WindowDriver` の deterministic
  fake — inject/pump/delivered/close_pending/ipc_disconnect fault）、
  `FakeHost`/`HeadlessIo`/driver lab。dev 依存として headless 検証
  に使える。
- `ruviform-ui-theme`（RF385）: `ThemeResolver`/`ResolvedTheme`/
  `TokenDocument` — `synvelis-theme::adapter` が既に委譲済み
  （SV177・PR179 merge 済み）。本 slice で Theme 解釈を複製しない。

## 不足契約（RF387 候補待ち）

最小 Vertical Slice（Tab+Fader+read-only project+gesture 往復+save/
reload/close/teardown）が必要とするが、現行 main に存在しない契約:

1. **Consumer view mount seam** — `GpuiDriver::create_window` が内部で
   fixture `RvfRoot`（title+inbox count 表示）を直接生成するため、
   Consumer が自前の view 内容（RF387 renderer 出力）を window root
   へ接続する入口が無い。RF387 の renderer/view-factory 契約が必要。
2. **view→driver intent 送出の Consumer 入口** — `IntentSink` は存在
   するが `GpuiDriver::intent_sink(window)` 経由で Consumer が取得
   する形。driver-neutral に揃えるか、driver extension として明示
   するかは RF387 側の決定。
3. **inbox `Event` の view 側消費点** — `post_event` が積む
   per-window inbox を誰が読むか（現状 fixture が count 表示のみ）。

これらが無い間は仮 ViewSpec/stub renderer を作らず、当該 slice を
NotRun として明記する（Issue #175「候補 renderer なしで仮 ViewSpec/
stub を完了としない」）。shell/lifecycle/Binding adapter は上記
実 API のみで完結するため先行して進める。

## Scope 境界

- 本 slice が Own: Native composition root・GPUI Driver 組立・
  non-RT typed ViewModel↔Ruviform Binding adapter・app lifecycle。
- 別 Issue が Own（Start 条件にしない）: #112 core 画面、#113 Dock/
  multiwindow UX、#176 実 plugin dock、#114 製品 E2E、#177 Theme
  （統合済み）。
- Driver 実 desktop input/IME は RF389 owner。実 OS/GPU/IME/実機
  検証は本 slice では NotRun/Unsupported を明記する。

## 並行作業との編集境界

- 別 SV worker が `synvelis-render`/`export.rs`/`spec.rs`/player/
  `tests/pc_b.rs` の RF305 型状態移行を別 worktree で担当。共有
  `Cargo.toml`（members/workspace deps）・`Cargo.lock`・`docs/*/
  README.md`（生成 index）・`mise.toml` は union 整合で扱い、他者
  変更を戻さない。
