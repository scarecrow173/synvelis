---
id: "DESIGN-034UqNokCXcUXgKk1tyv0p"
type: "design"
status: "approved"
title: "SV-GUI-04 slice2 dock transfer: workspace adoption と dock/UI 権威分界の詳細設計"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
  implemented-by: []
  depends-on:
    - "docs/adr/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
    - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
    - "docs/designs/workspace-docking-state-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/implement-sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/adr/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice2 dock transfer: workspace adoption と dock/UI 権威分界の詳細設計

## Context

Issue #113 の dock transfer（Detach→Reattach・move abort/rollback）
を GUI op として配線する設計。前提は ADR
`docs/adr/sv-gui-04-slice2-dock-transfer-workspace-adoption.md`
の決定 D1–D5: adoption seam・dock/UI 権威分界・`Destroyed` →
`DetachViews` 既定・two-phase + pending state・`syn.window.dock_*`
op family。

現行 main の構造（`ab235a5` 以降、close-veto decision フローは
別 dispatch `ctx_2aed081be01f` が編集中で main 未着の可能性あり
— 差分前提は §リスクに明記）:

- `SynvelisApp.workspace: WorkspaceManager<ShellWindowHost>`
  （`app.rs`）— `ShellWindowHost::create_window` は
  `BackendUnavailable`。`create_window_at` が host 成功後にのみ
  `dock.register_window` + `WindowRecord` を行うため、製品 app で
  `workspace.dock`/`windows` は空のまま（dormant）。
- window 生成権威は `WindowDriver`（`ruviform-ui::driver`、RF386）。
  両 lane（webview `run_multi` / GPUI `synvelis-native`）とも
  `NativeShell::handle(DriverEvent, &mut dyn WindowDriver)` に
  `WindowLifecycle::{Created,Ready,Resized,Moved,Focused,
  ScaleFactorChanged,CloseRequested,Destroyed,Action}` を流す。
- view の UI mount 権威は composition root
  `NativeShell::attach_view(driver, window, view_id, targets)`
  （`shell.rs`）— `syn.view.attach` + `syn.control.subscribe` を
  `rt.dispatch_window_intent` で発行し、`observe_result` が
  `shell.views`/`view_generation` を reconcile する。
- live UI binding の権威: `ui.views: HashMap<String, ViewEntry
  {window:String}>`（app-side registry）、`shell.views:
  HashMap<String, WindowId>`（view→owner window、
  `origin_view` 検証に使用）、`ControlsUi.views`/`VizUi`
  （per-view attach state・generation・subscription・gesture）。
- `syn.view.attach`（`ui_ops.rs`）は両 gateway の
  `attach_view` を呼び、別 window への attach は gateway 内部で
  implicit `detach(dock-transfer)` + generation bump + 再購読を
  行う。`syn.view.detach` は gesture cancel + generation bump +
  tracked state clear。`syn.view.close` は両 gateway の
  `close_view` + `ui.views`/`subscriptions`/`frame_seq`/`surfaces`
  の除去。
- `WorkspaceManager`（`synvelis-workspace`）の transfer 系 API は
  `host` に触れない: `begin_transfer`/`commit_transfer`/
  `abort_transfer`/`move_view`/`reattach`/`detach_to_pool`/
  `detach_to_new_window`（内部で `create_window` を呼ぶため
  host 依存）/`on_window_event`（`Destroyed` は disposition を
  適用、`CloseRequested` は `host.respond_close` を呼ぶ）。
- `DockWorkspace`（`ruviform-ui::dock`）契約:
  `begin_move(view, DockTarget)` → `MoveToken`（private u64、
  wire 非対応）、`commit_move(token)` → `MoveReport`（destination
  消失時 `TargetUnavailable` で自動 rollback）、`abort_move`、
  `move_view`（one-shot）、`detach_view`（pool）、
  `remove_window(window, WindowCloseDisposition)`、
  `register_window`、`add_detached`、`attach_view`、
  `owner_of`/`is_detached`/`detached_views`、`drain_events`
  （`MovePrepared`/`MoveCommitted`/`MoveAborted`/`WindowRemoved`/
  `TreeChanged`）。`MoveCommitted` が consumer の view-state
  handoff point。
- `WindowId`/`ViewInstanceId` は共に `NonZeroU64` newtype、
  Display は `"w{n}"`/`"v{n}"`。wire の `view_id` は
  `"view-{n}"`（`screens.rs` の `main_view_id`/`secondary_view_id`
  が `ViewInstanceId` と lockstep）。`WindowId::new(raw)`/
  `.get()`、`ViewInstanceId::new(raw)`/`.get()` が public。

## Scope

### In Scope

