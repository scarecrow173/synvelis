---
id: "PLAN-034RUl6aYDJVN0hwia8nWp"
type: "plan"
status: "approved"
title: "Issue #157 rev.3 matrix placeholder gate 実装計画"
created: "2026-11-05"
updated: "2026-11-05"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/157"
    - "https://github.com/scarecrow173/synvelis/pull/161"
    - "docs/designs/issue-157-matrix-placeholder-gate.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: "tools/ci-impact.mjs"
      - type: "file"
        path: "tools/ci-impact.test.mjs"
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
    - "docs/designs/issue-157-matrix-placeholder-gate.md"
  implemented-by:
    - "docs/tasks/issue-157-t1-matrix-placeholder-gate.md"
  depends-on:
    - "docs/designs/issue-157-matrix-placeholder-gate.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-148-impact-classification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
    - "docs/designs/issue-157-matrix-placeholder-gate.md"
  derived-by:
    - "docs/tasks/issue-157-t1-matrix-placeholder-gate.md"
    - "docs/test-specs/issue-157-matrix-placeholder-gate.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-157-matrix-placeholder-gate.md"
  references:
    - ".github/workflows/ci.yml"
    - "tools/ci-impact.mjs"
    - "tools/ci-impact.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #157 rev.3 matrix placeholder gate 実装計画

## Goal

skip 済み matrix lane の jobs API placeholder を evidence gate が正しく
分類・受理し、収集カウントを skip 収縮に正確化する — fail-closed を
維持したまま PR #161 型の fail を解消する。

## Scope

単一の実装 stream。変更は `tools/ci-impact.mjs`・
`tools/ci-impact.test.mjs`・`.github/workflows/ci.yml` の 3 ファイルに
閉じる。lane 構成・ruleset・分類規則には触れない。

## Upstream Documents

- `docs/specs/issue-157-matrix-placeholder-gate.md`(approved)
- `docs/specs/issue-157-selective-execution.md`(前提)

## Design Inputs

- `docs/designs/issue-157-matrix-placeholder-gate.md`(approved)

## File Map

- `tools/ci-impact.mjs` — `PLACEHOLDER_NAME_RE` 定数、`evaluateGate`
  の unclassified 検査に placeholder 分岐、`expectedJobs()` 純粋関数 +
  `expected-jobs` サブコマンド、usage/ヘッダコメント。
- `tools/ci-impact.test.mjs` — placeholder 受理/拒否/unclassified、
  `expectedJobs` 13/14/11、run 41894049161 fixture 再生の各ケース。
- `.github/workflows/ci.yml` — evidence job `expected` 式を
  `expected-jobs` 呼出しに置換 + コメント実態整合。

## Implementation Sequence

- [x] 欠陥機構の実測確認(run 41894049161 jobs API・PR #161 files)。
- [ ] `tools/ci-impact.test.mjs` に失敗テストを先に追加(red)。
- [ ] `tools/ci-impact.mjs` 実装(placeholder 判定 + expected-jobs)。
- [ ] `ci.yml` evidence job の `expected` 式置換。
- [ ] worktree 検証(`mise run test-ci-impact`・`mise run lint-actions`)。
- [ ] run 41894049161 fixture 再生(補助検証)。
- [ ] `git merge origin/main`・コミット・push・PR・
      `gh pr checks --watch --required`。

## Dependencies and Risks

- placeholder 名形式は GitHub 実装依存 — 形式変更時は unclassified
  violation(fail-closed)で検出、緑化リスクなし。
- `expected-jobs` の report 非読取 fallback は最大 universe(14)へ
  倒す — under-expect(欠落見逃し)を構造的に排除。
- 既存の `lanes` サブコマンドは残す(他参照は無いが診断用途として
  意味を変えない)。`expected` 計算は `expectedJobs` に一本化。

## Before / After・Rollback

### Before

- skip 済み matrix lane → `<lane> (${{ matrix.os }})` placeholder が
  unclassified violation → evidence fail。
- `expected=14` 固定 → skip run で retry 全回不足 → fail。
- 非 matrix lane skip は通常名 skipped entry で既に正しく扱えた
  (本変更で触らない)。

### After

- placeholder は lane の scheduled 状態で正当性判定。正当なら
  rows に `skipped-placeholder` 記録、不当なら required-lane-skipped
  violation。
- `expected` は impact report 由来の正確値(13/14/11 等)。
- gate の verdict 意味論は不変(required leg の skip/fail/absent は
  引き続き fail)。

### Rollback

- 本変更の revert で即時復旧(placeholder は再び violation になるが、
  `CI_SELECTIVE_DISABLE=1` kill switch で placeholder 自体を消せる —
  spec #157 の rollback 経路がそのまま効く)。

## Task Breakdown

- `docs/tasks/issue-157-t1-matrix-placeholder-gate.md` — 上記 3
  ファイル + 検証 + PR までを担う単一 task(gate 判定と収集カウントは
  同一欠陥の両面で不可分のため分割しない)。

## Verification

- [ ] `mise run test-ci-impact` pass(新規ケース + 既存非回帰)。
- [ ] `mise run lint-actions` pass。
- [ ] fixture 再生: run 41894049161 jobs + PR #161 files 由来の
      report で evaluateGate ok=true(補助検証・CI ではなく unit test)。
- [ ] `mise exec -- prek run --all-files` pass。
- [ ] PR の evidence run で fail しない(本 PR は tools/ci 変更のため
      全 lane 実行; placeholder 受理は unit test が正本)。

## Open Questions / Gaps

- なし(機構は coordinator 調査 + run 実測で確定済み)。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution(3 ファイル・単一 task・不可分)。
- Independent work streams: なし。
- User approval required before dispatch: yes(本 PR は coordinator
  dispatch により承認済みとして扱う)。

## Review Handoff

- レビュー観点: PLACEHOLDER_NAME_RE が実測名 `ui-verify (${{ matrix.os }})`
  に一致し通常 leg 名を誤認しないか、scheduled 判定が malformed report
  で fail-closed か、expectedJobs が docs-only(11)でも正しいか、
  ci.yml 側に判定ロジックが漏れていないか。
