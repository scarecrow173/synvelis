---
id: "TASK-ytgLcFdP0Y0EeeQKr0JW9t"
type: "task"
status: "done"
title: "SV#28 Ruviform pin を fd1d4d5f へ更新し検証を完了する"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added: []
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
    - "docs/plans/sv28-ruviform-pin-fd1d4d5f.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv28-ruviform-pin-fd1d4d5f.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/plans/sv28-ruviform-pin-fd1d4d5f.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-fd1d4d5f.md"
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin を fd1d4d5f へ更新し検証を完了する

## Work

1. `cargo update -p ruviform-*`（全 27 ruviform workspace 依存を
   `-p` 指定）で `Cargo.lock` を ruviform `main` HEAD
   `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` へ再解決する。
2. `git diff Cargo.lock` で pin 更新以外の drift がないことを確認
   する（差分は 27 箇所全て ruviform source fragment の SHA 更新
   ＋依存リストへの `windows 0.62.2`/`windows-core 0.62.2`/`x11rb`
   参照追加のみ）。
3. `mise run sync-ui-deps` で `ui/vendor/ruviform/` を新 SHA へ
   再生成し、`node tools/check-ruviform-rev.mjs --require-vendor`
   で三者一致を確認する。
4. `mise run fmt-check` / `check` / `lint` を実行する。API 非互換が
   あれば差分を記録する（`crates/` は別 Worker 占有のため本タスク
   では修正しない）。
5. `mise run test` / `test-rt` / `check-headless-deps` /
   `test-msrv` を実行する。
6. `mise run check-app-webview` / `check-app-native` /
   `lint-app-native`（および `lint-app-webview` /
   `test-app-webview`）で feature lane のコンパイル契約を検証する。
7. `mise run check-ui` / `test-ui` で frontend lane の非回帰を、
   `mise run lint-recording-real-input` で real-device feature
   （wasapi_input 経路を含む）の compile を検証する。
8. `git fetch origin main && git merge origin/main` で最新 main を
   取り込み、生成物を再生成する。
9. 文書チェーンと pin 更新を論理単位でコミットし、
   `chore/28-ruviform-pin-fd1d4d5f` を push して PR を作成する。
   impl record を `docs/impl/ir/` に記録する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sv28-ruviform-pin-fd1d4d5f.md`
- `docs/specs/sv28-ruviform-pin-fd1d4d5f.md`
- `docs/designs/sv28-ruviform-pin-fd1d4d5f.md`
- `docs/discovery/sv28-ruviform-pin-fd1d4d5f.md`

## Dependencies

- なし（ブロッカーなし）。ruviform `main` HEAD は
  `fd1d4d5f` を ruviform local repo の `git rev-parse
  origin/main` で確認済み。作業中に前進した場合は最新 HEAD を
  採用する。

## Affected Files

- `Cargo.lock` — ruviform 解決 revision の更新
- `ui/vendor/ruviform/` — `sync-ui-deps` 生成物（コミット対象外）
- `docs/{discovery,specs,designs,plans,test-specs,tasks,impl}/` —
  本タスクの文書チェーンと実装記録

## Done When

- [x] `Cargo.lock` の全 ruviform source が `fd1d4d5f` に解決され、
  差分が pin 更新＋依存リスト参照追加のみであることを確認済み。
- [x] `check-ruviform-rev --require-vendor` が pass。
- [x] 正規 lane 全件を実走して非回帰を確認（失敗時は pin 起因か
  既存/flaky かを切り分けて記録）。
- [x] 影響表と新規 API の採用判断（今回は採用しない）を文書・
  PR 本文に記録済み。
- [x] 実 OS/実 GUI/実 plugin/両 Consumer 横断は NotRun /
  Unsupported と記録。Issue #28 は Open のまま。

## Verification

- 対応 test-spec: [sv28-ruviform-pin-fd1d4d5f](../test-specs/sv28-ruviform-pin-fd1d4d5f.md)
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-headless-deps` / `test-msrv` が pass
  （`fmt-check` が main 既存破損で fail する場合は切り分けて記録）。
- [x] `mise run check-app-webview` / `check-app-native` /
  `lint-app-native` / `test-app-webview` が pass。
- [x] `mise run check-ui` / `test-ui` が pass。
- [x] `node tools/check-ruviform-rev.mjs --require-vendor` が
  pass。
- [x] 実 GPUI・実 OS・実 plugin・実 audio device 検証は NotRun
  として PR に記録。
