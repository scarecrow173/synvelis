---
id: "SPEC-nBrnIDkIfILvtzEXr42L56"
type: "spec"
status: "proposed"
title: "Issue #145 ci.yml manual-only 化 — pull_request/push trigger 撤去と dispatch 専用 Full CI"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/145"
    - "https://github.com/scarecrow173/synvelis/issues/166"
    - "https://github.com/scarecrow173/synvelis/issues/167"
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
  implements: []
  implemented-by:
    - "docs/adr/issue-145-ci-manual-only.md"
    - "docs/designs/issue-145-ci-manual-only.md"
  depends-on:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
    - "https://github.com/scarecrow173/synvelis/issues/145"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ci-synvelis-pr-push-regression-gate.md"
    - "docs/specs/issue-157-selective-execution.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/145"
  derived-by:
    - "docs/adr/issue-145-ci-manual-only.md"
    - "docs/designs/issue-145-ci-manual-only.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-145-ci-manual-only.md"
  references:
    - ".github/workflows/ci.yml"
    - ".github/workflows/daily-ci.yml"
    - ".github/workflows/ci-issue-tracker.yml"
    - ".github/workflows/ci-issue-tracker-reconcile.yml"
    - "tools/ci-issue-tracker-reconcile.test.mjs"
    - "tools/ci-summary.mjs"
    - "mise.toml"
  defers: []
  deferred-by: []
metadata:
  spec:
    acceptance-criteria-count: 10
---

# Issue #145 ci.yml manual-only 化 — pull_request/push trigger 撤去

## 目的

