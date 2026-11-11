---
id: "PLAN-TyFpOpS6aiBDCdppuSCl9s"
type: "plan"
status: "completed"
title: "SV#28 Ruviform pin 更新 実装計画 — 9865cd57 → fd1d4d5f"
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
    - "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
  implemented-by:
    - "docs/tasks/sv28-ruviform-pin-fd1d4d5f.md"
  depends-on:
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/designs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/discovery/sv28-ruviform-pin-fd1d4d5f.md"
  derived-by:
    - "docs/test-specs/sv28-ruviform-pin-fd1d4d5f.md"
    - "docs/tasks/sv28-ruviform-pin-fd1d4d5f.md"
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

# SV#28 Ruviform pin 更新 実装計画

## Goal

`Cargo.lock` の ruviform 解決 revision を `9865cd57` から ruviform
`main` HEAD `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` へ更新し、
headless lane と webview/native feature lane の検証を通す。

## Scope

単一の実装ストリーム。変更対象は `Cargo.lock`（pin 更新）と本計画の
文書チェーン。`Cargo.toml`・`.github/`・`.devin/`・`mise.toml`・
`crates/`（別 Worker 占有）・新規 ruviform crate/新 API の採用は
対象外。

## Upstream Documents

- `docs/specs/sv28-ruviform-pin-fd1d4d5f.md`（approved）
- `docs/designs/sv28-ruviform-pin-fd1d4d5f.md`（approved）

## Design Inputs

- `docs/designs/sv28-ruviform-pin-fd1d4d5f.md`（approved）
- `docs/discovery/sv28-ruviform-pin-fd1d4d5f.md`（差分調査・
  影響表・採用判断）

## File Map

- `Cargo.lock` — ruviform `git+...#<sha>` fragment を
  `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` へ更新。
  他パッケージの version drift が出た場合は pin 更新と分離して
  対処（原則 `-p` 指定で発生しない想定）。
- `ui/vendor/ruviform/` — `mise run sync-ui-deps` による生成物
  （gitignore 対象、コミットしない）。
- `docs/{discovery,specs,designs,plans,test-specs,tasks,impl}/` —
  本タスクの文書チェーンと managed index。

## Implementation Sequence

- [x] ruviform local repo で `git log 9865cd57..fd1d4d5f` を独立確認
  し、`crates/*/src/` の `pub` 削除スキャンで公開面が additive で
  あることを確認する（0 件）。
- [x] `cargo update -p ruviform-*`（全 27 ruviform workspace 依存を
  `-p` 指定）で `Cargo.lock` を再解決する。差分は 27 箇所全てが
  ruviform source fragment の SHA 更新＋依存リストへの
  `windows 0.62.2`/`windows-core 0.62.2`/`x11rb` 参照追加（いずれも
  lockfile 既存 version）のみで、推移的依存の version drift なし。
- [x] `git diff Cargo.lock` で pin 更新以外の drift がないことを
  確認する。
- [x] `mise run sync-ui-deps` で vendor を新 SHA へ再生成し、
  `node tools/check-ruviform-rev.mjs --require-vendor` で
  lockfile/vendor/manifest の一致を確認する
  （`resolved fd1d4d5f…`、27 sources、REV match）。
- [x] 文書チェーン（discovery/spec/design/plan/test-spec/task/
  impl）を作成し、managed index を再生成する。
- [x] `mise run fmt-check` / `check` / `lint` を実行する
  （`crates/` は編集範囲外のため、pin 起因の破壊が出た場合は
  修正せず差分を記録）。
- [x] `mise run test` / `test-rt` / `check-headless-deps` /
  `test-msrv` を実行する。
- [x] feature lane: `mise run check-app-webview` /
  `check-app-native` / `lint-app-native` / `lint-app-webview` /
  `test-app-webview` で webview/native のコンパイル契約を検証する。
- [x] 補助 lane: `mise run check-ui` / `test-ui` で frontend
  lane の非回帰を、`mise run lint-recording-real-input` で
  real-device feature（wasapi_input 経路を含む）の compile を
  検証する。
- [x] `git fetch origin main && git merge origin/main` で最新 main
  を取り込み、差分があれば lockfile・vendor・index を再生成する。
- [x] Conventional Commits で論理単位にコミット（文書チェーンと
  pin 更新は別コミット）、`chore/28-ruviform-pin-fd1d4d5f` を
  push、PR を作成する。

## Dependencies and Risks

- 依存: ruviform `main` HEAD が `fd1d4d5f` であること
  （`git rev-parse origin/main` で確認済み。作業中に前進した
  場合は最新を採用）。
- リスク: feature-gated lane でのみ顕在化する非互換 → webview/
  native の check/lint と `lint-recording-real-input` を個別に
  実行して検出。
- リスク: `cargo update` が ruviform 以外の依存を巻き込む →
  `-p` 指定と lockfile 差分検査で抑止。発生時は該当エントリを
  手で戻すか分離コミットとする。
- リスク: `wasapi_input` の挙動は Windows 実機経路でのみ顕在化 →
  compile は `lint-recording-real-input` で検証し、実機証拠は
  NotRun として記録。
- リスク: 実 GPUI/OS 固有の問題は本環境では検出不能 → NotRun
  として PR に記録し、CI lane の結果に委ねる。

## Task Breakdown

- `docs/tasks/sv28-ruviform-pin-fd1d4d5f.md` — 唯一のタスク。
  pin 更新・検証・PR 作成を一体で行う。

## Verification

- [x] `mise run check` / `lint` / `test` / `test-rt` /
  `check-headless-deps` / `test-msrv` が pass。`fmt-check` が
  fail する場合は pin 起因か main 既存破損かを切り分けて記録。
- [x] `mise run check-app-webview` / `check-app-native` /
  `lint-app-native`（および `lint-app-webview`/
  `test-app-webview`）が pass。
- [x] `node tools/check-ruviform-rev.mjs --require-vendor` が
  `resolved fd1d4d5f...` で pass。
- [x] `Cargo.lock` 差分が ruviform pin 更新＋依存リスト参照追加
  のみであることを `git diff` で確認。
- [x] 実 OS/実 GUI/実 plugin/両 Consumer 横断の検証は NotRun
  （範囲外）として記録。

## Open Questions / Gaps

- なし。API 差分は調査済みで全件 additive（`pub` 削除 0 件）。
  `crates/` は別 Worker 占有のため、pin 起因の破壊が出た場合は
  修正せず差分を記録する契約。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
  consistent.

## Implementation Handoff

- Recommended mode: inline execution（本 worktree の Worker が
  直接実施）。
- Independent work streams: none
- User approval required before dispatch: no（Orca Worker として
  dispatch 済み。review-protocol によるレビューで blocker 無しを
  もって承認相当とする）。
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute task-by-task
  with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- pin 更新の差分が SHA fragment＋依存リスト参照追加のみである
  こと、feature lane を含む全 lane の実走結果、NotRun 項目の
  記録がレビュー焦点。
