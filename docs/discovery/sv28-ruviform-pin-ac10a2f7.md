---
id: "DISC-oz2TWMUsnSkPXzinPhwaOL"
type: "discovery"
status: "resolved"
title: "SV#28 Ruviform pin 更新 — fd1d4d5f → ac10a2f7 差分調査と API 互換性評価"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "evidence/sv28/ruviform-ac10a2f7-diff-review.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv28-ruviform-pin-update.md"
    - "docs/tasks/sv28-ruviform-pin-b02942c2.md"
    - "docs/tasks/sv28-ruviform-pin-fd1d4d5f.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/plans/sv28-ruviform-pin-ac10a2f7.md"
  verifies: []
  verified-by: []
  references:
    - "Cargo.lock"
    - "Cargo.toml"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
    - "evidence/sv28/ruviform-ac10a2f7-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 — fd1d4d5f → ac10a2f7 差分調査

## Summary

現行 pin `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` から ruviform
`main` HEAD `ac10a2f7761c08db73aebd861c0a5c1f53f269b7` までの
**128 commits** を調査した。`crates/` 配下の diff は 21 crate・
108 files・+18,207/−656 と大規模だが、synvelis の消費面では
公開 API の破壊的変更が複数あるものの **synvelis が使用する
API への削除・シグネチャ変更は 0 件**（`cargo check --workspace
--all-targets` 全 14 package で実証）。新規 crate・新 feature は
全て opt-in/依存木外で、`Cargo.toml` の変更・新規採用は不要。
精査メモは
`evidence/sv28/ruviform-ac10a2f7-diff-review.md` に記録した。

## Exploration Goal

pin 更新で consumer（synvelis）が踏む API 差分を特定し、追従修正の
要否と範囲を確定する。あわせて ParentLease 語彙移行・wire v5・
MIDI event contract 修正・`MergeOutcome` などの破壊的変更が
synvelis の消費面に影響するか、および新規 upstream 機能
（plugin-dock scenario・`uitest` feature・`audio-test`・
`plugin-test-vst3`・io-e2e lane 等）の採用判断材料を揃える。

## Findings

### 差分に含まれる主な変更（fd1d4d5f..ac10a2f7、128 commits）

詳細な crate 別評価は evidence メモを参照。consumer 影響の要約:

