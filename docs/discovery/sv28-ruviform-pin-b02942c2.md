---
id: "DISC-034Vo8SrwAKrqHNijuTiCD"
type: "discovery"
status: "resolved"
title: "SV#28 Ruviform pin 更新 — 0f5b5737 → b02942c2 差分調査と API 互換性評価"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv28-ruviform-pin-update.md"
    - "docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
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

現行 pin `0f5b57372e9a43f84415f7964b2f9636774174f0` から ruviform
`main` HEAD `b02942c2f9e6e195d0d752803b077866468fe58e` までの
80 commits を調査した。synvelis が消費する 26 の `ruviform-*`
crate のうち変更があったのは 10 crate で、公開 API の削除・
シグネチャ変更は実質確認されなかった（唯一の root re-export
削除は `ruviform-ui-test` の `scenario::visual` 3 item で、
item 自体は `scenario::visual::*` path で存続し synvelis は
未使用）。新規 crate `ruviform-midi-device` 等は synvelis の
依存木に含まれない。したがって pin 更新に伴う SV 側の API
追従修正は不要の見込みであり、検証は既存 lane の pass で担保する。

## Exploration Goal

pin 更新で consumer（synvelis）が踏む API 差分を特定し、追従
修正の要否と範囲を確定する。あわせて新 API（`real-scenario`/
`ProbeSourceAdapter`/`ReportDrain`/observability opt-in feature
等）の採用判断材料を揃える。

## Findings

### 差分に含まれる主な変更（0f5b5737..b02942c2、80 commits）

| ruviform Issue | 内容 | consumer 影響 |
| --- | --- | --- |
| RF#253 | plugin→host report の host-graph consumer 配線。`ruviform-plugin-host` に `ReportDrain`/`PluginHost::drain_output_reports`/`InProcessHost::drain_output_reports`（pull 型・non-RT）、`ruviform-plugin-core` に `PluginProcessor::drain_output_reports` default method、`ruviform-plugin-vst3` の VST3 MIDI 正規経路、`ruviform-midi-device`（OS MIDI input backend、Windows WinMM）新設 | 追加のみ。`ReportDrain`/`drain_output_reports` は opt-in の pull API で、synvelis は未採用。`ruviform-midi-device`/`ruviform-plugin-vst3` は依存外 |
| RF#293 / OBS-11 | `ruviform-audio` に `typed::probe` module（`ProbeSourceAdapter`）と統合 edge 強化、`ruviform-observability` に `ObservabilityConfig`/`ObservationRunner`（`integration` feature） | 追加のみ。`typed::probe` は新規 module、observability の新機能は opt-in feature で未有効化 |
| OBS-03/08/09/10 | `ruviform-observability` に `tracing-export`/`allocation-probe`/`integration` opt-in feature、`tracing_export` module、Diagnostics/Evidence/Finding/Health/Report、CountingAllocator/AllocationScope、debug 基盤 | 追加のみ。synvelis は `logging` feature のみ使用、新 feature は off のまま |
| RF#314 / RF#388 / RF#552 | `ruviform-ui-gpui` に `real_window` module・`real-scenario` feature（`RealOsGuiTest`）、SendInput preflight、native dock slice 3（ancestor clip/focus-key routing/2-phase re-dock）、`gpui_driver::{scenario,os_input,capture,observe}` 拡張 | 追加のみ。`real-scenario`/`os_input`/`real_window` は opt-in feature・module で synvelis の `gpui` feature 組合せでは未有効化。dock slice 3 は内部挙動改善 |
| RF#464 / RF#314 slice5-6 | `ruviform-ui-test` に scenario runner/report 組立・Golden 環境正規化・fixture registry・consumer-shaped semantic oracle fixtures、driver-lab fixtures | 追加のみ。root re-export `scenario::visual::{CaptureOutcome, VisualGuiTest, record_golden_step}` が削除されたが item は `scenario::visual::*` で存続し synvelis は未使用 |
| Issue #410 | `ruviform-ui` の projection revision floor（epoch またぎ stale projection 防止）と view command ledger bounded retention/correlation hardening | 内部修正。`view.rs`/`behavior.rs` の diff は大きいが `pub` item の削除・シグネチャ変更なし（consumer が使う `ViewHost`/`ViewSpec`/`ComponentSpec`/`ProjectionOp` 等は不変） |
| Issue #418 | `ruviform-ui-theme` に `pub mod generated`（DTCG token → ResolvedTheme codegen） | 追加のみ |
| Issue #389 | `ruviform-plugin-host` editor session hardening（request deadline・明示 degrade・GraphicsLost）と新 export（`DEFAULT_REQUEST_TIMEOUT`/`SessionClock`/`SessionDeadlinePolicy`/`OpenOutcome`/`MonotonicSessionClock`） | 追加のみ。既存 export は不変 |
| IO-02/03/04/06、Issue #251/#306 | `ruviform-io-structured`/`io-compression`/`io-archive`/`io-e2e`、`ruviform-audio-device` 等の新規 crate・MSRV lane | いずれも synvelis の依存木外（lockfile に非出現） |
| docs/ci/mise | ruviform 側の設計文書・CI 配線・consumer-verify 追跡 | なし |

