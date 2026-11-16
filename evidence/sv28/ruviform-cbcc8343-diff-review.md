# SV#28 ruviform pin abb9f372 → cbcc8343 差分精査メモ

実施日: 2026-11-16 / Worker worktree: `G:/orca/workspaces/synvelis/sv28-cbcc8343`
branch: `feat/28-ruviform-cbcc8343`（base `origin/main` = `f05bdbf`）

## candidate SHA 再解決

- `git -C G:/repositry/audio-products/ruviform fetch origin` →
  `git rev-parse origin/main` =
  `cbcc83437616fdd7418da37183c6be66b6cfe225`（sha8 `cbcc8343`）。
  Spec 起稿時点 candidate と一致。upstream 前進なし
  （`scarecrow173/audio-lens#127` と同一 SHA の tuple 整合を維持）。
- `git merge-base --is-ancestor abb9f372 cbcc8343` → true
  （fast-forward 可能な直線履歴）。
- `git rev-list --count abb9f372..cbcc8343` = **12 commits**。

## upstream commit 一覧（abb9f372..cbcc8343）

| SHA | 件名 |
| --- | --- |
| `e65b8014` | feat(plugin-adapter): Issue #630 adapter state bridge — VST3 IBStream + CLAP ext.state (Refs #630) (#667) |
| `a6d57355` | feat(test-plugin): Issue #630 test_editor slice — plugin-side IPlugView + real child-HWND fixture (#668) |
| `0b510cf8` | feat(audio-test): Issue #628 close-out — RunMetadata JSON・Cancellation fault・docs 簿記 (#669) |
| `0fb18cc2` | feat(test/gui): Issue #631 real-editor-attach — test_editor real IPlugView + child HWND OS observation (#670) |
| `be7eeed3` | style: rustfmt drift on stream.rs from #669 (#672) |
| `eafe0a08` | feat(test-plugin): Issue #630 test_stateful slice — real state blob + L1 save/mutate/restore/save E2E (#671) |
| `6a481425` | feat(ruviform-audio-device): Issue #629 4th slice — WASAPI streaming verifier + endpoint_probe 拡張 (#673) |
| `7956bfc4` | feat(test-plugin): Issue #631 GPUI Test Lab / Scenario Console GUI v1 + mise lane 簿記 (#674) |
| `4cdf5c83` | docs: regenerate managed indexes for scarecrow173/ruviform#629 slice docs (#675) |
| `de3ed4e8` | feat(test-plugin): Issue #630 test_instrument slice — MIDI/transport fixture + L1 real-ABI oracle (#676) |
| `b6b72dbd` | feat(plugin-dock-test): Issue #532 real-dcomp-pixel regression case + GPUI_DISABLE_DIRECT_COMPOSITION 実機証拠化 (#677) |
| `cbcc8343` | docs: regenerate managed indexes for scarecrow173/ruviform#532 slice docs (#678) |

## upstream diff 規模（crates/）

- `git diff --stat abb9f372..cbcc8343 -- crates/`:
  **57 files changed, +17,074 / −558**（Coordinator 事前精査と一致）。
- 変更 crate dir 10 + `crates/README.md`（1 行・audio-test 説明文更新）:
  `ruviform-audio-device`、`ruviform-audio-test`、
  `ruviform-plugin-clap`、`ruviform-plugin-core`、
  `ruviform-plugin-dock-test`、`ruviform-plugin-export`、
  `ruviform-plugin-host`、`ruviform-plugin-test-clap`、
  `ruviform-plugin-test-vst3`、`ruviform-plugin-vst3`。
- `ui/`（TypeScript）差分 **0 件**。`ffi-poc/`・docs 系・
  `evidence/`・`mise.toml`（upstream 側 task 追加）は
  synvelis 依存木外。
- upstream `Cargo.lock` 差分は `ruviform-audio-test` への
  `serde`/`serde_json` 追加と dep-tree-external crate
  （`plugin-dock-test`/`plugin-test-clap`/`plugin-test-vst3`）の
  依存リスト更新のみ。

## synvelis 依存木との交差

lockfile の ruviform package は **28 のまま不変**
（`grep -c 'name = "ruviform-' Cargo.lock` = 28。前回 slice の
27 → `ruviform-audio-test` dev-dep 追加で 28）。今回の範囲で
新規 crate の dep-tree 入りはなし。

synvelis 依存木 ∩ upstream `crates/` 差分あり = **4 crate**
（Coordinator 事前精査と一致）:

