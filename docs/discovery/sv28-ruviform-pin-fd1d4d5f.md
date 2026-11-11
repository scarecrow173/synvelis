---
id: "DISC-E2h7ArIlocpw5psCMM5ytg"
type: "discovery"
status: "resolved"
title: "SV#28 Ruviform pin 更新 — 9865cd57 → fd1d4d5f 差分調査と API 互換性評価"
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
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv28-ruviform-pin-update.md"
    - "docs/tasks/sv28-ruviform-pin-b02942c2.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/plans/sv28-ruviform-pin-fd1d4d5f.md"
  verifies: []
  verified-by: []
  references:
    - "Cargo.lock"
    - "Cargo.toml"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
    - "crates/synvelis-recording/src/input.rs"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 — 差分調査

## Summary

現行 pin `9865cd57491f3f39061212a85f81f9ebce731bde` から ruviform
`main` HEAD `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` までの
6 commits（PR #566〜#571）を調査した。synvelis が消費する 27 の
`ruviform-*` crate のうち変更があったのは 4 crate
（`ruviform-audio-device`・`ruviform-audio-core`・
`ruviform-audio-plugin-bridge`・`ruviform-ui-gpui`）で、
`crates/*/src/` 全体の diff に `pub` item の削除・シグネチャ変更は
**1 件もない**（`pub(crate)` の内部シグネチャ変更のみ）。公開面は
全て additive。したがって SV 側の API 追従修正は不要であり、検証は
既存 lane の pass で担保する。

## Exploration Goal

pin 更新で consumer（synvelis）が踏む API 差分を特定し、追従修正の
要否と範囲を確定する。あわせて PR #567 の WASAPI input
`frame_position` 本線化が `synvelis-recording` に与える実機効果と、
新 API（`real-scenario`・`OsInputOutcome`・bridge unsupported
計数透過等）の採用判断材料を揃える。

## Findings

### 差分に含まれる主な変更（9865cd57..fd1d4d5f、6 commits）

| ruviform PR / Issue | 内容 | consumer 影響 |
| --- | --- | --- |
| PR #567 / Issue #565 | `ruviform-audio-device` に `#[cfg(windows)] mod wasapi_input`（約 1,700 行）を新設し、WASAPI-native input path が per-packet `IAudioCaptureClient::GetBuffer` device position を `InputCallbackInfo::frame_position` へ供給。cpal の portable callback が運べない値の platform gap-fill。`windows`/`windows-core` 0.62 を target-gated dep として追加（cpal と同版で lockfile 新規 package なし）。`device.rs`/`stream.rs`/`host.rs`/`convert.rs` は `pub(crate)` 内部経路の配線変更のみ | **実機効果あり・コード変更不要**。`synvelis-recording/src/input.rs` の `resolve_first_frame` は `info.frame_position` を優先経路として実装済みで、Windows 実機録音で本線化する（synvelis#115 が起票した上流修正）。`ruviform-audio-device` は `real-device` feature 配下の optional dep のため既定 build には入らない |
| PR #568 / Issue #251 Stage W | `ruviform-audio-device` の WASAPI callback timestamp 修正と実機 lane 証拠化 closeout。`tests/real_input_e2e.rs` 拡張 | 追加的。callback timestamp 経路は `frame_position` 経路に影響しない（input.rs 内の既存コメントどおり）。実機 lane 証拠は ruviform 側 evidence |
| PR #566 / Issue #314 RF-GT-05 | `ruviform-ui-gpui` の `gpui_driver::capture` に OS-window pixel capture の X11/Windows backend（`PrintWindow`/`BitBlt`・`GetImage`）、`surface`/`real_window` 拡張、Linux offscreen golden。optional dep `x11rb`（linux target-gated・gpui 由来で lockfile 既存）と dev-dep `env_logger` 追加 | 追加のみ。synvelis は `gpui` feature のみ有効化で `real-scenario` は未使用。`x11rb` は optional・linux-gated で lockfile の依存リストに既存 version で追加されるのみ |
| PR #569 / Issue #313 RF-GT-04 S1 | `ruviform-ui-gpui` の `gpui_driver::os_input` に `pub enum OsInputOutcome`（`families()` getter 付き）を追加。RealOsPointerOrKeyboard capability + Windows 実入力 E2E 証拠化 | 追加のみ。`os_input` は `gpui` feature 配下の `#[cfg(windows)]` 内部実装。実 OS 入力 lane の consumer 配線は別判断 |
| PR #570 / Issue #243 | Windows multiwindow verdict 再実走（v2）closeout — capability matrix/doc status 整合、`real_window.rs` の内部更新、evidence 整備 | 追加のみ。公開面差分なし |
| PR #571 / Issue #253 residual | `ruviform-audio-plugin-bridge` の unsupported 計数透過 — `pub unsupported_backend: u64`/`pub backend_unsupported: u64` field と `pub const fn unsupported_backend()` getter を status 系に追加、`status.rs` で atomic counter を mirror。`ruviform-audio-core` の `midi_input::mock` に `pub fn inject_unsupported`。`midi_wiring` 実機 E2E を `#[ignore]` opt-in lane 化（dev-dep `ruviform-midi-device`、consumer 依存木外） | additive。unsupported 計数の diagnostics/UI 消費は本 slice では採用しない。`ruviform-midi-device` は dev-dep 経由のみで synvelis lockfile に非出現 |

