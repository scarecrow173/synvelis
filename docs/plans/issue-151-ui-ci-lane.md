---
id: "PLAN-034RGXgmeBXSmxXfegDQ8f"
type: "plan"
status: "approved"
title: "Issue #151 CI ui-verify lane 実装計画"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/151"
    - "docs/designs/issue-151-ui-ci-lane.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/issue-151-ui-ci-lane.md"
    - "docs/specs/issue-151-ui-ci-lane.md"
  implemented-by:
    - "docs/tasks/issue-151-t1-ui-verify-lane.md"
  depends-on:
    - "docs/designs/issue-151-ui-ci-lane.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/ci-synvelis-pr-push-regression-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-151-ui-ci-lane.md"
    - "docs/designs/issue-151-ui-ci-lane.md"
  derived-by:
    - "docs/tasks/issue-151-t1-ui-verify-lane.md"
    - "docs/test-specs/issue-151-ui-ci-lane.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-151-ui-ci-lane.md"
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #151 CI ui-verify lane 実装計画 Implementation Plan

## Goal

`ui-verify` CI lane(3 OS・fail-closed)と、npm 依存導入を単一の
freshness-checked `install-ui` task へ集約する mise 配線を実装し、ui/
変更を PR/main の検証カバレッジに含める。

## Scope

単一の実装 stream。変更は `mise.toml` と `.github/workflows/ci.yml` の
2 ファイルに閉じる(新規スクリプト・ui/ コード・required contexts の
変更は含まない)。

## Upstream Documents

- `docs/specs/issue-151-ui-ci-lane.md`
- `docs/discovery/issue-151-ui-ci-lane-gap.md`

## Design Inputs

- `docs/designs/issue-151-ui-ci-lane.md`(approved)

## File Map

- `mise.toml` — `install-ui` task 新設(sources/outputs で freshness
  check)、`setup-ui` を depends 集約へ変更、`check-ui`/`test-ui`/
  `build-ui`/`dev-ui`/`build-app` に `depends = ["install-ui"]`。
- `.github/workflows/ci.yml` — `ui-verify` job(msrv-verify と dep-guard
  の間に配置)と `evidence` job の `needs`/証跡テーブル/fail 判定/
  stale な NotRun 行の更新。

## Implementation Sequence

- [ ] `mise.toml`: `install-ui` task を追加し、`setup-ui` を
      `depends = ["install-ui"]` の集約入口に変更、npm 消費 5 task に
      `depends = ["install-ui"]` を付与。
- [ ] `ci.yml`: `ui-verify` job を追加(3 OS matrix・dep-access・
      cargo cache・`cargo fetch --locked`・`sync-ui-deps`・`setup-ui`・
      `check-ui`/`test-ui`/`build-ui`・証跡・cleanup)。
- [ ] `ci.yml`: `evidence` job の `needs`・証跡テーブル・fail 判定へ
      `ui-verify` を追加し、stale な「SolidJS+TS …」NotRun 行を除去。
- [ ] worktree 検証(node_modules 削除→各 ui task 完走・freshness skip
      確認・actionlint・prek)。
- [ ] `git merge origin/main`、コミット、push、PR 作成、
      `gh pr checks --watch` で lane 緑を確認。

## Dependencies and Risks

- `sync-ui-deps` は `~/.cargo/git/checkouts` を参照するため、lane 内で
  `cargo fetch --locked` を先に実行することが前提。dep-access は既存
  composite action を再利用し新規 secret は追加しない。
- Windows/macOS leg で npm/platform 差異が出る可能性(想定内・matrix で
  検出する対象そのもの)。
- `mise install --locked ... node` は `mise.lock` の node エントリに
  依存する(全 OS 分の platform artifact が揃っていること)。

## Task Breakdown

- `docs/tasks/issue-151-t1-ui-verify-lane.md` — 上記 2 ファイル変更 +
  worktree/CI 検証を一括で担う単一 task(変更が 2 ファイル・1 lane の
  ため分割しない)。

## Verification

- [ ] `mise run check-ui` / `test-ui` / `build-ui` が worktree で pass
      (`ui/node_modules` 削除後の fresh 状態からも install-ui 経由で完走)。
- [ ] `mise run setup-ui` 直後の `check-ui` 等で `install-ui` が
      freshness skip されること(出力に skip が出る)。
- [ ] `mise run lint-actions`(actionlint)pass。
- [ ] `mise exec -- prek run --all-files` pass(check-yaml 等)。
- [ ] PR で `ui-verify` 3 leg + `evidence` が緑、`ui-verify` 失敗時は
      `evidence` が fail する構成(fail-open でない)を目視確認。

## Open Questions / Gaps

- なし。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution(2 ファイル・単一 task のため)。
- Independent work streams: なし。
- User approval required before dispatch: yes
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

## Review Handoff

- レビュー観点: `ui-verify` が既存 lane の構造(dep-access・cache・
  証跡・cleanup)を踏襲しているか、`evidence` の fail 判定が新 lane を
  正しく包含しているか、`MISE_ENABLE_TOOLS` の job 単位 override が
  他 lane に影響しないか。
