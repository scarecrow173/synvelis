---
id: "TASK-034VocaCYmo2jkluo0P1eY"
type: "task"
status: "done"
title: "SV#28 Ruviform pin を b02942c2 へ更新し検証を完了する"
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
    - "docs/plans/sv28-ruviform-pin-b02942c2.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv28-ruviform-pin-b02942c2.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
    - "docs/designs/sv28-ruviform-pin-b02942c2.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
    - "docs/designs/sv28-ruviform-pin-b02942c2.md"
    - "docs/plans/sv28-ruviform-pin-b02942c2.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-b02942c2.md"
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin を b02942c2 へ更新し検証を完了する

## Work

1. `cargo update -p ruviform-*`（全 ruviform workspace 依存を
   `-p` 指定）で `Cargo.lock` を ruviform `main` HEAD
   `b02942c2f9e6e195d0d752803b077866468fe58e` へ再解決する。
2. `git diff Cargo.lock` で pin 更新以外の drift がないことを確認
   する（差分は 26 箇所全て ruviform source fragment の SHA 更新）。
3. `mise run sync-ui-deps` で `ui/vendor/ruviform/` を新 SHA へ
   再生成し、`node tools/check-ruviform-rev.mjs --require-vendor`
   で三者一致を確認する。
4. `mise run fmt-check` / `check` / `lint` を実行し、API 非互換が
   あれば `crates/*/src/` を最小追従修正して再検証する。
5. `mise run test` / `test-rt` / `check-headless-deps` を実行する。
6. `mise run check-app-webview` / `check-app-native` /
   `lint-app-native`（および `lint-app-webview` /
   `test-app-webview`）で feature lane のコンパイル契約を検証する。
7. `mise run check-ui` / `test-ui` で frontend lane の非回帰を
   確認する。
8. `git fetch origin main && git merge origin/main` で最新 main を
   取り込み、生成物を再生成する。
9. 文書チェーンと pin 更新を論理単位でコミットし、
   `chore/28-ruviform-pin-b02942c2` を push して PR を作成する。
   impl record を `docs/impl/ir/` に記録する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sv28-ruviform-pin-b02942c2.md`
- `docs/specs/sv28-ruviform-pin-b02942c2.md`
- `docs/designs/sv28-ruviform-pin-b02942c2.md`
- `docs/discovery/sv28-ruviform-pin-b02942c2.md`

## Dependencies

- なし（ブロッカーなし）。ruviform `main` HEAD は
  `b02942c2` を `git ls-remote` で確認済み。作業中に前進した
  場合は最新 HEAD を採用する。

## Affected Files

- `Cargo.lock` — ruviform 解決 revision の更新
- `crates/*/src/` — API 非互換検出時のみ最小追従修正
- `ui/vendor/ruviform/` — `sync-ui-deps` 生成物（コミット対象外）
- `docs/{discovery,specs,designs,plans,test-specs,tasks,impl}/` —
  本タスクの文書チェーンと実装記録

## Done When

- [x] `Cargo.lock` の全 ruviform source が `b02942c2` に解決され、
  差分が pin 更新のみであることを確認済み。
- [x] `check-ruviform-rev --require-vendor` が pass。
- [x] 正規 lane 全件を実走して非回帰を確認（失敗時は pin 起因か
  既存/flaky かを切り分けて記録）。
- [x] 影響表と新規 API の採用判断（今回は採用しない）を文書・
  PR 本文に記録済み。
- [x] 実 OS/実 GUI/実 plugin/両 Consumer 横断は NotRun /
  Unsupported と記録。Issue #28 は Open のまま。

## Verification

- 対応 test-spec: [sv28-ruviform-pin-b02942c2](../test-specs/sv28-ruviform-pin-b02942c2.md)
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-headless-deps` が pass（`fmt-check` が
  main 既存破損で fail する場合は切り分けて記録）。
- [x] `mise run check-app-webview` / `check-app-native` /
  `lint-app-native` / `test-app-webview` が pass。
- [x] `mise run check-ui` / `test-ui` が pass。
- [x] `node tools/check-ruviform-rev.mjs --require-vendor` が
  pass。
- [x] 実 GPUI・実 OS・実 plugin 検証は NotRun として PR に記録。
