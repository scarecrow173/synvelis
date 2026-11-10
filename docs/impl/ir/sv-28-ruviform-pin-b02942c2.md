---
id: "IMPL-034VocnpYlIXEqErLG2I5w"
type: "impl"
status: "completed"
title: "SV#28 Ruviform pin 更新 — 0f5b5737 → b02942c2 API 追従"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/specs/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/plans/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/test-specs/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/tasks/sv28-ruviform-pin-b02942c2.md"
      - type: "file"
        path: "docs/impl/ir/sv-28-ruviform-pin-b02942c2.md"
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
    - "docs/tasks/sv28-ruviform-pin-b02942c2.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
    - "docs/designs/sv28-ruviform-pin-b02942c2.md"
    - "docs/plans/sv28-ruviform-pin-b02942c2.md"
    - "docs/tasks/sv28-ruviform-pin-b02942c2.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv28-ruviform-pin-b02942c2.md"
    - "docs/discovery/sv28-ruviform-pin-b02942c2.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-b02942c2.md"
  references:
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV#28 Ruviform pin 更新 — 0f5b5737 → b02942c2 API 追従

## Summary

SV#28 tracker の pin 更新 1 回分として、`Cargo.lock` の ruviform
git 依存解決 revision を `0f5b5737` から ruviform `main` HEAD
`b02942c2f9e6e195d0d752803b077866468fe58e` へ更新した。
`0f5b5737..b02942c2` の 80 commits（RF#253 plugin→host report
配線・VST3 MIDI 正規経路・`ruviform-midi-device` 新設、
RF#293/OBS-11 `ProbeSourceAdapter`・observability `integration`、
OBS-03/08/09/10 opt-in feature、RF#314/RF#388/RF#552 GPUI
real-scenario/real-window/OS input lane、RF#464 scenario runner
拡張、Issue #410 `ruviform-ui` 内部修正、Issue #418 ui-theme
codegen、Issue #389 plugin editor session hardening、IO-02/03/04/06
・Issue #251 の新規 IO/audio-device crate 等）を消費可能に
なった。公開 API の削除・シグネチャ変更は実質なく、SV 側の
追従修正は不要だった。新規 crate・新 API（`real-scenario`/
`ProbeSourceAdapter`/`ReportDrain`/observability opt-in
feature）は本 slice では採用せず、後続 Issue 候補として記録。

## Context

- `docs/tasks/sv28-ruviform-pin-b02942c2.md`（本タスク）
- `docs/plans/sv28-ruviform-pin-b02942c2.md` /
  `docs/designs/sv28-ruviform-pin-b02942c2.md` /
  `docs/specs/sv28-ruviform-pin-b02942c2.md` /
  `docs/discovery/sv28-ruviform-pin-b02942c2.md` /
  `docs/test-specs/sv28-ruviform-pin-b02942c2.md`（文書チェーン）
- 先行 pin 更新: PR #193（`0f5b5737` への更新）
- 再開作業: 前任 Worker のプロセスクラッシュ（0xc0000409）で
  中断した dispatch を引き継ぎ。worktree に残っていた未コミット
  `Cargo.lock` 更新が `0f5b5737` → `b02942c2` の pin 更新と
  一致することを確認して続行した（不整合なし・やり直し不要）。

## Implementation

### Changes

- `Cargo.lock`: `cargo update -p ruviform-*`（全 ruviform
  workspace 依存を `-p` 指定）で全 ruviform source を
  `b02942c2` へ再解決。差分は 26 箇所全てが `ruviform-*` の
  `source = "git+...#<sha>"` fragment の SHA 更新のみで、
  推移的依存を含む他パッケージの version drift なし。
  `ruviform-midi-device` は synvelis の依存木に含まれず
  lockfile に非出現。
- `crates/*/src/`: 変更なし。API 差分調査（discovery doc）の
  結果、公開 API の削除・シグネチャ変更は実質なく追従修正は
  不要。`mise run check`（workspace・全 target）で実コンパイル
  確認済み。
- `ui/vendor/ruviform/`: `mise run sync-ui-deps` で新 SHA へ
  再生成（gitignore 対象の生成物）。`REV` =
  `b02942c2f9e6e195d0d752803b077866468fe58e`。
- `docs/*/README.md`: `doc_maintenance apply --force-index` による
  managed index 再生成（本タスクの 6 文書分）。
  `docs/impl/ir/README.md` は hand-curated のため手動で 1 行追記。

### Decisions During Implementation

