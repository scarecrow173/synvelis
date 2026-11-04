---
id: "IMPL-034Rx4ef0v8Ch5IBl2s39L"
type: "impl"
status: "completed"
title: "SV-GUI-01 desktop app bootstrap webview frontend pipeline"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app"
      - type: "file"
        path: "ui"
      - type: "file"
        path: "tools/sync-ruviform-ui.mjs"
      - type: "file"
        path: "tools/check-ruviform-rev.mjs"
      - type: "file"
        path: "tools/check-headless-deps.mjs"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "mise.lock"
      - type: "file"
        path: ".gitignore"
      - type: "file"
        path: ".worktreeinclude"
      - type: "file"
        path: "docs/tasks/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
      - type: "file"
        path: "crates/synvelis-app/frontend/dist"
  implements:
    - "docs/tasks/sv-gui-01-task6-verification-impl-record.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md"
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
    - "docs/designs/sv-gui-01-desktop-app-bootstrap-design.md"
    - "docs/plans/sv-gui-01-desktop-app-bootstrap.md"
    - "docs/test-specs/sv-gui-01-e2e-desktop-bootstrap.md"
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

# SV-GUI-01 desktop app bootstrap webview frontend pipeline

## Summary

Issue #110 (SV-GUI-01) として、起動可能な Synvelis デスクトップ
アプリケーションの基盤を実装した。`crates/synvelis-app` が
composition root(`SynvelisApp` + `AppService` の `syn.*` op
面)を担い、実窓 host は Ruviform 既存の `ruviform-ui-webview`
を `webview` feature で接続。frontend は `ui/` の SolidJS +
TypeScript + Vite pipeline で、Ruviform の plain TS Custom
Elements を vendored `ui/*` パッケージ経由で直接利用する。
headless(core/recording/workspace/CLI 系)は webview 非依存の
まま、`syn.*` op 契約は GUI/headless 同一。

実 Windows 起動検証済み: `synvelis.exe` が `Synvelis` 窓を開き、
WebView2 が frontend bundle を描画、asset manifest 照合
(`asset bundle ok`)を通過し、窓 close で exit 0。

## Context

Spec: docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md
ADR: docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md
Design: docs/designs/sv-gui-01-desktop-app-bootstrap-design.md
Plan: docs/plans/sv-gui-01-desktop-app-bootstrap.md
Test spec: docs/test-specs/sv-gui-01-e2e-desktop-bootstrap.md
Tasks: docs/tasks/sv-gui-01-task{1..6}。

対象 Issue: synvelis #110。後続 #111(IPC/binding 接続)・#112 が
本基盤に依存する。AudioLens #105/#127 の al-gui 実装を先例として
採用した。

## Implementation

### Changes

- `crates/synvelis-app/Cargo.toml`: workspace member 追加。
  `ruviform-ui-webview`・`rust-embed` は optional、`webview`
  feature・bin `synvelis` の `required-features` で隔離。
  headless の lib/test は wry/tao 非依存。
- `build.rs`: `Cargo.lock` から `ruviform-ui` git source の
  解決 SHA を抽出し `SYNVELIS_RUVIFORM_REV` env で焼き込み
  (runtime は永久 SHA 直書きせず lockfile を正本とする)。
- `src/app.rs`: `SynvelisApp` — `WorkspaceManager<ShellWindowHost>`
  + 起動時 fresh `ProjectSession`(untitled project)+
  `ObservabilityOwner` + backend report + audio/recorder slot +
  `Arc<AtomicBool>` quit flag を束ねる composition root。
  `DataRegistry` は host/runtime で共有する単一 clone。
- `src/service.rs`: `AppService`(`Service` trait 実装)。
  `syn.app.info`/`syn.app.capabilities`/`syn.app.quit`、
  `syn.project.new/open/save/info`、`syn.transport.set`、
  `syn.workspace.mount`。error は `ErrorBody`/`service_error` に
  写像し observability counter を加算。`syn.app.quit` は共有
  flag を立て、host loop が `request_close` に変換する。
- `src/revision.rs`: `AssetManifest`(schema/host rev/frontend rev/
  per-file bytes+SHA-256)検証。schema・revision・IPC 互換・
  欠落・digest 不一致は `AssetError` に分類。`build.rs` 焼き込み
  rev と `synvelis-assets.json` の `host` を比較する。
