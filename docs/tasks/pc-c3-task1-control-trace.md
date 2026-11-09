---
id: "TASK-034UquDG2pRhDCtpLYPhPi"
type: "task"
status: "done"
title: "PC-C3: ControlTrace projection 実装タスク"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/100"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-c3-control-trace.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c3-control-trace.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/designs/pc-c3-control-trace-design.md"
    - "docs/adr/pc-c3-control-trace-stage-evidence-freshness.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c3-control-trace.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c3-e2e-control-trace.md"
  verified-by:
    - "docs/test-specs/pc-c3-e2e-control-trace.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C3: ControlTrace projection 実装タスク

## Work

- T1 — `crates/synvelis-project/src/trace.rs`: `ControlTraceKey`/`TraceSlot`/`ControlTrace`/stage evidence 型/`FreshnessContext`/`ObservationFreshness`/`StaleReason`/`UnavailableReason`/`ProbeObservation`/`MeasuredValue`/`PlanActivationState`/`ActivePlan`/`FailedPlanAttempt` + `freshness`/`plugin_reported_freshness`/`observed_freshness`/`is_coherent`。
- T2 — `lib.rs` additive export + `tests/trace_c3.rs` fixture test（TRACE-C3-01..04）。
- T3 — 最終検証（`git fetch origin main && git merge origin/main` → `mise run fmt-check`/`check`/`lint`/`test`）+ impl record + docs index 再生成。

## Classification

- feature / synvelis-project 層 projection slice（Issue #100 の TRACE-C3-01..04）

## Upstream Documents

- plan: `docs/plans/pc-c3-control-trace.md`
- spec: `docs/specs/pc-c3-control-trace-projection.md`
- design: `docs/designs/pc-c3-control-trace-design.md`
- adr: `docs/adr/pc-c3-control-trace-stage-evidence-freshness.md`
- test-spec: `docs/test-specs/pc-c3-e2e-control-trace.md`

## Dependencies

- 前提: `ab235a5`（PC-C2 merge）以降の `origin/main`。
- 参照のみ: `synvelis-player/src/dispatch.rs`（意味論合わせ）、`synvelis-workspace/src/control.rs`（変更しない）。

## Affected Files

- `crates/synvelis-project/src/trace.rs`（新規）
- `crates/synvelis-project/src/lib.rs`（additive export のみ）
- `crates/synvelis-project/tests/trace_c3.rs`（新規）
- `docs/` チェーン + `docs/impl/ir/`

## Done When

- [ ] TRACE-C3-01..04 が `tests/trace_c3.rs` で検証される
- [ ] `mise run fmt-check`/`check`/`lint`/`test` が全て exit 0
- [ ] impl record + index 再生成済み

## Verification

- canonical: `mise run fmt-check` / `mise run check` / `mise run lint` / `mise run test`
- 補助証拠: 直接 cargo 実行結果（canonical = mise）
