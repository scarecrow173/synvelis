---
id: "TASK-HpVlbSyIgBaWAyYMVxHYLp"
type: "task"
status: "in-progress"
title: "Issue #145 T1 ci.yml pull_request/push trigger 撤去 + 契約 test 転換 + PR 起動ゼロ実測"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/145"
  changes:
    added:
      - type: "file"
        path: "docs/specs/issue-145-ci-manual-only.md"
      - type: "file"
        path: "docs/adr/issue-145-ci-manual-only.md"
      - type: "file"
        path: "docs/designs/issue-145-ci-manual-only.md"
      - type: "file"
        path: "docs/plans/issue-145-ci-manual-only.md"
      - type: "file"
        path: "docs/test-specs/issue-145-ci-manual-only.md"
      - type: "file"
        path: "docs/tasks/issue-145-t1-ci-manual-only.md"
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
    - "docs/plans/issue-145-ci-manual-only.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-145-ci-manual-only.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-145-ci-manual-only.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "tools/ci-issue-tracker-reconcile.test.mjs"
    - "tools/ci-summary.mjs"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #145 T1 ci.yml pull_request/push trigger 撤去 + 契約 test 転換 + PR 起動ゼロ実測

## Work

`ci.yml` の `on:` を `workflow_dispatch` のみにし、trigger 前提の
陳腐コメントを実態に合わせる。契約 test の ci.yml assertion を
「dispatch 専用 + 全 workflow 陰性走査 + lane 構造存在」へ転換。
新 PR の opened/synchronize で `ci` 系 run が起動しないことを
Actions API で実測して完了とする。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/issue-145-ci-manual-only.md`
- `docs/adr/issue-145-ci-manual-only.md`
- `docs/designs/issue-145-ci-manual-only.md`
- `docs/plans/issue-145-ci-manual-only.md`
- `docs/test-specs/issue-145-ci-manual-only.md`

## Dependencies

- 前提は全て確認済み: reconcile run 42183098205 成功(mode
  reconcile・eed590e・最小権限・fail-closed noop)、3 repo
  `main-checkstatus` disabled・`main-protect` active・required 0。
- GitHub 書き込み(ruleset・merge・Issue close)は全てスコープ外。

## Affected Files

- `.github/workflows/ci.yml`(`on:` 差分 + コメント更新)
- `tools/ci-issue-tracker-reconcile.test.mjs`(ci.yml trigger
  assertion 転換)
- `docs/*/README.md`(生成 index 再生成)

## Done When

- [ ] `mise run test-ci-tracker-entry` pass(新契約 assertion)。
- [ ] `mise run test-ci-summary` pass(36/36)。
- [ ] `mise run lint-actions` pass。
- [ ] `mise exec -- prek run --all-files` pass。
- [ ] `git diff`: ci.yml は trigger 除去 + コメントのみ、他 workflow・
      mise.toml・製品コード無変更。
- [ ] docs index が新文書 6 件を含む。
- [ ] 新 PR で `gh run list`/`gh pr checks` が head SHA 以降の
      `ci` 系 run 起動 0 を示す(実測値を記録)。

## Verification

実施結果(Windows 11 ・ worktree sv-ci-reconcile ・ 2026-11-06):

- `mise run test-ci-tracker-entry` — exit 0、16/16 pass
  (dispatch 専用・全 workflow 陰性走査・lane/gate 構造不変)。
- `mise run test-ci-summary` — exit 0、36/36 pass。
- `mise run lint-actions`(actionlint) — exit 0。
- `mise exec -- prek run --all-files` — exit 0、全 hook pass。
- `git diff`: ci.yml は `on:` 除去 + コメント更新のみ、他
  workflow・mise.toml・製品コード無変更。diff 全体レビュー済み。
- `doc_maintenance.js apply` — 6 index 再生成、issue-145 文書の
  finding 0(残存 finding は全て pre-existing)。
- reconcile main 実受入: `gh run view/download 42183098205` —
  workflow_dispatch @ main e84a440・job reconcile success、
  permissions Actions:read/Contents:read/Issues:write、共有
  checkout `eed590e`、self-test 91/91、report `mode: reconcile`
  で権威 `ci` 履歴評価 → pending run 42183097054 に fail-closed
  noop(Issue 書き込みなし — 正しい挙動)。
- ruleset 実測(2026-11-06 `gh api repos/{repo}/rulesets` ×3):
  `main-checkstatus` disabled・`main-protect` active(PR 必須・
  reviews 0・non-ff/deletion 禁止・bypass なし)・
  `required_status_checks` 不在・classic protection 404 — SV/RF/AL
  全 repo 同一。
- PR 起動ゼロ実測: PR #173(head `b72c523f2766a6786d368fadbd63bc514c1a8bd9`、
  created 2026-11-06T08:04:18Z)で `gh pr checks 173` =
  "no checks reported"、`gh run list --branch
  fix/sv-ci-manual-only` = 0 件、repo-wide run list に作成時刻以降の
  `ci`/`daily-ci`/tracker 系 run なし。synchronize 後も同測定を
  記録(下記追記)。
- 未実施(NotRun): 定期 cron 実観測・実 Issue create/close・製品
  Rust/UI suite(変更範囲外のため)。