| 領域 | upstream 変更 | consumer 影響 |
| --- | --- | --- |
| `ruviform-plugin-core` | `GuiParentToken` → `ParentLease{generation,payload}` 語彙移行（Issue #389）、`WIRE_VERSION` 4→5（`Command::OpenGui.parent` encoding 変更）、`encode/decode_gui_parent_token` → `encode/decode_parent_lease`、`merge_call_events` が `Result<MergeOutcome{events,truncated}>` を返す semantics 変更（`MergeError::Overflow` 廃止） | **公開 API 破壊あり・synvelis 未使用**。synvelis は `GuiParentToken`/`ParentLease`/`open_gui`/`merge_call_events`/`MergeError`/wire `OpenGui` を一切参照しない |
| `ruviform-plugin-host` | `HostCommand::OpenGui.parent` 型変更、`LeaseRedeemer` 戻り値型変更、re-export `GuiParentToken`→`ParentLease` | **公開 API 破壊あり・synvelis 未使用**。使用 variant は `Activate`/`Deactivate`/`Destroy`/`Initialize`/`Instantiate`/`RestoreState`/`SaveState`/`Scan`/`StartProcessing`/`StopProcessing` のみ |
| `ruviform-audio-plugin-bridge` | `InputStageCounters` から `released`/`ledger_overflows` 削除・`anchor_rejected`/`event_merge_truncated` 追加、`InputStage::release_all`/`active_notes` 削除（note-state 権威は `PluginNode` ledger へ移管）。MIDI delivery contract 修正（Issue #253 V9/V11/V12、`d512334a`）+ `tests/midi_delivery.rs` | **公開 API 破壊あり・synvelis 未使用**。`InputStage`/`NodeStatusSnapshot`/`release_all`/`active_notes`/`ledger_overflows` 参照 0 件。MIDI 振る舞いは upstream 修正で改善 |
| `ruviform-ui-test` | `TestIo`/`TestApp`/`Journal*`/`HeadlessIo` の driver-neutral 語彙移行（`FakeHost`→`FakeDriver`・`WindowEvent`→`DriverEvent`・`create_window(WindowConfig)`→`(WindowSpec)` 等）、`plugin_dock` scenario module 追加（Issue #631）、`ScenarioDriver::PluginHostProbe` 追加、hello 交換 channel-death を `Handshake` 分類へ正規化（Issue #625） | **公開 API 破壊あり・synvelis 未使用**。synvelis-app tests は既に新語彙（`FakeDriver`/`DriverEvent`/`WindowSpec`/`ScenarioDriver`）で記述済み。scenario/driver API の消費面は additive のみで `cargo check --all-targets` 通過 |
| `ruviform-ui-gpui` | 新規 `uitest` feature（`WindowedIo`/`UiSide`/`windowed_test_app`、Issue #313 S2）、`real-scenario` cfg を `any(real-scenario, uitest)` へ拡張 | なし。`uitest` は opt-in feature で synvelis は未有効化 |
| `ruviform-io-file` | `#[cfg(test)] mod mock` → `#[cfg(any(test, feature = "fault-injection"))] #[doc(hidden)] pub mod mock` + `fault-injection` feature 新設（Issue #306） | なし。opt-in feature は synvelis で未有効化。既存 API 不変 |
| `ruviform-audio-device` | WASAPI input `E_ACCESSDENIED` → `Backend("permission_denied")` 分類追加 + `tests/real_input_e2e.rs` 拡張 | additive。`synvelis-recording` `real-device` opt-in lane 内の error mapping 改善のみ |
| `ruviform-numeric` | test/bench profile 修正（`#[cfg(debug_assertions)]` allocator 等、Issue #646） | なし。推移的依存のみ・test/bench profile のみ |
| `ruviform-observability` | `tests/logging.rs` のみ | なし |
| `ruviform-ui-webview` | `lib.rs` doc comment +12（legacy/preview 分類、Issue #465）、README | なし。doc のみ |
| 新規 crate（依存木外） | `ruviform-audio-test`（deterministic test runtime、Issue #628）、`ruviform-plugin-dock-test`（dock scenario harness、Issue #631/#389）、`ruviform-plugin-test-vst3`（VST3 fixture、Issue #630）、`ruviform-io-e2e`（G01/G05 fault-injection lane、Issue #306）等 | なし。lockfile 非含有を確認（27 package 不変） |
| dep-tree-external | `ruviform-plugin-clap`（editor-attach）、`ruviform-plugin-vst3`、`ruviform-poc-native-dock`、`ruviform-video-proc`（ICC）、`ruviform-midi-device`（README のみ） | なし |

### 互換性評価

- `git diff fd1d4d5f..ac10a2f7 -- crates/` の `pub` item 削除行
  スキャンで 17 件検出したが、全て rename/移行/restructure で
  削除後に同等または後継 API が存在。**synvelis が消費する公開
  API で削除・signature 変更されたものは 0 件**。
- 破壊的変更（ParentLease・wire v5・`MergeOutcome`・bridge
  counter/ledger 再構成・ui-test driver 語彙）は全て synvelis
  未使用面に限定されることを grep で確認済み。
- `cargo check --workspace --all-targets --locked`（全 14 package・
  dev-dependency の test code 含む）が exit 0 で実証。
- `Cargo.lock` 差分は 27 package × `-`/`+` の source fragment
  更新 54 行のみ。依存リスト・version・新規 package の変動なし。
  ruviform package 数 27 → 27。
- `ui/`（TypeScript）差分なし。`ffi-poc/`・`ci/`・docs 系は
  synvelis 依存木外。

### 影響表（feature/schema/project/graph/RT/plugin/GUI/recorder/
automation/export）

