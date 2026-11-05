---
id: "TASK-034SYN166T1CiSumX"
type: "task"
status: "in-progress"
title: "Issue #166 T1 Full CI 化・ci-summary 統一 — 第一段階実装"
created: "2026-11-05"
updated: "2026-11-05"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/166"
  changes:
    added:
      - type: "file"
        path: "tools/ci-summary.mjs"
      - type: "file"
        path: "tools/ci-summary.test.mjs"
      - type: "file"
        path: "docs/specs/issue-166-full-ci-ci-summary.md"
      - type: "file"
        path: "docs/designs/issue-166-full-ci-ci-summary.md"
      - type: "file"
        path: "docs/plans/issue-166-full-ci-ci-summary.md"
      - type: "file"
        path: "docs/test-specs/issue-166-ci-summary-gate.md"
      - type: "file"
        path: "docs/tasks/issue-166-t1-ci-summary-stage1.md"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "docs/specs/issue-148-impact-classification.md"
      - type: "file"
        path: "docs/specs/issue-157-selective-execution.md"
      - type: "file"
        path: "docs/specs/issue-157-matrix-placeholder-gate.md"
    deleted:
      - type: "file"
        path: "tools/ci-impact.mjs"
      - type: "file"
        path: "tools/ci-impact.test.mjs"
      - type: "file"
        path: "tools/fixtures/run-41894049161-jobs.json"
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-166-full-ci-ci-summary.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-166-full-ci-ci-summary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-166-full-ci-ci-summary.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/ci-summary.mjs"
    - "tools/ci-summary.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #166 T1 Full CI 化・ci-summary 統一 — 第一段階

## Work

`ci.yml` を毎回全 lane 実行の Full CI 形へ第一段階移行する。classify/
選択実行機構を撤去し、fail-closed 集約を `tools/ci-summary.mjs` に
移して `ci-summary` job を新設、ruleset required の `evidence` job は
macOS 非 required の互換 gate として維持する。`workflow_dispatch` に
`expected_sha` 契約を実装する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/issue-166-full-ci-ci-summary.md`
- `docs/designs/issue-166-full-ci-ci-summary.md`
- `docs/plans/issue-166-full-ci-ci-summary.md`
- `docs/test-specs/issue-166-ci-summary-gate.md`

## Dependencies

- なし(単一 task)。GitHub 側書き込みは全てスコープ外。

## Affected Files

- `tools/ci-summary.mjs`(新設)
- `tools/ci-summary.test.mjs`(新設)
- `tools/ci-impact.mjs` / `tools/ci-impact.test.mjs` /
  `tools/fixtures/run-41894049161-jobs.json`(削除)
- `.github/workflows/ci.yml`
- `mise.toml`
- `docs/specs/issue-148|issue-157*`(superseded 標記)
- `docs/*/README.md`(index 行)

## Done When

- [x] `mise run test-ci-summary` pass(29 tests・検証仕様の全契約 pin)。
- [x] `mise run lint-actions` pass。
- [x] `mise exec -- prek run --all-files` pass。
- [x] ci.yml: classify/lane `if:`/`CI_SELECTIVE_DISABLE` が無く、
      `evidence`(required 互換)と `ci-summary`(full 集約)が
      `if: always()` で併存、全 lane が trigger 毎回実行。
- [x] `workflow_dispatch.inputs.expected_sha` と checkout pin・
      `ci-summary` の HEAD 一致検証が実装済み。
- [x] 旧 spec 3件に superseded 標記、README index 更新。
- [x] ローカル検証の実行結果(コマンド・exit code)が報告済み。

## Verification

- [x] `mise run test-ci-summary` — 29 pass / 0 fail(Windows・mise 管理 node 26.12.1)。
- [x] `node tools/ci-summary.mjs gate`(合成 run-jobs 再生) —
      pr: macOS fail を observed として pass(exit 0)、full: macOS
      fail/absent + evidence in_progress を violation として fail
      (exit 1)、all-green: pass(exit 0)。
- [x] `node tools/ci-summary.mjs expected-jobs --mode pr|full` — 13 / 14。
- [x] `mise run lint-actions` — actionlint clean(exit 0)。
- [x] `mise exec -- prek run --all-files` — 全 hook pass(exit 0)。
- [x] `mise run fmt-check` / `check` / `lint` / `test` — 全 pass
      (CARGO_NET_OFFLINE=1 で依存 download 無しを強制、exit 0)。
- GitHub Actions 実 run・PR check — 実行不可(GitHub 書き込み禁止、
  coordinator へブロッキング条件として報告)。
