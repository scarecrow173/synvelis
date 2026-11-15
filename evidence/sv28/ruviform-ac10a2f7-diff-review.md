# SV#28 ruviform pin fd1d4d5f → ac10a2f7 差分精査メモ

実施日: 2026-11-15 / Worker worktree: `G:/orca/workspaces/synvelis/sv28-pin`
branch: `chore/28-ruviform-pin-ac10a2f7`（base `origin/main` = `d54d7ae`）

## candidate SHA 再解決

- `git -C G:/repositry/audio-products/ruviform fetch origin` →
  `git rev-parse origin/main` =
  `ac10a2f7761c08db73aebd861c0a5c1f53f269b7`（sha8 `ac10a2f7`）。
  Spec 起稿時点と同一。upstream 前進なし。
- `git merge-base --is-ancestor fd1d4d5f ac10a2f7` → true
  （fast-forward 可能な直線履歴）。
- `git rev-list --count fd1d4d5f..ac10a2f7` = **128 commits**。

## upstream diff 規模（crates/）

- `git diff --stat fd1d4d5f..ac10a2f7 -- crates/`:
  **108 files changed, +18,207 / −656**（Coordinator 事前精査と一致）。
- 変更 crate dir 21 + `crates/README.md`:
  `ruviform-audio`, `ruviform-audio-device`,
  `ruviform-audio-plugin-bridge`, `ruviform-audio-test`(新規),
  `ruviform-io-e2e`, `ruviform-io-file`, `ruviform-midi-device`,
  `ruviform-numeric`, `ruviform-observability`,
  `ruviform-plugin-clap`, `ruviform-plugin-core`,
  `ruviform-plugin-dock-test`(新規), `ruviform-plugin-host`,
  `ruviform-plugin-test-vst3`(新規), `ruviform-plugin-vst3`,
  `ruviform-poc-native-dock`, `ruviform-ui-gpui`, `ruviform-ui-test`,
  `ruviform-ui-webview`, `ruviform-video-proc`。
- `ui/`（TypeScript）差分なし。`ffi-poc/`・`ci/`・docs 系は
  synvelis 依存木外。

## synvelis 依存木との交差

lockfile の ruviform package は **27 のまま不変**（`cargo update` 後も
`grep -c 'name = "ruviform-' Cargo.lock` = 27）。新規 crate
（`ruviform-audio-test`/`ruviform-plugin-dock-test`/
`ruviform-plugin-test-vst3`/`ruviform-io-e2e`/`ruviform-midi-device`/
`ruviform-plugin-vst3`/`ruviform-plugin-clap`/
`ruviform-poc-native-dock`/`ruviform-video-proc`）は **dep-tree-external**
で lockfile 非含有を確認。

synvelis 直接依存 ∩ upstream src 差分あり（11 crate 中 9 が直接依存、
`ruviform-numeric` は推移的、`ruviform-observability` は logging
feature 経由）:

| crate | upstream src 差分 | synvelis 消費面 | 破壊的変更の有無と影響 |
| --- | --- | --- | --- |
| `ruviform-audio` | なし（`tests/alternate_evaluation.rs` +1320 のみ） | app/player/render が直接依存 | なし |
| `ruviform-audio-device` | `wasapi_input.rs` +15（`E_ACCESSDENIED` → `Backend("permission_denied")` 分類追加 + test）、`tests/real_input_e2e.rs` 拡張 | `synvelis-recording` `real-device` optional dep | なし。error 分類の additive 改善のみ |
| `ruviform-audio-plugin-bridge` | `input.rs`/`node.rs`/`status.rs` 再構成（Issue #253 V9/V11/V12 MIDI event contract 修正 `d512334a`）+ `tests/midi_delivery.rs` | app/player が `AttachConfig`/`attach`/`Attached`/`PluginNode`/`PluginNodeFactory`/`PluginNodeControl`/`DispatchStage`/`ParamValueObservation`/`FrameBaseMismatch`/`InstallPlanError`/`AttachError` を消費 | **公開 API 破壊あり・synvelis 未使用**: `InputStageCounters` から `released`/`ledger_overflows` 削除・`anchor_rejected` 追加、`InputStage::release_all`/`active_notes` 削除（note-state 権威は `PluginNode` ledger へ移管）、`NodeStatusSnapshot` の `released`/`ledger_overflows` → `anchor_rejected`/`event_merge_truncated`。synvelis は `InputStage`/`NodeStatusSnapshot`/`release_all`/`active_notes`/`ledger_overflows` を一切参照しない（grep 0 件）。`merge_call_events` の戻り値変更は plugin-core 側。**MIDI 振る舞いは upstream 修正どおり改善される**（anchor republish pending 保存・V11/V12 contract） |
| `ruviform-io-file` | `lib.rs`/`mock.rs`/`staged.rs`: `#[cfg(test)] mod mock` → `#[cfg(any(test, feature = "fault-injection"))] #[doc(hidden)] pub mod mock` + `fault-injection` feature 新設（Issue #306 `6027242b`） | library/player/project/recording/render が直接依存 | なし。新規 opt-in feature は synvelis で未有効化。公開面は `#[doc(hidden)]` facade の拡張のみで既存 API 不変 |
| `ruviform-numeric` | `corr.rs` `mod tests` 内 `assert_eq!` 型注記、`lib.rs` `rt_tests` の allocator に `#[cfg(debug_assertions)]`（Issue #646 `d5aa1b4b`） | 推移的依存のみ（直接依存なし） | なし。test/bench profile 修正のみ |
| `ruviform-observability` | `tests/logging.rs` +12 のみ | app が `logging` feature で直接依存 | なし |
| `ruviform-plugin-core` | `boundary.rs`/`event.rs`/`lib.rs`/`plugin.rs`/`wire.rs`: `GuiParentToken` → `ParentLease{generation,payload}` 語彙移行（Issue #389 `66fcce0b`）、`Plugin::open_gui`/`ErasedPlugin::open_gui` 引数型変更、`WIRE_VERSION` 4→5（`Command::OpenGui.parent` encoding 変更）、`encode/decode_gui_parent_token` → `encode/decode_parent_lease`、`merge_call_events` が `Result<MergeOutcome>`（`MergeError::Overflow` 廃止・truncation semantics 化、`MergeOutcome` 新設） | app/player/project が `Event`/`EventKind`/`EventList`/`ParamInfo`/`ParamId`/`ParamValue`/`ParamFlags`/`ParamWriteMeans`/`TimedParamEvent`/`check_param_write`/`PluginFactory`/`BufferView(Mut)`/`erase`/`LifecycleState` 等を消費。`GuiParentToken`/`ParentLease`/`open_gui`/`merge_call_events`/`MergeError`/`WireCommand::OpenGui` は **synvelis 未使用**（grep 0 件、`Plugin` trait 実装なし） | **公開 API 破壊あり・synvelis 未使用**: wire v5 は `OpenGui` verb のみ変更・他 tag 不変。synvelis は in-process host の `OpenGui`/`CloseGui` command を発行しない（`HostCommand::` 使用 variant に OpenGui/CloseGui なし） |
| `ruviform-plugin-host` | `command.rs`/`editor_session.rs`/`host.rs`/`lib.rs`: `HostCommand::OpenGui.parent` 型変更、`LeaseRedeemer` 戻り値 `GuiParentToken`→`ParentLease`、re-export `GuiParentToken`→`ParentLease` | app/player が `HostCommand`/`HostReply`/`PluginHandle`/`PluginHost`/`EndpointBounds`/`HostError`/`ReportDrain` を消費 | **公開 API 破壊あり・synvelis 未使用**: 使用 variant は `Activate`/`Deactivate`/`Destroy`/`Initialize`/`Instantiate`/`RestoreState`/`SaveState`/`Scan`/`StartProcessing`/`StopProcessing` のみで `OpenGui`/`CloseGui`/`InProcessEditorOps`/`LeaseRedeemer` 非使用 |
| `ruviform-ui-gpui` | `lib.rs`/`gpui_driver/{capture,surface,windowed_io}.rs`/`render.rs`: 新規 `uitest` feature（`WindowedIo`/`UiSide`/`windowed_test_app` bin、Issue #313 S2 `4bcf1402`）、`real-scenario` cfg を `any(real-scenario, uitest)` へ拡張、capture/surface は doc comment、render.rs は test cfg | app `native`/`gpui-test` feature が `gpui`/`gpui-test` feature のみ有効化（`uitest` 未使用） | なし。`uitest` は opt-in feature で既定 off。`gpui-test` feature 経路の公開面は不変 |
| `ruviform-ui-test` | `agent.rs`/`driver.rs`/`fake_driver.rs`/`fixture.rs`/`headless.rs`/`io.rs`/`journal.rs`/`lib.rs`/`scenario.rs`/`scenario/plugin_dock.rs`: `TestIo` trait の driver-neutral 語彙移行（`emit(WindowEvent)`→`emit(DriverEvent)`・`emit_ipc_body` 追加・`create_window(WindowConfig)`→`(WindowSpec)`）、`TestApp::on_event`/`shutdown` 引数型変更、`Journal::record_in`/`record_suppressed`/`JournalHandle::new`/`deliver_routed` の signature 変更、`HeadlessIo::host()` 戻り値 `FakeHost`→`FakeDriver`、`ScenarioDriver::PluginHostProbe` variant 追加、`plugin_dock` scenario module 追加（Issue #631）、hello 交換の channel-death error を `Handshake` 分類へ正規化（Issue #625 `7aab6302`） | app dev-dependency が `FakeDriver`/`FakeGuiTest`/`ScenarioDriver`/`SemanticGuiTest`/`ScenarioCapability`/`ScenarioVerdict`/`ScenarioError`/`ScenarioReport`/`ScenarioId`/`ScenarioTarget`/`TargetQuery`/`CaseId`/`GuiAction`/`MintedIntent`/`RunId`/`CaptureOutcome`/`VisualGuiTest`/`parity_gesture_scenario`/`golden::*` を消費。`TestIo`/`TestApp`/`FixtureController`/`HeadlessIo`/`Journal*`/`FakeHost`/`FakeWindow`/`WindowEvent`/`WindowHandle`/`WindowConfig` は **synvelis 未使用**（synvelis-app tests は既に `DriverEvent`/`WindowDriver`/`WindowSpec`/`WindowLifecycle` の新語彙で記述済み） | **公開 API 破壊あり・synvelis 未使用**: synvelis が使う scenario/driver API は additive のみ（`plugin_dock` module・`PluginHostProbe` variant・`FakeDriver::ready()` は追加）。`cargo check --workspace --all-targets` で synvelis-app tests 含め compile 確認済み |
| `ruviform-ui-webview` | `lib.rs` +12（Issue #465 legacy/preview 分類の doc comment のみ）、`README.md` +9 | app `webview` feature が optional dep として依存 | なし。doc comment のみ |

