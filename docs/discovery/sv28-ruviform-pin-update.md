---
id: "DISC-Zzd7iQXYxdgVA5gHuoryYx"
type: "discovery"
status: "resolved"
title: "SV#28 Ruviform pin 更新 — 863a7a9d..cc7d5296 差分調査と API 互換性評価"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
  verifies: []
  verified-by: []
  references:
    - "Cargo.lock"
    - "Cargo.toml"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 — 差分調査

## Summary

現行 pin `863a7a9dc132e4558cb18689dd80ac5288855bb0` から ruviform
`main` HEAD `cc7d5296d3fbc65ce9b925834de4dadf5db666d9` までの
15 commits を調査した。`crates/` 配下の変更は全て追加的で、
公開 API の削除・シグネチャ変更は確認されなかった
（`pub fn`/`pub struct`/`pub enum`/`pub trait`/`pub use` の
削除行ゼロ）。`ruviform-ui-webview` は無変更。したがって pin
更新に伴う SV 側の API 追従修正は原則不要の見込みであり、
検証は既存 lane の pass で担保する。

## Exploration Goal

pin 更新で consumer（synvelis）が踏む API 差分を特定し、追従
修正の要否と範囲を確定する。

## Findings

### 差分に含まれる主な変更（863a7a9d..cc7d5296）

| ruviform Issue | 内容 | consumer 影響 |
| --- | --- | --- |
| RF#464 | Consumer-neutral GUI Scenario Contract。`ruviform-ui-test` に `scenario` module（`SemanticGuiTest`/`FakeGuiTest`/`VisualGuiTest`）、`ruviform-ui` の `ViewHost::deliver_event`/`attach_epoch()`、`ruviform-ui-gpui` に `GpuiGuiTest`（`gpui-test` feature） | 追加のみ。新規テスト基盤として利用可能 |
| RF#314 slice1/2 | headless Pixel Golden Utility コア。`ruviform-ui-test` に `golden` module（key/diff/image/report/store）。`png`/`base64` 依存追加（いずれも既存 lock tree 内） | 追加のみ |
| RF#451 FU-1b | `ruviform-ui-gpui` で Tab/Shift-Tab を `focus_next`/`focus_prev` に配線 | 挙動追加。synvelis 側コード変更不要 |
| RF#243/#482 | GPUI 実機ライフサイクル probe（example + 実窓証拠） | example 追加のみ |
| RF#429 残 DoD | 長尺/RT seam 検証と NumericInput/Transport 再評価（test のみ） | なし |
| RF#465 P0/P1 | Legacy WebView 廃止の棚卸し・文書歴史化・API 分類（docs のみ） | なし。legacy API の物理削除は P2 で未実施 |
| RF#461/#463 | `ruviform-ui-gpui` の TimeAxis label O(n²) 解消・Spectrogram intensity 修正 | 内部修正 |
| RF#468 | `ruviform-io-file` RootedDir canonical 前提の doc 明記 | doc のみ |
| RF#478/#442/#443/#435/#436 | docs/検証記録 | なし |

### 互換性評価

- `ruviform-ui`: `ViewHost::deliver_event(&Event) -> bool` と
  `attach_epoch()` が追加。`AttachEpoch` が `Serialize`/
  `Deserialize` を derive。既存メソッドの削除・変更なし。
- `ruviform-ui-gpui`: `render.rs`/`observe.rs` の内部で wire
  `Event` の decode が `ViewHost::deliver_event` へ集約された
  （driver 内部リファクタ）。`GpuiDriver`/`ViewMount`/`version()`
  等 synvelis が使う公開面に変更なし。
- `ruviform-ui-webview`: diff なし。`SessionConfig`/
  `WebViewWindowConfig`/`run_multi`/`detect_backend`/`EmbedAssets`
  は不変。
- `ruviform-ui-test`: `golden`/`scenario` module と re-export
  追加。`FakeDriver`（synvelis の dev-dependency が使用）は
  無変更。
- `ruviform-io-file`: doc comment のみ。
- audio/graph/rt/media-*/plugin-*/viz/observability/exec/
  ui-theme: diff なし。

### synvelis 側の利用面

- `ruviform-ui` / `ruviform-ui-test`: `synvelis-app`（webview/
  native feature 含む）と `synvelis-workspace` が使用。
- `ruviform-ui-webview`: `synvelis` bin（webview feature）のみ。
  既に `SessionConfig`/`run_multi` へ移行済み（fa98f41）。
- `ruviform-ui-gpui`: `synvelis-native` bin（native feature）のみ。
- 残りの ruviform crate は headless lane（player/render/project/
  recording/workspace/library/theme/gateway 系）が使用し、
  今回の diff 範囲では無変更。

### Legacy API の確認

RF#465 の legacy WebView API（`HostConfig`/`run`/`HostEvent`/
`WindowConfig::url`/`devtools`/`WindowHandle::eval`）は P0/P1 で
棚卸し・分類のみ行われ、物理削除は P2 の follow-up。synvelis
webview bin は既に driver-neutral API 面を使用しており、削除済み
API への依存は現時点で存在しない（`check-app-webview` で最終確認）。

## Open Questions

- なし。pin 更新は `cargo update -p ruviform-*` で `main` HEAD を
  解決し、`check-ruviform-rev` の単一 SHA 契約で検証する。
