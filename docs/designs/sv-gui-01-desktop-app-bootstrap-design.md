---
id: "DESIGN-lW54zR76NmEm8cc3NlMskX"
type: "design"
status: "approved"
title: "SV-GUI-01 Design — Desktop App Bootstrap・Composition Root・Frontend Asset Pipeline"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/workspace-docking-state-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
    - "docs/designs/overview.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md"
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/156"
    - "https://github.com/scarecrow173/ruviform/issues/181"
    - "https://github.com/scarecrow173/ruviform/issues/257"
    - "https://github.com/scarecrow173/ruviform/issues/312"
  defers: []
  deferred-by: []
---

# SV-GUI-01 Design — Desktop App Bootstrap・Composition Root・Frontend Asset Pipeline

## 概要

`synvelis-app` は Synvelis デスクトップアプリの Composition Root と GUI
entrypoint を担う app crate。headless な lib 層 (app 組立・`Service` op・
asset bundle 検査) と `webview` feature のみで有効化する wry/tao host bin
を持つ。frontend は `ui/` の SolidJS + TypeScript SPA で、vite production
build → `rust-embed` で single-binary 配布する。

依存方向:

```
synvelis (bin, webview feature)
  → synvelis-app (lib, headless)
      → synvelis-workspace (ProjectSession/WorkspaceManager)
      → synvelis-project  (ProjectController/ProjectStore)
      → synvelis-recording (Recorder slot の型)
      → ruviform-ui       (UiRuntime/Service/AssetSource/DataRegistry)
      → ruviform-ui-webview (optional: host::run/EmbedAssets/detect)
      → rust-embed        (optional: dist embed)
```

GUI/frontend 依存はこのチェーンの末端に閉じ、core crate へ逆流しない。

## Composition Root (`app.rs`)

`SynvelisApp` がアプリ全体の所有権を束ねる。Window ごとの正本複製を防ぐため
唯一の `ProjectSession` を保持し、Service 層・frontend の双方がここを経由する。

```rust
pub struct SynvelisApp {
    session: Option<ProjectSession>,   // project.new/open までは None
    store: Option<DirectoryBundleStore>,
    workspace: Option<WorkspaceManager<ShellWindowHost>>,
    audio: AudioRuntimeSlot,           // 実 device backend なし → stub 状態を報告
    recorder: RecorderSlot,            // synvelis-recording session 状態 (Idle)
    observability: ObservabilityOwner, // ruviform-observability counter ペア
}
```

- `AudioRuntimeSlot`: `enum { HeadlessOnly }`。synvelis-player の
  `HeadlessDriver` は存在するが実 device backend crate は無いため、
  capability では `audio.device: unavailable`・`audio.headless_driver:
  available` と正直に報告する。実 backend 接続は別 Issue。
- `RecorderSlot`: `SessionState` (Idle) を保持。録音開始等の実 op は
  SV-GUI-02 以降。SV-GUI-01 では slot の存在と状態報告のみ。
- `ObservabilityOwner`: `ruviform_observability::counter()` で intent/job
  カウンタを保有し、`syn.app.info` が読み出す。本格的な process owner は
  Observability #81 系の後続 Issue。

`ShellWindowHost` は `ruviform_ui::window::WindowHandle` を実装する app 内
adapter: 単一 Window 構成では `create_window`/`destroy_window`/
`respond_close` は `BackendUnavailable` (multi-window 未有効) を返し、
`eval`/`resize`/`focus`/`monitors`/`windows` は単一 Window id に写像する。
`WorkspaceManager::new(host, session)` の構築経路を headless 契約のまま
使えるのが目的で、dock detach 実機配線 (SV-GUI-02/03) で差し替える。

## Service 表面 (`service.rs`)

`AppService: ruviform_ui::Service`。op は `syn.*` namespace、`sys.*` は
`UiRuntime` 組込 (sys.ping/sys.capabilities/job.cancel) をそのまま使う。
全 op は `CallOutcome::Result` (同期応答) — 長時間 job は SV-GUI-02 以降。

