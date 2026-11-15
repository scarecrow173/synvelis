---
id: "IMPL-9lysTGra2gwN96PqkF09YI"
type: "impl"
status: "completed"
title: "SV#28 Ruviform pin 更新 — fd1d4d5f → ac10a2f7 API 追従評価"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/plans/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/test-specs/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/tasks/sv28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "docs/impl/ir/sv-28-ruviform-pin-ac10a2f7.md"
      - type: "file"
        path: "evidence/sv28/ruviform-ac10a2f7-diff-review.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/discovery/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/tasks/sv28-ruviform-pin-ac10a2f7.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/designs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/plans/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/tasks/sv28-ruviform-pin-ac10a2f7.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv28-ruviform-pin-ac10a2f7.md"
    - "docs/discovery/sv28-ruviform-pin-ac10a2f7.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-ac10a2f7.md"
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
    - "mise.toml"
    - "evidence/sv28/ruviform-ac10a2f7-diff-review.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV#28 Ruviform pin 更新 — fd1d4d5f → ac10a2f7 API 追従評価

## Summary

synvelis workspace の ruviform 解決 revision を
`fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` から
`ac10a2f7761c08db73aebd861c0a5c1f53f269b7`（ruviform `main`
HEAD、128 commits 前進）へ更新した。`Cargo.lock` の 27
`ruviform-*` package source を scoped `cargo update -p` で
更新し、`ui/vendor/ruviform` を `sync-ui-deps` で再生成した。
upstream 差分精査の結果、破壊的 API 変更（ParentLease 語彙・
wire v5・`MergeOutcome`・bridge counter/ledger 再構成・
ui-test driver 語彙）は全て synvelis 未使用面に限定され、
消費面の削除・signature 変更は 0 件。製品コード変更なしで
canonical 検証 lane が全て exit 0 で非回帰を確認した。

## Context

- Issue #28 は Ruviform 継続追従 tracker。pin の正本は
  `Cargo.lock` のみ（`Cargo.toml` の git 依存は `rev` 未指定）。
- dispatch 時点の ruviform `main` HEAD は spec 起稿時と同一の
  `ac10a2f7`（前進なし）。`fd1d4d5f` は `ac10a2f7` の ancestor。
- 差分規模: `crates/` 108 files・+18,207/−656・21 crate 変更。
  新規 crate（`ruviform-audio-test`/`plugin-dock-test`/
  `plugin-test-vst3`/`io-e2e` 等）は全て依存木外。

## Implementation

### Changes

- `Cargo.lock`: `cargo update -p`（27 package 全指定）で
  `ruviform-*` の `source = "git+...#<sha>"` fragment を
  `fd1d4d5f…` → `ac10a2f7…` へ更新。差分は 27 insertions +
  27 deletions の 54 行のみで、依存リスト・version・新規
  package の変動なし（`git diff Cargo.lock` で確認。
  ruviform 以外の差分行 0 件）。
- `ui/vendor/ruviform/`: `mise run sync-ui-deps` で
  `ac10a2f776` へ再生成（gitignore 対象の生成物）。
- `crates/*/src/`: 変更なし。消費面の破壊的変更 0 件のため
  追従修正は不要（`check --workspace --all-targets` で実証）。
- `evidence/sv28/ruviform-ac10a2f7-diff-review.md`: upstream
  差分精査メモ（crate 別影響・`pub` 削除スキャン・採用判断）。
- `docs/{discovery,specs,designs,plans,test-specs,tasks}/
  sv28-ruviform-pin-ac10a2f7.md`・`docs/impl/ir/
  sv-28-ruviform-pin-ac10a2f7.md`: 7-doc chain。
- `docs/*/README.md`: managed index 再生成
  （`doc_maintenance index apply`）。

### Decisions During Implementation

- pin 更新は repo 規約どおり `cargo update -p`（既定ブランチ
  追従、`--precise` 不使用）で実施。`Cargo.toml` 不変。
- upstream 差分評価は ruviform repo の `git log`/`git diff
  fd1d4d5f..ac10a2f7`（128 commits）+ `pub` 削除行スキャン
  （17 件・全て rename/移行）+ synvelis 消費面 grep +
  `check --all-targets` で実施。詳細は evidence メモ。
- 破壊的変更は全て synvelis 未使用面:
  - `ruviform-plugin-core`/`plugin-host`: ParentLease 語彙・
    wire v5・`MergeOutcome` — synvelis は `GuiParentToken`/
    `ParentLease`/`open_gui`/`merge_call_events`/wire
    `OpenGui`/`CloseGui` を未使用。
  - `ruviform-audio-plugin-bridge`: `InputStageCounters`/
    `NodeStatusSnapshot` field 再編・`release_all`/`active_notes`
    削除 — synvelis は `InputStage`/`NodeStatusSnapshot`/
    `ledger_overflows` を未使用（自身の `probe_release_all` は
    別概念の product 側 semantics）。
  - `ruviform-ui-test`: driver 語彙移行 — synvelis-app tests は
    既に `FakeDriver`/`DriverEvent`/`WindowSpec`/`ScenarioDriver`
    の新語彙で記述済みで、消費する scenario/driver API の差分は
    additive のみ。
- 新規 API・新規 crate の採用判断は「今回は採用しない」で統一
  （design doc 判断表・spec Deferred Design Concerns に記録）。
  plugin-dock scenario・`PluginHostProbe`・`uitest` feature・
  `plugin-test-vst3`・`audio-test`・`io-e2e`・`fault-injection`・
  endpoint probe は全て別 slice の判断事項。