Daily Full CI 移行(#166)+ Scheduler/Tracker/reconcile(#167)の
main 受入が完了したため、Issue #145 の受入条件
「`pull_request`/`push(main)` の重い CI 自動起動を削除」を実施し、
`ci.yml` を `workflow_dispatch` 専用の手動/日次 Full CI にする。
SV は RF PR #381・AL PR #164 に続く 3 番目(最後)の consumer 適用。

## 前提証拠(実 API 確認済み — 本 task で再確認)

- Ruleset `main-checkstatus` (id 26548278): **disabled** — required
  status checks は実効 0(#145 の「Required 解除を先行確認」条件を
  満たす)。`main-protect` (id 26530045): **active** —
  pull_request 必須・reviews 0・non-fast-forward/deletion 禁止・
  bypass なし。classic branch protection なし(404)。
- Ruviform/audio-lens も同構造(3 repo とも required_status_checks
  なし・main-protect active)を 2026-11-06 に rulesets API で実測。
- Reconcile 入口 main 実受入: `ci-issue-tracker-reconcile` run
  `42183098205`(workflow_dispatch @ main e84a440)success —
  job 権限 Actions:read/Contents:read/Issues:write、共有 checkout
  `eed590e0a3837e61c943d9de1d3efcfd7df32cb0`、self-test 91/91、
  report `mode: reconcile` で権威 `ci` 履歴を評価(pending run
  → fail-closed noop)。
- RF PR #381・AL PR #164 は MERGED 済みで、各 PR/main push の CI
  起動 0 を coordinator が実 API 確認済み。

## Requirements

- R1: `.github/workflows/ci.yml` の `on:` から `pull_request` と
  `push` を除去し、`workflow_dispatch`(input `expected_sha`
  任意文字列)のみを残す。`pull_request`/`push`/`schedule`/
  `workflow_run`/`pull_request_target` いずれの trigger も
  持たないことを契約 test で pin する。
- R2: 12 leg(verify×{ubuntu,windows}・verify-macos・rt-verify・
  gw10-verify×{ubuntu,windows}・gw10-verify-macos・msrv-verify・
  ui-verify×{ubuntu,windows}・ui-verify-macos・dep-guard)と
  `evidence`(mode pr・macOS 非 required 互換集約)・`ci-summary`
  (mode full・macOS + evidence を required に含む)job の構造・
  needs・fail-closed 判定は一切変更しない。
- R3: `expected_sha` input・全 checkout の
  `ref: ${{ inputs.expected_sha || github.sha }}`・
  `ci-summary` の verify-sha 3 点一致契約(expected_sha ==
  github.sha == checked-out HEAD、未指定時は記録のみ)は不変 —
  daily-ci.yml の dispatch 契約の受け口。
- R4: `daily-ci.yml`(schedule+dispatch)・`ci-issue-tracker.yml`
  (`workflow_run` のみ)・`ci-issue-tracker-reconcile.yml`
  (schedule+dispatch)は無変更。正当な手動・定期・event 入口と
  concurrency 契約を維持する。
- R5: concurrency 定義
  `group: ${{ github.workflow }}-${{ github.ref }}` /
  `cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}` は
  維持(dispatch on main は完走させ、他 ref dispatch は最新のみ)。
- R6: 陳腐化するコメントのみ更新する — PR/push trigger 前提の記述・
  「main-checkstatus required 解除まで trigger 維持」という解除済み
  前提条件・ruleset required context としての `evidence` 位置づけ。
  実行契約を変える変更はしない。
- R7: 契約 test `tools/ci-issue-tracker-reconcile.test.mjs` の
  ci.yml trigger 記述を新契約へ転換する(dispatch 専用 +
  pull_request/push 不存在の陰性 assertion)。`pull_request` 系
  trigger が全 workflow に残らないことを走査で保証する。
- R8: 新 PR open/synchronize で `ci` run が起動しないことを、PR
  head SHA・作成時刻以降の Actions API(pr checks / run list)で
  実測し PR 本文と worker report に記録する。
- R9: `ci-issue-tracker-reconcile` の定期 cron は未観測のまま
  NotRun として報告する(manual run 成功を cron 成功と混同しない)。
- R10: GitHub 書き込みは本 task では行わない — ruleset 変更・
  merge・Issue close は owner/coordinator の権限。worker は
  読み取り確認と PR 作成まで。

## Out of Scope

- `daily-ci.yml`・`ci-issue-tracker.yml`・`ci-issue-tracker-reconcile.yml`
  の trigger・job 構造の変更(本 task は `ref:` 行すら触れない)。
- `tools/ci-summary.mjs` の判定ロジック変更(fail-closed 契約は
  不変 — mode pr/full の意味は trigger 非依存)。
- 製品 Rust/SDK/UI コード、Cargo.lock、ruleset 書き込み、
  `main` への直接 push、Issue 編集/close。
- `CI_BASE_SHA` 式の書換え — `github.event.pull_request.base.sha`
  は dispatch では空文字に解決し report context の
  `baseSha: null` 相当として記録されるのみ(gate 判定に使わない)。

## Acceptance Criteria

- [ ] `mise run test-ci-tracker-entry` pass — ci.yml dispatch 専用・
      pull_request/push 不存在・expected_sha 維持・既存入口不変。
- [ ] `mise run test-ci-summary` pass(36/36 — fail-closed 契約不変)。
- [ ] `mise run lint-actions` pass。
- [ ] `mise exec -- prek run --all-files` pass。
- [ ] `git diff`: ci.yml は `on:` trigger 除去 + 説明コメント更新のみ、
      他 workflow・mise.toml・製品コード無変更。
- [ ] Ruleset API 実測: 3 repo とも `main-checkstatus` disabled・
      `main-protect` active・required_status_checks 不在・classic
      protection 404。
- [ ] Reconcile 実受入: run 42183098205 の artifact/log が mode
      reconcile・pin eed590e・最小権限・noop(fail-closed)を示す。
- [ ] 新 PR で Actions API 上 `ci` run が opened/synchronize に
      対応する起動を持たない(head SHA・作成時刻で実測)。
- [ ] cron 未観測は NotRun 明記。実 cron・実 Issue 更新の受入は
      main merge 後の運用段階。
- [ ] GitHub 書き込み(ruleset 変更・merge・issue close)未実施。

## Deferred Design Concerns

- `evidence` job の pr-mode 集約は required context ではなくなったが
  ci-summary の needs 構成と失敗伝播のために維持する。将来的に
  ci-summary へ畳み込む simplification は別工程で判断する(現行は
  両者で異なる universe を照合する役割分担が残る)。
- `cancel-in-progress` の main 完走契約は push trigger 撤去後も
  daily dispatch の連続起動を直列化する意味を持つ。concurrency
  キーに ref を含む現行設計は維持。
- PR 作成時の検証責任は #145 の運用方針どおり実装者のローカル
  テスト記録 + レビューに移る — Daily/Tracker/reconcile が main
  上の失敗検知を担う。
