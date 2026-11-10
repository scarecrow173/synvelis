---
id: "TASK-034VoSREP91HGvxpLROrrC"
type: "task"
status: "done"
title: "SF-03 slice8 T1 — app/workspace Revision fixture"
created: "2026-11-10"
updated: "2026-11-10"
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
    - "docs/plans/sf-03-slice8-revision-fixture-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-03-slice8-revision-fixture-plan.md"
    - "docs/designs/sf-03-slice8-revision-fixture-design.md"
    - "docs/specs/sf-03-slice8-revision-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-03-slice8-revision-fixture-plan.md"
    - "docs/designs/sf-03-slice8-revision-fixture-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice8-revision-fixture.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice8 T1 — app/workspace Revision fixture

## Work

app/workspace same-Revision fixture を headless test のみで追加
する（production 変更なし）。

- `crates/synvelis-app/tests/sf03_revision.rs`（新規・4 件）:
  - `same_committed_revision_across_receipt_map_probe_and_view`:
    receipt build key・source map・probe binding・
    `signal_flow_view` status が同一 committed revision を指し、
    semantic node が `Resolved { generation }` で照合される。
  - `viewstate_and_semantic_noop_keep_revision_axes_honest`:
    visual op は revision/engine/receipt 不変、semantic no-op は
    `Equivalent` だが projection は旧 map を `stale` と表示。
  - `controller_bypass_surfaces_as_reconcile_drift_then_converges`:
    bypass commit → `ProjectRevision` mismatch + fault →
    session apply で収束。
  - `save_is_persistence_evidence_not_activation`: `Published`
    状態の save は activation 証拠を生まない。
- `crates/synvelis-workspace/tests/sf03_revision.rs`（新規・3 件）:
  - `session_engine_projection_and_evidence_share_one_revision`:
    `EnginePort` 通知・`session.revision`・view・合成 evidence
    の同一 revision 軸 + semantic node 単位の resolution 写像。
  - `revision_transitions_stale_prior_generation_evidence`:
    apply/undo/redo で旧世代 evidence が `stale` 化、live evidence
    で復帰。
  - `save_is_persistence_evidence_on_the_same_revision_axis`:
    save は engine 通知・revision・projection を動かさない。
- docs チェーン（spec/design/plan/task/test-spec/impl）と
  managed index 更新。

## Classification

- 種別: test（fixture 追加のみ、production 変更なし）
- リスク: 低（既存公開 API の観測面を固定するのみ）

## Upstream Documents

- `docs/plans/sf-03-slice8-revision-fixture-plan.md`
- `docs/designs/sf-03-slice8-revision-fixture-design.md`
- `docs/specs/sf-03-slice8-revision-fixture.md`
- `docs/test-specs/sf-03-slice8-revision-fixture.md`

## Dependencies

- slice5–7 の公開面（`SessionRoutingEngine`・
  `signal_flow_view`・`signal_flow_graph`・`EnginePort`）が main
  に存在すること。

## Affected Files

- `crates/synvelis-app/tests/sf03_revision.rs`（新規）
- `crates/synvelis-workspace/tests/sf03_revision.rs`（新規）
- `docs/specs/sf-03-slice8-revision-fixture.md`（新規）
- `docs/designs/sf-03-slice8-revision-fixture-design.md`（新規）
- `docs/plans/sf-03-slice8-revision-fixture-plan.md`（新規）
- `docs/tasks/sf-03-task1-revision-fixture.md`（新規）
- `docs/test-specs/sf-03-slice8-revision-fixture.md`（新規）
- `docs/impl/ir/sf-03-slice8-revision-fixture-slice.md`（新規）
- `docs/{specs,designs,plans,tasks,test-specs}/README.md`・
  `docs/impl/ir/README.md`（index 再生成・手動追記）

## Done When

- app 4 fixture・workspace 3 fixture が green。
- `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`check-headless-deps` が exit 0。
- production src・無関係ファイルに変更がない。

## Verification

- `cargo test -p synvelis-app --test sf03_revision`・
  `cargo test -p synvelis-workspace --test sf03_revision`（補助証拠）。
- 上記 mise タスク群（canonical）。
