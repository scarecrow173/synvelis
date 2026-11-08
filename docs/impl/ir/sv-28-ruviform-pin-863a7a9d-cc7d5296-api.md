---
id: "IMPL-034Ug8sNPL1sqhquNm1PYh"
type: "impl"
status: "completed"
title: "SV#28 Ruviform pin 更新 — 863a7a9d → cc7d5296 API 追従"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/sv28-ruviform-pin-update.md"
      - type: "file"
        path: "docs/specs/sv28-ruviform-pin-cc7d5296.md"
      - type: "file"
        path: "docs/designs/sv28-ruviform-pin-cc7d5296.md"
      - type: "file"
        path: "docs/plans/sv28-ruviform-pin-cc7d5296.md"
      - type: "file"
        path: "docs/test-specs/sv28-ruviform-pin-cc7d5296.md"
      - type: "file"
        path: "docs/tasks/sv28-ruviform-pin-update.md"
      - type: "file"
        path: "docs/impl/ir/sv-28-ruviform-pin-863a7a9d-cc7d5296-api.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/adr/README.md"
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
    - "docs/tasks/sv28-ruviform-pin-update.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/designs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/plans/sv28-ruviform-pin-cc7d5296.md"
    - "docs/tasks/sv28-ruviform-pin-update.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/discovery/sv28-ruviform-pin-update.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-cc7d5296.md"
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

# SV#28 Ruviform pin 更新 — 863a7a9d → cc7d5296 API 追従

## Summary

SV#28 tracker の pin 更新 1 回分として、`Cargo.lock` の ruviform
git 依存解決 revision を `863a7a9d` から ruviform `main` HEAD
`cc7d5296d3fbc65ce9b925834de4dadf5db666d9` へ更新した。
`863a7a9d..cc7d5296` の 15 commits（RF#464 Scenario Contract、
RF#314 Golden slice1/2、RF#451 FU-1b、RF#243/#482 lifecycle
probe、RF#429 seam 残 DoD、RF#465 P0/P1 棚卸し、RF#461/#463
GPUI 修正等）を消費可能になった。公開 API の削除・変更は
確認されず、SV 側の追従修正は不要だった。

## Context

- `docs/tasks/sv28-ruviform-pin-update.md`（本タスク）
- `docs/plans/sv28-ruviform-pin-cc7d5296.md` /
  `docs/designs/sv28-ruviform-pin-cc7d5296.md` /
  `docs/specs/sv28-ruviform-pin-cc7d5296.md` /
  `docs/discovery/sv28-ruviform-pin-update.md`（文書チェーン）
- 先行 pin 更新: PR #185（`fa98f41`、769d595a → 863a7a9d +
  WebView host 新 API 移行）

## Implementation

### Changes

- `Cargo.lock`: `cargo update -p ruviform-*`（21 パッケージ指定、
  推移的に 26 パッケージ全て）で全 ruviform source を
  `cc7d5296` へ再解決。差分は SHA fragment の更新と
  `ruviform-ui-test` の新規依存 `base64 0.22.1`/`png 0.18.1`
  の dependency 列挙追加のみ（両者とも既存 lock tree 内、
  ruviform 側 Cargo.toml の宣言どおり）。他の crates.io 依存に
  version drift なし。
- `crates/*/src/`: 変更なし。API 差分調査（discovery doc）の
  結果、公開 API の削除・シグネチャ変更はなく追従修正は不要。
- `ui/vendor/ruviform/`: `mise run sync-ui-deps` で新 SHA へ
  再生成（gitignore 対象の生成物）。ruviform `ui/*` TS は
  pin 間で無変更のため内容差分は REV のみ。
- `docs/*/README.md`: `doc_maintenance apply --force-index` による
  managed index 再生成（本タスクの 6 文書に加え、index 未収録
  だった SF-03 slice1/slice2 spec 等の既存文書が追記された）。

### Decisions During Implementation

- pin 更新は repo 規約どおり `cargo update -p`（既定ブランチ追従、
  `--precise` 不使用）で実施。`Cargo.toml` の依存定義は不変。
- ruviform 差分の互換性評価は `git diff 863a7a9d..cc7d5296` の
  公開 API 面スキャン（`pub` item 削除行ゼロ）+ consumer 利用面
  （`ruviform-ui`/`ui-webview`/`ui-gpui`/`ui-test`）の個別確認で
  実施。`ruviform-ui-webview` は pin 間で無変更。
- RF#465 legacy WebView API（`HostConfig`/`run`/`HostEvent` 等）の
  物理削除は P2 で本 pin に未収録。synvelis webview bin は
  `SessionConfig`/`WebViewWindowConfig`/`run_multi` を使用しており
  （fa98f41 で移行済み）、削除済み API への依存は存在しない。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

Windows 11・mise 管理 rust 1.99.0・cargo 1.99.0 で実施。

| 検証 | 結果 |
| --- | --- |
| `mise run check`（`cargo check --workspace --all-targets`） | Passed（exit 0） |
| `mise run lint`（`cargo clippy --workspace --all-targets -- -D warnings`） | Passed（exit 0） |
| `mise run test`（`cargo test --workspace --locked`） | Passed（exit 0、全 suite ok） |
| `mise run test-rt`（`cargo test --workspace --locked rt_`） | Passed（exit 0） |
| `mise run check-headless-deps` | Passed（exit 0、GUI 系依存なし） |
| `mise run check-app-webview` | Passed（exit 0） |
| `mise run lint-app-webview` | Passed（exit 0） |
| `mise run test-app-webview` | Passed（exit 0、全テスト ok） |
| `mise run check-app-native` | Passed（exit 0） |
| `mise run lint-app-native` | Passed（exit 0） |
| `mise run check-ui`（tsc --noEmit + vendor 照合） | Passed（exit 0） |
| `mise run test-ui`（vitest） | Passed（exit 0、56 tests） |
| `node tools/check-ruviform-rev.mjs --require-vendor` | Passed（`resolved cc7d5296…`、26 sources、REV match） |
| `mise run fmt-check` | **Failed — main 既存の破損**。`crates/synvelis-player/src/routing_engine.rs` の rustfmt 差分 3 箇所（PR #192 由来、当該ファイルは本変更で無変更・origin/main と同一）。pin 更新とは無関係のためスコープ外として未修正・報告。 |

NotRun: 実 GPUI ウィンドウ・実 OS 入力を伴う検証（`run-app-native`、
window lifecycle probe 等）は本タスク範囲外。実機 lane は CI/手動
検証に委ねる。

## Risks

- `fmt-check` の main 既存失敗は後続の全 PR の fmt ゲートを赤く
  し続ける可能性がある。`routing_engine.rs` の rustfmt 正規化
  （3 箇所の折り畳み）は機械的修正で済むが、本タスクの編集範囲
  外のため未対処。別 Issue/PR での対応を推奨。
- RF#464 Scenario Contract / RF#314 Golden の consumer 側利用は
  未着手（pin 更新のみ）。`ruviform-ui-test` の `scenario`/`golden`
  module は新 pin で利用可能になった段階。

## Follow-ups

- `crates/synvelis-player/src/routing_engine.rs` の fmt 正規化
  （main 既存破損、別タスクで対応）。
- SV#190 等の後続作業で RF#464/RF#314 の consumer 側テスト基盤を
  利用するかの判断。
- RF#465 P2: legacy WebView adapter 面の物理削除と webview lane
  の退役/移行判断（Coordinator 側の別 Issue）。
