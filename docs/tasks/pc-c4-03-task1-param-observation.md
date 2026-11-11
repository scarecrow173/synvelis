---
id: "TASK-PCc4030bs3vN1pQ8wXyZ2aE"
type: "task"
status: "done"
title: "PC-C4-03 task1 — reported observation 消費 実装"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-c4-03-param-observation-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-03-param-observation-plan.md"
    - "docs/specs/pc-c4-03-param-observation.md"
    - "docs/designs/pc-c4-03-param-observation-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C4-03 task1 — reported observation 消費 実装

## 作業内容

- `RoutingControlDispatch::drain_output_reports` — generation/active
  gate → endpoint 毎に注入 drain → `Drained` の `ParamValue` を session
  binding で `ParamId → target` 逆引きし `record_reported`、
  `Unavailable`/stale・failed handle を `mark_reporting_unavailable`、
  非終端 `HostError` を typed `errors` に記録。
- `RoutingControlDispatch::observe_cycle` — `ParamDispatch::observe_cycle`
  の generation-scoped 公開面（scheduler report channel の唯一の drain
  owner）。
- `ReportDrainReport` — drain 証拠 counter 群。
- `lib.rs` re-export 追加。
- `tests/pc_c4_03_observation.rs` — 8 test。

## 完了条件

- [x] drain → `record_reported` → `observe_cycle` → `Reported` 投影の
  headless test Pass（8/8）。
- [x] `Unavailable` → `mark_reporting_unavailable` 正直終端。
- [x] generation mismatch gate 回帰。
- [x] L1 実値 `NotRun` を test-spec/PR に明記。