### 互換性評価

- `ruviform-ui`: `view.rs`/`behavior.rs` に Issue #410 の内部
  修正（693/184 diff 行）があるが、削除行に `pub fn`/`pub
  struct`/`pub enum`/`pub trait`/`pub use` の宣言はなく、
  synvelis が import する公開面（`ViewHost`/`ViewSpec`/
  `ComponentSpec`/`AttachEpoch`/`ProjectionOp`/`TAB_SELECT_ACTION`/
  `behavior::*`/`dock::*`/`driver::*`/`ipc::*`/`window::*`）の
  シグネチャは不変。
- `ruviform-ui-gpui`: `gpui_driver` 配下に `real_window`/
  `scenario`/`os_input`/`capture`/`observe` の拡張と新 feature
  `real-scenario`。既存 `pub use` は `OsCapturedFrame` →
  `{OsCapturedFrame, OsWindowGeometry}` への拡張（追加）。
  `pub(crate) fn view_of` の削除は crate 内部で consumer 影響なし。
- `ruviform-ui-test`: `golden`/`scenario` module の拡張。root
  re-export 3 item の削除は module path 経由で存続、synvelis の
  使用面（`FakeDriver`/`FakeGuiTest`/`scenario::{GuiAction,
  SemanticGuiTest, TargetQuery, parity_gesture_scenario}`）は不変。
- `ruviform-plugin-host`: `ReportDrain` と `drain_output_reports`
  系の追加、editor session 系の新 export 追加。既存
  `PluginHost`/`PluginHandle`/`HostCommand`/`HostReply`/
  `HostError`/`EndpointBounds` は不変。
- `ruviform-plugin-core`: `PluginProcessor::drain_output_reports`
  default method 追加（実装側の強制なし）。
- `ruviform-observability`: opt-in feature 3 件追加。synvelis は
  `features = ["logging"]` のみで新 feature は off。
- `ruviform-audio`: `typed::probe`（`ProbeSourceAdapter`）新規
  module。`ruviform-audio-core`: `midi_input` 周辺の追加的変更。
  `ruviform-audio-plugin-bridge`: `cap.rs` の追加的変更。
- `ruviform-ui-theme`: `generated` module 追加。
- `ruviform-ui-webview`: diff なし。`SessionConfig`/
  `WebViewWindowConfig`/`run_multi`/`detect_backend`/`EmbedAssets`
  は不変。
- audio-analysis/dsp/exec/graph/io-file/io-symphonia/io-wav/
  math/media-*/numeric/rt/viz/video-core: diff なし。

### synvelis 側の利用面

- `ruviform-ui` / `ruviform-ui-test`: `synvelis-app`（webview/
  native feature 含む）と `synvelis-workspace` が使用。
- `ruviform-ui-webview`: `synvelis` bin（webview feature）のみ。
  driver-neutral API 面は不変。
- `ruviform-ui-gpui`: `synvelis-native` bin（native feature）のみ。
  `gpui` feature のみ有効化、`real-scenario` は未使用。
- `ruviform-plugin-host`/`plugin-core`/`audio-plugin-bridge`/
  `audio`/`audio-core`: `synvelis-player` 等 headless lane が使用。
- `ruviform-observability`: `synvelis-app` が `logging` feature
  で使用。
- `ruviform-ui-theme`: `synvelis-theme` が使用。
- 残りの ruviform crate は headless lane が使用し、今回の diff
  範囲では無変更。

### 影響表（feature/schema/project/graph/RT/plugin/GUI/recorder/
automation/export）

