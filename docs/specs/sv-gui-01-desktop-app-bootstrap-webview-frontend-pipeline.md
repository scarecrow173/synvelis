---
id: "SPEC-ZdwjK7WkdHy90Oaw1mTGUy"
type: "spec"
status: "approved"
title: "SV-GUI-01 Desktop Application Bootstrap — WebView Host・Frontend Asset Pipeline"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/110"
    - "https://github.com/scarecrow173/synvelis/issues/23"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/workspace-docking-state-architecture.md"
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/overview.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/156"
    - "https://github.com/scarecrow173/ruviform/issues/257"
    - "https://github.com/scarecrow173/ruviform/issues/312"
    - "https://github.com/scarecrow173/synvelis/issues/111"
  defers: []
  deferred-by: []
---

# SV-GUI-01 Desktop Application Bootstrap — WebView Host・Frontend Asset Pipeline

## What Should Be Built

Issue #110: **起動可能な Synvelis デスクトップアプリケーションの基盤**を構築
する。Headless PoC 群(VS-A〜X, WS-A, PC-A/B, GW-*)を本番画面と取り違えず、
Synvelis 所有の App entrypoint・Application Shell・実 Window/WebView asset
bundle・開発/本番 assets loading・起動時エラー/Capability 提示・終了/再起動・
配布前提の build 手順を用意する。