- `src/assets.rs`: `EmbedAssets<UiAssets>`(rust-embed、
  `frontend/dist`)+ 検証失敗時に bundle を一切配信しない自己
  完結の CSP 制限付きエラーページ。
- `src/host.rs`: `ShellWindowHost` — `WindowHandle` の shell 窓
  実装(eval/resize/focus/close 記録)、headless でも同一経路。
- `src/bin/synvelis.rs`: `webview` feature 入口。backend probe →
  asset bundle 検証 → `UiRuntime` + `AppService` 登録 →
  `ruviform_ui_webview::run`。bundle 不合格は generated error
  page で説明して安全停止せず誠実に表示(仕様上の安全側)。
- `tests/service_ops.rs` 12 件: op 正常系/必須 param/
  capabilities 正直表示/store 無し save 失敗/quit flag/
  wire(`rt.handle`)と `dispatch_op` の同一契約、等。
- `tests/asset_check.rs` 10 件: manifest 欠落/破損/rev・schema・
  IPC・digest 不一致/unsafe path 全てが説明され、valid bundle は
  通過。
- `ui/`: `ipc.ts`(`window.__rvf` request/response 相関・
  event listener・`rvf://data` URL)、`store.ts`(応答/イベントを
  UI projection に畳む SolidJS store、Rust が唯一の domain 正本)、
  `adapters/ruviform.ts`(`@ruviform/*` Custom Elements の型付き
  prop/event ヘルパ)、App Shell コンポーネント群
  (MenuBar/ErrorBanner/ProjectPanel/TransportBar/StatusBar/
  app.tsx/main.tsx/styles.css)。
- `tools/sync-ruviform-ui.mjs`: `Cargo.lock` の ruviform 解決 SHA
  と同一 rev で `ui/vendor/ruviform/ui/{core,components,binding,
  viz}` をローカル cargo checkout から再生成(`--check` 照合、
  `REV` ファイル記録)。
- `ui/scripts/write-manifest.mjs`: vite build 後に
  `synvelis-assets.json`(host rev + per-file digest)を出力。
- `tools/check-ruviform-rev.mjs`: lockfile SHA 単一性・vendor REV・
  frontend manifest host の三方照合(fail-closed)。
- `tools/check-headless-deps.mjs`: 8 headless crate の dep tree に
  GUI/WebView framework(`ruviform-ui-webview` 含む)が無いこと
  を検証。
- `mise.toml`: `node = "26.12.1"` pin、sync/setup/check/test/
  build-ui、check/lint/test-app-webview、dev-ui、build-app、
  check-ruviform-rev、check-headless-deps タスク。
  `mise.lock`: node 7 platform entry 追加。
- `.gitignore`/`/.worktreeinclude`: `ui/node_modules`・`ui/vendor`・
  `frontend/dist/*` は生成物として除外(`dist/.gitkeep` のみ
  追跡して fresh checkout の embed folder を保証)、vendor と
  dist は子 worktree へコピー。

### Decisions During Implementation

- `syn.app.quit` は共有 `Arc<AtomicBool>` 経由。`UiRuntime` が
  service を move するため、host loop が観測できる quit 状態は
  外部共有が唯一の形。
- frontend テストは `vitest + happy-dom`。`solid-js/web` は Node
  export condition で SSR build になるため `node --test` では
  client render 出来ず(設計段階で検証済み)。
- CSP: `connect-src 'self' rvf: http://rvf.app http://rvf.data` —
  `rvf://data` は webview 側で `http://rvf.data` origin に
  書き換わるため両 origin を許可。design 初版で `rvf.data` 抜けを
  指摘され修正。
- RF-GT-03 test control port は不採用。upstream 未実装かつ
  release 成果物に test 制御を含められない(受入の opt-in 経路
  要件は「存在すれば」の条件付きと解釈)。
- vendored `ui/*` は npm registry ではなく Cargo.lock 解決 SHA の
  ローカル cargo checkout から生成。npm パッケージ未公開のため
  rev 一致を保証できる唯一の経路(AudioLens 同型)。
- `Package` 名 `@ruviform/ui-components` と vendor dir 名
  `components` の差は vite alias regex(`^@ruviform/ui-(.*)$`)
  で吸収。