- `synvelis-workspace`: `WorkspaceManager::adopt_window` /
  `adopt_view` の API 設計（署名・error・`next_view` 前進・
  重複拒否）。
- `synvelis-app`: `syn.window.dock_*` op family の params/result/
  error 契約、pending move 状態、`syn.view.*` の dock-aware 拡張、
  window/view adoption の呼出し点、`NativeShell` lifecycle
  forwarding と `Destroyed` disposition flow、new-window detach
  の shell orchestration。
- fake/headless fixture での検証方針と、env-gated 実測への接続。

### Out of Scope

- 実装・テストコード（後続 dispatch）。
- `WindowDriver` → `WindowHandle` adapter（driver-backed host）。
- `WindowCloseDisposition::RelocateTo`・layout snapshot/restore
  （#177）・monitor 失踪復帰。
- DPI/IME（RF#388/#389/#114）・Plugin Dock（#176）・実 OS 証拠
  （#114）。
- close-veto decision フロー本体（別 dispatch。本設計は接続点
  `WindowRecord.veto_close` のみ規定）。

## Components and Boundaries

### `synvelis-workspace` — adoption seam

`WorkspaceManager<H: WindowHandle>` に 2 メソッドを追加する
（`host.create_window` は経由しない）:

```rust
/// Registers a window the host/driver already created.
///
/// `id` must not already be registered; `geometry` seeds the
/// WindowRecord (Resized/Moved events keep it current).
/// # Errors
/// [`WorkspaceError::DuplicateWindow`] on re-adoption;
/// propagates [`WorkspaceError::Dock`] from `dock.register_window`.
pub fn adopt_window(
    &mut self,
    id: WindowId,
    title: &str,
    geometry: WindowGeometry,
) -> Result<(), WorkspaceError>;

/// Registers a view the composition root already mounted under an
/// existing [`ViewInstanceId`].
///
/// `target: None` places the view in the detached pool;
/// `Some(DockTarget::Window(w))` / `Some(DockTarget::Zone{..})`
/// docks it. Advances `next_view` past `id` so later `spawn_view`
/// mints never collide.
/// # Errors
/// [`WorkspaceError::Dock`] (`DuplicateView`/`UnknownWindow`/
/// `InvalidTarget`) on invalid placement or re-adoption.
pub fn adopt_view(
    &mut self,
    id: ViewInstanceId,
    kind: ViewType,
    title: &str,
    target: Option<DockTarget>,
) -> Result<(), WorkspaceError>;
```

内部動作（`create_window_at`/`spawn_view` の登録部と同じ構成）:

- `adopt_window`: `windows.iter().any(|w| w.id == id)` で重複拒否
  → `dock.register_window(id)` → `WindowRecord { id, title,
  geometry, veto_close: false, disposition:
  WindowCloseDisposition::DetachViews }` を push。
- `adopt_view`: `registry.contains(id)` で重複拒否（`DockError::
  DuplicateView { view: id, owner: self.dock.owner_of(id) }` を
  `WorkspaceError::Dock` で返す）→ `target` が `Some` なら
  `dock.attach_view(id, target)`、`None` なら `dock.add_detached(id)`
  → `registry.register(id, kind, title)` → `states.ensure(id)` →
  `self.next_view = self.next_view.max(id.get())`。
- `adopt_window` の重複には `WorkspaceError` へ additive に
  `DuplicateWindow(WindowId)` variant を追加する（既存 variant
  の意味を変えない。`Display`/`Error` impl を追従）。

`WindowGeometry` は `ruviform_ui::dock::WindowGeometry`（既存の
`x`/`y`/`width`/`height`/`monitor` field そのまま）。app 側は
window spec 既知の時点（`Created` 処理時に spec を保持している
composition root）で完全な値を渡し、以後は `on_window_event`
の `Resized`/`Moved` が更新する。

### `synvelis-app` — op 層（`ui_ops.rs`/`service.rs`）

新設 op family（全て `SynvelisApp` の `pub fn op_*` +
`service.rs` の dispatch 登録）:

| op | params | result | 意味 |
| --- | --- | --- | --- |
| `syn.window.dock_prepare` | `view_id`, `window_id` | `{view_id, window_id, prepared: true}` | 既存 window 宛の two-phase move 開始（`begin_transfer`）。destination は adopt 済み window であること。 |
| `syn.window.dock_prepare_new_window` | `view_id`, `title?` | `{view_id, pending: true, request_id}` | 新規 window 宛 move の phase 1。view を pending 化し window 生成要求を app→shell へ通知する。 |
| `syn.window.dock_commit` | `view_id` | `{view_id, committed: true, from, to}` | pending/transfer 中の move を commit し UI binding を destination へ付け替える。 |
| `syn.window.dock_abort` | `view_id` | `{view_id, aborted: true}` | pending/transfer を abort。view は source ownership・UI binding を維持。 |
| `syn.window.dock_detach` | `view_id` | `{view_id, detached: true}` | one-shot で detached pool へ（`detach_to_pool` + UI detach）。 |
| `syn.window.dock_reattach` | `view_id`, `window_id` | `{view_id, reattached: true, from, to}` | pool/別 window からの one-shot move（`move_view`/`reattach` + UI attach）。 |

