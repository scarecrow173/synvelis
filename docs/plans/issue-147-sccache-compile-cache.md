---
id: "PLAN-034RIMqW7C96l1jlnfBaLc"
type: "plan"
status: "approved"
title: "Issue #147 sccache 導入実装計画"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/147"
    - "docs/designs/issue-147-sccache-compile-cache.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/issue-147-sccache-compile-cache.md"
    - "docs/specs/issue-147-sccache-compile-cache.md"
  implemented-by:
    - "docs/tasks/issue-147-t1-sccache-verify-gw10.md"
  depends-on:
    - "docs/designs/issue-147-sccache-compile-cache.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/ci-synvelis-pr-push-regression-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-147-sccache-compile-cache.md"
    - "docs/designs/issue-147-sccache-compile-cache.md"
    - "docs/discovery/issue-147-sccache-eval.md"
  derived-by:
    - "docs/tasks/issue-147-t1-sccache-verify-gw10.md"
    - "docs/test-specs/issue-147-sccache-compile-cache.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-147-sccache-compile-cache.md"
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #147 sccache 導入実装計画 Implementation Plan

## Goal

`verify`・`gw10-verify` job(各 3 OS)へ ruviform #329 同型の sccache
GHA backend step 群を導入し、cold/warm の実測で採否を判定する。
kill switch・fail-open・cache 分離設計を備え、10 required contexts・
検証粒度・証跡構造を不変に保つ。

## Scope

単一の実装 stream。変更は `.github/workflows/ci.yml` 1 ファイルに閉じる
(文書チェーンを除く)。新規 mise タスク・スクリプト・検証ロジック・
required contexts の変更は含まない。

## Upstream Documents

- `docs/specs/issue-147-sccache-compile-cache.md`
- `docs/discovery/issue-147-sccache-eval.md`

## Design Inputs

- `docs/designs/issue-147-sccache-compile-cache.md`(approved)

## File Map

- `.github/workflows/ci.yml` — `verify` job と `gw10-verify` job に
  sccache step 群(Set up → env export → env verify → statistics)を
  cargo cache restore の直後・検証 step 群の前後に挿入。いずれも
  `if: vars.CI_SCCACHE_DISABLE != '1'` で kill switch 対象。
- `docs/impl/exp/issue-147-sccache-compile-cache.md` — cold/warm 実測
  記録(実装後に追記)。
- `docs/impl/ir/issue-147-sccache-compile-cache.md` — 実装記録。

## Implementation Sequence

- [ ] `ci.yml` `verify` job: cargo cache restore 直後に sccache 3 step
      (set up / env export / env verify)を挿入。`SCCACHE_GHA_VERSION` は
      `synvelis-v1-<RUNNER_OS>`。
- [ ] `ci.yml` `verify` job: `lockdiff` step の後・evidence 記録の前に
      `sccache --show-stats || true` step を挿入。
- [ ] `ci.yml` `gw10-verify` job: 同型の 4 step を同位置に挿入
      (namespace は verify と同一 `synvelis-v1-<RUNNER_OS>`)。
- [ ] `mise run lint-actions` + `mise exec -- prek run --all-files` で
      ローカル検証。
- [ ] `git merge origin/main` → 文書 index 再生成 → commit → push →
      PR 作成 → `gh pr checks --watch`。
- [ ] cold run(PR 初回 push)の `sccache --show-stats` と job wall time
      を取得。warm run(続く push/rerun)と比較して exp doc に記録。
- [ ] 悪化した OS/lane があれば該当 sccache step を除去して再 push
      (不採用判断を記録)。

## Dependencies and Risks

- `actions/github-script` の SHA pin `3a2844b7e9c422d3c10d287c895573f7108da1b3`
  (v9.0.0)は ruviform で使用済みの同一 pin。`GITHUB_TOKEN` は既定の
  `permissions: contents: read` で足りる(env export のみで API 呼出し
  なし)。
- cold run の GHAC write rate limit で初回 hit が出にくい(fail-open
  のため green は維持。warm run で評価)。
- 既存 flake `gw10f_delegated_apply_commits_and_survives_restart_real`
  (Connection reset by peer)が ubuntu leg で再発する可能性 — rerun で
  対処し、スコープ外失敗として記録。
- `RUSTC_WRAPPER` は GITHUB_ENV 経由で後続全 step に効くが、sccache は
  rustc wrapper であり cargo/test 実行・証跡には関与しない。

## Task Breakdown

- `docs/tasks/issue-147-t1-sccache-verify-gw10.md` — 上記 1 ファイル
  変更 + ローカル/CI 検証 + 実測記録を一括で担う単一 task(変更が
  同一 job 内の同型 step 繰返しのため分割しない)。

## Verification

- [ ] `mise run lint-actions` pass(actionlint)。
- [ ] `mise exec -- prek run --all-files` pass。
- [ ] PR cold run で全 13 job が起動し、sccache step が env 揃いで
      実行される(Verify sccache environment の log)。
- [ ] warm run で hit が発生し、write/read error があっても全 lane
      green(fail-open 実証)。
- [ ] `rt-verify`/`msrv-verify`/`ui-verify`/`dep-guard`/`evidence` に
      sccache step がない(diff で確認)。
- [ ] 10 required contexts の名称・needs・evidence 判定が不変
      (diff で確認)。
- [ ] Before/After の wall time・hit rate・write error を exp doc に
      記録し、Rollback 手順を IR に記録。

## Open Questions / Gaps

- なし。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution(1 ファイル・単一 task のため)。
- Independent work streams: なし。
- User approval required before dispatch: yes
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

## Review Handoff

- レビュー観点: sccache step が `vars.CI_SCCACHE_DISABLE != '1'` 条件を
  全 step で持つか、`SCCACHE_GHA_VERSION` が `synvelis-v1-<RUNNER_OS>`
  で一貫するか、検証 step・evidence・cleanup の順序と `always()` が
  壊れていないか、非対象 job に diff がないか。
