---
id: "PLAN-ztcxOYrIjvxF2hswb1Wnb0"
type: "plan"
status: "in-progress"
title: "Issue #145 ci.yml manual-only 実装計画 — trigger 撤去 + 契約 test 転換 + 実起動ゼロ検証"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/145"
    - "docs/specs/issue-145-ci-manual-only.md"
    - "docs/designs/issue-145-ci-manual-only.md"
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
    - "docs/designs/issue-145-ci-manual-only.md"
  implemented-by:
    - "docs/tasks/issue-145-t1-ci-manual-only.md"
  depends-on:
    - "docs/specs/issue-145-ci-manual-only.md"
    - "docs/designs/issue-145-ci-manual-only.md"
    - "docs/adr/issue-145-ci-manual-only.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-145-ci-manual-only.md"
  derived-by:
    - "docs/tasks/issue-145-t1-ci-manual-only.md"
    - "docs/test-specs/issue-145-ci-manual-only.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-145-ci-manual-only.md"
  references:
    - ".github/workflows/ci.yml"
    - "tools/ci-issue-tracker-reconcile.test.mjs"
    - "tools/ci-summary.mjs"
  defers: []
  deferred-by: []
---

# Issue #145 ci.yml manual-only 実装計画 Implementation Plan

## Goal

`ci.yml` の `pull_request`/`push` trigger を撤去して
`workflow_dispatch` 専用にし、契約 test を新契約へ転換する。
lane 構造・fail-closed 集約・expected_sha dispatch 契約・他 3
workflow は不変。新 PR で CI 自動起動ゼロを Actions API で実測して
完了とする。

## Scope

1 本の実装ストリーム: doc chain → ci.yml trigger/comment 差分 +
test 転換 → 検証 → index 再生成 → 独立レビュー → logical commit →
push → 日本語 PR → 起動ゼロ実測 → worker_done。

## Upstream Documents

- `docs/specs/issue-145-ci-manual-only.md`
  (SPEC-nBrnIDkIfILvtzEXr42L56)
- `docs/adr/issue-145-ci-manual-only.md`
  (ADR-qs0t3aMSfTG19Vm5F4r2fp)
- `docs/designs/issue-145-ci-manual-only.md`
  (DESIGN-7Y20yWdQNliSve36HXn2Hy)

## 外部正本(変更しない)

- Ruviform Issue #365 / Synvelis Issue #145(運用方針の正本)。
- RF `ci.yml`(#381 merge 済み)の dispatch 専用形 — 参照実装。
- daily-ci.yml・ci-issue-tracker.yml・ci-issue-tracker-reconcile.yml
  — 本 task で触れない。

## File Map

- `.github/workflows/ci.yml` — `on:` から pull_request/push を除去
  (workflow_dispatch + expected_sha input は不変)。trigger 実在を
  述べる冒頭/concurrency/evidence コメントを実態に合わせる。
- `tools/ci-issue-tracker-reconcile.test.mjs` — ci.yml trigger
  test を「dispatch 専用 + 陰性 + lane 構造存在」へ転換。
- `docs/specs|adr|designs|plans|test-specs|tasks/README.md` —
  `doc_maintenance.js apply` で index 再生成。

## Implementation Sequence

- [x] T0: 最新 main 取込(e84a440)+ reconcile run 42183098205 の
      実受入確認(artifact/log)+ 3 repo ruleset 再検証。
- [x] T1: doc chain(spec/adr/design/plan/test-spec/task)作成。
- [ ] T1: ci.yml `on:` 差分 + 陳腐コメント更新。
- [ ] T1: 契約 test 転換(新契約 assertion)。
- [ ] T1: `doc_maintenance.js apply` で index 再生成。
- [ ] 検証: `mise run test-ci-tracker-entry`・`mise run
      test-ci-summary`・`mise run lint-actions`・
      `mise exec -- prek run --all-files`・`git diff` 確認。
- [ ] 独立レビュー(review-protocol — 本 task は subagent 禁止の
      ため分離 pass 実施)で blocker なし確認。
- [ ] Conventional Commits → `fix/sv-ci-manual-only` push → 日本語 PR。
- [ ] PR head SHA/作成時刻以降の Actions API で `ci` 系 run 起動 0
      を実測(pr checks / run list)し PR 本文・report に記録。

## Dependencies and Risks

- ruleset は read のみ — `main-checkstatus` disabled・
  `main-protect` active は 2026-11-06 実測済み(ADR 前提証拠)。
- 撤去後の `pull_request` 復活混入は契約 test の全 workflow 走査で
  検出。
- `CI_BASE_SHA` 空解決は report context 記録のみ — gate 判定に
  影響しない(ci-summary 36/36 で mode 契約検証済み)。
- cron 未観測は NotRun — manual run 42183098205 の成功を cron
  success に混同しない。
- worktree の `nul`(他者生成物)は触らない。

## Verification Matrix

| 検証 | 入口 | 期待 |
|---|---|---|
| ci.yml dispatch 専用 + 不変契約 | `mise run test-ci-tracker-entry` | 全 assertion pass |
| fail-closed 集約契約 | `mise run test-ci-summary` | 36/36 pass |
| workflow 構文 | `mise run lint-actions` | pass |
| yaml/toml/hook | `mise exec -- prek run --all-files` | pass |
| ruleset 前提 | `gh api repos/{repo}/rulesets` ×3 | required 0・main-protect active |
| reconcile 実受入 | `gh run view/download 42183098205` | mode reconcile・eed590e・noop |
| PR 起動ゼロ | `gh pr checks` + `gh run list` | head SHA 以降に ci run なし |
| cron 観測 | — | NotRun(次 tick 以降・本工程範囲外) |

## Rollback

- `git revert` で trigger が復帰するだけ — 他への波及なし。
  ruleset は変更していないので required が死ぬ心配もない
  (main-checkstatus は disabled のまま)。

## Task Breakdown

- T1: `docs/tasks/issue-145-t1-ci-manual-only.md`
  (TASK-HpVlbSyIgBaWAyYMVxHYLp)
