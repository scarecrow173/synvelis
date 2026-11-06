---
id: "TSPEC-K0owvECAJq2cZ4wvoyMdzy"
type: "test-spec"
status: "proposed"
title: "Issue #167 Tracker reconcile test specification — Consumer 契約(共有 pin・mode 分離・guard・入口)"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
    - "docs/plans/issue-167-tracker-reconcile-entry.md"
    - "docs/adr/issue-167-tracker-reconcile-entry.md"
  changes:
    added:
      - type: "file"
        path: ".github/workflows/ci-issue-tracker-reconcile.yml"
      - type: "file"
        path: "tools/ci-issue-tracker-reconcile.test.mjs"
    modified:
      - type: "file"
        path: ".github/workflows/ci-issue-tracker.yml"
      - type: "file"
        path: ".github/workflows/daily-ci.yml"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
    - "docs/plans/issue-167-tracker-reconcile-entry.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
  verified-by: []
  references:
    - ".github/workflows/ci-issue-tracker-reconcile.yml"
    - ".github/workflows/ci-issue-tracker.yml"
    - ".github/workflows/daily-ci.yml"
    - ".github/workflows/ci.yml"
    - "mise.toml"
  defers: []
  deferred-by: []
metadata: {}
---

# Issue #167 Tracker reconcile test specification — Consumer 契約(共有 pin・mode 分離・guard・入口)

`tools/ci-issue-tracker-reconcile.test.mjs`(`node --test`)で検証する
Consumer 側 workflow 契約の仕様。YAML パーサ依存を避け、workflow
テキストに対する構造 assertion とする。共有 evaluator 自体の判定契約
(NEWEST/SHA/unknown/pending/compare/Issue 冪等)は Ruviform 側の
91 mock test が正本であり、ここでは「SV が正しくその実装を呼ぶ」
Consumer 契約のみを pin する。

## Feature: 共有実装 pin の単一性

### Rule: 共有 checkout する 3 workflow の `ref:` は同一 pin で一致する

- Example: `daily-ci.yml` / `ci-issue-tracker.yml` /
  `ci-issue-tracker-reconcile.yml` の `repository: scarecrow173/ruviform`
  checkout ブロックが全て `ref:
  eed590e0a3837e61c943d9de1d3efcfd7df32cb0` を持つ。
- Example: 旧 pin `73d51d875e2876a75fb2077c471ea8d04478c082` が
  `.github/workflows/` 配下のいずれにも残らない。
- Example: 各ファイルの共有 checkout は `path: .ci-shared/ruviform`・
  `ssh-key: ${{ secrets.CROSS_REPO_DEPLOY_KEY }}`・
  `persist-credentials: false` を維持する。

## Feature: reconcile 入口契約(`ci-issue-tracker-reconcile.yml`)

### Rule: `on:` は `schedule` + `workflow_dispatch` のみ

- Example: `schedule:` 配下に cron 記述(`47 */6 * * *`)があり、
  `workflow_dispatch:` を持つ。
- Example: `pull_request`・`push`・`workflow_run` を含まない
  (event 専用契約の維持 — trigger を持つ cron は当該ファイルに閉じる)。

### Rule: schedule payload 欠落による silent skip を禁止し、manual 非 default ref は skip する

- Example: job `reconcile` が
  `if: github.event_name == 'schedule' ||
  github.ref_name == github.event.repository.default_branch`
  を持つ — schedule 側は無条件、dispatch は default branch のみ。
- Example: own repo checkout が `ref: ${{ github.event.repository.
  default_branch || 'main' }}` + `persist-credentials: false` で
  trusted default-branch code に pin される。

### Rule: write 直列化・最小権限・bounded 実行

- Example: `concurrency.group` が
  `ci-issue-tracker-${{ github.repository }}` で event 入口と一致し、
  `cancel-in-progress: false`。
- Example: workflow 直下 `permissions: {}`、job は
  `contents: read` / `actions: read` / `issues: write` のみ。
- Example: `timeout-minutes: 15` を持つ。

### Rule: mode 分離と self-test 先行

- Example: reconcile step の `env:` に `TRACKER_EVENT_PATH` key が
  存在しない(env key 構文として — コメント中の言及は対象外)。
- Example: `mise exec -- node --test .ci-shared/ruviform/ci/tracker/`
  の self-test が `tracker.mjs --reconcile` 実行より**前**に位置する。
- Example: 実行が `tracker.mjs --reconcile` と
  `--report ci-issue-tracker-report.json` を持ち、reconcile step の
  `env:` が `GH_TOKEN` のみを持つ(排他 — 他 env key なし)。
- Example: artifact 名が `ci-issue-tracker-reconcile-report-attempt-`
  で始まり event 入口の report と別名。

## Feature: 既存入口の不変(誤混入ガード)

### Rule: `ci-issue-tracker.yml` は event 専用のまま

- Example: `on:` が `workflow_run` のみを持ち、`schedule`・
  `workflow_dispatch`・`pull_request`・`push` を追加していない。
- Example: tracker 実行に `TRACKER_EVENT_PATH:` env を渡し、
  `--reconcile` を含まない。

### Rule: `daily-ci.yml` は dispatch 判定専用のまま

- Example: `tracker.mjs` への参照も `--reconcile` も含まない。

### Rule: `ci.yml` の trigger は本工程で変更しない

- Example: `on:` が `pull_request`・`push`・`workflow_dispatch` を
  維持する(trigger 撤去は後段工程であり、本 PR で混入させない)。

## Non-goals

- 共有 evaluator の判定ロジック再検証 — Ruviform 側 mock test が正本。
- GitHub Actions 実 run・実 Issue・実 cron の E2E — merge 後の運用
  段階(未観測を Pass にしない)。
- YAML の完全構文検証 — `mise run lint-actions`(actionlint)が担う。
  本 test は契約ピン留めであり lint の代替ではない。
