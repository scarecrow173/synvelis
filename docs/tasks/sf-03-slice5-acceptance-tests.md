---
id: "TASK-034Uj82AYjkBHzNn8uXCCu"
type: "task"
status: "done"
title: "SF-03 slice5 T3 — headless session→routing e2e・test-spec"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
    - "docs/tasks/sf-03-slice5-session-routing-engine.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice5-session-engine-connect.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice5 T3 — headless session→routing e2e・test-spec

## Work

`crates/synvelis-app/tests/sf03_session_engine.rs` を新規作成し、
session 経路の受入契約を実測する。`docs/test-specs/
sf-03-slice5-session-engine-connect.md` の Rule/Example に対応する
テストを実装する。

- `session_apply_publishes_and_activates`: `ProjectSession` +
  `SessionRoutingEngine`（`StaticProcessorHost`）で `session.apply`
  → receipt `Published` → `run(1, Every(1))` → `Active` +
  `last_good_generation`。
- `failed_build_keeps_last_good`: 有効 plan 後に unresolved
  processor binding を apply → receipt `Failed`+
  `last_good_generation`、旧 generation が `run` で継続。
- `semantic_noop_is_equivalent`: rename apply で通知は届くが
  generation/receipt 不変。
- `stale_revision_rejected`: `session.apply(op, ずれ revision)` が
  `Err(StaleRevision)` で `graph_builds` 不変。controller bypass
  後の `reconcile` が revision mismatch を報告。
- `last_prepare_error` 経路（adapter 側 clip 解決失敗）は可能なら
  実測、困難なら test-spec の Non-goal/補足に記録。

## Classification

- 種別: test（受入 e2e）
- リスク: 低〜中（engine 既存契約の session 経路写像）

## Upstream Documents

- `docs/plans/sf-03-slice5-session-engine-connect.md`
- `docs/designs/sf-03-slice5-session-engine-connect-design.md`
- `docs/specs/sf-03-slice5-session-engine-connect.md`
- `docs/test-specs/sf-03-slice5-session-engine-connect.md`
- `docs/tasks/sf-03-slice5-session-routing-engine.md`

## Dependencies

- task 2（`SessionRoutingEngine` が必要）

## Affected Files

- `crates/synvelis-app/tests/sf03_session_engine.rs`（新規）
- `docs/impl/ir/sf-03-slice5-session-engine-connect.md`（新規・実装記録）

## Done When

- e2e が publish→activation receipt→last-good→equivalence→stale
  を session 経路で green に実測する。
- impl record が作成され、docs index が更新される。

## Verification

- `cargo test -p synvelis-app --test sf03_session_engine`（補助）。
- `mise run test` / `mise run test-rt` / `mise run lint` /
  `mise run fmt-check` / `mise run check-headless-deps`。
