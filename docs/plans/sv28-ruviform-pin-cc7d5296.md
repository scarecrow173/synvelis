---
id: "PLAN-pOkUKpNa9zeQiK8ZbP5nvk"
type: "plan"
status: "completed"
title: "SV#28 Ruviform pin 更新 実装計画 — 863a7a9d → cc7d5296"
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
    - "docs/designs/sv28-ruviform-pin-cc7d5296.md"
  implemented-by:
    - "docs/tasks/sv28-ruviform-pin-update.md"
  depends-on:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/designs/sv28-ruviform-pin-cc7d5296.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/designs/sv28-ruviform-pin-cc7d5296.md"
  derived-by:
    - "docs/test-specs/sv28-ruviform-pin-cc7d5296.md"
    - "docs/tasks/sv28-ruviform-pin-update.md"
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

# SV#28 Ruviform pin 更新 実装計画

## Goal

`Cargo.lock` の ruviform 解決 revision を `863a7a9d` から ruviform
`main` HEAD `cc7d5296d3fbc65ce9b925834de4dadf5db666d9` へ更新し、
API 追従が必要な箇所を最小修正した上で、headless lane と
webview/native feature lane の検証を通す。

## Scope

単一の実装ストリーム。変更対象は `Cargo.lock`（pin 更新）、
必要時のみ `crates/*/src/` の追従修正、および本計画の文書
チェーン。`Cargo.toml`・`.github/`・`.devin/`・UI 大規模変更は
対象外。

## Upstream Documents

- `docs/specs/sv28-ruviform-pin-cc7d5296.md`
- `docs/designs/sv28-ruviform-pin-cc7d5296.md`（approved）

## File Map

- `Cargo.lock` — ruviform `git+...#<sha>` fragment を
  `cc7d5296d3fbc65ce9b925834de4dadf5db666d9` へ更新。
  他パッケージの version drift が出た場合は pin 更新と分離して
  対処（原則 `-p` 指定で発生しない想定）。
- `crates/*/src/` — API 非互換が検出された場合のみ最小追従修正。
- `ui/vendor/ruviform/` — `mise run sync-ui-deps` による生成物
  （gitignore 対象、コミットしない）。
- `docs/{specs,designs,plans,test-specs,tasks,impl}/` — 本タスクの
  文書チェーン。

## Implementation Sequence

- [x] 文書チェーン（spec/design/plan/test-spec/task）を作成し、
  `doc_maintenance` で index を再生成する。
- [x] `mise exec -- cargo update -p ruviform-io-file -p
  ruviform-media-core -p ruviform-media-io -p ruviform-media-reader
  -p ruviform-io-symphonia -p ruviform-graph -p ruviform-audio
  -p ruviform-audio-core -p ruviform-rt -p ruviform-io-wav
  -p ruviform-exec -p ruviform-ui -p ruviform-ui-theme -p
  ruviform-viz -p ruviform-plugin-core -p ruviform-plugin-host
  -p ruviform-audio-plugin-bridge -p ruviform-ui-webview
  -p ruviform-ui-gpui -p ruviform-ui-test -p ruviform-observability`
  で `Cargo.lock` を再解決する。
- [x] `Cargo.lock` 差分が ruviform pin のみであることを確認
  （`git diff Cargo.lock` で source fragment と必要な依存木の
  変化を検査。`png`/`base64` は既存 lock tree 内のため新規
  crates.io 追加は想定しない）。
- [x] `mise run sync-ui-deps` で vendor を新 SHA へ再生成し、
  `mise run check-ruviform-rev -- --require-vendor`（task 経由は
  `node tools/check-ruviform-rev.mjs --require-vendor`）で
  lockfile/vendor/manifest の一致を確認する。
- [x] `mise run fmt-check` / `check` / `lint` を実行し、API 非互換が
  あれば最小追従修正を行って再検証する。
- [x] `mise run test` / `test-rt` / `check-headless-deps` を実行する。
- [x] feature lane: `mise run check-app-webview` /
  `check-app-native`（必要に応じ `lint-app-webview` /
  `lint-app-native` / `test-app-webview`）で webview/native の
  コンパイル契約を検証する。
- [x] `git fetch origin main && git merge origin/main` で最新 main
  を取り込み、差分があれば lockfile・vendor・index を再生成する。
- [ ] Conventional Commits で論理単位にコミット（文書チェーンと
  pin 更新は別コミット）、`chore/28-ruviform-pin-update` を push、
  PR を作成する。

## Dependencies and Risks

- 依存: ruviform `main` HEAD が `cc7d5296` であること
  （`git ls-remote` で確認済み。作業中に前進した場合は最新を採用）。
- リスク: feature-gated lane でのみ顕在化する非互換 → webview/
  native の check/lint を個別に実行して検出。
- リスク: `cargo update` が ruviform 以外の依存を巻き込む →
  `-p` 指定と lockfile 差分検査で抑止。発生時は該当エントリを
  手で戻すか分離コミットとする。
- リスク: 実 GPUI/OS 固有の問題は本環境（Windows）では検出不能
  → NotRun として PR に記録し、CI lane の結果に委ねる。

## Task Breakdown

- `docs/tasks/sv28-ruviform-pin-update.md` — 唯一のタスク。
  pin 更新・追従修正・検証・PR 作成を一体で行う。

## Verification

- [x] `mise run check` / `lint` / `test` / `test-rt` /
  `check-headless-deps` が pass。`fmt-check` は main 既存破損
  （`routing_engine.rs`）で fail — pin 更新と無関係、報告対象。
- [x] `mise run check-app-webview` / `check-app-native`（および
  `lint-app-webview`/`lint-app-native`/`test-app-webview`）が pass。
- [x] `node tools/check-ruviform-rev.mjs --require-vendor` が
  `resolved cc7d5296...` で pass。
- [x] `Cargo.lock` 差分が ruviform pin 更新のみであることを
  `git diff` で確認。
- [x] 実 GPUI・実 OS 検証は NotRun（範囲外）として記録。

## Open Questions / Gaps

- なし。API 差分は調査済みで追加のみ。追従修正が必要になった
  場合は本計画の「API 追従の境界」に従い最小修正とする。
