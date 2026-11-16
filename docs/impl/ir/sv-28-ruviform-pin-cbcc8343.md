---
id: "IMPL-0mcr9rVaeMBEuTzB4Zcpv4"
type: "impl"
status: "completed"
title: "SV#28 Ruviform pin 更新 — abb9f372 → cbcc8343 API 追従評価"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/specs/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/plans/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/test-specs/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/tasks/sv28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "docs/impl/ir/sv-28-ruviform-pin-cbcc8343.md"
      - type: "file"
        path: "evidence/sv28/ruviform-cbcc8343-diff-review.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/tasks/sv28-ruviform-pin-cbcc8343.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/designs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/plans/sv28-ruviform-pin-cbcc8343.md"
    - "docs/tasks/sv28-ruviform-pin-cbcc8343.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv28-ruviform-pin-cbcc8343.md"
    - "docs/discovery/sv28-ruviform-pin-cbcc8343.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-cbcc8343.md"
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
    - "mise.toml"
    - "evidence/sv28/ruviform-cbcc8343-diff-review.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV#28 Ruviform pin 更新 — abb9f372 → cbcc8343 API 追従評価

## Summary

synvelis workspace の ruviform 解決 revision を
`abb9f372d3ee95ee824b3c4f7dc68e8083ce2774` から
`cbcc83437616fdd7418da37183c6be66b6cfe225`（ruviform `main`
HEAD、12 commits 前進）へ更新した。`Cargo.lock` の 28
`ruviform-*` package source を scoped `cargo update -p` で
更新し、`ui/vendor/ruviform` を `sync-ui-deps` で再生成した。
upstream 差分精査の結果、公開 API の破壊的変更は 0 件
（`pub` 削除行スキャン 0 件の純粋 additive 範囲）で、依存木内
4 crate の差分は全て additive/内部修正。製品コード変更なしで
canonical 検証 lane を実施した。`lint-app-native` /
`lint-app-gpui` は `scarecrow173/synvelis#488` 由来の既存
`clippy::large_enum_variant`（`synvelis-workspace` の
`LibraryAction`、serde_json `preserve_order` feature 統合で
`Value` が肥大化する潜在問題）で fail — pin 非起因として
切り分け記録し、当該 crate は修正していない。

## Context

- Issue #28 は Ruviform 継続追従 tracker。pin の正本は
  `Cargo.lock` のみ（`Cargo.toml` の git 依存は `rev` 未指定）。
- dispatch 時点の ruviform `main` HEAD は spec 起稿時と同一の
  `cbcc8343`（前進なし）。`abb9f372` は `cbcc8343` の ancestor。
  `scarecrow173/audio-lens#127` が同一 SHA で rev-watch slice
  を実施中（candidate tuple 整合）。
- 差分規模: `crates/` 57 files・+17,074/−558・10 crate 変更。
  大半は dep-tree-external の test plugin / dock test 基盤
  （Issue #630/#631/#532）。依存木内の差分は 4 crate のみ。
- `ruviform-audio-test` は dev-dep として依存木入り済み
  （2026-11-15 承認）。今回初めて同 crate に差分が出たが
  additive のみで既存 rf628 oracle と非衝突。

## Implementation

### Changes

- `Cargo.lock`: `cargo update -p`（28 package 全指定）で
  `ruviform-*` の `source = "git+...#<sha>"` fragment を
  `abb9f372…` → `cbcc8343…` へ更新。差分は 30 insertions /
  28 deletions — 28 package の fragment -/+ ペア（56 行）+
  `ruviform-audio-test` dependencies への `serde`・
  `serde_json` 2 行追加（upstream `Cargo.toml` の新規 dep を
  反映。両 crate は lockfile 既存: serde 1.0.229・
  serde_json 1.0.154）。依存リスト・version・新規 package の
  他の変動なし（`git diff Cargo.lock` で確認）。ruviform
  package 数 28 → 28。
- `ui/vendor/ruviform/`: `mise run sync-ui-deps` で
  `cbcc834376` へ再生成（gitignore 対象の生成物）。`ui/`
  （TypeScript）差分は upstream で 0 件のため REV bump のみ。
- `crates/*/src/`: 変更なし。消費面の破壊的変更 0 件のため
  追従修正は不要。
