---
id: "TSPEC-O00hUfnnLCmwn9LGFITe1d"
type: "test-spec"
status: "proposed"
title: "Issue #145 ci.yml manual-only test specification — dispatch 専用・陰性 trigger・lane 構造不変"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "docs/specs/issue-145-ci-manual-only.md"
    - "docs/designs/issue-145-ci-manual-only.md"
    - "docs/plans/issue-145-ci-manual-only.md"
    - "docs/adr/issue-145-ci-manual-only.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "tools/ci-issue-tracker-reconcile.test.mjs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-145-ci-manual-only.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-167-tracker-reconcile-entry.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-145-ci-manual-only.md"
    - "docs/plans/issue-145-ci-manual-only.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-145-ci-manual-only.md"
    - "docs/designs/issue-145-ci-manual-only.md"
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - ".github/workflows/daily-ci.yml"
    - ".github/workflows/ci-issue-tracker.yml"
    - ".github/workflows/ci-issue-tracker-reconcile.yml"
    - "tools/ci-issue-tracker-reconcile.test.mjs"
    - "tools/ci-summary.mjs"
  defers: []
  deferred-by: []
metadata: {}
---

# Issue #145 ci.yml manual-only test specification — dispatch 専用・陰性 trigger・lane 構造不変

`tools/ci-issue-tracker-reconcile.test.mjs`(`node --test`)に対する
契約転換の仕様。YAML パーサ非依存の構造テキスト assertion を維持する。
fail-closed 判定契約自体は `tools/ci-summary.test.mjs`(36 件)が
正本 — ここでは trigger/入口の構造契約のみを pin する。

## Feature: ci.yml は workflow_dispatch 専用

### Rule: `on:` は `workflow_dispatch` のみを持つ

- Example: ci.yml の `on:` block が `workflow_dispatch:` を持ち、
  `pull_request`・`push:`・`schedule`・`workflow_run`・
  `pull_request_target` を含まない。
- Example: `expected_sha` input の description/required/type 行が
  残る(daily-ci dispatch 契約の受け口)。

### Rule: 全 workflow に PR/push 系 trigger が残らない(陰性走査)

- Example: `.github/workflows/` 配下の全 `.yml`/`.yaml` を走査し、
  `on:` block に `pull_request`・`pull_request_target`・`push:`
  を持つ workflow が存在しない。

### Rule: lane 構造・fail-closed 集約は不変

- Example: 12 leg の job key(`verify`・`verify-macos`・`rt-verify`・
  `gw10-verify`・`gw10-verify-macos`・`msrv-verify`・`ui-verify`・
  `ui-verify-macos`・`dep-guard`)と `evidence`・`ci-summary` が
  `jobs:` 配下に存在する。
- Example: `evidence` が `CI_SUMMARY_MODE: pr`・`ci-summary` が
  `CI_SUMMARY_MODE: full` を持ち、ci-summary の needs が
  `evidence` を含む。

### Rule: checkout pin・verify-sha 契約は不変

- Example: `ref: ${{ inputs.expected_sha || github.sha }}` 式が残る。
- Example: `ci-summary` が verify-sha step
  (`ci-summary.mjs verify-sha`)を持つ。

## Feature: 既存入口の不変(誤混入ガード)

### Rule: 他 3 workflow の trigger 契約は維持

- Example: `daily-ci.yml` が `schedule` + `workflow_dispatch` を
  持ち `ci-issue-tracker.yml` が `workflow_run` のみを持ち、
  `ci-issue-tracker-reconcile.yml` が `schedule` +
  `workflow_dispatch` のみを持つ(既存 assertion の継続)。

### Rule: 共有 pin eed590e は不変

- Example: 3 workflow の `repository: scarecrow173/ruviform`
  checkout が `eed590e0a3837e61c943d9de1d3efcfd7df32cb0` のまま。

## Feature: PR 起動ゼロの実測(GitHub API — 実施記録)

### Rule: 新 PR の opened/synchronize で ci run が起動しない

- Example: PR head SHA・PR 作成時刻以降に `gh run list` /
  `gh pr checks` で `ci`・`daily-ci`・`ci-issue-tracker*` の
  新規 run が観測されない(結果を PR 本文と task verification に
  記録する — 未観測を Pass にしない)。

## Non-goals

- `tools/ci-summary.mjs` gate 判定の再検証 — 36 test が正本。
- reconcile/daily/tracker の runtime 再検証 — main 実受入
  (run 42183098205)が正本記録。cron tick は未観測=NotRun。
- actionlint 構文検証の代替 — `mise run lint-actions` が担う。
