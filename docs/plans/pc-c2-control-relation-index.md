---
id: "PLAN-034UpbrhX5dkELUwZUoS0i"
type: "plan"
status: "approved"
title: "PC-C2 Control Relation Index 実装計画"
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
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/designs/pc-c2-control-relation-index-design.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-a-parameter-control.md"
    - "docs/plans/pc-b-parameter-dispatch.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/designs/pc-c2-control-relation-index-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c2-e2e-control-relation-index.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
  defers: []
  deferred-by: []
---

# PC-C2 Control Relation Index 実装計画 Implementation Plan

## Goal

`Project::control` から構築する revision-tagged 派生双方向 Relation
Index(`synvelis-project::relation`)と bounded query API を実装し、
REL-C2-01〜05 の headless 受入を test で担保する。

## Scope

単一実装ストリーム。`crates/synvelis-project` 内の新規 module +
additive export + tests のみ。`synvelis-workspace`/`synvelis-app`/
他 crate・`Cargo.toml`/`Cargo.lock`・snapshot schema は変更しない
(新規依存なし)。

## Upstream Documents

- `docs/specs/pc-c2-control-relation-index.md`(approved)
- `docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md`(accepted)
- `docs/designs/pc-c2-control-relation-index-design.md`(approved)

## Design Inputs

- `docs/designs/pc-c2-control-relation-index-design.md` — 辺モデル・
  状態優先順位・BFS/truncation・SCC cycle 判定の確定仕様。

## File Map

- `crates/synvelis-project/src/relation.rs` — `ControlEntityRef` /
  `RelationKind` / `RelationDirection` / `RelationState` /
  `RelationCoverage` / `RelationQuery` / `ControlRelation` /
  `RelationReply` / `RelationIndex`(build + query + adjacency +
  enabled-only SCC)。
- `crates/synvelis-project/src/lib.rs` — `pub mod relation;` と
  型の additive re-export。
- `crates/synvelis-project/tests/pc_c2_relation.rs` — REL-C2-01〜05
  対応の acceptance test(双方向・複数 assignment/depth・Disabled・
  Stale/Unresolved・truncation/cycle/budget/read-only 不変)。
- `docs/impl/ir/pc-c2-*.md` — implementation record(完了時)。

## Implementation Sequence

- [ ] T1: `relation.rs` — 型定義 + `RelationIndex::build`
  (entities/edges/adjacency + dangling→Unresolved + enabled-only
  SCC→cyclic 辺 Unresolved)。
- [ ] T2: `RelationIndex::query` — revision/anchor gate、方向別
  bounded BFS、Truncated/Complete 判定、additive export。
- [ ] T3: `tests/pc_c2_relation.rs` — REL-C2-01〜05 全件 +
  fixture 規模の budget 実測記録。
- [ ] T4: 最終検証 + impl record
  (`git fetch origin` → `git merge origin/main` → `mise run
  fmt-check/check/lint/test` → impl record → commit)。

## Dependencies and Risks

- 依存: PC-A control model(#40)・PC-B dispatch(#43)は main に
  存在。新規 crate 依存なし(BTreeMap/VecDeque のみ)。
- リスク: SCC 実装の stack 安全性 → 反復 Kosaraju/Tarjan で
  明示 stack、深い再帰を避ける。巨大 graph の再帰 depth 問題を
  排除済み。
- リスク: `Both` direction での edge 二重 emit → visited_edges で
  排除(設計済み)。

## Task Breakdown

- T1+T2 は同一 module の不可分な単位、T3 は T1/T2 完了後、
  T4 は全完了後。task doc は `pc-c2-task1-relation-index` の
  1 件に集約(設計上の分割利益が薄いため)。

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` / `test` 全 green。
- [ ] REL-C2-01〜05 が `tests/pc_c2_relation.rs` で個別に検証される。
- [ ] fixture(数百 edge)で build/query 時間を実測し test に記録。
  巨大 graph 実測は NotRun として impl record / PR に明記。

## Open Questions / Gaps

- なし(設計で全て確定済み)。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution(単一 worker dispatch)。
- Independent work streams: none。
- User approval required before dispatch: Orca worker 契約により
  review-protocol の blocker 無しレビューで代替。
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

## Review Handoff

- reviewer は `docs/designs/pc-c2-control-relation-index-design.md`
  の辺モデル表・状態優先順位・BFS 規則と `relation.rs` の実装が
  一致するかを重点確認。特に disabled の traversal 除外、
  enabled-only SCC、Stale/Unavailable の空返却を確認。