- `evidence/sv28/ruviform-cbcc8343-diff-review.md`: upstream
  差分精査メモ（crate 別影響・`pub` 削除スキャン・採用判断）。
- `docs/{discovery,specs,designs,plans,test-specs,tasks}/
  sv28-ruviform-pin-cbcc8343.md`・`docs/impl/ir/
  sv-28-ruviform-pin-cbcc8343.md`: 7-doc chain。
- docs index（`docs/**/README.md`）は**変更しない** — Spec
  で明示禁止（lease 外）。`doc_maintenance.js index plan` /
  `index apply` をローカルで実行し plan → apply →
  `blocked` 0 件・全 target `in-sync` を確認したが、
  生成された index 差分は PR から除外し、Coordinator が
  統合時に再生成する。

### Decisions During Implementation

- pin 更新は repo 規約どおり `cargo update -p`（既定ブランチ
  追従、`--precise` 不使用）で実施。`Cargo.toml` 不変。
- upstream 差分評価は ruviform repo の `git log`/`git diff
  abb9f372..cbcc8343`（12 commits）+ `pub` 削除行スキャン
  （削除行 615 行中 `pub` 含有 0 件）+ 依存木交差判定 +
  synvelis 消費面 grep で実施。詳細は evidence メモ。
- 依存木内 4 crate の差分は全て additive/内部修正:
  - `ruviform-audio-test`: serde derive・`RunMetadata` JSON
    メソッド・`StreamFault::Cancellation` variant（
    `#[non_exhaustive]`）追加。synvelis の rf628 oracle 消費
    API（`DeviceSpec`/`InputSpec`/`ScriptedBackend`/
    `ScriptedDeviceHandle`/`StreamScript`/`StreamDirection`/
    `PumpReport`/`clock`/`oracle`/`pcm`）は不変。
  - `ruviform-plugin-core`: `wire.rs` に
    `encode_event_record`/`decode_event_record` 追加
    （shm slot 固定 stride record codec）。
  - `ruviform-plugin-host`: `l1/data.rs` の RtLeg が shm event
    書き込みに `encode_event_record` を使う内部修正。
  - `ruviform-audio-device`: `examples/endpoint_probe.rs`・
    `tests/virtual_endpoint.rs` のみで `src/` 差分 0 件。
- 新規 API・新規 crate の採用判断は「今回は採用しない」で統一
  （design doc 判断表・spec Deferred Design Concerns に記録）。
- `lint-app-native`/`lint-app-gpui` の fail は
  `scarecrow173/synvelis#488` 由来の既存問題:
  `synvelis-workspace` の `LibraryAction` enum に対する
  `clippy::large_enum_variant`。発火条件は feature 統合 —
  `synvelis-app` の `native`/`gpui-test` feature 経路で
  `gpui_perf`→`gpui`→`ruviform-ui-gpui` 経由の
  `serde_json/preserve_order` が有効化され `serde_json::Value`
  が肥大化する（`cargo tree -e features` で確認）。
  `ruviform-ui-gpui` の Cargo.toml は本範囲で無変更・
  serde_json version も不変のため、旧 pin でも同一 fail が
  再現する既存破損。pin 非起因として記録し、
  `crates/synvelis-workspace` は修正していない
  （HARD BOUNDARY — `scarecrow173/synvelis#488` Worker 占有中）。

## Related Experiments

### Adopted

- なし（新規 upstream 機能の採用なし）。

### Rejected

- `RunMetadata::{to_json,from_json,save_json,load_json}` と
  serde derive: additive だが synvelis の rf628 oracle は
  `RunMetadata` 未消費。採用は test runtime 運用 slice の
  判断事項。
- `StreamFault::Cancellation`: additive variant。open-fault
  oracle への組込みは別 slice 判断事項。
- `wire::encode_event_record`/`decode_event_record`: plugin
  wire 固定 stride codec。synvelis は wire 層を直接使わない。
- `ruviform-plugin-test-{vst3,clap}` 新 fixture・
  `ruviform-plugin-dock-test` 拡張・`ruviform-audio-device`
  endpoint_probe/virtual_endpoint: dep-tree-external または
  example/test のみ。GUI-L2/EXT-L3・実 ABI Test Plugin・OS
  仮想 Endpoint 受入の別 slice 判断事項。