## pub 削除スキャン結果

`git diff fd1d4d5f..ac10a2f7 -- crates/`（21,447 行）に対し
`-.*pub (fn|struct|enum|trait|use|mod|const|type|static)` の削除行は
**17 件**。内訳は全て「rename/移行/restructure」で、削除後に同等
または後継 API が存在:

- `GuiParentToken`/`encode_gui_parent_token`/`decode_gui_parent_token`
  → `ParentLease`/`encode_parent_lease`/`decode_parent_lease`
  （plugin-core、`66fcce0b`）
- `InputStage::{release_all, active_notes}`、
  `InputStageCounters::{released, ledger_overflows}`（bridge、
  `d512334a`）
- `NodeStatusSnapshot` の `released`/`ledger_overflows` field →
  `anchor_rejected`/`event_merge_truncated`（bridge status.rs）
- `MergeError::Overflow` variant → `MergeOutcome.truncated`
  semantics（plugin-core event.rs）
- `WIRE_VERSION` 4→5（wire v5、OpenGui parent encoding 変更）
- ui-test の `WindowEvent`/`WindowHandle`/`HostEvent` 語彙 →
  `DriverEvent`/`WindowDriver`/`WindowLifecycle`（`TestIo`/`TestApp`/
  `Journal*`/`deliver_routed`/`HeadlessIo::host()` 戻り値型）
- `LeaseRedeemer`/`HostCommand::OpenGui.parent` の型変更
  （plugin-host）

**synvelis が消費する公開 API で削除・signature 変更されたものは
0 件** — 上記の破壊的変更は全て synvelis 未使用の項目に限定され、
`cargo check --workspace --all-targets --locked`（全 14 package・
テスト含む）が exit 0 で実証。

## 影響軸まとめ（design doc の影響評価表と同一内容）