### 互換性評価

- `git diff 9865cd57..fd1d4d5f -- 'crates/*/src/'` の削除行に
  `pub fn`/`pub struct`/`pub enum`/`pub trait`/`pub use`/`pub mod`
  の宣言は **0 件**。確認した変更は `pub(crate)` 内部項目
  （`CpalDevice::new` の引数追加・`handle` シグネチャ・
  `instant_to_media_time` の private→`pub(crate)` 変更）のみで、
  consumer 観測の公開面は不変。
- `ruviform-audio-device`: 新設 `wasapi_input` は private mod で
  `pub use` を追加しない。公開 API 面の追加は無く、
  `InputCallbackInfo::frame_position`（`ruviform-audio-core`
  由来・既存 field）に値が流れる内部挙動変更のみ。
- `ruviform-audio-plugin-bridge`: `input.rs`/`status.rs` の
  計数 field・getter 追加は field 追加型の additive。既存
  constructor/呼出面は不変。
- `ruviform-audio-core`: `midi_input::mock` は test/PoC 基盤の
  public mock module で、`inject_unsupported` 追加のみ。
- `ruviform-ui-gpui`: `OsInputOutcome` は新規 enum の追加。
  `capture`/`real_window`/`surface` の拡張も additive。
- audio/audio-analysis/dsp/exec/graph/io-*/math/media-*/numeric/
  observability/plugin-*/rt/ui/ui-test/ui-theme/ui-webview/
  video-core/viz: diff なし。

### synvelis 側の利用面

- `ruviform-audio-core`: `synvelis-recording` が
  `device::{InputCallbackInfo, InputDataCallback, ...}` を使用。
  `frame_position` 優先経路（`input.rs` L710）は実装済み。
- `ruviform-audio-device`: `synvelis-recording` の `real-device`
  feature 配下 optional dep（`lint-recording-real-input`/
  `test-recording-real-input` の opt-in lane でのみ compile）。
- `ruviform-audio-plugin-bridge`: `synvelis-player` 等 headless
  lane が使用。新 field は未消費のまま共存可能。
- `ruviform-ui-gpui`: `synvelis-native` bin（native feature）のみ。
  `real-scenario` 未有効化。
- 残り 23 crate は diff なし。

### 影響表（feature/schema/project/graph/RT/plugin/GUI/recorder/
automation/export）

| 軸 | 評価 |
| --- | --- |
| feature | 影響なし。`Cargo.toml` の依存・feature 宣言は不変。ruviform 側の新規 `real-scenario` feature・`x11rb`/`windows` optional dep は synvelis の feature 経路では有効化されない |
| schema | 影響なし。永続化 format・IPC schema の変更は diff 範囲に確認されず |
| project | 影響なし。`ruviform-graph`/`media-*`/`io-*` は無変更 |
| graph | 影響なし。`ruviform-graph` 無変更 |
| RT | 影響なし。`ruviform-rt` 無変更。`wasapi_input` は `real-device` opt-in lane の input callback 経路改善で、既定 build/headless lane には混入しない |
| plugin | additive のみ。bridge の `unsupported_backend`/`backend_unsupported` 計数透過は新規 field/getter で既存呼出面は不変。実 plugin/実 VST3 検証は NotRun |
| GUI | additive のみ。`OsInputOutcome` 追加・capture/os_input/real_window 拡張。synvelis は `gpui` feature のみで `real-scenario` 未有効。実 GPUI/実 OS 検証は NotRun |
| recorder | additive + 実機効果。WASAPI-native input path が `frame_position` を供給し、`synvelis-recording` の `frame_position` 優先経路が Windows 実機で本線化（synvelis#115 起票の上流修正）。コード変更不要。実機検証は opt-in lane の管轄 |
| automation | 影響なし。該当 crate に実質 diff なし |
| export | 影響なし。`ruviform-io-wav`/`media-*`/`io-symphonia` 無変更 |