補助 op（adoption・観測用、additive）:

| op | params | result | 意味 |
| --- | --- | --- | --- |
| `syn.window.adopt` | `window_id`, `title?`, `width?`, `height?`, `x?`, `y?` | `{window_id, adopted: true}` | driver/composition root が生成した window を workspace へ登録。shell が `Created` 時に内部 dispatch する想定だが op 公開で fixture/test からも駆動可能。**op 層は冪等**: 同一 `window_id` の再 adopt は既存 `WindowRecord` と params が一致すれば `adopted: true` を返し、矛盾する title/geometry なら error。workspace 層の `adopt_window` 自体は重複を拒否する（ADR D1）— 冪等性は op 層の責務。 |
| `syn.window.list` | — | `{windows: [{window_id,title,geometry,veto_close,disposition}], docked_views, detached_views}` | workspace の window/dock 観測面（検証・debug 用）。 |

`UiOpsState` への追加（app 側状態、op 層のみが書く）:

```rust
/// In-flight dock move keyed by wire `view_id`. `MoveToken` never
/// crosses the wire — the op layer owns the token.
pub(crate) dock_moves: HashMap<String, DockMove>,

pub(crate) enum DockMove {
    /// begin_transfer issued; destination window exists.
    Prepared { token: MoveToken, target: WindowId },
    /// New-window saga: window creation requested, awaiting
    /// adopt+Ready+attach before commit is legal.
    AwaitingWindow { title: String },
}
```

`DockMove` は `Prepared`/`AwaitingWindow` の 2 状態。view が
`dock_moves` に存在する間は当該 view への編集系 op
（`syn.control.*` 等 `require_attached_view`/gesture 系）と
`syn.view.attach`/`detach`/`close`、および別の `dock_prepare*`
を `ViewBusy` 相当（`codes::INVALID_PARAMS` or 専用 code —
既存 `ErrorBody`/`codes` の規約に従い impl slice で確定）で
拒否する。`dock.is_transferring`（`DockWorkspace` 側の lock）と
`AwaitingWindow`（dock 未 begin の pending）は別層の lock であり、
両方を op 層が検査する。

### wire codec（app 層 helper、`ui_ops.rs` または `screens.rs`）

```rust
fn parse_window_id(s: &str) -> Option<WindowId>   // "w{n}" → WindowId::new(n)
fn parse_view_instance(s: &str) -> Option<ViewInstanceId> // "view-{n}" → ViewInstanceId::new(n)
```

`view_id` → `ViewInstanceId` の変換は `syn.view.*` op でも必要に
なるため、attach/detach/close の dock-aware 拡張と共通化する。
未知・非数値・`0` は `invalid_params`。

### `syn.view.*` の dock-aware 拡張（additive、外部契約不変）

**適用範囲（dock-domain scoping）**: dock 更新は「dock-domain
view」にのみ適用する。dock-domain view とは (a) `view_id` が
`"view-{n}"` 形式で `ViewInstanceId` に parse でき、かつ (b)
composition root が `ViewType`/title metadata を登録済みの
view（現行は `view-1`/`view-2` 系）を指す。それ以外の
`view_id`（既存 test の `"console"`/`"meters"`/`"transport"`
等の任意文字列、将来の ad-hoc view）は **legacy UI-only 経路を
維持**し、workspace 呼出しを一切行わない — これにより既存
test・fixture の view_id 語彙を壊さず、「UI attached だが
dock 未登録」の状態は dock-domain 外の view に限り許容される
（権威分界の不変条件は adopt 済み view にのみ適用）。

