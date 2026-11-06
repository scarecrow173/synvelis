---
id: "DESIGN-7Y20yWdQNliSve36HXn2Hy"
type: "design"
status: "approved"
title: "Issue #145 ci.yml manual-only 設計 — trigger 撤去の最小差分と不変契約の境界"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/145"
    - "docs/specs/issue-145-ci-manual-only.md"
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
    generated: []
  implements:
    - "docs/specs/issue-145-ci-manual-only.md"
    - "docs/adr/issue-145-ci-manual-only.md"
  implemented-by:
    - "docs/plans/issue-145-ci-manual-only.md"
  depends-on:
    - "docs/specs/issue-145-ci-manual-only.md"
    - "docs/adr/issue-145-ci-manual-only.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-166-full-ci-ci-summary.md"
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-145-ci-manual-only.md"
    - "docs/adr/issue-145-ci-manual-only.md"
  derived-by:
    - "docs/plans/issue-145-ci-manual-only.md"
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

# Issue #145 ci.yml manual-only 設計 — trigger 撤去の最小差分

## Context

`ci.yml` 自体の実行契約(全 lane 毎回実行・2 層集約・expected_sha)
は #166/#167 で確立済みで、今回変えるのは「誰がこの workflow を
起動するか」だけ。設計上の問いは (1) `on:` 差分の正確な形、
(2) trigger 前提で書かれたコメントのうち更新が必要なものと
残すものの線引き、(3) 契約 test をどう転換して新契約を pin するか。

## Components and Boundaries

### `on:` 差分(ci.yml)

```yaml
# before
on:
  pull_request:
    branches: [main]
  push:
    branches: [main]
  workflow_dispatch:
    inputs:
      expected_sha:
        description: "Expected main candidate SHA this run verifies — checked out by every job and asserted by ci-summary"
        required: false
        type: string

# after — workflow_dispatch のみ
on:
  workflow_dispatch:
    inputs:
      expected_sha:
        description: "Expected main candidate SHA this run verifies — checked out by every job and asserted by ci-summary"
        required: false
        type: string
```

`workflow_dispatch` 節と input 定義は一字も変えない — daily-ci の
dispatch 契約(`expected_sha`)の受け口は正本のまま残す。

### コメント更新の線引き

更新するのは **trigger の実在を述べた記述が事実と食い違う箇所**だけ:

| 位置 | 現行 | 更新方針 |
|---|---|---|
| 冒頭(行3-4) | 「本 repo 自身の PR/main push/手動起動を検証する」 | 手動/日次 dispatch 専用の Full CI と明記(#145 による trigger 撤去済み)。 |
| 行24-26 | 「pull_request/push trigger と `evidence` context は owner が main-checkstatus required を解除するまで維持する(#365 決定6…)」 | 前提達成の記録へ: main-checkstatus disabled(2026-11-06 rulesets API 実測)→ trigger 撤去を実施。 |
| 行34-35 | 「未指定の手動起動・PR/push では既定 ref」 | 「expected_sha 未指定の手動起動では既定 ref」に限定。 |
| concurrency(行51) | 「main push の証跡は完走、PR/dispatch は最新のみ」 | 「main への dispatch は完走、他 ref は最新のみ」へ実態合わせ。 |
| evidence 冒頭コメント | 「ruleset required context `evidence` を維持する互換層」 | required context は解除済み — pr-mode(macOS 非 required)集約として ci-summary 手前の fail-closed 層の役割に言い換え。 |

残すコメント: lane の fail-closed 記述、expected_sha 契約、
dep-guard/verify 系の実行説明 — trigger に依存しないので触らない。

### 不変の実行契約(差分に含めない)

- 12 leg + `evidence`(needs 非 macOS leg、mode pr)+ `ci-summary`
  (needs 全 leg + evidence、mode full)の job 構造・`if: always()`・
  permissions・env。
- `ref: ${{ inputs.expected_sha || github.sha }}` の全 checkout pin。
- `CI_BASE_SHA: ${{ github.event.pull_request.base.sha }}`:
  dispatch では空文字に解決され `tools/ci-summary.mjs` が report
  context の `baseSha` として記録するのみ(gate 判定入力ではない)。
  式の書換えは意味論を変える可能性があるため本差分では触らない。
- `concurrency.group`/ `cancel-in-progress` 式。
- 他 3 workflow(daily-ci・ci-issue-tracker・ci-issue-tracker-
  reconcile) — 本 task では一切触れない。

### `tools/ci-issue-tracker-reconcile.test.mjs` の転換

`"ci.yml triggers unchanged"` test を新契約へ置換:

- `on:` block が `workflow_dispatch` を持ち、`pull_request`・
  `push:`・`schedule`・`workflow_run` を持たない(陰性)。
- `expected_sha` input 記述が残る(daily 契約の不変性 pin)。
- 全 workflow の走査で `pull_request`/`pull_request_target`/
  `push:` trigger が存在しない(補助入口の混入ガード)。
- 12 leg + evidence + ci-summary の job key 存在を pin(撤去が
  lane 構造を壊さないことの構造 assertion)。

test-spec は上記 Example として記述し、実装は構造テキスト
assertion(YAML パーサ非依存)を維持する。

## 境界と非目標

- ruleset・Issue・main への書き込みは行わない。
- `evidence` の pr-mode 集約を ci-summary へ畳み込む refactor は
  行わない(スコープ分離 — trigger 撤去と構造変更を混ぜない)。
- PR 上での CI 実行証跡の代替運用(ローカルテスト記録)は #145 の
  運用方針に従い、本 PR で新たな仕組みを作らない。
- `nul` など worktree 内の無関係な untracked ファイルは触らない。

## Failure Modes

- **trigger 撤去後に誰かが pull_request を復活させる**: 契約 test の
  陰性 assertion + 全 workflow 走査で検出。
- **dispatch 時 `github.event.pull_request` 不在で式が壊れる**:
  GitHub expression は不在を空文字に解決し、`CI_BASE_SHA` は
  report 記録のみで gate 判定に使わない — 影響なし(ローカル契約
  test で mode 評価を確認済み:ci-summary 36/36)。
- **PR 作成時の自動起動誤認**: Actions API を head SHA・作成時刻で
  照合し、`ci`/`daily-ci`/`ci-issue-tracker*` の新規 run が PR
  に紐づかないことを実測してから worker_done する。
- **`evidence` 廃止要求**: 本 task では構造維持 — ci-summary の
  needs 構成を壊さない。
