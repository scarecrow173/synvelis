---
id: "TASK-DqvyFE4jtl1dHhjkpI0Awe"
type: "task"
status: "done"
title: "SV#28 Ruviform pin を cc7d5296 へ更新し API 追従・検証を完了する"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/plans/sv28-ruviform-pin-cc7d5296.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv28-ruviform-pin-cc7d5296.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/designs/sv28-ruviform-pin-cc7d5296.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/designs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/plans/sv28-ruviform-pin-cc7d5296.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-cc7d5296.md"
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "tools/check-headless-deps.mjs"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin を cc7d5296 へ更新し API 追従・検証を完了する

## Work

1. `cargo update -p ruviform-*`（全 ruviform workspace 依存を
   `-p` 指定）で `Cargo.lock` を ruviform `main` HEAD
   `cc7d5296d3fbc65ce9b925834de4dadf5db666d9` へ再解決する。
2. `git diff Cargo.lock` で pin 更新以外の drift がないことを確認
   する。
3. `mise run sync-ui-deps` で `ui/vendor/ruviform/` を新 SHA へ
   再生成し、`node tools/check-ruviform-rev.mjs --require-vendor`
   で三者一致を確認する。
4. `mise run fmt-check` / `check` / `lint` を実行し、API 非互換が
   あれば `crates/*/src/` を最小追従修正して再検証する。
5. `mise run test` / `test-rt` / `check-headless-deps` を実行する。
6. `mise run check-app-webview` / `check-app-native`（および
   必要に応じ `lint-app-webview` / `lint-app-native` /
   `test-app-webview`）で feature lane のコンパイル契約を検証する。
7. `git fetch origin main && git merge origin/main` で最新 main を
   取り込み、生成物を再生成する。
8. 文書チェーンと pin 更新を論理単位でコミットし、
   `chore/28-ruviform-pin-update` を push して PR を作成する。
   impl record を `docs/impl/ir/` に記録する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sv28-ruviform-pin-cc7d5296.md`
- `docs/specs/sv28-ruviform-pin-cc7d5296.md`
- `docs/designs/sv28-ruviform-pin-cc7d5296.md`

## Dependencies

- なし（ブロッカーなし）。ruviform `main` HEAD は
  `cc7d5296` を確認済み。作業中に前進した場合は最新 HEAD を採用する。

## Affected Files

- `Cargo.lock` — ruviform 解決 revision の更新
- `crates/*/src/` — API 非互換検出時のみ最小追従修正
- `ui/vendor/ruviform/` — `sync-ui-deps` 生成物（コミット対象外）
- `docs/{specs,designs,plans,test-specs,tasks,impl}/` — 本タスクの
  文書チェーンと実装記録

## Implementation Sequence

- [x] 文書チェーン作成（spec/design/plan/test-spec/task + discovery
  + impl record、managed index 再生成）。
- [x] pin 更新と lockfile 差分検査（`cargo update -p ruviform-*`、
  差分は SHA fragment + `ruviform-ui-test` の `png`/`base64`
  dependency 列挙のみ）。
- [x] vendor 再生成と `check-ruviform-rev --require-vendor` 確認
  （`resolved cc7d5296…`、26 sources、REV match）。
- [x] headless lane 検証: `check`/`lint`/`test`/`test-rt`/
  `check-headless-deps` は pass。`fmt-check` は main 既存の
  `routing_engine.rs` rustfmt 差分で fail（本変更と無関係、
  origin/main と同一ファイル。スコープ外として未修正・報告）。
- [x] feature lane 検証: `check-app-webview`/`lint-app-webview`/
  `test-app-webview`/`check-app-native`/`lint-app-native` は pass。
  補助として `check-ui`/`test-ui` も pass。
- [x] 最新 main 取り込み確認（HEAD == origin/main `3fabfef`、
  merge 不要を確認）。
- [ ] コミット・push・PR 作成（impl record 作成済み）。

## Verification

- [x] `mise run check` / `lint` / `test` / `test-rt` /
  `check-headless-deps` が pass。`fmt-check` は main 既存破損
  （`routing_engine.rs`、pin 更新と無関係）で fail — 報告対象。
- [x] `mise run check-app-webview` / `check-app-native` /
  `lint-app-webview` / `lint-app-native` / `test-app-webview` が pass。
- [x] `node tools/check-ruviform-rev.mjs --require-vendor` が pass。
- [x] 実 GPUI・実 OS 検証は NotRun として PR に記録。
