---
id: "TASK-034RUl0VioQz9IinY5Qc7G"
type: "task"
status: "in-progress"
title: "Issue #157 rev.3 T1 matrix placeholder gate 修正"
created: "2026-11-05"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/157"
    - "https://github.com/scarecrow173/synvelis/pull/161"
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
    - "docs/plans/issue-157-matrix-placeholder-gate.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-157-matrix-placeholder-gate.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-157-matrix-placeholder-gate.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "https://github.com/scarecrow173/synvelis/blob/3f4f4450131b0258a1919b213de3ad394fbbb709/tools/ci-impact.mjs"
    - "https://github.com/scarecrow173/synvelis/blob/3f4f4450131b0258a1919b213de3ad394fbbb709/tools/ci-impact.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #157 rev.3 T1 matrix placeholder gate 修正

## Work

`evaluateGate` に skip 済み matrix lane placeholder の分類・判定を追加
し、`expected-jobs` サブコマンドで収集カウントを正確化する。ci.yml の
evidence job `expected` 式を置換する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/issue-157-matrix-placeholder-gate.md`
- `docs/specs/issue-157-matrix-placeholder-gate.md`
- `docs/test-specs/issue-157-matrix-placeholder-gate.md`

## Dependencies

- なし(単一 task・先行依存なし)。

## Affected Files

- `tools/ci-impact.mjs`
- `tools/ci-impact.test.mjs`
- `.github/workflows/ci.yml`

## Done When

- [x] placeholder 契約(a)(b)(c)と `expected-jobs`(d)が test で pin
      され全件 pass、既存テスト非回帰(e)。
- [x] run 41894049161 fixture 再生で evaluateGate ok=true。
- [x] `mise run lint-actions` pass、`prek run --all-files` pass。
- [ ] PR 作成・Required Check 緑。

## Verification

- [x] `mise run test-ci-impact`
- [x] `mise run lint-actions`
- [x] `mise exec -- prek run --all-files`
- [ ] `gh pr checks --watch --required`