| 軸 | 評価 |
| --- | --- |
| feature | `ruviform-ui-gpui` `uitest` feature・`ruviform-io-file` `fault-injection` feature が新設されたが synvelis `Cargo.toml` 不変で未有効化。`Cargo.toml` 差分なし |
| schema | 永続化 format・Project schema の変更なし（diff 範囲に project/io schema 変更を確認せず） |
| project | `ruviform-graph`/`media-*`/`io-*`（io-file 以外）無変更。io-file は opt-in facade 追加のみ |
| graph | `ruviform-graph` 無変更 |
| RT | `ruviform-rt` 無変更。`InputStage`/`merge_call_events` の RT 契約は維持（bounded・no-alloc を upstream が明記）。`#[cfg(debug_assertions)]` 付与は bench profile 修正のみ |
| plugin | **破壊的変更あり（未使用面）**: ParentLease 語彙移行・wire v5・merge truncation semantics・MIDI event contract 修正（V9/V11/V12）。synvelis が使う plugin API（host command・param/event・bridge node）は不変。MIDI 入力経路の振る舞いは upstream 修正で改善（`PluginNode` ledger が note-state 権威） |
| GUI | additive + opt-in。`uitest` feature/`windowed_io`/`plugin_dock` scenario/`PluginHostProbe` は未採用。`ui-webview` は doc のみ。synvelis-app tests は既存の `gpui`/`gpui-test`/`webview` feature 経路で非回帰を lane 検証 |
| recorder | additive。`wasapi_input` の `E_ACCESSDENIED` 分類追加は `real-device` opt-in lane 内の error mapping 改善。`frame_position` 経路（前 slice PR #567）は不変 |
| automation | 実質 diff なし（対象 crate に変更なし） |
| export | `ruviform-io-wav`/`io-symphonia`/`media-*` 無変更 |

## 新 API / 新 crate の採用判断（全て「今回は採用しない」）

- `ruviform-plugin-dock-test`（test-only dock scenario harness、
  Issue #631/#389）: 依存木外・test-only。採用は GUI-L2/EXT-L3
  dock 受入 slice の判断事項。
- `ruviform-plugin-test-vst3`（VST3 `test_gain` fixture、Issue
  #630）: 依存木外。実 ABI Test Plugin 基盤の採用は EXT-L3 要件
  slice の判断事項。
- `ruviform-audio-test`（deterministic test runtime、Issue #628）:
  依存木外。共通 Test Runtime 採用は別途設計判断。
- `ruviform-ui-gpui` `uitest` feature + `WindowedIo`/`UiSide`/
  `windowed_test_app`（Issue #313 S2 windowed TestIo）:
  `rvf.uitest.v1` control surface。synvelis の gpui-test scenario
  lane とは別経路。採用は GUI scenario 基盤の設計判断。
- `ruviform-ui-test` `plugin_dock` scenario/`PluginHostProbe`
  （Issue #631）: dock 受入接続は未定義。
- `ruviform-io-file` `fault-injection` feature + `#[doc(hidden)]`
  mock facade（Issue #306）: staged publish fault 試験の採用は
  SYN-IO 系試験要件確定時。
- `ruviform-io-e2e` G01/G05 opt-in lanes（Issue #306）: 依存木外。
- `ruviform-{audio,midi}-device` endpoint probe examples
  （Issue #629）: example binary で library API ではない。
- plugin-core MIDI event contract 修正（V9/V11/V12）・ParentLease
  語彙: API 追従は発生しなかった（synvelis 未使用面）が、
  `Plugin` trait 実装や `open_gui`/wire v5 を将来使う場合は
  `ParentLease` 語彙への移行が必要 — 記録のみ。
- `ruviform-plugin-clap` editor-attach（Issue #389 #637）・
  `ruviform-plugin-vst3`/`poc-native-dock`/`video-proc`/`midi-device`:
  dep-tree-external。

## Cargo.lock 差分

`cargo update -p`（27 package 全指定）後の `git diff Cargo.lock`:
**54 行の `source = "git+...#<sha>"` fragment 更新のみ**
（27 package × -/+ ペア）。依存リスト・version・新規 package の
変動なし。ruviform package 数 27 → 27。
`ui-gpui` の `uitest` feature が参照する `base64 0.22.1` は
optional dep で lockfile 既存（依存リストに既出・非コンパイル）。

## 検証 lane 実施記録（詳細は impl/ir doc）

Windows 11・mise 管理 toolchain で実施。`*-msrv`/`*-full` は
ユーザー指定により未実施。実 device/実 GUI/実 plugin opt-in lane
は NotRun。
