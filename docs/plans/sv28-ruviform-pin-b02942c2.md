---
id: "PLAN-034VobWDT9p1N0HGeD67OV"
type: "plan"
status: "completed"
title: "SV#28 Ruviform pin 更新 実装計画 — 0f5b5737 → b02942c2"
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
    - "docs/designs/sv28-ruviform-pin-b02942c2.md"
  implemented-by:
    - "docs/tasks/sv28-ruviform-pin-b02942c2.md"
  depends-on:
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
    - "docs/designs/sv28-ruviform-pin-b02942c2.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv28-ruviform-pin-b02942c2.md"
    - "docs/designs/sv28-ruviform-pin-b02942c2.md"
  derived-by:
    - "docs/test-specs/sv28-ruviform-pin-b02942c2.md"
    - "docs/tasks/sv28-ruviform-pin-b02942c2.md"
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

# SV#28 Ruviform pin 更新 実装計画

## Goal

`Cargo.lock` の ruviform 解決 revision を `0f5b5737` から ruviform
`main` HEAD `b02942c2f9e6e195d0d752803b077866468fe58e` へ更新し、
API 追従が必要な箇所を最小修正した上で、headless lane と
webview/native feature lane の検証を通す。

## Scope

単一の実装ストリーム。変更対象は `Cargo.lock`（pin 更新）、
必要時のみ `crates/*/src/` の追従修正、および本計画の文書
チェーン。`Cargo.toml`・`.github/`・`.devin/`・UI 大規模変更・
新規 ruviform crate/新 API の採用は対象外。

## Upstream Documents

- `docs/specs/sv28-ruviform-pin-b02942c2.md`
- `docs/designs/sv28-ruviform-pin-b02942c2.md`（approved）

## Design Inputs

- `docs/designs/sv28-ruviform-pin-b02942c2.md`（approved）
- `docs/discovery/sv28-ruviform-pin-b02942c2.md`（差分調査・
  影響表・採用判断）

## File Map

- `Cargo.lock` — ruviform `git+...#<sha>` fragment を
  `b02942c2f9e6e195d0d752803b077866468fe58e` へ更新。
  他パッケージの version drift が出た場合は pin 更新と分離して
  対処（原則 `-p` 指定で発生しない想定）。
- `crates/*/src/` — API 非互換が検出された場合のみ最小追従修正。
- `ui/vendor/ruviform/` — `mise run sync-ui-deps` による生成物
  （gitignore 対象、コミットしない）。
- `docs/{discovery,specs,designs,plans,test-specs,tasks,impl}/` —
  本タスクの文書チェーンと managed index。

## Implementation Sequence

- [x] 文書チェーン（discovery/spec/design/plan/test-spec/task/
  impl）を作成し、managed index を再生成する。
- [x] `cargo update -p ruviform-*`（全 ruviform workspace 依存を
  `-p` 指定）で `Cargo.lock` を再解決する。差分は 26 箇所全てが
  ruviform source fragment の SHA 更新のみで、推移的依存の
  version drift なし。
- [x] `git diff Cargo.lock` で pin 更新以外の drift がないことを
  確認する。
- [x] `mise run sync-ui-deps` で vendor を新 SHA へ再生成し、
  `node tools/check-ruviform-rev.mjs --require-vendor` で
  lockfile/vendor/manifest の一致を確認する
  （`resolved b02942c2…`、26 sources、REV match）。
- [x] `mise run fmt-check` / `check` / `lint` を実行し、API 非互換が
  あれば最小追従修正を行って再検証する。
- [x] `mise run test` / `test-rt` / `check-headless-deps` を実行する。
- [x] feature lane: `mise run check-app-webview` /
  `check-app-native` / `lint-app-native`（必要に応じ
  `lint-app-webview` / `test-app-webview`）で webview/native の
  コンパイル契約を検証する。
- [x] 補助 lane: `mise run check-ui` / `test-ui` で frontend
  lane の非回帰を確認する。
- [x] `git fetch origin main && git merge origin/main` で最新 main
  を取り込み、差分があれば lockfile・vendor・index を再生成する。
- [x] Conventional Commits で論理単位にコミット（文書チェーンと
  pin 更新は別コミット）、`chore/28-ruviform-pin-b02942c2` を
  push、PR を作成する。

## Dependencies and Risks

- 依存: ruviform `main` HEAD が `b02942c2` であること
  （`git ls-remote` で確認済み。作業中に前進した場合は最新を採用）。
- リスク: feature-gated lane でのみ顕在化する非互換 → webview/
  native の check/lint を個別に実行して検出。
- リスク: `cargo update` が ruviform 以外の依存を巻き込む →
  `-p` 指定と lockfile 差分検査で抑止。発生時は該当エントリを
  手で戻すか分離コミットとする。
- リスク: Issue #410 の `ruviform-ui` 内部修正が consumer の
  観測挙動を変える可能性 → 既存 contract test の非回帰で確認し、
  差異があれば pin 起因か切り分けて記録。
- リスク: 実 GPUI/OS 固有の問題は本環境では検出不能 → NotRun
  として PR に記録し、CI lane の結果に委ねる。

## Task Breakdown

- `docs/tasks/sv28-ruviform-pin-b02942c2.md` — 唯一のタスク。
  pin 更新・追従修正・検証・PR 作成を一体で行う。

## Verification

- [x] `mise run check` / `lint` / `test` / `test-rt` /
  `check-headless-deps` が pass。`fmt-check` が fail する場合は
  pin 起因か main 既存破損かを切り分けて記録。
- [x] `mise run check-app-webview` / `check-app-native` /
  `lint-app-native`（および `lint-app-webview`/
  `test-app-webview`）が pass。
- [x] `node tools/check-ruviform-rev.mjs --require-vendor` が
  `resolved b02942c2...` で pass。
- [x] `Cargo.lock` 差分が ruviform pin 更新のみであることを
  `git diff` で確認。
- [x] 実 OS/実 GUI/実 plugin/両 Consumer 横断の検証は NotRun
  （範囲外）として記録。

## Open Questions / Gaps

- なし。API 差分は調査済みで実質 additive。追従修正が必要に
  なった場合は本計画の「API 追従の境界」（design doc）に従い
  最小修正とする。

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

- pin 更新の差分が SHA fragment のみであること、feature lane を
  含む全 lane の実走結果、NotRun 項目の記録がレビュー焦点。