確定方針(Issue #110 ・ Ruviform #257 の 2026-11-04 更新):

- **Frontend は SolidJS + TypeScript**。Application Shell と View Component
  Tree を SolidJS で実装し、Ruviform の plain TS Custom Elements
  (`ui/components`) を直接利用する。SolidJS 依存は Synvelis UI/Build 範囲に
  閉じ、Ruviform 標準/Runtime/Showcase と Rust core/CLI/RT へ逆流させない。
- **Ruviform `ruviform-ui-webview` (wry/tao Host) と Window/IPC lifecycle を
  再利用**し、Host や Dock・Guest 間 IPC を複製しない。`rvf.ipc.v1` の
  Intent/Event schema、`UiRuntime`、`Service` ポート、`AssetSource`/
  `DataRegistry` の pull path をそのまま採用する。
- **Rust App Host は Composition Root** として `ProjectController`/
  `ProjectSession`・Audio Runtime・Recorder・Observability Process Owner を
  組み立て、Window ごとに別 Project/Collector/Plugin Engine 正本を生成しない。
  Frontend は ViewModel/IPC Adapter のみ、Project/Graph への直接 Mutable
  Access は禁止。
- 本番とテストで**同一 frontend build artifact** を使用し、test/fixture
  backend は明示的に差し替える。Test Host は Ruviform RF-GT-03 (#312) の
  opt-in 経路のみ利用する方針 — pin 済み ruviform rev (`073a3ff`, 現在
  main) には RF-GT-03 実装が存在しないため、本 Issue では独自のテスト制御
  ポートを追加せず、headless (UiRuntime/Service 層) 検証と実機起動検証に
  留める。

初期画面: Window Shell・基本メニュー/エラー表示・Project 起動/読み込み口・
Workspace mount point。具体業務 View/操作は SV-GUI-02 (#111) / SV-GUI-03
(#112) が担う。

## Why Now

Execution DAG #23 で本 Issue は並行 GUI トラックの最初の開始点
(#110→#111→#112→#113→#114)。Ruviform 側では #156 (GUI Host foundation)、
#181 (multi-window host contract)、#182 (Dock Tree)、#257〜#263
(framework-neutral UI core/components/binding/viz/showcase) が main に
マージ済みで、pin 済み rev `073a3ff` (= 現在 main) に全て含まれる。
AudioLens 側でも `al-gui` crate + vendored `ui/*` TS lane の消費形態が確立
済みで、同一境界を Synvelis に適用できる。

## Users and Value

- DAW ユーザー: インストール/起動/終了できる実アプリケーションの土台。
  起動時に backend capability と未実装機能が正確に表示され、asset 破損時は
  説明付きで安全停止する。
- 開発者: GUI/CLI/headless が同一の `UiRuntime`/`Service`/`ProjectController`
  契約を通ることが検証で固定され、SV-GUI-02 の binding 接続点が明文化される。

## Scope

### In Scope

1. 新クレート `crates/synvelis-app`:
   - headless な lib (Composition Root・`Service` 実装・asset bundle 検査)。
   - `webview` feature 時のみビルドされる bin `synvelis` (wry/tao host
     entrypoint)。既定 feature は headless のまま保ち、`mise run check`/
     `lint`/`test` が GUI 依存なしで通る。
2. `ui/` SolidJS + TypeScript frontend:
   - SolidJS Application Shell (menu bar・error boundary・project gate・
     workspace mount point・status bar)。
   - `window.__rvf` の薄い型付き IPC adapter (ViewModel のみ)。
   - `tools/sync-ruviform-ui.mjs` による ruviform `ui/*` vendored 同期 —
     解決 revision の正本は `Cargo.lock` (Synvelis は既定ブランチ追従 git
     依存のため)。`ui/vendor/` は生成物として gitignore。
   - vite production build → `ui/dist/`。テストは `node --test` + happy-dom
     (ruviform/audio-lens と同一 convention)。
3. Asset bundle 契約: frontend build が埋め込む manifest
   (`appSchema`/`ipcSchema`/`revision`/file listing) と、Rust 側起動時の
   revision/schema 整合照合。missing/invalid asset 時は説明付きで安全停止
   (Window を開く前に終了)。
4. Service op 表面 (`syn.*` namespace): app info/capabilities・project
   new/open/save/info・transport set/state・workspace mount。`sys.*` は
   `UiRuntime` 組み込みのまま使う。
5. CSP・`rvf://` origin・navigation allowlist・IPC 経路の明示 (design 文書に
   権限モデルを記述)。
6. 配布前提の build 手順: `mise run` タスクで `ui` 依存復元 → vendored 同期 →
   typecheck/test/build → `--features webview` ビルドまで一連で通る手順と
   検証タスク。

### Out of Scope

- 全 DAW 画面 (Arrangement/Mixer/Inspector 等の業務 View は SV-GUI-02/03)、
  Signal Flow (#108)、AI 機能、Native Renderer 採否 (ruviform #262)、
  製品 Gate A 全体合格。
- Multi-window/dock detach の実機配線 (`run_multi` は #181 で存在するが、
  SV-GUI-01 の初期 Window は単一。SV-GUI-02/03 で有効化)。
- RF-GT-03 由来でない独自 Test Host Control ポート。実 GUI E2E harness は
  SV-GUI-05 (#114) と ruviform #310/#313。
- 実 audio device backend・実 plugin host UI・実 DSP UI — 未実装分は
  capability で `unavailable` と正直に報告する。
- Ruviform 側への変更 (SolidJS 依存や Synvelis 固有 op を逆流させない)。
- macOS/Linux の実機検証 (本機は Windows。他 OS は「未検証」として明記)。

## Requirements

- `synvelis-app` は `synvelis-project`・`synvelis-workspace`・
  `synvelis-recording`・`ruviform-ui` に依存し、`ruviform-ui-webview` と
  `rust-embed` は `webview` feature 時のみの optional 依存とする。
- GUI 依存 (wry/tao/rust-embed/SolidJS) は `synvelis-app` の webview lane と
  `ui/` に閉じる。`synvelis-project`/`synvelis-recording`/`synvelis-workspace`/
  `synvelis-gateway` 等の core crate へ新規 GUI 依存を加えない。
- Frontend は `Project`/`ProjectController`/graph へ直接触れず、全操作を
  `window.__rvf.call` (`rvf.ipc.v1` intent) 経由で `syn.*` op に委譲する。
- 起動時に backend probe (`wry::webview_version`) → asset bundle 検査 →
  host loop の順で fail-fast し、どの段階の失敗でも人が読める説明と
  非ゼロ終了コードで停止する。
- 本番 bundle/lockfile/asset と Rust host の revision 整合を起動時に照合する
  (`appSchema`・`ipcSchema`・file hash)。offline 起動 (全 asset が `rvf://`
  内で完結し外部 URL を参照しない)。
- Window ごとに Project/Collector/Engine 正本を作らない — Composition Root が
  唯一の `ProjectSession` を所有する。
- lockfile (`Cargo.lock`・`ui/package-lock.json`) を追跡し、依存解決を
  再現可能にする。

## Acceptance Criteria

Issue #110 本文の Acceptance を検証可能な形に分解したもの。

- [ ] **SV-GUI-01-A**: `webview` feature でビルドした `synvelis` binary が
  実 Windows 上で起動・Window 表示・正常終了できる (dev build と
  production profile 双方。実測範囲を明記し、headless 証拠と区別する)。
- [ ] **SV-GUI-01-B**: GUI/CLI/headless が同一 `UiRuntime`/`Service`/
  `ProjectController` 契約を通る — `syn.*` op が headless `UiRuntime`
  dispatch と webview host 経由で同一結果を返し、GUI 依存が core/RT crate
  へ逆流しない (依存検査で示す)。
- [ ] **SV-GUI-01-C**: asset bundle の `appSchema`/`ipcSchema`/file listing
  が host 期待と照合される。manifest 欠落・parse 不可・schema 不一致・
  必須 asset 欠落の各経路で説明付き安全停止。全 asset が `rvf://` 完結で
  外部 URL 依存なし (offline 起動)。
- [ ] **SV-GUI-01-D**: `sys.capabilities`/`syn.app.info` が backend
  (webview2 等)・OS/arch・op 一覧・audio/recorder/observability/plugin の
  有無を正確に返し、未実装分は `unavailable` 表示。Ruviform 側に framework
  必須依存なし (vendored `ui/*` は plain TS のまま)。
- [ ] **SV-GUI-01-E**: 実 GUI 実測 (起動/終了/Window Shell 表示) と headless
  証拠を分けて報告し、未検証 OS/機能を `未検証` と明記。SV-GUI-02 の
  binding 接続点 (op 表面・Custom Element 統合・mount point) を design 文書
  に固定する。
- [ ] **SV-GUI-01-F**: SolidJS Component Tree 上で `rvf-transport` 等の
  Reference Custom Elements が mount/update/dispose・typed property・
  CustomEvent で動作することを frontend テストで検証し、Ruviform 側に
  SolidJS 依存を追加していない。

## Source Evidence

- Issue #110 本文 (Acceptance が正本)・Issue #23 Execution DAG。
- Ruviform `ruviform-ui`/`ruviform-ui-webview` crate 実装
  (`UiRuntime`・`Service`・`host::run`・`EmbedAssets`・scheme handler)。
- AudioLens `al-gui` crate・`ui/` vendored lane・`tools/sync-ruviform-ui.mjs`
  の消費前例 (同一 maintainer・同一 ruviform pin 契約)。