- pin 更新は repo 規約どおり `cargo update -p`（既定ブランチ追従、
  `--precise` 不使用）で実施。`Cargo.toml` の依存定義は不変。
- ruviform 差分の互換性評価は `git log 0f5b5737..b02942c2`（80
  commits）+ cargo checkout 間 `diff -rq`/`diff` による crate 別
  変更集計 + `pub` item 削除行スキャン + synvelis 側 import 面の
  grep 照合で実施。消費 26 crate 中変更は 10 crate、残り 16
  crate は無変更。
- 唯一の root re-export 削除（`ruviform-ui-test` の
  `scenario::visual::{CaptureOutcome, VisualGuiTest,
  record_golden_step}`）は item が `scenario::visual::*` で
  存続し synvelis 未使用のため対応不要。`ruviform-ui-gpui` の
  `OsCapturedFrame` re-export は `OsWindowGeometry` 追加への
  拡張。
- 新規 API・新規 crate の採用判断は「今回は採用しない」で統一
  （discovery doc の「新 API の採用判断」節・spec の Deferred
  Design Concerns に記録）。
- 検証中に `synvelis-library` の webdav test
  （`open_pinned_response_etag_mismatch_is_source_changed`）が
  1 度だけ `TransientIo`（os error 10053、ローカル mock server
  への書込み中断）で fail。単独再実行で pass、`mise run test`
  全件再実行でも pass を確認。pin 非起因の flaky（同 crate の
  ruviform 依存は無変更の `ruviform-io-file` のみ）と切り分け。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

Windows 11・mise 管理 rust 1.99.0・cargo 1.99.0 で実施
（`git merge origin/main` で `9a75336` 取り込み済みの状態）。

| 検証 | 結果 |
| --- | --- |
| `mise run check`（`cargo check --workspace --all-targets`） | Passed（exit 0） |
| `mise run lint`（`cargo clippy --workspace --all-targets -- -D warnings`） | Passed（exit 0） |
| `mise run test`（`cargo test --workspace --locked`） | Passed（exit 0、183 test binary 全て ok。初回実行で webdav test が 1 件 flaky fail → 単独・全件再実行で pass） |
| `mise run test-rt`（`cargo test --workspace --locked rt_`） | Passed（exit 0） |
| `mise run check-headless-deps` | Passed（exit 0、10 headless crates に GUI 系依存なし） |
| `mise run fmt-check` | Passed（exit 0） |
| `mise run check-app-webview` | Passed（exit 0） |
| `mise run lint-app-webview` | Passed（exit 0） |
| `mise run test-app-webview` | Passed（exit 0） |
| `mise run check-app-native` | Passed（exit 0） |
| `mise run lint-app-native` | Passed（exit 0） |
| `mise run check-ui`（vendor 照合 + tsc --noEmit） | Passed（exit 0） |
| `mise run test-ui`（vitest） | Passed（exit 0、56 tests） |
| `node tools/check-ruviform-rev.mjs --require-vendor` | Passed（`resolved b02942c2…`、26 sources、REV match） |
| `node tools/check-headless-deps.mjs` | Passed（`mise run check-headless-deps` 経由） |

NotRun: 実 GPUI ウィンドウ・実 OS 入力・実 plugin/VST3・両
Consumer 横断の検証は本タスク範囲外（Unsupported/NotRun として
記録。実機 lane は CI/手動検証に委ねる）。Issue #28 は Open の
まま。

## Risks

- Issue #410 の `ruviform-ui` 内部修正（projection revision
  floor・command ledger bounded retention）は API を変えないが
  consumer の観測挙動を変え得る。既存 contract test は全て
  pass したが、実 GUI lane での確認は NotRun のまま。
- `ruviform-ui-test` の root re-export 削除は将来 synvelis が
  `scenario::visual` item を root path で import する場合に
  影響し得る（現状未使用・module path で存続）。
- 新規 API（`ruviform-midi-device`/`real-scenario`/
  `ProbeSourceAdapter`/`ReportDrain`/observability opt-in
  feature）の採用判断は後続タスクに委ねる。

## Follow-ups

- 新規 ruviform API の consumer 採用判断（`ruviform-midi-device`、
  `real-scenario`/`real_window`/`os_input`、`ProbeSourceAdapter`、
  `ReportDrain`/`drain_output_reports`、observability opt-in
  feature）は必要に応じて後続 Issue で扱う。
- SV#28 tracker は Open のまま継続。次回 pin 更新は main HEAD
  追従で実施。