## Validation

環境: Windows 11・mise 管理 toolchain（rustc 1.99.1）・
worktree `G:/orca/workspaces/synvelis/sv28-cbcc8343`・
branch `feat/28-ruviform-cbcc8343`・base `origin/main` =
`f05bdbf`。全コマンド `mise run` 経由（cargo 直実行なし）。

| Lane | コマンド | 結果 |
| --- | --- | --- |
| format | `mise run fmt-check` | Pass (exit 0) |
| check ×14 | `mise run check <pkg>`（synvelis-theme/project/library/recording/recording-poc/player/render/workspace/gateway/automation/script/cli/mcp-poc/app） | Pass (全 package exit 0、`--all-targets` で test code 含む) |
| lint ×14 | `mise run lint <pkg>`（同上 14 package、warnings denied） | Pass (全 package exit 0。`synvelis-workspace` 単体でも exit 0 — 後述の #488 fail は feature 統合時のみ発火) |
| test ×14 | `mise run test <pkg>`（同上 14 package) | Pass (全 package exit 0。`synvelis-recording` の rf628 oracle 含む — `ruviform-audio-test` @ cbcc8343 で非回帰) |
| RT | `mise run test-rt` | Pass (exit 0) |
| headless deps | `mise run check-headless-deps` | Pass (`no GUI framework deps in 12 headless crates`) |
| pin/vendor | `mise run sync-ui-deps` → `node tools/check-ruviform-rev.mjs --require-vendor` | Pass (`synced ruviform ui/* @ cbcc834376` / `resolved cbcc83437616fdd7418da37183c6be66b6cfe225 (28 lockfile sources, lockfile-primary, REV match)`) |
| UI | `mise run check-ui` / `mise run test-ui` | Pass (typecheck exit 0 / 8 files・73 tests passed) |
| app webview | `mise run check-app-webview` / `lint-app-webview` / `test-app-webview` | Pass (全て exit 0) |
| app native | `mise run check-app-native` / `lint-app-native` / `build-app-native` | check Pass・build Pass (`synvelis-native` bin build 成功)。**lint Fail (exit 101)** — `scarecrow173/synvelis#488` 由来の既存 `clippy::large_enum_variant`（下記「既存 fail の切り分け」参照。pin 非起因・当該 crate 未修正） |
| app gpui | `mise run check-app-gpui` / `lint-app-gpui` / `test-app-gpui` | check Pass・test Pass。**lint Fail (exit 101)** — `lint-app-native` と同一の #488 由来 `clippy::large_enum_variant`（`LibraryAction`、gpui feature 経由の `serde_json/preserve_order` 統合で発火。同一機構・pin 非起因） |
| real-device lint | `mise run lint-recording-real-input` | Pass (exit 0) |
| hooks | `mise exec -- prek run --all-files` | Pass (exit 0) |
| lockfile diff | `git diff Cargo.lock` | 30 insertions/28 deletions — 28×source fragment + audio-test 依存 2 行。ruviform 以外の差分 0 件 |

### 既存 fail の切り分け（#488・pin 非起因の証拠）

- `mise run lint-app-native`（`cargo clippy -p synvelis-app
  --all-targets --features native -- -D warnings`）が
  `synvelis-workspace` lib の `LibraryAction` enum
  （`crates/synvelis-workspace/src/library_ui.rs:454`）で
  `clippy::large_enum_variant` を error 化（exit 101）。
- 発火機構: `synvelis-app` の `native` feature が
  `ruviform-ui-gpui`/`gpui` を有効化し、`gpui_perf` 経由で
  `serde_json/preserve_order` が feature 統合される
  （`cargo tree -p synvelis-app --features native -i serde_json
  -e features` で確認）。`preserve_order` で
  `serde_json::Value::Object` が `BTreeMap`→`IndexMap` となり
  `Value` が肥大化、`ImportSubmit`/`ImportRebase` variant
  （`serde_json::Value` field 保持）を含む enum が threshold
  超過。workspace member の path dep は `--cap-lints` 対象外の
  ため `-D warnings` がそのまま適用される。