| crate | upstream 差分 | synvelis 消費面 | 破壊的変更の有無と影響 |
| --- | --- | --- | --- |
| `ruviform-audio-device` | `examples/endpoint_probe.rs` +283、`tests/virtual_endpoint.rs` +1371（新規）。**`src/` 差分 0 件** | `synvelis-recording` `real-device` optional dep | なし。lib 非影響。endpoint probe / WASAPI streaming verifier は example/test のみ |
| `ruviform-audio-test` | `src/{lib,backend,backend/stream,clock,metadata}` + `Cargo.toml`（serde/serde_json 追加）+ tests 2 件新規。`RunMetadata`/`RunParameters`/`JitterSpec`/`VirtualClockConfig`/`CaseRequirement`/`CaseResult`/`VerificationLevel`/`EnvironmentKind` に `Serialize`/`Deserialize` derive、`RunMetadata::{to_json,from_json,save_json,load_json}` 追加、`StreamFault::Cancellation` variant 追加（`#[non_exhaustive]` enum のため additive）、backend pump の Cancellation fault 実装 | `synvelis-recording` dev-dep。`tests/rf628_oracle.rs`・`rf628_oracle_rt_alloc.rs` が `backend::{DeviceSpec,InputSpec,PumpReport,ScriptedBackend,ScriptedDeviceHandle,StreamFault,StreamScript}`・`clock::{JitterSpec,VirtualClock,VirtualClockConfig,VirtualClockDriver}`・`oracle::{PcmCompare,pcm_assert_eq,pcm_reference_block}`・`pcm::{PcmFixture,PcmPattern,PcmScenario,PcmSegment}` を消費 | **公開 API 破壊なし（全て additive）**。`StreamFault` は `#[non_exhaustive]` で synvelis は variant 構築のみ（exhaustive match なし）。`PumpReport.invalidated` の doc が Cancellation を含む意味に更新（field 自体は不変）。`Cancellation` は production contract に無い概念を既存 observable（error callback `Backend("cancelled")` + terminate）で表現 — synvelis 既存 fault（`Frames`/`FramePosition`/`Error`/`Invalidate`）の経路は不変 |
| `ruviform-plugin-core` | `src/wire.rs` +43: `encode_event_record`/`decode_event_record` 追加（shm slot 固定 stride `EVENT_WIRE_SIZE` の record codec）、`EVENT_WIRE_SIZE` doc 更新 | app/player/project が `Event`/`EventKind`/`ParamInfo`/`ParamId`/`PluginFactory`/`BufferView(Mut)`/`erase`/`LifecycleState` 等を消費。`encode_event_record`/`decode_event_record` は synvelis 未使用 | なし。純粋 additive（新規 pub fn 2 件）。既存 `encode_event`/`decode_event`/`WIRE_VERSION` 不変 |
| `ruviform-plugin-host` | `src/l1/data.rs` 1 行: `RtLeg` の shm event 書き込みを `encode_event` → `encode_event_record` へ変更（固定 stride record codec 使用の内部修正） | app/player が `HostCommand`/`HostReply`/`PluginHandle`/`PluginHost`/`EndpointBounds`/`HostError`/`ReportDrain` を消費 | なし。`l1` は内部 module で公開 API 不変。shm slot の event record が可変長 encoding のまま stride 前提で書かれていた不整合の upstream 修正 — synvelis は in-process host 経路で `l1` data plane を直接触れない |

## dep-tree-external の変更 crate（synvelis 非影響・参考記録）

| crate | 内容 |
| --- | --- |
| `ruviform-plugin-clap` | `src/host.rs` +393（CLAP host の event/state bridge 系。dep-tree-external） |
| `ruviform-plugin-export` | `src/lib.rs` +7: shm event 読み出しを `decode_event_record` へ移行（`encode/decode_event_record` の consumer 側適応） |
| `ruviform-plugin-vst3` | `src/host.rs` +1114、`src/controller.rs` +32（VST3 host 拡張・state verb の `kNotImplemented` 正直化。dep-tree-external） |
| `ruviform-plugin-dock-test` | `real_dock.rs` +1724、`ospix.rs` 新規 +521、`plugin-dock-lab` bin 新規 +1194、dcomp-pixel regression case（Issue #532/#631。dep-tree-external） |
| `ruviform-plugin-test-clap` | `block.rs`/`instrument.rs`/`instrument_core.rs`/`stateful.rs` 等 +〜2,900（test_stateful/test_instrument fixture。dep-tree-external） |
| `ruviform-plugin-test-vst3` | `component.rs`/`controller.rs`/`editor.rs`/`editor_view.rs`/`events.rs`/`factory.rs`/`instrument.rs`/`stateful.rs` 等 +〜4,300（同上 VST3 側。dep-tree-external） |

## pub 削除スキャン結果

