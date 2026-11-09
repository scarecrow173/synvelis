---
id: "TASK-034Upd3TZPjW8QcMPatmyb"
type: "task"
status: "in-progress"
title: "PC-C2: Control Relation Index/Projection 実装タスク"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/99"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-c2-control-relation-index.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c2-control-relation-index.md"
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/designs/pc-c2-control-relation-index-design.md"
    - "docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c2-control-relation-index.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c2-e2e-control-relation-index.md"
  verified-by:
    - "docs/test-specs/pc-c2-e2e-control-relation-index.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C2: Control Relation Index/Projection 実装タスク

## Work

## T1 — `relation.rs` 型 + `RelationIndex::build`

- `ControlEntityRef`(Parameter/AutomationLane/ModulationSource/
  Assignment)、`RelationKind`、`RelationDirection`、`RelationState`、
  `RelationCoverage`、`RelationQuery`、`ControlRelation`、
  `RelationReply`。
- build: entities(BTreeSet)・edges(Vec、決定的順)・outgoing/
  incoming adjacency。辺規則は設計書の表どおり。dangling endpoint
  → Unresolved、`!enabled` → Disabled、enabled-only SCC 参加 →
  Unresolved(反復 Kosaraju/Tarjan、明示 stack)。

## T2 — `RelationIndex::query` + export

- revision 不一致 → Stale(辺なし)、anchor 非実在 → Unavailable、
  `max_depth == 0` → Complete 空。方向別 bounded BFS
  (Incoming=逆 adjacency、Outgoing=順、Both=両方・visited_edges
  で重複排除)、`max_edges` 超過 → Truncated。
- `lib.rs` に `pub mod relation;` + additive re-export。

## T3 — `tests/pc_c2_relation.rs`

- REL-C2-01: LFO/Macro/StepSeq→複数 target 双方向、lane 含む
  Incoming、3 direction の到達差。
- REL-C2-02: 同一 source/target 複数 assignment の別辺、depth
  driver 辺、Disabled の表示/除外/非寄与(evaluate_control 一致)。
- REL-C2-03: revision delta → Stale、削除反映、dangling →
  Unresolved、Unavailable/Complete 空。
- REL-C2-04: max_edges Truncated・max_depth・cycle → Unresolved +
  compile ControlCycle 対応、fixture(数百 edge)build/query 時間
  実測。
- REL-C2-05: project PartialEq 不変・eval/compile 不変・undo 不変・
  query 決定性。

## T4 — 最終検証 + impl record

- `git fetch origin` → `git merge origin/main` → `mise run
  fmt-check/check/lint/test` → `docs/impl/ir/pc-c2-*.md` → commit。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/pc-c2-control-relation-index.md`
- `docs/specs/pc-c2-control-relation-index.md`
- `docs/designs/pc-c2-control-relation-index-design.md`
- `docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md`
- `docs/test-specs/pc-c2-e2e-control-relation-index.md`

## Dependencies

- PC-A control model・PC-B dispatch は main に存在。新規依存なし。

## Affected Files

- `crates/synvelis-project/src/relation.rs`(新規)
- `crates/synvelis-project/src/lib.rs`(additive)
- `crates/synvelis-project/tests/pc_c2_relation.rs`(新規)
- `docs/impl/ir/pc-c2-*.md`(新規)

## Done When

- [ ] REL-C2-01〜05 が `tests/pc_c2_relation.rs` で検証される。
- [ ] `mise run fmt-check`/`check`/`lint`/`test` 全 green。
- [ ] impl record が `docs/impl/ir/` にある。

## Verification

- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test`(全て実 exit code 記録)。