| op | params | result | 責務 |
|---|---|---|---|
| `syn.app.info` | — | versions/backend/os/arch/capabilities/observability counters | 起動・診断情報。frontend capability 表示の正本 |
| `syn.project.new` | `{name?}` | project id/name/revision | 新規 `Project` + `ProjectSession` 組立。workspace は unmount に戻る |
| `syn.project.open` | `{path}` | project id/name/revision/saved head | `DirectoryBundleStore` open → `controller.load` |
| `syn.project.save` | — | saved head/revision | `controller.save(store)`。project 未 open 時は `unavailable` |
| `syn.project.info` | — | id/name/revision/saved/track・lane 数等 | 読み出し専用 projection |
| `syn.transport.set` | `{playing: bool}` | transport state | `ProjectSession::set_playing` (stub だが実契約) |
| `syn.transport.state` | — | `{playing, position_ticks}` | 読み出し |
| `syn.workspace.mount` | — | `{mounted: true, views: []}` | `WorkspaceManager` を session 上に組立 (冪等) |
| `syn.workspace.state` | — | mounted/dock/view 数 | mount point の状態投影 |

エラーの code は `rvf.ipc.v1` の stable codes (`invalid_params`/
`service_error`/`unavailable`) を使い、message に内部 path を漏らさない。

## Host entrypoint (`bin/synvelis.rs`, `webview` feature)

起動シーケンス (各段で fail-fast、説明付き stderr + 非ゼロ終了):

1. `detect_backend()` — `wry::webview_version()` probe。`Unavailable` なら
   install hint (WebView2 Evergreen) を出して exit 2。
2. asset bundle 構築 + 検査 — `AppAssets` (rust-embed `frontend/dist`) から
   `synvelis-assets.json` を読み `check_bundle` で整合照合。失敗は
   `AssetError` の説明を出して exit 3 (Window は開かない = 安全停止)。
3. `SynvelisApp::new` → `AppService` → `UiRuntime::new(DataRegistry)` +
   `register_service`。
4. `host::run(&HostConfig, assets, registry, on_event)` —
   `HostConfig { title: "Synvelis", url: "rvf://app/index.html", tick:
   16ms, devtools: cfg!(debug_assertions), data_dir }`。
   `on_event` は `rt.handle(event)` の `Event` 群を
   `dispatch_script` で `host.eval` に渡すだけの薄いループ。
   `HostEvent::Ready` で初期 `notify` (app state projection) を push。
5. `CloseRequested` → runtime が job cancel + `lifecycle closing` を
   返し、host loop が exit → `main` は exit 0。`request_close` は
   frontend の Quit メニュー (`sys` ではなく `syn.app.quit` op →
   `host.request_close`) 経路。

CLI 引数: `synvelis [--project <path>]` (起動時に `syn.project.open` 相当を
実行) / `--data-dir <path>` (WebView2 UDF 固定・既定は process-unique temp)。

## Frontend (`ui/`)

### 構成

```
ui/
  package.json          # @synvelis/app-frontend (private)
  package-lock.json
  tsconfig.json         # solid preset: jsx preserve + solid-js import source
  vite.config.ts        # base "./", outDir "dist", solid plugin, manifest emit
  index.html            # <div id="app"> + CSP meta
  src/
    main.tsx            # registerRvfElements() + <App/> mount
    ipc.ts              # window.__rvf typed client (call/onEvent/dataUrl)
    types.ts            # op result 型 (wire schema mirror)
    store.ts            # createStore: connection/project/transport/caps/errors
    App.tsx             # shell composition + error boundary
    components/
      MenuBar.tsx       # File(New/Open/Save/Quit)・View(Capabilities)
      ErrorBanner.tsx   # intent/job error surface
      ProjectGate.tsx   # project.new/open form (path 入力)
      WorkspaceMount.tsx# mount point panel (syn.workspace.mount state)
      TransportBar.tsx  # rvf-transport CE 統合 (SV-GUI-02 接続点の実例)
      StatusBar.tsx     # revision/backend/connection
    adapters/
      ruviform.ts       # CE property/event helper (typed ref setter)
  test/
    env.ts              # happy-dom bootstrap (audio-lens と同型)
    ipc.test.ts         # bridge call/onEvent/dispatch 契約
    store.test.ts       # store reducer 契約
    app.test.tsx        # mount/transport CE/error banner smoke
  vitest.config.ts      # happy-dom env + vite-plugin-solid (browser 解決)
  vendor/ruviform/ui/   # tools/sync-ruviform-ui.mjs 生成物 (gitignore)
```