- pin 非起因の根拠: 失敗 unit の入力は (a) synvelis-workspace
  のソース（本 slice で無変更）、(b) feature 統合された
  serde_json（version 1.0.154 で pin 前後不変）、(c)
  `ruviform-ui-gpui`→`gpui` の依存 spec（`abb9f372..cbcc8343`
  で `ruviform-ui-gpui` の Cargo.toml 無変更）。Cargo.lock
  差分は ruviform source fragment + audio-test 依存のみで、
  上記機構に関与する差分は 0 件。旧 pin でも同一 fail が
  再現する既存破損であり、`scarecrow173/synvelis#488` が
  まさにこの修正を扱っている。
- 単体 `mise run lint synvelis-workspace`（`-p
  synvelis-workspace --all-targets`）は exit 0 — 同 crate が
  gpui を含まない依存木で compile されるため `preserve_order`
  が統合されず lint が発火しない。両者の差は feature 統合の
  有無のみで、pin とは無関係。
- 対応: `crates/synvelis-workspace` は HARD BOUNDARY
  （`scarecrow173/synvelis#488` Worker 占有中）のため修正
  せず、証拠をここに記録して進める。

### NotRun / Unsupported（正直な記録）

- `mise run test-recording-real-input`: **NotRun**。実オーディオ
  デバイスを要する opt-in 実機 lane であり、本 slice の検証
  範囲外（`lint-recording-real-input` で compile 面は検証）。
- 実 OS GUI（`run-app-native` 実窓操作）: **NotRun**。手動・実機
  検証入口であり自動 lane 対象外。
- 実 plugin/VST3・CLAP ABI: **Unsupported/NotRun**。実 ABI
  plugin は EXT-L3 要件 slice の管轄。
- 両 consumer（synvelis × audio-lens 等）横断検証: **NotRun**。
  別リポジトリの検証環境が必要。
- `*-msrv` / `*-full` lane: **NotRun**。ユーザー指定により
  本 slice では未実施。
- ruviform `uitest`/`real-scenario` windowed lane・dock-test
  real_dock・io-e2e fault-injection lane: **NotRun**。
  synvelis が当該 feature/crate を採用していないため対象外。
- docs index 再生成: **Coordinator 委譲**。ローカルでは
  `doc_maintenance.js index plan`/`index apply` で生成可能
  であることと `blocked` 0 件・全 target `in-sync` を確認
  済みだが、index 差分は Spec の lease 外のため PR に
  含めない（`docs/**/README.md` は origin/main と同一に
  戻した）。

## Risks

- 大規模差分（57 files）の見落とし: crate 別評価 + `pub`
  削除行スキャン（0 件）+ 依存木交差判定 + `check
  --all-targets` で担保。evidence メモに記録済み。
- `ruviform-audio-test` 新 API との非互換: additive のみで
  既存消費面と非衝突。rf628 oracle の `test` lane が
  非回帰を検証（全件 Pass）。
- `plugin-host` `l1/data.rs` 内部修正: 公開 API 不変。
  synvelis は `l1` data plane を直接消費しない。
- #488 由来 fail との混同: 上記切り分け証拠を記録。当該
  crate は修正しない。

## Follow-ups

- Issue #28 は継続 tracker として open のまま。次回 pin slice
  で `cbcc8343..` の差分を同様に評価する。
- 新規 upstream 機能（RunMetadata JSON・Cancellation fault・
  event record codec・test plugin fixture・dock-test 拡張・
  endpoint probe）の採用判断は、対応する GUI-L2/EXT-L3・
  Test Runtime・IO 試験要件の slice で行う。
- `scarecrow173/synvelis#488` の `LibraryAction` 修正が main
  に merge 済み（`16de9ff`、base `f05bdbf` より後）のため、
  merge/rebase 後に `lint-app-native`/`lint-app-gpui` の
  再検証が必要（本 slice では pin 非起因の既存 fail として
  記録）。
- docs index の再生成は Coordinator が統合時に実施
  （`doc_maintenance index apply`。本 slice では
  `docs/**/README.md` を変更しない）。
- 実機 lane（`test-recording-real-input`・実 GUI・実 plugin）の
  検証は opt-in 環境で別途実施。
