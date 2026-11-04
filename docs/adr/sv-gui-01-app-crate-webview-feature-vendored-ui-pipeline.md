---
id: "ADR-2kAD08Yeuxmx8U4t2mD2wr"
type: "adr"
status: "accepted"
title: "SV-GUI-01 app crate boundary・webview feature gate・vendored ui/* TS pipeline"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/110"
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
    - "docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md"
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/156"
    - "https://github.com/scarecrow173/ruviform/issues/257"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: []
    consulted: []
    informed: []
---

# SV-GUI-01 app crate boundary・webview feature gate・vendored ui/* TS pipeline

## Context and Problem Statement

Issue #110 は起動可能なデスクトップアプリ基盤を要求するが、既存の Synvelis
workspace は headless crate のみで、GUI 依存 (wry/tao/rust-embed) を持つ
クレートが無い。また確定方針の SolidJS frontend は ruviform の `ui/*`
plain TS パッケージ (`private` npm パッケージ・TS source 配布) を直接利用
する必要があり、npm registry/git-subdir 依存という仕組みが存在しない。

配置を誤ると (a) GUI 依存が `cargo check --workspace` の headless 検証経路を
壊す、(b) SolidJS が Rust core/CLI/RT や Ruviform へ逆流する、(c) frontend
asset と Rust host の revision が乖離したまま配布される、という既知の失敗
モードがある。

## Decision

1. **`crates/synvelis-app` を新設し、GUI 依存を `webview` feature に隔離する。**
   - lib は headless (Composition Root・`Service` 実装・asset 検査ロジック)。
   - bin `synvelis` は `required-features = ["webview"]`、
     `ruviform-ui-webview`/`rust-embed` は optional 依存。
   - AudioLens `al-gui` と同一形。`mise run check`/`lint`/`test` は
     feature 無しで headless のまま通り、webview lane は専用タスク
     (`check-app-webview` 等) で別検証する。
2. **ruviform `ui/*` TS パッケージは `ui/vendor/ruviform/` へ vendored 生成物
   として同期する** (`tools/sync-ruviform-ui.mjs`)。revision の正本は
   `Cargo.lock` の解決済み ruviform git revision — Synvelis は
   `Cargo.toml` rev pin ではなく既定ブランチ追従 + `Cargo.lock` 記録の
   契約 (AGENTS.md) のため、AudioLens 版スクリプトの pin 読み取りだけを
   `Cargo.lock` 読み取りへ変更する。vendor ディレクトリは gitignore 済み
   生成物で `.worktreeinclude` に登録する。
3. **frontend は `ui/` に SolidJS + TypeScript + vite で配置し、成果物は
   `ui/dist/` → `rust-embed` で埋め込む。** 本番/テスト/開発で同一 build
   artifact を使い、asset manifest (`synvelis-assets.json`) が
   `appSchema`/`ipcSchema`/`revision`/file hash を運ぶ。Rust host は起動時に
   照合し、不整合は説明付き安全停止。
4. **単一 Window は `ruviform-ui-webview::host::run` を使う。** `run_multi`
   (dock detach) は SV-GUI-02/03 の有効化対象。Service 層は `UiRuntime`
   を共有し headless dispatch と GUI dispatch を同一契約にする。

## Consequences

- `synvelis-project`/`synvelis-recording`/`synvelis-workspace`/
  `synvelis-gateway` に GUI/frontend 依存が一切入らない (逆流なしは依存
  検査で固定)。
- frontend の ruviform 参照は vendored 相対 TS import で、`package-lock.json`
  に ruviform 側 node_modules 依存は発生しない。SolidJS/vite/ツール依存は
  `ui/package.json` に閉じる。
- `Cargo.lock` の ruviform rev 更新時は `mise run sync-ui-deps` で vendor も
  追随させる (check 系タスクが `--check` で乖離を検出し失敗する)。
- RF-GT-03 test-only host control は pin rev に未実装のため存在しない前提で
  設計する。独自の制御ポートは増やさず、将来 ruviform 側に opt-in 経路が
  出たら bin 側で明示 opt-in するだけで済む構造に留める。

## Alternatives Considered

- **frontend assets を `include_bytes!`/`MapAssetSource` で手埋め
  (al-gui 方式そのまま)**: SolidJS 製 bundle は build 生成物で、手埋めは
  生成物管理と revision 照合を壊す。`rust-embed` の debug=FS/release=embed
  モデルが既に `ruviform-ui-webview` 側に用意されているため採用した。
- **npm workspace + git 依存で `@ruviform/ui-*` を参照**: パッケージは
  `private: true` で registry 配布がなく、npm は git リポジトリの
  subdirectory 依存を解決できない。vendored 生成物が唯一の確実な経路
  (audio-lens で実績あり)。
- **`synvelis-gui` 命名**: `synvelis-app` を選んだ。GUI shell だけでなく
  Composition Root (Project/Runtime/Recorder/Observability の組立) を担う
  app crate であり、名称が責務に一致するため。