- `syn.view.attach {view_id, window_id}`: dock-domain view の
  場合、成功時に
  (a) `parse_view_instance`/`parse_window_id` で id 解決
  （非 dock-domain または parse 失敗は legacy 経路へ）、
  (b) `dock_moves`/`is_transferring` 検査 — busy なら拒否
  （`AwaitingWindow` 中は例外: destination window への attach
  のみ UI-only で許可し、dock 配置は commit 時に確定する。
  他 window への attach は busy 拒否）、
  (c) workspace 側: view が未 adopt なら
  `adopt_view(view, kind, title,
  Some(DockTarget::Window(window)))`、adopt 済みなら
  `move_view(view, DockTarget::Window(window))`（owner が
  同一 window なら skip — `move_view` は pool/別 window
  双方からの one-shot で、別途 `detach_to_pool` は不要）、
  (d) UI 側: 従来通り `controls.attach_view` +
  `viz.attach_view` + `ui.views.insert`（implicit
  detach(dock-transfer)+generation bump 内蔵）、
  (e) `ui_pump` + notify（従来通り）。
  - workspace 側 (c) が失敗した場合は UI attach を呼ばず
    error 返却 — 「dock 失敗・UI 成功」の片側更新を作らない。
    UI attach は infallible（`controls.attach_view`/
    `viz.attach_view` は Result を返さない）ため逆方向の
    部分失敗は発生しない。
  - view kind/title の解決: app は view_id → `ViewType`/title
    の対応を composition root（`screens.rs` の view spec 群）
    から既知。metadata 未登録の parse 可能 view_id は
    dock-domain 外として legacy 経路（adopt しない）。
- `syn.view.detach {view_id, reason}`: dock-domain view の場合、
  成功時に `workspace.detach_to_pool(view)`（pool/docked 双方
  から合法、未 adopt なら skip）+ 従来の UI detach。
  `ui.views` entry の window association は detach 後に
  「pool」を示す表現へ更新する（`ViewEntry` に `attached: bool`
  追加または window を `Option` 化 — impl slice で確定、
  `syn.view.list` が attached/pool を区別できることが要件）。
- `syn.view.close {view_id}`: dock-domain view の場合、成功時に
  `workspace.remove_view(view)`（dock/registry/states/links/
  focus を除去）+ 従来の UI close。`remove_view` は `ViewBusy`
  （transfer 中）を拒否するため、transfer 中の close は
  `dock_moves`/`is_transferring` を検査し、進行中の move を
  `abort_transfer`（または `AwaitingWindow` 解除 + UI 復帰）
  してから `remove_view` する順序とする — 「close は transfer
  を abort してから remove」で確定。

これにより「attach = 論理配置の確定 + UI binding の貼り替え」
が 1 op で atomic（同期 dispatch のため外部不可分）になる。

### shell bookkeeping（`track_view_op`/`observe_result`）の拡張

`shell.views`/`view_generation` の reconcile は
`track_view_op` が認識する op の result でのみ行われる
（現行: `syn.view.attach`/`detach`/`close` のみ）。dock op が
view の owner を変える以上、同じ機構を拡張する:

- `ViewOp` に `DockCommit`/`DockReattach`/`DockDetach` を追加
  （または既存 variant の owner 解決規則を拡張）。
- `syn.window.dock_commit`: owner は **result の `to`**
  （window_id 文字列）から解決 — commit は view_id のみを
  param に取り destination は `dock_moves` 内部状態なので、
  result に `to`/`from` を window_id 文字列で含めることを
  op 契約に含める（既に result 案に記載）。`observe_result`
  は `data["to"]` を live windows に対して解決する
  （`ViewOp::Attach` の declared 解決と同規則）。
- `syn.window.dock_reattach`: owner は params の `window_id`
  （`ViewOp::Attach` と同規則）。
- `syn.window.dock_detach`・`syn.view.detach`: owner をクリア
  （pool — fail closed。現行 `ViewOp::Detach => {}` no-op を
  「association クリア」へ変更）。
- `syn.window.dock_prepare`/`dock_prepare_new_window`/
  `dock_abort`: owner 不変のため tracking 不要
  （`AwaitingWindow` 中の UI 先行 attach は `syn.view.attach`
  経由で既に `ViewOp::Attach` として reconcile される）。

これで dock op 経由の移動後も `shell.views`（`origin_view`
検証の権威）・`ui.views`・`WorkspaceManager.dock` の 3 者が
同一 window を指す。

### `NativeShell` — lifecycle forwarding と Destroyed flow

`on_windowed` の各 `WindowLifecycle` を workspace へ写像する
（`rt`/`SynvelisApp` への公開 accessor 経由 — shell は sealed
app を直接触れないため、app 側に `pub(crate)`/`pub` の薄い
forwarding method を用意するか、対応する `syn.*` op を
`rt.dispatch_op` で発行する。**採用方式: lifecycle 系は op
ではなく app の直接メソッド呼出し** — `NativeShell` は
`UiRuntime` 越しに `AppService`/`SynvelisApp` を所有する構造を
確認のうえ、`rt.app_mut()` 相当の経路が無ければ小さな forwarding
op `syn.window.lifecycle` {window_id, event} を 1 本追加して内部
dispatch する。実装 slice で経路を確定するが、意味論は以下）:

- `Created` → `adopt_window(window, title, geometry)`。title/
  geometry は composition root が window spec を知る時点の値
  （`WindowSpec` → `WindowGeometry` 変換 helper を app/shell に
  置く。`x`/`y` は `WindowSpec` に position が無ければ 0、
  monitor は `None` で開始し `ScaleFactorChanged`/実測で更新）。
- `Resized{width,height}` → `HostEvent::Resized` 相当を
  `workspace.on_window_event` へ（physical px — `WindowRecord.
  geometry` の単位規約は `create_window_at` が logical を
  期待するか確認し、impl slice で統一。dpi slice までは
  physical をそのまま記録し単位混在を避ける注記を残す）。
- `Moved{x,y}` → `HostEvent::Moved` 相当。
- `Focused(bool)` → `HostEvent::Focused` 相当（active view 導出
  に既に使われる `active_view_in` が生きる）。
- `ScaleFactorChanged` → workspace へは写像しない（#388/#389
  DPI slice の範囲。geometry 記録のみ継続）。
- `CloseRequested` → 現行の decision flow（close-veto dispatch
  成果物）に委譲し、**`workspace.on_window_event` へは転送
  しない** — `HostEvent::CloseRequested` arm（`host.
  respond_close` 呼出し）は headless host 経路専用であり、
  製品では driver の `respond_close` が close の唯一の
  応答経路。本設計の接続点: veto 判定が
  `WindowRecord.veto_close`（`set_close_veto` で設定）を読む
  よう、app 側に `pub fn window_close_veto(window) -> bool` 相当の
  read API を用意する。**現行 main は `respond_close(Approve)`
  固定** — 本 slice の設計は「veto record が読める seam」までを
  規定し、decision 本体は別 dispatch の差分に委ねる。
- `Destroyed` → `workspace.on_window_event(window,
  &HostEvent::Destroyed)` を呼び、記録済み disposition を適用
  した**後**、view ごとの UI teardown を disposition に応じて
  分岐する（下記フロー）。

### `Destroyed` disposition flow（`NativeShell::on_destroyed` 改訂案）

現行: `cancel_window_jobs` → `windows.remove` → 各 orphan view に
`syn.view.close` replay → `views`/`view_generation` 除去 →
`driver.windows().is_empty()` なら `driver.exit()`。

改訂後:

1. `cancel_window_jobs(window)`（現行通り — job は window に
   属するため cancel は正しい。Project/Graph/Recording/Job は
   `ProjectSession`/engine 側に属し window 非依存なので不変）。
2. `windows.remove(window)`。
3. `workspace.on_window_event(window, &HostEvent::Destroyed)`
   — `WindowRemoved` event が `drain_dock_events` で
   `{window, views}` を返す。disposition:
   - `DetachViews`（既定）: views は detached pool へ。各 view に
     `syn.view.detach {view_id, reason:"unmount"}` 相当を dispatch
     （gesture cancel・generation bump・subscription release）。
     **`syn.view.close` は replay しない**。`shell.views`/`ui.views`
     の window association は pool を示す状態へ更新
     （`observe_result` の `ViewOp::Detach` が association を
     更新するよう拡張 — 現行は no-op なので、detach で owner を
     クリアする変更が必要。これにより pool view の `origin_view`
     command は fail closed になる）。
   - `ReleaseViews`: views は workspace から除去済み。各 view に
     `syn.view.close` replay（現行と同じ teardown）。
   - `RelocateTo{target}`: **本 slice では到達不能** —
     `set_close_disposition` を呼ぶ経路を作らず既定は常に
     `DetachViews`（`ReleaseViews` は app quit/明示 close-all
     の内部経路のみ）。将来の layout slice が有効化する際は
     detach→attach(target) の 2 段 dispatch を本 flow に追加
     する — Deferred Design Concerns に記録。
4. `view_generation`/`views` bookkeeping: DetachViews では view
   entry を残し owner のみクリア（pool）、ReleaseViews では除去
   （現行通り）。
5. `driver.windows().is_empty()` → `driver.exit()`（現行通り —
   app-exit policy は window 寿命に依存する既存製品判断であり、
   本 slice では変更しない。最後の window が閉じれば app は
   終了するが、それは「view を暗黙破棄する」のではなく
   「プロセス終了」なので非破壊要件と矛盾しない — session
   永続化は別経路）。

### new-window detach saga（`dock_prepare_new_window` → commit/abort）

window 生成は `UiRuntime` 内からは不可能（driver は shell/bin
所有）なため、two-phase + app→shell 通知で実現する:

1. `syn.window.dock_prepare_new_window {view_id, title?}`:
   - view が adopt 済み・dock 配置済み・非 transferring・
     `dock_moves` 未登録であることを検査。
   - `dock_moves[view_id] = DockMove::AwaitingWindow { title }`
     を登録（view はこの時点で編集 lock — `ViewBusy` 相当）。
   - `UiOpsState`（または共有 cell — `outbox`/`clock`/`quit` と
     同じ `Rc<RefCell>` 共有パターン）へ `WindowRequest { kind:
     DockDetach { view_id }, spec: WindowSpec }` を push。
   - result `{view_id, pending: true, request_id}`。
2. `NativeShell` が tick/`handle` で `WindowRequest` を drain
   （app→shell の逆方向通知は `quit` flag と同型の共有 cell
   で実現 — `pub` accessor を `SynvelisApp` に用意し shell が
   polling する。詳細は impl slice）:
   - `driver.create_window(&spec)` → `WindowId` 発行。
   - `Created` lifecycle で `adopt_window`（既存 forwarding 経路）。
   - destination window の `ViewHost` を mount
     （`screens.rs` に view spec → `ViewHost` 生成の helper を
     用意 or 既存 `mount_secondary_view` を一般化）。
   - `Ready` で `shell.attach_view(driver, window, view_id,
     targets)`（view を destination へ UI attach —
     `syn.view.attach` が dock-aware 拡張により論理配置も
     更新するが、この時点ではまだ `AwaitingWindow` なので
     `syn.view.attach` は `dock_moves` 検査で拒否される —
     **例外規則: `AwaitingWindow` 中は pending destination
     window への attach のみ UI-only で許可**（他 window への
     attach は busy 拒否）し、dock 配置は commit 時に確定する。`ControlsUi::attach_view`
     は既に implicit detach(dock-transfer) を内蔵するため、
     UI binding は destination へ移り、論理 owner は source
     のまま — この「UI 新/dock 旧」の過渡状態は同期 dispatch
     内でのみ存在し、commit/abort で解消される）。
   - attach 成功 → `syn.window.dock_commit {view_id}` を内部
     dispatch: `workspace.begin_transfer(view,
     DockTarget::Window(new_window))`…ではなく、UI は既に
     destination に居るため **commit 経路は `move_view`
     one-shot**（`begin`→UI attach→`commit` の三段を、
     「UI 先行 attach + `move_view`」の二段に畳んだ等価形）
     — 詳細は下記「commit 意味論」。
   - いずれかの失敗 → `syn.window.dock_abort {view_id}` +
     `driver.destroy_window(window)`（window が既に destroy
     済みなら no-op）。view は source の UI binding…に戻す
     必要がある — UI attach が destination に成功した後の
     abort では `syn.view.attach {view_id, source_window}` を
     再 dispatch して UI を戻す（source window が生きている
     前提 — 死んでいれば pool 行きで detach のみ）。

**commit 意味論の確定**: two-phase（`begin_transfer`→
`commit_transfer`）が本質的に必要なのは「destination の
準備が非同期で、prepare 時点では dock target が未確定」
な場合。既存 window 宛（`dock_prepare`）は target が
adopt 済みで即 `begin_transfer` 可能 → UI attach →
`commit_transfer` の順で token を生かす。新規 window 宛は
target window が saga 中にしか存在しない → `AwaitingWindow`
→ window adopt → UI 先行 attach → `move_view` one-shot
（`begin_move` の token を saga 中に保持する設計も可能だが、
`begin_move` は target 登録済みが前提で adopt 前には呼べず、
adopt 後に begin→即 commit しても得るものが無い — one-shot
`move_view` が等価で単純）。abort は `AwaitingWindow` 解除 +
UI 復帰 + window 破棄。

**Window 生成失敗の安全復帰**（Issue 受入「Window 生成失敗・
安全復帰」に対応）:

- `driver.create_window` が失敗 → `WindowRequest` drain で
  error → `dock_moves` から除去 + `syn.window.state` notify
  （`{view_id, dock_move: "failed", reason}`）。view は source
  に untouched（UI binding も dock も変更されていない）。
- window は生成されたが `Ready`/attach 前に `Destroyed` →
  `on_destroyed` が `adopt_window` 済みの空 window を
  DetachViews で除去（views 空なので pool 行きなし）→
  shell は pending `DockMove::AwaitingWindow` を検出して
  `dock_abort` 相当（`dock_moves` 除去 + 通知）。
- `syn.view.attach`（destination）が失敗 → `dock_abort` +
  `driver.destroy_window`。view は source に残る。
- `move_view`/commit が失敗（target 消失等）→
  `DockWorkspace` が `TargetUnavailable` で自動 rollback
  （one-shot `move_view` は失敗時に source を維持する契約 —
  headless test `sv113_dock.rs` が固定済み）→ `dock_moves`
  除去 + UI を source へ戻す再 attach。