- rust-embed `#[folder]` は `frontend/dist` 直下(AudioLens の
  `..` 相対ではなく crate 内収容)。fresh checkout 対応で
  `dist/.gitkeep` を追跡。
- merge 競合: `mise.toml` は upstream の
  `rust = { version, components }` 形と node pin を union、
  `docs/tasks/README.md` は open index の行を union。
  `mise.lock` は auto-merge で rust options + node entries 共存。

## Related Experiments

### Adopted

なし(exp ログ未作成 — 先例 al-gui の確立パターンを踏襲)。

### Rejected

- `node --test` による frontend component テスト:
  `solid-js/web` の Node resolution が SSR build で client
  render 不可。vitest(Vite browser resolution)を採用。
- RF-GT-03 control port: upstream 未実装・release 混入禁止。
- npm registry 経由の `@ruviform/*` 依存: 未公開・rev 不一致
  リスクのため vendored checkout を採用。

## Validation

実施済み(merged tree `7ac830b`、Windows 実機):

| lane | result |
| --- | --- |
| `mise run fmt-check` | pass |
| `mise run check` | pass(workspace 全 crate headless) |
| `mise run lint` | pass(`-D warnings`) |
| `mise run test` | pass(70 suites、0 failed) |
| `mise run build` | pass(`--locked`) |
| `mise run check-ui` | pass(tsc --noEmit + vendor sync 照合) |
| `mise run test-ui` | pass(vitest 16/16、3 files) |
| `mise run build-ui` | pass(vite build + manifest 生成) |
| `mise run check-ruviform-rev` | pass(`073a3ff6…`、22 lockfile sources、REV match) |
| `mise run check-headless-deps` | pass(8 headless crate に GUI framework 無し) |
| `mise run check-app-webview` | pass |
| `mise run lint-app-webview` | pass |
| `mise run test-app-webview` | pass(service_ops 12 + asset_check 10) |
| `prek run --all-files` | pass(全 hook) |
| 実 GUI 起動(Windows) | pass — `Synvelis` 窓表示、WebView2 動作、`asset bundle ok (frontend @synvelis/ui@0.1.0+7ac830b85129)`、窓 close で clean exit |

### 未検証(正直な記録)

- macOS/Linux の実 GUI 起動 — 未検証(本機 Windows のみ。
  WebKitGTK/`WebKit` backend 経路は ruviform 側実装に従うが
  実機未確認)。
- `build-app`(release embed)lane — タスク定義済み・未実行
  (release ビルドは時間がかかるため debug で検証。embed path
  は `cargo check --features webview` で型・embed 解決のみ
  確認済み)。
- 再起動/クラッシュ復帰・multi-window — 本 Issue スコープ外
  (single shell window)。
- `rvf://data` pull の実測 — adapter は実装済み・GUI 上での
  end-to-end pull は SV-GUI-02 以降の画面接続時に検証。

## Risks

- `rvf://data` origin rewrite(`http://rvf.data`)に依存した CSP。
  upstream の rewrite 規約が変わると connect-src が不整合になる
  (check-ruviform-rev は rev 照合のみ;CSP 規約の自動照合は無い)。
- `ui/vendor/` は生成物。`sync-ui-deps` はローカル cargo git
  checkout 依存のため、checkout が無い環境では cargo が先に
  fetch する必要がある(通常の cargo build 前提で充足)。
- WebView2 プロセス数の実測は環境依存(本機では 60 プロセス
  確認 — システム共用含む)。起動判定は窓 title + asset ok +
  exit code で実施済み。
- `dist/.gitkeep` のみの bundle は起動時 manifest 欠落で
  エラーページ表示 → 安全側だが「空 bundle で起動緑」には
  ならないよう意図的に fail-closed。

## Follow-ups

- #111: `syn.*` op 面への binding/viewmodel 接続(本 IR の
  `store.ts` projection と `adapters/ruviform.ts` が接続点)。
- #112+: Workspace mount/transport 表示の具体業務 View 実装
  (App Shell の panel 領域)。
- `build-app` release lane の CI 化と macOS/Linux 実 GUI 検証
  (Product Gate 側の論点)。
- RF-GT-03 が upstream 実装された場合の opt-in test host
  経路追加(release 非含有契約は維持)。