| 軸 | 評価 |
| --- | --- |
| feature | `ruviform-ui-gpui` `uitest`・`ruviform-io-file` `fault-injection` が新設されたが synvelis `Cargo.toml` 不変で未有効化 |
| schema | 影響なし。永続化 format・Project schema の変更を diff 範囲に確認せず |
| project | `ruviform-graph`/`media-*`/`io-*`（io-file 以外）無変更。io-file は opt-in facade 追加のみ |
| graph | `ruviform-graph` 無変更 |
| RT | `ruviform-rt` 無変更。`InputStage`/`merge_call_events` の RT 契約は維持（bounded・no-alloc を upstream が明記） |
| plugin | **破壊的変更あり（未使用面）**: ParentLease・wire v5・merge truncation semantics・MIDI contract 修正。synvelis 消費面の plugin API（host command variant・param/event・bridge node factory/control）は不変。MIDI 入力経路の振る舞いは upstream 修正で改善 |
| GUI | additive + opt-in。`uitest`/`plugin_dock` scenario/`PluginHostProbe` 未採用。`ui-webview` doc のみ。synvelis-app tests の既存 lane で非回帰を検証 |
| recorder | additive。`wasapi_input` の permission_denied 分類は `real-device` opt-in lane 内の改善。`frame_position` 経路は不変 |
| automation | 実質 diff なし |
| export | `ruviform-io-wav`/`io-symphonia`/`media-*` 無変更 |

### 新 API / 新 crate の採用判断

全て **今回は採用しない**（詳細は design doc の判断表）:

- `ruviform-plugin-dock-test`・`ruviform-plugin-test-vst3`・
  `ruviform-audio-test`・`ruviform-io-e2e`: 依存木外の test
  基盤。採用は GUI-L2/EXT-L3・Test Runtime・IO 試験要件の
  別 slice の判断事項。
- `ruviform-ui-gpui` `uitest` feature・`ruviform-ui-test`
  `plugin_dock` scenario/`PluginHostProbe`: GUI scenario 基盤の
  設計判断。
- `ruviform-io-file` `fault-injection` feature: SYN-IO 系試験
  要件確定時。
- endpoint probe examples・ParentLease/wire v5: synvelis 未使用
  面。将来 `Plugin` trait 実装や `open_gui`/wire v5 を使う場合は
  `ParentLease` 語彙への移行が必要 — 記録のみ。
- `synvelis-recording` の `frame_position` 経路: 既存消費経路の
  継続であり新規採用ではない。

## Open Questions

- なし。pin 更新は `cargo update -p ruviform-*`（全 27 package を
  `-p` 指定）で `main` HEAD を解決し、`check-ruviform-rev` の
  単一 SHA 契約で検証する。

## Sources and Relations

- Issue #28（Ruviform 継続追従 tracker。managed コメントは
  編集しない）。
- 先行 pin 更新: PR #193（`cc7d5296`）、PR #301（`b02942c2`）、
  `fd1d4d5f` pin slice（本 chain の前身）。
- pin 更新規約: `docs/tasks/sv-gui-02-ruviform-rev-rust-syn-ui-op.md`、
  `tools/check-ruviform-rev.mjs`（Cargo.lock が唯一の正本）。
- 差分精査メモ:
  `evidence/sv28/ruviform-ac10a2f7-diff-review.md`。
- ruviform 差分範囲: `fd1d4d5f..ac10a2f7`（128 commits、
  ruviform repo の `git log`/`git diff` で独立確認）。

## Evidence

- ruviform local repo: `git rev-parse origin/main` →
  `ac10a2f7761c08db73aebd861c0a5c1f53f269b7`（2026-11-15 確認。
  spec 起稿時点 candidate と一致・以後の前進なし）。
- `git merge-base --is-ancestor fd1d4d5f ac10a2f7` → true。
- `git rev-list --count fd1d4d5f..ac10a2f7` → 128。
- `git diff --stat fd1d4d5f..ac10a2f7 -- crates/` →
  108 files・+18,207/−656（変更 crate dir 21 + README）。
- `pub` 削除行スキャン: 17 件全て rename/移行/restructure。
  synvelis 消費面の削除・signature 変更は 0 件。
- `Cargo.lock` 差分（本 worktree、`cargo update -p` 実施済み）:
  54 行の source fragment 更新のみ（27 package × -/+）。
  依存リスト・version・新規 package の変動なし。
- `node tools/check-ruviform-rev.mjs --require-vendor`:
  `resolved ac10a2f7761c08db73aebd861c0a5c1f53f269b7 (27 lockfile
  sources, lockfile-primary, REV match)`。
- `mise run sync-ui-deps`: `synced ruviform ui/* @
  ac10a2f776 -> ui/vendor/ruviform`。
