---
id: "PLAN-034SYN166StG1CiSm"
type: "plan"
status: "proposed"
title: "Issue #166 Full CI 化・ci-summary 統一 — 第一段階実装計画"
created: "2026-11-05"
updated: "2026-11-05"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/166"
    - "docs/specs/issue-166-full-ci-ci-summary.md"
    - "docs/designs/issue-166-full-ci-ci-summary.md"
  changes:
    added:
      - type: "file"
        path: "tools/ci-summary.mjs"
      - type: "file"
        path: "tools/ci-summary.test.mjs"
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
    - "docs/designs/issue-166-full-ci-ci-summary.md"
  implemented-by:
    - "docs/tasks/issue-166-t1-ci-summary-stage1.md"
  depends-on:
    - "docs/designs/issue-166-full-ci-ci-summary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-166-full-ci-ci-summary.md"
  derived-by:
    - "docs/tasks/issue-166-t1-ci-summary-stage1.md"
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

# Issue #166 Full CI 化・ci-summary 統一 — 第一段階実装計画

## 方針

単一の実装 stream。変更は `tools/ci-summary.mjs`・
`tools/ci-summary.test.mjs`・`.github/workflows/ci.yml`・`mise.toml`・
docs(superseded 標記)に閉じる。gate の核となる verdict 規則は
`tools/ci-impact.mjs` の `evaluateGate` 実績ロジックを移植し、分類・
期待集合・shadow・placeholder 受理の経路を削除して単純化する。

## 実装順序

1. **`tools/ci-summary.mjs` 新設** — universe 定数
   (REQUIRED_LEGS/MACOS_LEGS/GATE_JOBS/DECLARED_NOT_RUN)、
   `evaluateGate({jobs, attempt, mode})`、`expectedJobs(mode)`、
   markdown 出力(SHA contract セクション含む)、CLI
   (`gate`/`expected-jobs`/`lanes`)。
2. **`tools/ci-summary.test.mjs` 新設** — `docs/test-specs/
   issue-166-ci-summary-gate.md` の契約を node:test で pin
   (失敗ケースを先に書き red 確認してから実装を完成させる)。
3. **`mise.toml`** — `ci-impact`/`ci-evidence-gate`/`test-ci-impact`
   を削除し `ci-summary-gate`/`test-ci-summary` を追加。
4. **`ci.yml` 書換** — ヘッダコメント・`expected_sha` input・
   classify 撤去・lane `if:`/`needs` 除去・checkout `ref:` pin・
   `evidence` 配線置換・`ci-summary` job 追加。
5. **旧資産撤去** — `tools/ci-impact.mjs`・`tools/ci-impact.test.mjs`・
   `tools/fixtures/run-41894049161-jobs.json` 削除(他参照なしを
   grep 確認済み)。旧 spec 3件に superseded 標記。
6. **docs index 更新** — specs/designs/plans/tasks/test-specs README
   の generated-index 表へ新行を追加し、退役 spec の status 表示を
   整合させる。

## 検証手順(worktree 内・GitHub 書き込みなし)

- [ ] `mise run test-ci-summary` — 契約 pin 全件 pass。
- [ ] `node tools/ci-summary.mjs gate --mode pr|full` を合成
      run-jobs JSON で再生 — pass/fail/violation を実測。
- [ ] `node tools/ci-summary.mjs expected-jobs --mode pr|full`
      — 13/14。
- [ ] `mise run lint-actions` — ci.yml が actionlint clean。
- [ ] `mise exec -- prek run --all-files` — whitespace/yaml/toml。
- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
      `mise run test` — Rust workspace 非回帰(本変更は workflow/
      tools/docs のみだが、lockfile・manifest 非回帰を確認)。
- [ ] `git diff` 自己レビュー — 検証範囲縮小・context 消失なし。

## 実行しない検証と理由

- GitHub Actions 上の実 run(dispatch/PR/push): GitHub 側書き込み
  禁止のため実施不可。構造は actionlint + jobs API 契約のローカル
  再生で代替検証する。
- `mise run gw10`/`test-msrv` 等の lane 実測: workflow 側で task 呼出し
  を変更していないため非対象(本変更は実行制御層のみ)。

## リスク

- **`ci-summary` の jobs API 収集**: `evidence` を needs に含めるため
  evidence 完走後に起動し、14 件の収集は成立する(bounded retry で
  eventual consistency も吸収)。
- **expected_sha 未指定時**: `github.sha` fallback により PR/push で
  副作用なし。dispatch で ref HEAD と github.sha が一致することは
  GitHub の event payload 契約上自明。
- **checkout `ref:` 指定の副作用**: pull_request で `github.sha` は
  merge commit を指し既定 checkout と同一ツリー(detached HEAD)。
  push は github.sha で race-free。いずれも現行挙動と等価。
- **`evidence` job 失敗時の ci-summary**: required として violation
  になり fail — 意図どおり(gate infra 障害を成功にしない)。
