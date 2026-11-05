---
id: "TASK-034AUT04uqfg9sevPu0zlr"
type: "task"
status: "todo"
title: "AUT-04 task1: Viewport LOD・RT bounded queue・型付き喪失 provenance 実装 + headless テスト"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/134"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_04.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_04_rt.rs"
      - type: "file"
        path: "docs/impl/ir/aut-04-automation-curve-lod-record-budget.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/plans/aut-04-automation-curve-lod-record-budget.md"
  implemented-by: []
  depends-on:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/adr/aut-04-viewport-lod-bounded-record-queue.md"
    - "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-04-e2e-automation-curve-lod-record-budget.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
  defers: []
  deferred-by: []
---

# AUT-04 task1: Viewport LOD・有界記録キュー・喪失 provenance

## 目的

design §2–§7 のとおり、`synvelis-project` に bounded Viewport LOD
投影を、`synvelis-workspace` `ControlSession` に RT bounded SPSC
ingress・drain・型付き喪失 provenance・session lane view を実装
する。test-spec F1–F9 を `tests/aut_04.rs`・`aut_04_rt.rs` で
pin する。

## チェックリスト

- [ ] `CurveQuery`/`ViewBin`/`CurveFeatureKind`/`CurveFeature`/
      `CurveQuality`/`AutomationCurveView` + `automation_view`
      (project, exact envelope, NaN gap, coverage, quality)
- [ ] `AutomationRecordEvent`/`RecordPushError`/`RecordBudgets`/
      `RecordGap`/`RecordTargetHealth`/`RecordDrainReport`/
      `RecordRejectKind`/`RecordNotification` + 新定数
- [ ] `record_event_rt`(alloc 0)・`drain_record_events`・
      `record_health`・`record_notifications`・
      `set_record_budgets`/`record_budgets`
- [ ] `RecordingTransaction.{record_gaps,deliveries,record_dropped}`
      + `record_points` の budget 参照切替と delivery 記録
- [ ] `ControlSession::automation_view` + `AutomationLaneView`
      組立(pending/deliveries/gaps/revision/health)
- [ ] `ruviform-rt` 依存追加・`stats_alloc` dev-dep・`Cargo.lock`
      再解決・`lib.rs` export
- [ ] `tests/aut_04.rs`: test-spec F1–F8
- [ ] `tests/aut_04_rt.rs`: test-spec F9(alloc 0)
- [ ] `mise run fmt`/`check`/`lint`/`test`/`test-rt`/`test-msrv`
      全緑 + 既存 aut_01/02/03・pc_c1 等無退行
- [ ] docs index 再生成 + `docs/impl/ir/` 実装記録

## 完了条件

- test-spec の Feature 群が `tests/aut_04.rs`/`aut_04_rt.rs` で
  pin され全緑。
- 既存 aut_01/02/03・pc_a/pc_b/pc_b_rt/pc_c1 テストが無退行。
- LOD query が `ProjectRevision`/Undo/graph build を変化させず、
  喪失は点列捏造なしに gap provenance で型付き通知される。
