---
id: "PLAN-v1VQEMk92Nx1lQHUlSVh1t"
type: "plan"
status: "approved"
title: "SV-GUI-01 実装計画 — Desktop App Bootstrap・WebView Host・Frontend Asset Pipeline"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source: []
  implements:
    - "docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md"
  implemented-by: []
  depends-on:
    - "docs/plans/ws-a-workspace-foundation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md"
    - "docs/designs/sv-gui-01-desktop-app-bootstrap-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-01-e2e-desktop-bootstrap.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-01 実装計画 — Desktop App Bootstrap・WebView Host・Frontend Asset Pipeline

## Goal

`synvelis-app` crate (headless lib + `webview` feature bin) と `ui/` SolidJS
frontend を新設し、Issue #110 の SV-GUI-01-A〜F acceptance を検証可能にする。
Ruviform `ruviform-ui-webview` host・`UiRuntime`/`Service`/`rvf.ipc.v1` 契約と
vendored `ui/*` plain TS Custom Elements をそのまま再利用する。

## Non-Goals

- 業務 View (Arrangement/Mixer/Inspector)・dock detach・実 E2E driver・
  独自 test-control port・実 audio device/plugin host UI。
- Ruviform 側・既存 core crate の API 変更。
- macOS/Linux 実機検証。

## Scope / File Map

単一実装ストリーム (Rust crate と frontend が asset manifest と op 表面で
結合しており分断検証できない)。

| 領域 | パス |
|---|---|
| app crate | `crates/synvelis-app/{Cargo.toml,src/{lib,app,service,assets,revision,error,host}.rs,src/bin/synvelis.rs,tests/}` |
| frontend | `ui/{package.json,package-lock.json,tsconfig.json,vite.config.ts,index.html,src/**,test/**,vendor/}` |
| vendor tool | `tools/sync-ruviform-ui.mjs` |
| 検証 | `tools/check-headless-deps.mjs` (GUI 依存逆流検査) |
| build | `Cargo.toml` (member/dep 追加)・`Cargo.lock`・`mise.toml` (node tool + ui 系 task)・`.gitignore`・`.worktreeinclude` |

## Upstream Documents

- docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md
- docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md
- docs/designs/sv-gui-01-desktop-app-bootstrap-design.md

## Task Breakdown (ordered)

### task1 — `synvelis-app` crate skeleton + Composition Root

- `Cargo.toml` member 追加、`ruviform-ui` workspace dep は既存。
  `ruviform-ui-webview`・`rust-embed` を workspace deps に追加
  (`ruviform-ui-webview` は git dep、`rust-embed = "8"` crates.io)。
- `synvelis-app` lib: `error.rs` (`AppError`/`AssetError`)、`app.rs`
  (`SynvelisApp` + `AudioRuntimeSlot`/`RecorderSlot`/`ObservabilityOwner`)、
  `host.rs` (`ShellWindowHost`)、`assets.rs` (`AppAssets` rust-embed +
  dev fs fallback)、`revision.rs` (`check_bundle` + `AssetManifest` 型)。
- `service.rs`: `AppService` op 実装 (`syn.app.info`/`syn.project.*`/
  `syn.transport.*`/`syn.workspace.*`/`syn.app.quit`)。

### task2 — headless service/bundle 検査テスト

- `crates/synvelis-app/tests/service_ops.rs`: op dispatch 契約
  (tempdir `DirectoryBundleStore` で new→save→open→info round-trip、
  transport、workspace mount、capabilities、quit の host intent flag)。
- `tests/asset_check.rs`: manifest parse・schema 不一致・file 欠落・
  hash 不一致・正常受理の行列。

### task3 — `ui/` frontend pipeline + vendor sync

- `tools/sync-ruviform-ui.mjs` (Cargo.lock rev 読み取り版)・
  `ui/package.json`/`tsconfig.json`/`vite.config.ts`/`index.html`・
  `src/{main.tsx,ipc.ts,types.ts,store.ts,App.tsx,components/*,adapters/*}`。
- `mise.toml`: `node` tool・`sync-ui-deps`/`setup-ui`/`check-ui`/`test-ui`/
  `build-ui`/`check-app-webview`/`lint-app-webview`/`test-app-webview`/
  `run-app` タスク。`.gitignore` (ui/vendor・dist・node_modules)・
  `.worktreeinclude` (`ui/vendor/`)。

### task4 — frontend headless test

- `ui/test/env.ts` + `ipc.test.ts`・`store.test.ts`・`app.test.tsx`
  (SolidJS mount・`rvf-transport` CE・error banner)。

### task5 — webview bin + 実機起動検証

- `src/bin/synvelis.rs` (backend probe → bundle check → host::run)。
- `mise run check-app-webview`/`lint-app-webview`/`test-app-webview` で
  feature lane 検証後、実 Windows で `mise run run-app` 起動・終了。

### task6 — 最終検証 + impl record

- `git fetch origin` → `git merge origin/main` → 生成物再生成 →
  `mise run` 全系統実行 → `docs/impl/ir/` 記録。

## Risks / Mitigations

| Risk | Mitigation |
|---|---|
| vendored `ui/*` が Cargo.lock rev と乖離 | check/test 系タスクが `sync --check` で fail-fast。乖離時は `sync-ui-deps` を促す |
| `rust-embed` `#[folder]` の crate 外参照が不安定 | vite `build.outDir` を `crates/synvelis-app/frontend/dist` に固定し `#[folder = "frontend/dist"]` (EmbedAssets doc と同形・crate 内完結)。frontend source は `ui/` に残り成果物位置だけ crate 内にする |
| happy-dom が SolidJS の DOM 要求を満たさない | ruviform/audio-lens と同一 env を使い、solid `render` smoke のみ。失敗時は component test を mount 契約 (props/event) 単位へ縮小 |
| WebView2 未 install | `detect_backend` の install hint 経路を起動検証で確認 |
| Windows 固有 `http://rvf.app` rewrite と CSP 衝突 | CSP meta に `rvf:` と `http://rvf.app` を併記、実機で navigation を確認 |

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` (headless・feature 無し)
- `mise run check-ui` / `test-ui` / `build-ui` (frontend)
- `mise run check-app-webview` / `lint-app-webview` / `test-app-webview`
- `mise run run-app` (実 Windows GUI 起動・終了、手動観測)
- `node tools/check-headless-deps.mjs` (逆流検査)

## Rollback

新規 crate・新規 `ui/`・`mise.toml` task 追加のみで既存コードの破壊的変更を
含まない。`Cargo.toml` member から外せば全体を revert できる。
`Cargo.lock` の ruviform rev 更新は発生しない見込み (現在 pin = main)。

## Implementation Handoff

task1〜2 (Rust lib+test) と task3〜4 (frontend) は op 表面 (design §Service)
を介して独立に実施可能だが、同一 worktree で逐次実施する。実機 GUI 検証は
全実装完了後に一括で行う。
