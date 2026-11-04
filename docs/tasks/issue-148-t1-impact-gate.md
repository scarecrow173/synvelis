---
id: "TASK-034SVCIOPT02zA1sDfGh"
type: "task"
status: "in-progress"
title: "Issue #148 T1 impact classifier + fail-closed evidence gate 実装"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/148"
  changes:
    added:
      - type: "file"
        path: "tools/ci-impact.mjs"
      - type: "file"
        path: "tools/ci-impact.test.mjs"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-148-impact-classification.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-148-impact-classification.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-148-impact-classification.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/ci-impact.mjs"
    - "tools/ci-impact.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #148 T1 impact classifier + fail-closed evidence gate 実装

## Objective

`tools/ci-impact.mjs`(分類 + gate)と `tools/ci-impact.test.mjs` を新設し、
`ci.yml` に `classify` job と fail-closed `evidence` gate を配線する。

## Steps

- [x] `tools/ci-impact.mjs`: LANE_LEGS/PATH_RULES/CRATE_DEPS/
      RT_OWNERS/GW_OWNERS/DECLARED_NOT_RUN + classify/gate/lanes。
- [x] `tools/ci-impact.test.mjs`: node:test 28 ケース(全 pass)。
- [x] `mise.toml`: `ci-impact`/`ci-evidence-gate`/`test-ci-impact`。
- [x] `ci.yml`: `classify` job 追加、`evidence` を jobs API 照合の
      fail-closed gate に改修(`actions: read` は evidence のみ)。
- [x] ローカル検証: node:test・actionlint・dry-run。
- [ ] prek run --all-files。
- [ ] docs チェーン + README index 更新。
- [ ] merge origin/main・コミット・push・PR・CI watch。

## Acceptance

- `node --test tools/ci-impact.test.mjs` pass。
- `mise run lint-actions` pass。
- PR で classify/evidence が起動し、leg 単位証跡・Shadow 照合・
  revision・Declared NotRun が出る。
- required 10 contexts の job 名不変。

## Notes

- selective execution・Ruleset 移行は本 task では行わない
  (spec §移行・別変更)。