- MIDI 振る舞いは upstream 修正（V9/V11/V12 contract・
  `PluginNode` ledger への note-state 権威移管）どおり改善
  される。synvelis 側のコード変更は不要。
- `synvelis-recording` の `frame_position` 経路は既存消費経路の
  継続。今回の `wasapi_input` 差分は `permission_denied` 分類の
  additive のみで、実機効果の検証は `real-device` opt-in lane
  の管轄。

## Related Experiments

### Adopted

- なし（新規 upstream 機能の採用なし）。

### Rejected

- `ruviform-plugin-dock-test`・`ruviform-plugin-test-vst3`・
  `ruviform-audio-test`・`ruviform-io-e2e`: 依存木外の test
  基盤。GUI-L2/EXT-L3・Test Runtime・IO 試験要件の別 slice
  判断事項。
- `ruviform-ui-gpui` `uitest` feature・`ruviform-ui-test`
  `plugin_dock` scenario/`PluginHostProbe`: GUI scenario 基盤の
  設計判断事項。
- `ruviform-io-file` `fault-injection` feature: SYN-IO 試験
  要件確定時。
- endpoint probe examples・ParentLease/wire v5 語彙: synvelis
  未使用面。将来使用時の移行要否を doc に記録。

## Validation

環境: Windows 11・mise 管理 toolchain（rustc は `mise.toml`/
`mise.ci.toml` pin）・worktree `G:/orca/workspaces/synvelis/
sv28-pin`・branch `chore/28-ruviform-pin-ac10a2f7`・
base `origin/main` = `d54d7ae`。全コマンド `mise run` 経由。

| Lane | コマンド | 結果 |
| --- | --- | --- |
| format | `mise run fmt-check` | Pass (exit 0) |
| check ×14 | `mise run check <pkg>`（synvelis-theme/project/library/recording/recording-poc/player/render/workspace/gateway/automation/script/cli/mcp-poc/app） | Pass (全 package exit 0、`--all-targets` で test code 含む) |
| lint ×14 | `mise run lint <pkg>`（同上 14 package、warnings denied） | Pass (全 package exit 0) |
| test ×14 | `mise run test <pkg>`（同上 14 package) | Pass (全 package exit 0) |
| RT | `mise run test-rt` | Pass (exit 0) |
| headless deps | `mise run check-headless-deps` | Pass (`no GUI framework deps in 12 headless crates`) |
| pin/vendor | `mise run sync-ui-deps` → `node tools/check-ruviform-rev.mjs --require-vendor` | Pass (`synced ruviform ui/* @ ac10a2f776` / `resolved ac10a2f7761c08db73aebd861c0a5c1f53f269b7 (27 lockfile sources, lockfile-primary, REV match)`) |
| UI | `mise run check-ui` / `mise run test-ui` | Pass (typecheck exit 0 / 8 files・73 tests passed) |
| app webview | `mise run check-app-webview` / `lint-app-webview` / `test-app-webview` | Pass (全て exit 0) |
| app native | `mise run check-app-native` / `lint-app-native` / `build-app-native` | Pass (全て exit 0。`synvelis-native` bin build 成功) |
| app gpui | `mise run check-app-gpui` / `lint-app-gpui` / `test-app-gpui` | Pass (全て exit 0。GPUI-simulated scenario lane 非回帰) |
| real-device lint | `mise run lint-recording-real-input` | Pass (exit 0。`real-device` feature compile 確証) |
| hooks | `mise run prek` | Pass (exit 0) |
| lockfile diff | `git diff Cargo.lock` | 54 行の source fragment 更新のみ（27×-/+）。ruviform 以外の差分 0 件 |

### NotRun / Unsupported（正直な記録）

- `mise run test-recording-real-input`: **NotRun**。実オーディオ
  デバイスを要する opt-in 実機 lane であり、本 slice の検証
  範囲外（`lint-recording-real-input` で compile 面は検証済み）。
- 実 OS GUI（`run-app-native` 実窓操作）: **NotRun**。手動・実機
  検証入口であり自動 lane 対象外。
- 実 plugin/VST3・CLAP ABI: **Unsupported/NotRun**。実 ABI
  plugin は EXT-L3 要件 slice の管轄。
- 両 consumer（synvelis × audio-lens 等）横断検証: **NotRun**。
  別リポジトリの検証環境が必要。
- `*-msrv` / `*-full` lane: **NotRun**。ユーザー指定により
  本 slice では未実施。
- ruviform `uitest`/`real-scenario` windowed lane・
  `plugin_dock` scenario・io-e2e fault-injection lane: **NotRun**。
  synvelis が当該 feature/crate を採用していないため対象外。

## Risks

- 大規模差分（108 files）の見落とし: crate 別評価 + `pub`
  削除行スキャン + 消費面 grep + `check --all-targets` で担保。
  evidence メモに記録済み。
- MIDI 振る舞い変化: upstream 修正は `PluginNode` ledger への
  note-state 権威移管で、synvelis 消費面は不変。`test` lane が
  非回帰を検証。
- 将来の API 追従: ParentLease/wire v5 を使う場合は語彙移行が
  必要 — spec の Deferred Design Concerns に記録。

## Follow-ups

- Issue #28 は継続 tracker として open のまま。次回 pin slice
  で `ac10a2f7..` の差分を同様に評価する。
- 新規 upstream 機能（plugin-dock・test-vst3・audio-test・
  io-e2e・uitest・fault-injection）の採用判断は、対応する
  GUI-L2/EXT-L3・Test Runtime・IO 試験要件の slice で行う。
- 実機 lane（`test-recording-real-input`・実 GUI・実 plugin）の
  検証は opt-in 環境で別途実施。