`git diff abb9f372..cbcc8343 -- crates/` の削除行（`-` で始まる行、
**615 行**）に対し `pub` を含む行は **0 件**。
公開 item の削除・rename・signature 変更は本範囲に存在しない
—— **純粋 additive な範囲**（前回 ac10a2f7 slice のような
ParentLease/wire v5 系の破壊的変更は無し）。

## 影響軸まとめ（design doc の影響評価表と同一内容）

| 軸 | 評価 |
| --- | --- |
| feature | 新規 feature 追加なし（dep-tree 内 crate の `Cargo.toml` で feature 変更を確認せず。`ruviform-audio-test` は serde/serde_json の通常 dep 追加のみ） |
| schema | 永続化 format・Project schema の変更なし。`RunMetadata` JSON は Case Schema v1 `kebab-case` 語彙の serialize/deserialize 追加で wire format 新設だが test-only crate の非 RT API |
| project | `ruviform-graph`/`media-*`/`io-*` 無変更 |
| graph | `ruviform-graph` 無変更 |
| RT | `ruviform-rt` 無変更。`ruviform-audio-test` の `VirtualClockConfig`/`JitterSpec` は `Copy` 維持で serde derive は RT 経路非呼出と upstream が明記。`plugin-host` `l1/data.rs` は shm 書き込みの record codec 修正（RT loop 内だが bounded・no-alloc 維持） |
| plugin | `plugin-core` additive（`encode/decode_event_record`）。`plugin-host` 内部修正のみ。`plugin-export`/`plugin-vst3`/`plugin-clap` は dep-tree-external。synvelis 消費の host command/param/event API 不変 |
| GUI | `ui/` 差分 0。`plugin-dock-test` の real_dock/ospix/plugin-dock-lab・`plugin-test-*` の editor fixture は dep-tree-external で未採用 |
| recorder | `audio-device` は example/test のみ（lib 不変）。`audio-test` の `Cancellation` fault・RunMetadata JSON は additive で既存 `rf628_oracle*` の消費 API（`StreamFault` 構築・`PumpReport`・`ScriptedBackend`・`oracle`/`pcm`/`clock`）は不変 |
| automation | 実質 diff なし（対象 crate に変更なし） |
| export | `ruviform-io-wav`/`io-symphonia`/`media-*` 無変更 |

## 新 API / 新 crate の採用判断（全て「今回は採用しない」）

- `RunMetadata::{to_json,from_json,save_json,load_json}` と serde
  derive: additive API だが synvelis の rf628 oracle は
  `RunMetadata` を未消費（消費は `backend`/`clock`/`oracle`/
  `pcm` のみ）。JSON 永続化の採用は test runtime 運用 slice の
  判断事項。
- `StreamFault::Cancellation`: additive variant。synvelis 側の
  open-fault/recovery oracle（PR #487 系）への組込みは別 slice
  判断事項。既存 fault 経路は不変。
- `wire::encode_event_record`/`decode_event_record`: plugin
  wire の固定 stride record codec。synvelis は wire 層を直接
  使わないため不採用（upstream 内部 consumer のみ）。
- `ruviform-plugin-test-vst3`/`test-clap` の新 fixture
  （test_editor/test_stateful/test_instrument）、
  `ruviform-plugin-dock-test` の real_dock/ospix/plugin-dock-lab、
  `ruviform-audio-device` の endpoint_probe 拡張・
  `tests/virtual_endpoint.rs`: 全て dep-tree-external または
  example/test のみ。GUI-L2/EXT-L3・実 ABI Test Plugin・OS 仮想
  Endpoint 受入の別 slice 判断事項。
- `ruviform-plugin-vst3` host 拡張・`plugin-clap` host 拡張・
  `plugin-export` record codec 適応: dep-tree-external。

## Cargo.lock 差分

`cargo update -p`（28 package 全指定）後の `git diff Cargo.lock`:
**30 insertions / 28 deletions**。

- 28 package × `source = "git+...#abb9f372…"` →
  `#cbcc8343…` の fragment 更新（56 行の -/+ ペア）。
- `ruviform-audio-test` の dependencies に `serde`・`serde_json`
  の 2 行追加（upstream `Cargo.toml` の新規 dep を反映。
  両 crate は lockfile 既存: serde 1.0.229・serde_json 1.0.154）。
- 依存リスト・version・新規 package の他の変動なし。
  ruviform package 数 28 → 28。

## 検証 lane 実施記録（詳細は impl/ir doc）

Windows 11・mise 管理 toolchain で実施。`*-msrv`/`*-full` は
ユーザー指定により未実施。実 device/実 GUI/実 plugin opt-in lane
は NotRun。`scarecrow173/synvelis#488` 由来の既存 fail
（`clippy::large_enum_variant` in `synvelis-workspace`）が
出た場合は pin 非起因として切り分け記録し、当該 crate は
修正しない。
