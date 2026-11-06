---
id: "PLAN-FJFu7KEhoGpsO1UDDzKK6a"
type: "plan"
status: "in-progress"
title: "Issue #167 Tracker reconcile 実装計画 — 共有 pin eed590e + Consumer reconcile workflow + 契約 test"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/167"
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
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
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
  implemented-by:
    - "docs/tasks/issue-167-t1-tracker-reconcile-entry.md"
  depends-on:
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
    - "docs/adr/issue-167-tracker-reconcile-entry.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
  derived-by:
    - "docs/tasks/issue-167-t1-tracker-reconcile-entry.md"
    - "docs/test-specs/issue-167-tracker-reconcile-entry.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-167-tracker-reconcile-entry.md"
  references:
    - ".github/workflows/ci-issue-tracker.yml"
    - ".github/workflows/daily-ci.yml"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #167 Tracker reconcile 実装計画 Implementation Plan

## Goal

共有 CI 実装の Consumer pin を `eed590e0a3837e61c943d9de1d3efcfd7df32cb0`
へ揃え、`ci-issue-tracker-reconcile.yml`(`schedule` +
`workflow_dispatch` のみ)を Consumer 共有 checkout パターンで新設し、
GITHUB_TOKEN dispatch 完了の通知 gap を `--reconcile` 経路で埋める。
`ci-issue-tracker.yml` には trigger を追加せず、最小権限・既存
secret・fail-closed 契約を不変とする。

## Scope

1 本の実装ストリーム。doc chain(spec/adr/design/plan/test-spec/task)
→ Consumer 契約 test(RED)→ workflow 実装 + pin 更新(GREEN)→
生成 index 再生成 → 検証 → logical commit → push → 日本語 PR。

## Upstream Documents

- `docs/specs/issue-167-tracker-reconcile-entry.md`
  (SPEC-jY8M1QCzHBuilakae6Hmyw)
- `docs/adr/issue-167-tracker-reconcile-entry.md`
  (ADR-XTlIzRk1SB1nTV82KYSW6m — Consumer 採用契約)
- `docs/designs/issue-167-tracker-reconcile-entry.md`
  (DESIGN-RBfO0jdqOYWxb6xvtT5z0p)

## 外部正本(Ruviform 側、本 repo では変更しない)

- Ruviform PR #380 → main `eed590e0a3837e61c943d9de1d3efcfd7df32cb0`:
  `ci/tracker/tracker.mjs --reconcile` + `ci-issue-tracker-reconcile.yml`
  参照実装 + ADR-vJvyZNtrHi2xnycJxbnSPv。
- RF `ci-issue-tracker.yml` の `workflow_run` 専用契約 — SV 側も同契約を
  採用済み(#169/#170)。

## File Map

- `.github/workflows/ci-issue-tracker-reconcile.yml` — 新設。schedule
  `47 */6 * * *` + `workflow_dispatch`・`permissions: {}`・同一
  concurrency group・job guard・15 分 timeout・最小権限・2 段
  checkout + self-test + `tracker.mjs --reconcile --report` + 別名
  artifact。
- `.github/workflows/ci-issue-tracker.yml` — 共有 checkout `ref:` のみ
  `73d51d8…` → `eed590e…`(1 行)。
- `.github/workflows/daily-ci.yml` — 同上(1 行)。
- `tools/ci-issue-tracker-reconcile.test.mjs` — 新設。Consumer 契約の
  `node --test` 静的 test(YAML パーサ非依存の構造 assertion)。
- `mise.toml` — `[tasks.test-ci-tracker-entry]` 追加。
- `docs/specs|adr|designs|plans|test-specs|tasks/README.md` —
  doc-maintenance `apply` で index 再生成。

## Implementation Sequence

- [ ] T1: `tools/ci-issue-tracker-reconcile.test.mjs` を先に書く
      (RED — 新 workflow 非存在・旧 pin で fail)。
- [ ] T1: `ci-issue-tracker-reconcile.yml` 新設 + 2 ファイルの `ref:`
      更新 + `mise.toml` task 追加(GREEN)。
- [ ] T1: `doc_maintenance.js apply` で docs index 再生成。
- [ ] 検証: `mise run test-ci-tracker-entry`・`mise run lint-actions`・
      `mise run test-ci-summary`・pin 版 tarball 上の共有 tracker/daily
      test・`mise exec -- prek run --all-files`・`git diff` で既存
      workflow が `ref:` 行のみ・`ci.yml` 無変更。
- [ ] 独立レビュー(review-protocol)で blocker なし確認後、
      Conventional Commits(日本語)で論理単位コミット → branch push →
      日本語 PR。

## Dependencies and Risks

- 共有 evaluator は Ruviform 所有 — `--reconcile`・guard・Issue 冪等は
  pin eed590e の中身に委譲し、SV 側で再実装しない。
- `docs/*/README.md` は生成 index — `doc_maintenance.js apply` で
  再生成し手編集しない。
- `ci.yml` の trigger 撤去・ruleset 変更は本工程スコープ外(後段)。
  contract test が `ci.yml` の trigger 不変を pin して誤混入を防ぐ。
- 実 cron・実 Issue・実 dispatch は行わない — offline 検証まで。
  新実装の cron 未観測を Pass にしない(merge 後の実受入は別工程)。
- 既存の graph blocker(削除済み `ci-impact.mjs` への旧 references 等)
  は本変更と無関係 — 触れない。

## Verification Matrix

| 検証 | 入口 | 期待 |
|---|---|---|
| Consumer 契約(pin/mode/guard/入口) | `mise run test-ci-tracker-entry` | 全 assertion pass |
| workflow 構文 | `mise run lint-actions` | pass |
| ci-summary 契約(無変更回帰) | `mise run test-ci-summary` | pass |
| 共有 evaluator tracker 契約 | `node --test <pin tarball>/ci/tracker/` | 91 pass |
| 共有 evaluator daily 契約 | `node --test <pin tarball>/ci/daily/` | 50 pass |
| yaml/toml/hook | `mise exec -- prek run --all-files` | pass |
| 既存 workflow 不変 | `git diff` | `ref:` 行のみ・`ci.yml` 無変更 |

## Rollback

- 全変更は worktree 内 git commit。問題時は当該 commit の revert。
  `ci-issue-tracker-reconcile.yml` の revert は reconcile 入口のみを
  止め、event 入口・daily・ci.yml には無影響。pin 戻しは `ref:` を
  `73d51d8` に戻すだけで旧契約へ復帰(reconcile workflow も pin を
  参照するため evaluator 不整合にはならない — ただし revert 時は
  新 workflow ごと revert する)。

## Task Breakdown

- T1: `docs/tasks/issue-167-t1-tracker-reconcile-entry.md`
  (TASK-QSUlMIQI1FYoKq116Yk7xJ)