### 新 API の採用判断

- `ruviform-ui-gpui` の `real-scenario` feature / `real_window` /
  `os_input` / `OsInputOutcome` / OS-window pixel capture:
  **今回は採用しない**。実 OS 入力・実窓 capture の検証 lane 配線は
  GUI 実機検証系 Issue（SV#114・SV#175 系）の判断事項。
- `ruviform-audio-plugin-bridge` の unsupported 計数透過
  （`unsupported_backend`/`backend_unsupported`/
  `unsupported_backend()`）: **今回は採用しない**。diagnostics/UI
  への計数消費配線は別タスク。
- `ruviform-audio-core` の `midi_input::mock::inject_unsupported`:
  **今回は採用しない**。MIDI bridge 試験基盤の需要確定時に判断。
- WASAPI input `frame_position` 供給: 新規採用ではなく
  **既存消費経路の本線化**。`input.rs` は既に `frame_position` を
  優先参照しており、pin 更新で実機供給が有効になるだけ（コード変更
  不要）。

## Open Questions

- なし。pin 更新は `cargo update -p ruviform-*`（全 27 package を
  `-p` 指定）で `main` HEAD を解決し、`check-ruviform-rev` の
  単一 SHA 契約で検証する。

## Sources and Relations

- Issue #28（Ruviform 継続追従 tracker。managed コメントは編集しない）。
- Issue #115（WASAPI input `frame_position` の上流修正を起票した
  synvelis 側 Issue。PR #567 がその実装）。
- 先行 pin 更新: PR #193（`cc7d5296`）、PR #301（`b02942c2`）。
- pin 更新規約: `docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md`、
  `tools/check-ruviform-rev.mjs`（Cargo.lock が唯一の正本）。
- AudioLens 側の同一 bump 評価:
  `audio-lens` worktree `docs/specs/ruviform-fd1d4d5f-rev-watch.md`
  （AL#127）。AL は `b02942c2..fd1d4d5f` の 7 commit を評価済みで、
  本差分の additive 評価と整合する。
- ruviform 差分範囲: `9865cd57..fd1d4d5f`（6 commits、ruviform
  local repo `F:/repositry/audio-products/ruviform` の `git log`
  で独立確認）。

## Evidence

- `git ls-remote origin` / ruviform local repo:
  `git rev-parse origin/main` →
  `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（2026-11-11 確認。
  dispatch 時点 HEAD と一致・以後の前進なし）。
- `git log 9865cd57..fd1d4d5f --oneline`（ruviform repo）: 6
  commits — `f93d105a`(#567) `ae70b57e`(#566) `68accdc7`(#568)
  `4eea9115`(#569) `5dd432f2`(#570) `fd1d4d5f`(#571)。
- `git diff 9865cd57..fd1d4d5f -- crates/`（ruviform repo）:
  24 files・+4,649/−146。変更 crate は audio-device（wasapi_input
  新設含む）・audio-core(mock)・audio-plugin-bridge(input/status)・
  ui-gpui(capture/os_input/real_window/surface)・midi-device
  （README/test のみ・依存木外）の 5 crate。synvelis 消費面では
  midi-device を除く 4 crate。
- `pub` 削除行スキャン: `pub fn`/`pub struct`/`pub enum`/
  `pub trait`/`pub use`/`pub mod` の削除は 0 件。
- `Cargo.lock` 差分（本 worktree、`cargo update -p` 実施済み）:
  27 箇所全てが `ruviform-*` の `source = "git+...#<sha>"`
  fragment の `9865cd57…` → `fd1d4d5f…` 更新。追加差分は
  `ruviform-audio-device` の依存リストに `windows 0.62.2`・
  `windows-core 0.62.2`、`ruviform-ui-gpui` の依存リストに
  `x11rb` が加わるのみで、いずれも lockfile 既存 version の
  参照追加（新規 package entry・version drift なし）。
  `ruviform-midi-device` は lockfile に非出現。
- `node tools/check-ruviform-rev.mjs --require-vendor`:
  `resolved fd1d4d5f…（27 lockfile sources, lockfile-primary,
  REV match）`。