## Data and Control Flow

### 移動（既存 window 宛）— `dock_prepare` → `dock_commit`

```
op dock_prepare {view_id, window_id}
  → parse ids → view が adopt 済みか検査
  → workspace.begin_transfer(view, DockTarget::Window(w))
     → DockError::ViewBusy/UnknownView/UnknownWindow/InvalidTarget
       は ErrorBody へ写像
  → dock_moves[view_id] = Prepared { token, target: w }
  → result {prepared: true}

(op 間で任意の event — view は ViewBusy で編集 lock、
 UI binding はまだ source)

op dock_commit {view_id}
  → dock_moves から Prepared{token} 取得（無ければ
    AwaitingWindow 経路 or reject）
  → workspace.commit_transfer(token)
     → MoveReport{view, from, to} or TargetUnavailable
       (dock 側で自動 rollback — MoveAborted event)
  → 成功時: UI binding 付け替え
      controls.attach_view(session, view, to_window)
        （implicit detach(dock-transfer)+generation bump 内蔵）
      viz.attach_view(view, to_window)
      ui.views[view].window = to_window
      （shell.views は `ViewOp::DockCommit` 拡張により result
        の `to` から reconcile — 「shell bookkeeping」節参照）
  → dock_moves 除去 → ui_pump → notify
  → result {committed: true, from, to}（from/to は window_id
    文字列）
```

失敗経路: `commit_transfer` が `TargetUnavailable` → dock は
source に rollback 済み → `dock_moves` 除去 → UI binding は
source のまま無変更 → result error（`{code, message}`）。
`dock_abort {view_id}` → `abort_transfer(token)` →
`dock_moves` 除去 → UI 無変更 → `{aborted: true}`。

### Detach→Reattach（pool 経由）

```
dock_detach {view_id}
  → workspace.detach_to_pool(view)  (dock.detach_view)
  → UI: controls.detach_view(reason=unmount or dock-transfer)
        viz.detach_view; ui.views entry → pool 表現
  → shell.views owner クリア（observe_result Detach 拡張）
  → result {detached: true}

dock_reattach {view_id, window_id}
  → workspace.move_view(view, DockTarget::Window(w))
     (pool → window の one-shot、MoveReport{from:None,to:w})
  → UI: controls.attach_view(view, w) + viz.attach_view
        (pool view の attach は generation を新規 mint)
  → ui.views/shell.views 更新 → result {reattached: true,...}
```

### Window `Destroyed`（DetachViews 既定）

```
WindowLifecycle::Destroyed
  → on_destroyed(window):
      cancel_window_jobs(window)
      windows.remove(window)
      workspace.on_window_event(window, Destroyed)
        → dock.remove_window(window, DetachViews)
          → WindowRemoved{window, views} event
      drain_dock_events → views 一覧取得
      for each view:
        dispatch syn.view.detach {view_id, reason:"unmount"}
          (dock-aware 拡張により detach_to_pool も冪等に成立)
        shell.views owner クリア / ui.views → pool
      if driver.windows().is_empty() → driver.exit()
```

view は `ViewStateRegistry`（tool/viewport/selection）・
`ControlsUi`/`VizUi` の binding state（generation は detach で
bump、再 attach で新 epoch）・`ViewRegistry` descriptor を
保持したまま pool に残り、`dock_reattach` で復帰する。
Project/Graph/Recording/Job は `ProjectSession`/engine に属し
window 非依存 — 一切触れない。

## Non-Functional Constraints

- **RT 契約**: 全 op は non-RT（UI thread）— 既存 `syn.*` op と
  同じ。`MoveToken`/`DockWorkspace`/`WorkspaceManager` は
  non-RT only 契約を維持。
- **atomicity**: op 層の複数 step は同期 dispatch 内で不可分。
  window 生成のみ非同期 — `AwaitingWindow` + `dock_moves` が
  その pending を所有する。
- **wire 非直列化**: `MoveToken`/`WindowId`/`ViewInstanceId` の
  raw 値を client に露出するのは `window_id`/`view_id` 文字列
  のみ。token は app 内 map に留める。
- **依存方向**: `synvelis-app → synvelis-workspace →
  synvelis-project` を維持。`synvelis-workspace` は
  `ruviform-ui` contract のみ使用し webview/gpui adapter に
  依存しない（WS-A ADR
  `docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md`
  の制約）。
- **headless lane 保全**: `ShellWindowHost`/`HeadlessWindowHost`
  の既存 test（`tests/sv113_dock.rs`・`tests/two_window.rs`）は
  無変更で通ること。`syn.view.*` の additive 拡張は既存
  params/result を壊さない。

## Risks and Trade-offs

