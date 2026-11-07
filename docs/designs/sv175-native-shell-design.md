---
id: "DESIGN-BWzJVIfJHZS8suU7aFFAGf"
type: "design"
status: "approved"
title: "SV175 Native App Shell 設計 — NativeShell routing・ViewBinding adapter・native feature lane"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/ruviform/issues/384"
    - "https://github.com/scarecrow173/ruviform/issues/386"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
  implemented-by: []
  depends-on:
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-02-syn-binding-ipc-surface-notify-outbox-bounded-wire-dto.md"
    - "docs/discovery/sv175-native-shell-inventory.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV175 Native App Shell 設計

## 構成と依存方向

```
crates/synvelis-app (headless lib)
  src/shell.rs   NativeShell  — WindowDriver port 消費・DriverEvent routing
  src/binding.rs ViewBinding  — typed intent 生成 + Event demux
  src/lib.rs     既存: SynvelisApp / AppService / UiOpsState
      ↑ UiRuntime + syn.* ops (ruviform_ui::runtime/ipc)
      ↑ ControlsUi/VizUi (synvelis-workspace) ← 第二 truth を作らない
  src/bin/synvelis-native.rs  [feature "native"] — GpuiDriver 組立のみ
      → ruviform-ui-gpui (gpui/OS 型はここに隔離)
```

- `synvelis-app` lib は `ruviform-ui`（driver/ipc/runtime）までを
  通常依存とし、`ruviform-ui-gpui` は `optional` + feature `native`
  （`dep:ruviform-ui-gpui` + `ruviform-ui-gpui/gpui`）。
- `check-headless-deps` の EXACT に `gpui`/`ruviform-ui-gpui` を
  追加し、headless crate（`synvelis-app` lib を含む）への混入を
  検査で拒否。bin ターゲットは検査対象外（feature 依存のみ
  有効化されるため）。webview 側の `ruviform-ui-webview`/`wry`/
  `tao` も同じ EXACT で固定（既に有効なら追記）。
- `synvelis-workspace`・domain crate へは一切 GUI 依存を増やさない。

## NativeShell（`src/shell.rs`）

```text
pub struct NativeShell {
    rt: UiRuntime,                 // AppService 登録済み
    quit: Arc<AtomicBool>,
    outbox: UiOutbox,              // Rc<RefCell<VecDeque<Event>>>
    initial_notify: Option<Event>, // Ready 時に送る app-state notify
    clock: Rc<Cell<u64>>,          // SynvelisApp::ui_clock() と共有
    windows: BTreeMap<WindowId, ShellWindow>,  // ready/pending 状態
    views: HashMap<String, WindowId>,          // view_id → 所在 window
    pending_view_ops: HashMap<String, PendingViewOp>, // intent id → op
    main_window: Option<WindowId>,
}
```

- `new(app: SynvelisApp)`: `SynvelisApp::into_ui_parts()` で
  `AppService`/quit/outbox/initial_notify/clock を取り出し
  `UiRuntime::new` + `register(Box::new(service))`。
  （`SynvelisApp` に `ui_clock() -> Rc<Cell<u64>>` accessor を追加し、
  `set_ui_now`/`ui_pump` が同じ cell を経由するよう内部的に統一。
  `SynvelisApp` API は破壊しない。）
- `open_main(&mut dyn WindowDriver)`: `WindowSpec` を `create_window`
  へ渡し返り `WindowId` を main として保持。
- `handle(DriverEvent, &mut dyn WindowDriver)`:
  - `Windowed{Created}`: window を `pending` で記録。
  - `Windowed{Ready}`: `ready` へ。`initial_notify` を
    `post_event`、outbox 滞留分も flush。
  - `Windowed{Action(intent)}`: `pending_view_ops` に
    `syn.view.{attach,detach,close}` の `(intent.id, view_id,
    window)` を記憶 → `rt.dispatch_window_intent` → 各
    `RoutedEvent` を `post_routed`（`window: Some(w)` はその
    window、`None` は broadcast）で返送。Result が ok なら
    `views` を更新。
  - `Windowed{CloseRequested}`: `Event::lifecycle(Closing)` をその
    window へ送り `respond_close(Approve)`（初期 slice。unsaved
    guard は設計上 veto 可能な hook として後続）。
  - `Windowed{Destroyed}`: `rt.cancel_window_jobs(window)`、当該
    window の `views` 各 view へ `syn.view.close` を `dispatch_op`
    で replay（結果は broadcast で流れるが受信 view は既に無い—
    teardown 確認として破棄可）、`views`/`windows` から除去。
    `windows.is_empty()` なら `driver.exit()`。
  - `Windowed{Resized/Moved/Focused/ScaleFactorChanged}`: shell は
    記録のみ（初期 slice で意味づけしない）。
  - `Tick`: `rt.drain_jobs_routed` → `post_routed`、`clock` を
    tick 周期分 advance、outbox drain → ready window 全てへ
    `post_event`、quit flag 立っていれば全 window へ
    `request_close`。
