---
id: "TASK-6mWzK4pRyT7uI0oP3aS9dF"
type: "task"
status: "done"
title: "SF-04 slice10 T1 — syn.probe.compare app op（ProbeComparisonRequest → AcousticObservation の wire 露出）"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-04-slice10-probe-compare-op-plan.md"
  implemented-by: []
  depends-on: []
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
    - "docs/test-specs/sf-04-slice10-probe-compare-op.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice10 T1 — syn.probe.compare app op

## Work Items

- [x] T1-1: `crates/synvelis-app/src/service.rs` に
  `syn.probe.compare` `OpInfo` + dispatch arm を追加。
- [x] T1-2: `crates/synvelis-app/src/ui_ops.rs` に
  `op_probe_compare(&self)` + `probe_observation_json` 系
  mapper helpers を追加（`&self` read-only・engine 未接続は
  `engine_attached:false` + `observation:null`・malformed
  request は `INVALID_PARAMS`）。
- [x] T1-3: `crates/synvelis-app/tests/sf04_probe_compare_op.rs`
  を新規追加 — `AppService` dispatch 経由で
  Aligned/Conditional/Incomparable・fail-closed・
  engine 未接続・read-only 不変条件を検証。
- [x] T1-4: docs chain（spec/design/plan/task/test-spec/
  impl-ir）作成 + managed index 再生成。
- [x] T1-5: canonical `mise` 検証全件実行・commit/push/PR・
  Issue #107 コメント。

## Done When

- `syn.probe.compare` が `AppService::dispatch` 経由で
  `ProbeComparisonRequest → AcousticObservation` を wire
  JSON として返す。
- `Aligned`/`Conditional`/`Incomparable` と consumer 側
  fail-closed reason が正しく投影され、`aligned` の
  捏造がない。
- op が drain・project revision・bindings を変えないことが
  テストで確認される。
- engine 未接続で `{engine_attached:false,
  observation:null}`、malformed request で
  `INVALID_PARAMS`。
- `mise run check`/`lint`/`test`（対象範囲）が green。