`tools/sync-ruviform-ui.mjs` は AudioLens 版を port し、pin 読み取りを
`Cargo.toml rev` から **`Cargo.lock` の `ruviform-*` source git revision**
へ変更する (Synvelis は既定ブランチ追従契約)。packages は
`ui/{core,components,binding,viz}`。

### SolidJS ↔ Custom Elements 統合契約 (adapter 層)

- `main.tsx` で `registerRvfElements()` を 1 回呼ぶ (import 副作用なし・
  冪等は Ruviform 側が保証)。
- JSX 内では `<rvf-transport ref={...} />` として `ref` 経由で **typed
  property** を代入する (object property を文字列 attribute へ JSON 化
  しない — Issue #110 明記の禁止事項)。helper `setProp(el, key, value)` を
  `adapters/ruviform.ts` に置き、SolidJS の `createEffect` → property 代入の
  一方向更新に限定する (signal → property でフィードバックループを作らない)。
- CustomEvent (`rvf-transport`/`rvf-gesture`) は `onMount` で addEventListener、
  `onCleanup` で removeEventListener。event は op 呼出へ translate するだけで
  SolidJS store を直接 mutate しない (receipt は `__rvf.onEvent` notify が
  正本 — 二重 command 防止)。
- remount/dispose: CE connect/disconnect はブラウザ任せ、Solid 側は
  `onCleanup` で listener と ref を解放するだけ。

### Store / IPC

`store.ts` は `createStore` で以下を保持 (Project/Graph の第二正本にしない —
全て op 応答と notify の投影):

- `connection`: `connecting | ready | error`
- `capabilities`: `sys.capabilities` + `syn.app.info` の統合 projection
- `project`: `{id,name,revision,saved} | null`
- `transport`: `{playing, position_ticks}`
- `workspace`: `{mounted, viewCount}`
- `errors`: 直近の ErrorBody 列 (banner 表示用)

`ipc.ts` は `window.__rvf` の型付けだけ (存在しない環境 = ブラウザ dev では
`call` が `unavailable` で reject)。dispatch される Event は `result`(call
promise 解決) と `notify`/`job`/`lifecycle` (onEvent → store 投影) に分岐。

## Asset bundle 契約

frontend build 時に `ui/dist/synvelis-assets.json` を生成:

```json
{
  "appSchema": 1,
  "ipcSchema": 1,
  "revision": "<ruviform-lock-sha12>-<build-id>",
  "files": { "index.html": "sha256:...", "assets/app-xxxx.js": "..." }
}
```

- `appSchema` (`APP_ASSET_SCHEMA = 1` 定数と照合): Synvelis asset 契約の
  version。manifest 形状・必須エントリが変わったら bump。
- `ipcSchema`: frontend が想定する `rvf.ipc.v1` SCHEMA_VERSION。host は
  `ruviform_ui::ipc::SCHEMA_VERSION` と照合。
- `files`: `synvelis-assets.json` 自身を除く dist 全 file の sha256。
  host は `AssetSource.get` で各 file を取り sha256 を再計算して照合
  (missing → `AssetMissing`、hash 不一致 → `AssetCorrupt`)。
  **release build は hash 完全一致を強制、debug build は存在確認のみ**
  (debug は filesystem passthrough で手編集を許す)。
- `revision`: 表示用 (app info に露出)。gate ではない。
- 起動時に `index.html` + manifest が無い/parse 不可 → `AssetError` で
  説明 + 安全停止。offline 起動 = 全参照が `rvf://app` 完結。

## 権限・セキュリティモデル

- **Asset origin**: `rvf://app` は `AssetSource` のみ。`resolve_path` の
  traversal/absolute 拒否をそのまま利用。`rvf://data` は `DataRegistry`
  の bounded pull (既定 16 MiB chunk)。
- **Navigation allowlist**: `rvf://app`(+ Windows の `http://rvf.app`
  rewrite) と `about:blank` のみ (`ruviform-ui-webview` 既定挙動。Synvelis
  側で拡張しない)。
- **IPC**: `window.ipc.postMessage` の string payload → `parse_intent`
  (size cap・version・kind・op 名検証)。`sys.*` 予約と `syn.*` のみ。
- **CSP** (`index.html` meta): `default-src 'self'; script-src 'self';
  style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'
  rvf: http://rvf.app http://rvf.data`. WebView2 custom scheme は
  `http://rvf.*` に rewrite され、`rvf://data` pull path は別 origin
  (`rvf.data`) なので両 scheme/host を明示許可。外部 origin への
  fetch/XHR/WS は CSP で拒否。
- **WebView 権限**: devtools は debug build のみ (`cfg!(debug_assertions)`)。
  WebView2 UDF は process-unique temp (既定) または `--data-dir` 固定。
  test-only の制御 port は **設けない** (RF-GT-03 未実装; release artifact
  に test control が混入しない構造を維持)。

## SV-GUI-02 への binding 接続点 (文書化)

1. **op 表面**: `syn.workspace.mount`/`state` が `WorkspaceManager` の mount
   点。SV-GUI-02 は view registry→`rvf-view` 相当の mount をこの op 経路の
   延長に追加する。Project mutation は `syn.project.*` ではなく SV-GUI-02 で
   定義する `syn.edit.*`/`syn.transport.*` 拡張へ。
2. **notify 経路**: `HostEvent::Ready` で `notify` push する `app state
   projection` に revision を含める — frontend store は revision guard 済み
   projection (stale notify を seq で捨てる) の接続点を持つ。
3. **CE 統合**: `adapters/ruviform.ts` の property/event helper は
   knob/fader/seekbar へ拡張可能な汎用形。`rvf-gesture` intent→binding
   経路 (`ui/binding` の `BindingPort` 実装) は SV-GUI-02 で `ui/binding`
   vendored package を使って実装する。
4. **run_multi**: dock detach を入れる時点で `host::run` → `session::run_multi`
   へ移行し `UiRuntime::handle_window`/`drain_jobs_routed` 経路を有効化。
   `ShellWindowHost` は `WindowHandle` port を実装済みなので
   `WorkspaceManager` 側は無変更。

## 検証戦略

- **headless (cargo test, feature 無し)**: `AppService` op 契約
  (project new/open/save/info round-trip via tempdir bundle store、
  transport、workspace mount、capabilities)、`check_bundle` 受理/拒否
  行列、`ShellWindowHost` 振る舞い、`UiRuntime` 経由の dispatch 同一性。
- **frontend (vitest + happy-dom)**: ipc client 契約、store reducer、
  `<rvf-transport>` mount/typed property/CustomEvent、error banner、
  SolidJS mount/cleanup。runner は **vitest** を採用する — `node --test`
  では `solid-js/web` が `node` condition で server (SSR) build に解決
  され client `render` を実行できない上、`.tsx` の JSX を直接実行
  できないため。vite browser 解決 + vite-plugin-solid で実 client
  render を検証する (Issue #110 の「テストライブラリの選定は実装で
  判断」に基づく)。
- **webview feature lane**: `cargo check/clippy/test --features webview`
  (型整合のみ。実 window は立てない)。
- **実 GUI (Windows 実機)**: `cargo run -p synvelis-app --features webview`
  で起動 → Window Shell 表示 → メニュー/Project gate 動作 → 終了。実測は
  Windows のみ、macOS/Linux は「未検証」。
- **逆流検査**: `synvelis-app` 以外の crate が `ruviform-ui-webview`/
  `rust-embed`/`solid-js` を参照しないことを依存検査 (grep + cargo tree)。

## 未検証範囲 (本 Issue 完了時点で残るもの)

- macOS (WKWebView) / Linux (WebKitGTK) の実機起動。
- Multi-window dock detach の実機動作 (`run_multi` 未使用)。
- RF-GT-03 test host control / RF-GT-04 driver 経路 (ruviform 未実装)。
- 実 audio device backend・実 plugin host UI・実 DSP UI。
- WebView2 runtime が存在しない環境での配布 (probe が説明を出すのみ)。
