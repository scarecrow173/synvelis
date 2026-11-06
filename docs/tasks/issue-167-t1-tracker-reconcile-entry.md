---
id: "TASK-QSUlMIQI1FYoKq116Yk7xJ"
type: "task"
status: "in-progress"
title: "Issue #167 T1 共有 pin eed590e + ci-issue-tracker-reconcile.yml + Consumer 契約 test"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/167"
  changes:
    added:
      - type: "file"
        path: ".github/workflows/ci-issue-tracker-reconcile.yml"
      - type: "file"
        path: "tools/ci-issue-tracker-reconcile.test.mjs"
      - type: "file"
        path: "docs/specs/issue-167-tracker-reconcile-entry.md"
      - type: "file"
        path: "docs/adr/issue-167-tracker-reconcile-entry.md"
      - type: "file"
        path: "docs/designs/issue-167-tracker-reconcile-entry.md"
      - type: "file"
        path: "docs/plans/issue-167-tracker-reconcile-entry.md"
      - type: "file"
        path: "docs/test-specs/issue-167-tracker-reconcile-entry.md"
      - type: "file"
        path: "docs/tasks/issue-167-t1-tracker-reconcile-entry.md"
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
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
  implements:
    - "docs/plans/issue-167-tracker-reconcile-entry.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-167-tracker-reconcile-entry.md"
    - "docs/test-specs/issue-167-tracker-reconcile-entry.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-167-tracker-reconcile-entry.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci-issue-tracker-reconcile.yml"
    - ".github/workflows/ci-issue-tracker.yml"
    - ".github/workflows/daily-ci.yml"
    - ".github/workflows/ci.yml"
    - "tools/ci-issue-tracker-reconcile.test.mjs"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #167 T1 共有 pin eed590e + ci-issue-tracker-reconcile.yml + Consumer 契約 test

## Work

共有 CI 実装 checkout の pin を
`eed590e0a3837e61c943d9de1d3efcfd7df32cb0` へ揃え、Consumer 版
`ci-issue-tracker-reconcile.yml` を新設し、mode/guard/pin/入口契約の
`node --test` 契約 test と mise task を追加する。既存 event 入口・
daily・ci.yml は `ref:` 行以外不変。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/issue-167-tracker-reconcile-entry.md`
- `docs/adr/issue-167-tracker-reconcile-entry.md`
- `docs/designs/issue-167-tracker-reconcile-entry.md`
- `docs/plans/issue-167-tracker-reconcile-entry.md`
- `docs/test-specs/issue-167-tracker-reconcile-entry.md`

## Dependencies

- 単一 task。GitHub 書き込み(merge/Issue close/trigger 撤去)は全て
  スコープ外。

## Affected Files

- `.github/workflows/ci-issue-tracker-reconcile.yml`(新設)
- `.github/workflows/ci-issue-tracker.yml` / `daily-ci.yml`(`ref:` 1 行)
- `tools/ci-issue-tracker-reconcile.test.mjs`(新設)
- `mise.toml`(`[tasks.test-ci-tracker-entry]` 追加)
- `docs/*/README.md`(生成 index 再生成)

## Done When

- [ ] `mise run test-ci-tracker-entry` pass(契約 test 全 assertion)。
- [ ] `mise run lint-actions` pass。
- [ ] `mise run test-ci-summary` pass。
- [ ] pin 版 tarball 上の共有 tracker(91)/daily(50)test pass。
- [ ] `mise exec -- prek run --all-files` pass。
- [ ] `git diff`: `ci-issue-tracker.yml`/`daily-ci.yml` は `ref:` 行のみ、
      `ci.yml` 無変更。
- [ ] `ci-issue-tracker-reconcile.yml` が schedule+dispatch・同一
      concurrency・guard・15 分 timeout・最小権限・self-test 先行・
      `--reconcile`・別名 artifact を持つ。
- [ ] docs index が新文書 6 件を含む。

## Verification

実施結果をここに記録する(コマンド・exit code・OS)。
