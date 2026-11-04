---
id: "TASK-034RINq1C9IMn7tPyqXY2h"
type: "task"
status: "done"
title: "Issue #147 T1 sccache verify/gw10 導入・実測評価"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/147"
  changes:
    added: []
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-147-sccache-compile-cache.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-147-sccache-compile-cache.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-147-sccache-compile-cache.md"
  references:
    - ".github/workflows/ci.yml"
  defers: []
  deferred-by: []
---

# Issue #147 T1 sccache verify/gw10 導入・実測評価

## Work

`verify`・`gw10-verify` job に ruviform #329 同型の sccache step 群
(set up → env export → env verify → statistics)を挿入し、PR の
cold/warm run で hit rate・job 時間を実測して採否を判定する。
`SCCACHE_GHA_VERSION=synvelis-v1-<RUNNER_OS>`、kill switch は
`vars.CI_SCCACHE_DISABLE != '1'`。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/issue-147-sccache-compile-cache.md`
- `docs/designs/issue-147-sccache-compile-cache.md`
- `docs/specs/issue-147-sccache-compile-cache.md`

## Dependencies

- 先行評価: ruviform #329 / PR #342(手順・pin・fail-open 実績)。
- 前提 repair: syn-obs-01 IR の broken `defers` 関係を外部 URL 化
  (doc-driven graph の artifact-graph blocker 解消)。

## Affected Files

- `.github/workflows/ci.yml`(verify / gw10-verify job のみ)

## Done When

- [x] `verify`・`gw10-verify` に sccache step 群が挿入され、他 job に
      diff がない。
- [x] cold/warm run の `sccache --show-stats` と job wall time が
      `docs/impl/exp/issue-147-sccache-compile-cache.md` に記録される。
- [x] 全 lane green、または悪化 lane の sccache step 除去 + 不採用記録。
- [x] IR `docs/impl/ir/issue-147-sccache-verify-gw10.md` が記録される。

## Verification

- [x] `mise run lint-actions` pass。
- [x] `mise exec -- prek run --all-files` pass。
- [x] `gh pr checks --watch` で 10 required contexts 全緑(必要に応じて
      scope 外 flake の rerun を実施)。
- [x] rt-verify/msrv-verify/ui-verify/dep-guard/evidence に sccache 非
      導入であること(diff 確認)。