- 全 `post_event` は fire-and-forget。`Err` は捨てず debug log
  （閉じた window 宛の再送は driver が捨てる契約）。

## ViewBinding（`src/binding.rs`）

```text
pub struct ViewBinding { view: String, window: String, seq,
    pending: HashMap<String,String>, /* intent id → op */
    outbox: VecDeque<Intent>, generation: u32,
    issued_at: u64, attached: bool }
```

- intent 生成（各 `Intent::new("<view>-<seq>", op, params)`、`ctx`
  は `DispatchContext` から `serde_json::to_value`）:
  `attach`/`detach(reason)`/`close`、`subscribe(target)`/
  `unsubscribe(target)`、`gesture_begin(target,control,value)→
  GestureId`（view 側 mint、`g<view>-<seq>`）+ `gesture_preview`/
  `commit`/`cancel(gesture_id)`、`action(target,control,action)`、
  `project_new/open/save/info`、`transport_set(playing)`、
  `app_info`/`app_capabilities`/`app_quit`、`view_list`、
  `control_targets`。`set_clock(now)` で `issuedAt` を駆動。
- `take_outbox() -> Vec<Intent>` で view が driver `Action` へ渡す
  （`synvelis-native` では `IntentSink`/`DriverEvent::Windowed
  {Action}` 経路に乗る）。
- `apply(&Event) -> Vec<BindingNotice>`:
  - `Result` → `pending` 相関で `CallResult{op,ok,data,error}`。
    attach ok なら `generation` を採用 + `Attached` 通知。
  - `Notify` → `data["channel"]` 別 demux:
    `syn.control.receipt`/`syn.control.projection`（`view_id`
    一致のみ、不一致は `Ignored`）、`syn.view.state`、`syn.viz.*`、
    channel 無し → `AppState`。
  - `Lifecycle`→`Lifecycle(Closing)`、`Job`→`Job{..}`、
    `Error`→`Error`。
- `ProjectionRow` は wire 形の typed mirror（`#[serde(default)]` で
  forward compat、`observed` は `Value`）。`CommandReceipt` は
  `synvelis-workspace` 型へ `serde_json::from_value`。
- 非 RT・allocation bound 固定（outbox は VecDeque + 呼び出し側
  cap）。GPUI/OS 型ゼロ。

## `synvelis-native` bin（`src/bin/synvelis-native.rs`, required `native`）

1. `synvelis_app::logging::install()`（既存 webview bin と同型）。
2. `GpuiDriver::new()` → `capabilities()` を info log（`ime`
   Unsupported/`native_surface_slot` backend 依存を正直に出力）。
3. `SynvelisApp::new(BackendReport::available("gpui", <gpui ver>))`
   → `NativeShell::new(app)` → `shell.open_main(&mut driver)`。
4. `driver.run(Some(Duration::from_millis(16)), move |ev, d|
   shell.handle(ev, d))`。

## RF387 接続面（未確定・gate）

- `create_window` が内部で fixture `RvfRoot` を作るため、consumer
  view の mount seam・view→driver `Action` 送出の consumer 入口・
  inbox `Event` の消費点の 3 契約が不足。RF387 候補入手後に
  `ViewBinding` の `take_outbox`/`apply` を GPUI view 側の typed
  seam へ接続する。それまでは最小 Tab+Fader slice を `NotRun` と
  明記する（仮 ViewSpec/stub 非作成）。

## 検証（headless = `FakeDriver`）

- `tests/shell.rs`: `FakeDriver::pump` で `NativeShell` を駆動し、
  window lifecycle・Action→typed result・view attach→control
  gesture（begin/preview/commit/cancel）→receipt/projection
  到達・project new/save→再 open・CloseRequested→closing+
  Destroyed→view close+exit・quit flag→request_close を検証。
- `tests/binding.rs`: intent 形（op/params/ctx.generation は
  attach 応答採用）、`apply` demux・view_id 絞込・result 相関。
- `ruviform-ui-test` を `[dev-dependencies]` に追加。
