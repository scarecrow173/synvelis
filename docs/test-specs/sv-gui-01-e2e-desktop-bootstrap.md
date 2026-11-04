---
id: "TSPEC-A8sVfBvTuQextSVcPY8yp9"
type: "test-spec"
status: "approved"
title: "SV-GUI-01 検証仕様 — App Bootstrap・Service 契約・Asset Pipeline"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source: []
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
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv-gui-01-desktop-app-bootstrap-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-01 検証仕様 — App Bootstrap・Service 契約・Asset Pipeline

検証本体: `crates/synvelis-app/tests/service_ops.rs`・
`crates/synvelis-app/tests/asset_check.rs`・`ui/test/*.test.ts(x)`
(vitest + happy-dom; `node --test` では `solid-js/web` client render を
解決できないため)・実 Windows 起動確認。headless 証拠と実 GUI 実測は
分けて記録する。

## Purpose

起動可能なデスクトップアプリ基盤が「GUI/CLI/headless 同一 Service 契約」
「asset bundle 整合→安全停止」「capability 正確報告」の各不変条件を満たす
ことを保証する。SV-GUI-02 が接続する mount/binding 点がここで固定される。

## Feature: SV-GUI-01-B Service op 契約 (headless)

### Rule: `syn.*` op は `UiRuntime` dispatch で一貫した結果を返す

- `syn.app.info` は versions・backend 名・os/arch・capabilities
  (audio/recorder/observability/plugin の有無)・observability counter を含む。
- `syn.project.new` → `syn.project.save` → `syn.project.open` →
  `syn.project.info` が tempdir `DirectoryBundleStore` で round-trip し、
  revision・saved head・id が一致する。
- `syn.project.save`/`info` は project 未 open 時 `unavailable`。
- `syn.transport.set` → `syn.transport.state` が状態を往復。
- `syn.workspace.mount` は冪等で `mounted: true` を返し、
  `syn.workspace.state` が投影を返す。project 再 open で mount は解除。
- `syn.app.quit` は quit 要求 flag を返す (host が `request_close` へ写像)。
- `parse_intent` 不良 body・未知 op・invalid params は rvf.ipc.v1 の
  stable code で失敗する。

### Rule: 同一 Service を GUI dispatch (`host::run` on_event) と headless
`UiRuntime::handle` で共有する

- `HostEvent::Message` → `rt.handle` → `Event` の往復が headless でも同じ
  応答列を生成する (bin と test が同じ `rt.handle` 経路を使う)。

## Feature: SV-GUI-01-C Asset bundle 整合・安全停止

### Rule: 起動時に manifest 整合を照合し、不整合は安全停止

- `synvelis-assets.json` 欠落 → `AssetError::ManifestMissing`。
- manifest parse 不可 → `ManifestInvalid` (説明文字列付き)。
- `appSchema`/`ipcSchema` が host 期待と不一致 → `SchemaMismatch`。
- `files` 列挙のいずれかが `AssetSource` に無い → `AssetMissing(path)`。
- `files` 列挙の hash が実体と不一致 → `AssetCorrupt(path)`。
- 全一致 → `Ok(BundleInfo)` で `index.html` 必須確認も通る。
- `index.html` が files 未収録/未解決 → `index.html missing`。

## Feature: SV-GUI-01-F Frontend (SolidJS + vendored CE)

### Rule: `window.__rvf` 経路と store が wire schema を守る

- `ipc.call` は `{v:1,kind:"intent",id,op,params}` を postMessage し、
  `result.ok` で resolve、`error` で code 付き reject。
- `onEvent` は `job`/`notify`/`lifecycle`/`error` を受け取り `result` は
  受け取らない。
- store は project/transport/capabilities/workspace/errors を
  `sys.capabilities` + `syn.*` 応答と notify から投影する (第二正本に
  しない — op 応答と notify のみが入力)。

### Rule: Ruviform Custom Elements が SolidJS 管理下で動作する

- `registerRvfElements()` 後に `<rvf-transport>` が mount され、
  `ref` 経由の typed property 代入 (object を attribute JSON 化しない)
  と `rvf-transport` CustomEvent listener/cleanup が機能する。
- error banner は ErrorBody を表示し dismiss で除去できる。
- ProjectGate は `syn.project.new`/`syn.project.open` intent を発行し、
  応答で store が更新される。

## Feature: SV-GUI-01-A/D/E 実 GUI・capability (実機検証・手動観測)

- 実 Windows で `synvelis` binary が起動し Window Shell (menu bar・
  project gate・workspace mount・status bar) を表示する。
- File → New/Open/Save/Quit が動作し、Quit で正常終了する。
- Capabilities 表示が backend=webview2・os=windows・op 一覧・
  audio.device=unavailable・plugin=unavailable を正確に示す。
- 実測 OS は Windows のみ。macOS/Linux・multi-window dock・実 device/
  plugin・RF-GT-03 系は `未検証` として報告に明記する。

## Guarantees

- GUI/CLI/headless が同一 `UiRuntime`/`Service`/`ProjectController` 契約を
  通り、`synvelis-app` 以外の crate に GUI/frontend 依存が入らない。
- asset 不整合・missing 時は Window を開く前に説明付き非ゼロ終了。
- frontend は `__rvf.call` 経由のみで Project/Graph に触れ、直接 mutable
  access を持たない。

## Non-goals

- 業務 View・dock detach・実 device/plugin・RF-GT-03 control・実 E2E
  driver・macOS/Linux 実機・multi-monitor 実測。

## Risk

- happy-dom が SolidJS render 要件を完全に満たさない可能性。component test
  は mount/property/event/cleanup 契約に留め、ブラウザ差異は実機検証へ。
- `rvf-transport` の gesture→intent 変換は SV-GUI-02 の binding 層が正本。
  本仕様では transport CE の mount/typed property/CustomEvent のみを検証。