| 軸 | 評価 |
| --- | --- |
| feature | 影響なし。`Cargo.toml` の依存・feature 宣言は不変。ruviform 側の新 feature（`real-scenario`/`tracing-export`/`allocation-probe`/`integration`）は opt-in で未有効化 |
| schema | 影響なし。永続化 format・IPC schema の変更は diff 範囲に確認されず |
| project | 影響なし。`ruviform-graph`/`media-*`/`io-*` は無変更 |
| graph | 影響なし。`ruviform-graph` 無変更 |
| RT | 影響なし。`ruviform-rt` 無変更。`drain_output_reports` は明示的に non-RT 契約。`rt_` lane で回帰検証 |
| plugin | additive のみ。`ReportDrain`/`drain_output_reports`/editor session deadline は新規 API で既存呼出面は不変。実 plugin/実 VST3 検証は NotRun |
| GUI | additive + 内部修正。Issue #410 の stale projection 修正は consumer 挙動を改善方向に変える可能性があるが API は不変。実 GPUI/実 OS 検証は NotRun |
| recorder | 影響なし。`synvelis-recording` が使う `ruviform-rt`/`audio-core` は無変更〜追加的 |
| automation | 影響なし。該当 crate に実質 diff なし |
| export | 影響なし。`ruviform-io-wav`/`media-*`/`io-symphonia` 無変更 |

### 新 API の採用判断

- `ruviform-midi-device`（OS MIDI input backend）: **今回は採用
  しない**。synvelis の依存木に含める変更は pin 更新の範囲外。
  実 MIDI device 入力の consumer 統合が必要になった時点で
  後続 Issue 候補。
- `ruviform-ui-gpui` の `real-scenario`/`real_window`/
  `os_input`: **今回は採用しない**。opt-in feature で有効化すると
  実 OS 入力・実窓 capture の検証 lane が必要になり、本 slice の
  範囲を超える。GUI 実機検証の後続 Issue 候補。
- `probe::source::ProbeSourceAdapter`（`ruviform-audio` typed::
  probe）: **今回は採用しない**。probe source の consumer 配線は
  別タスクの判断事項。
- `plugin-host::ReportDrain` / `drain_output_reports`: **今回は
  採用しない**。plugin→host report の consumer 配線は
  synvelis-player の host 層設計判断が必要で、pin 更新に混ぜない。
- `ruviform-observability` の opt-in feature
  （`tracing-export`/`allocation-probe`/`integration`）:
  **今回は採用しない**。`logging` のみの現行構成を維持。

## Open Questions

- なし。pin 更新は `cargo update -p ruviform-*` で `main` HEAD を
  解決し、`check-ruviform-rev` の単一 SHA 契約で検証する。

## Sources and Relations

- Issue #28（Ruviform 継続追従 tracker。managed コメントは編集しない）。
- 先行 pin 更新: PR #193（`0f5b5737` への更新 + doc chain）。
- pin 更新規約: `docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md`、
  `tools/check-ruviform-rev.mjs`（Cargo.lock が唯一の正本）。
- ruviform 差分範囲: `0f5b5737..b02942c2`（80 commits、
  `git log` で確認。公開 API の削除は `ruviform-ui-test` root
  re-export 1 件のみで item は存続）。

## Evidence

- `git ls-remote https://github.com/scarecrow173/ruviform.git
  refs/heads/main` → `b02942c2f9e6e195d0d752803b077866468fe58e`
  （2026-11-11 確認）。
- `git diff Cargo.lock`（本 worktree）: 26 箇所全てが
  `ruviform-*` の `source = "git+...#<sha>"` fragment の
  `0f5b5737…` → `b02942c2…` 更新。他パッケージの version
  drift なし。`ruviform-midi-device` は lockfile に非出現。
- crate 差分集計（`~/.cargo/git/checkouts/ruviform-*/` の
  0f5b573/b02942c checkout 間 `diff -rq`）: 変更ありは
  ui-test(14)/ui-gpui(13)/observability(12)/plugin-host(9)/
  ui-theme(7)/audio(5)/ui(2)/plugin-core(2)/
  audio-plugin-bridge(2)/audio-core(2) の 10 crate、
  残り 16 crate は無変更。crate 追加は `ruviform-midi-device`
  のみ。
- `pub` 削除行スキャン: `pub fn`/`pub struct`/`pub enum`/
  `pub trait`/`pub use` の削除は `ruviform-ui-test` の
  `scenario::visual` re-export 1 件と `ruviform-ui-gpui` の
  `capture::OsCapturedFrame` 単独 re-export（`OsWindowGeometry`
  追加への書き換え）のみ。`pub(crate)` の `view_of` 削除は
  内部項目。
- synvelis 側の `VisualGuiTest`/`CaptureOutcome`/
  `record_golden_step`/`OsCapturedFrame`/`BusReport`/
  `InProcessHost`/`ReportDrain`/`ProbeSourceAdapter`/
  `ruviform-midi-device` 参照 grep: `crates/` `ui/src` に
  一致なし。