- **Risk: dock/UI drift**。`syn.view.*` 拡張と `dock_*` op の
  更新順序を誤ると両権威が乖離する。Mitigation: 更新を op 層の
  1 関数に集約（`apply_dock_move_commit` 的な helper）、
  fixture test で全 op 組合せの両権威一致を assert、
  `syn.window.list` + `syn.view.list` の観測面を検証に使う。
- **Risk: `AwaitingWindow` saga のリーク**。window 生成要求後に
  shell が応答しない（driver 不在等）と `dock_moves` が残る。
  Mitigation: `dock_abort` は `AwaitingWindow` からも合法、
  window `Destroyed`/timeout 相当の lifecycle event で pending
  を掃除、`syn.window.list` で pending 可視化。
- **Risk: close-veto 差分**。別 dispatch `ctx_2aed081be01f` が
  `shell.rs`/`service.rs` の `CloseRequested`/`respond_close`
  を変更中で main 未着の可能性。本設計は「veto record
  `WindowRecord.veto_close` を読む seam」までを規定し、
  `on_destroyed` の disposition flow は veto と直交するため
  両者は衝突しない想定 — merge 時に `on_windowed` の
  `CloseRequested` 分岐だけ差分調整が必要になり得る。
- **Risk: `ViewEntry`/`shell.views` の pool 表現変更**が
  `observe_result`・`syn.view.list`・`origin_view` 解決の
  既存前提を壊す。Mitigation: detach で owner をクリアするのは
  「pool view は command を mint できない」fail-closed 方向で
  あり安全側。既存 test（attach→detach→再 attach）の回帰を
  impl slice で確認。
- **Trade-off: one-shot vs two-phase**。`dock_reattach`/
  `dock_detach`/`syn.view.attach` の dock 更新は one-shot
  `move_view`/`detach_view` で十分（destination readiness が
  問題にならない既存 window/pool 間移動）。two-phase は
  「destination がまだ無い/準備中」の場合のみ必要 —
  `DockWorkspace` の契約と一致。
- **Trade-off: `adopt_view` kind/title の出所**。app は view_id
  → `ViewType`/title を composition root の view spec から
  既知とする。未知 view_id の attach は従来通り UI attach を
  試み dock 登録は skip…ではなく reject — 「UI attached だが
  dock 未登録」の状態を作らないため、adopt に必要な metadata
  が無い view_id は `invalid_params` で明示拒否する
  （silent skip は権威分界を壊す）。

## Deferred Design Concerns

- `WindowDriver` → `WindowHandle` adapter（driver-backed host）|
  生成権威は `WindowDriver` にあり adoption で要件充足 |
  `detach_to_new_window` convenience を製品で使う slice |
  GUI detach が shell orchestration のまま残る。
- `RelocateTo` disposition・layout snapshot/restore・monitor
  失踪復帰 | #177 owner 範囲 | #177/#114 着手時 | close 時の
  view 救済が detach-to-pool のみ。
- DPI/IME（`ScaleFactorChanged` の workspace 記録・座標系統一）|
  RF#388/#389/#114 | native evidence slice | #114 gate 時 |
  geometry 記録が physical px のまま。
- Plugin Dock（`ViewType::PluginDock` の実体）| #176 owner 範囲 |
  plugin host 実装 slice | vocabulary のみ存在。
- 実 OS 証拠（window 生成失敗・close・detach の native 観測）|
  #114 gate | env-gated native slice | headless Pass を実 OS
  Gate へ誤昇格する危険。

## Verification Notes

- [ ] `adopt_window`/`adopt_view` の synvelis-workspace 単体
  test（重複拒否・`next_view` 前進・pool/window target・
  `host.create_window` 非呼出し）。
- [ ] `syn.window.dock_prepare`→`commit` で `WorkspaceManager.
  dock.owner_of`・`shell.views`・`ui.views` が一致し
  `MoveCommitted` が drain される（synvelis-app fixture）。
- [ ] `dock_abort`・commit 時 `TargetUnavailable` で view が
  source ownership・UI binding を維持。
- [ ] `dock_prepare_new_window` + 生成失敗/Ready 前 Destroyed/
  attach 失敗で view が source に残り孤児 window が残らない。
- [ ] `Destroyed`（DetachViews）で view が pool へ退避し
  `syn.view.close` が replay されない。`ReleaseViews` 経路では
  view が除去される。
- [ ] `syn.view.attach` の dock-aware 拡張が既存
  `tests/two_window.rs`・`tests/sv113_dock.rs` を壊さない
  （additive 回帰）。
- [ ] env-gated native 証拠は `SYNVELIS_SECOND_WINDOW` 系 gate
  として後続 slice — 本 slice では NotRun と記録。
