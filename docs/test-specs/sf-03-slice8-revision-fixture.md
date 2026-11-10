---
id: "TSPEC-034VoSrQGRatV4fFu1xhFo"
type: "test-spec"
status: "approved"
title: "SF-03 slice8 — app/workspace same-Revision fixture test-spec"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/sf-03-slice8-revision-fixture.md"
    - "docs/designs/sf-03-slice8-revision-fixture-design.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice8-revision-fixture.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-slice8-revision-fixture-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-03-slice8-revision-fixture.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice8 — app/workspace same-Revision fixture test-spec

## Feature: app same-Revision fixture

### Rule TS-8-01: one committed revision across all evidence surfaces

`crates/synvelis-app/tests/sf03_revision.rs::
same_committed_revision_across_receipt_map_probe_and_view`

- fixture WAV + track strip + clip + chain processor
  （`StaticProcessorHost`/`GainProcessor`）+ master 接続を
  `attach_routing_engine` 前に構築し、`session.apply` で初回
  build → `engine.run` で `Active`。
- `receipt.build_key.project_revision`・
  `source_map().project_revision`・`source_map.generation`・
  `map.build_key.project_revision`・`receipt.published_generation`・
  `probe_bindings()[0].project_revision`/
  `active_plan_generation`・`signal_flow_view().project_revision`/
  `status.project_revision`/`source_map_revision`/
  `source_map_generation`/`activation(Active{project_revision,
  generation})`/`probes[0].{project_revision,generation}` が
  全て `session.revision()` と一致。
- `map.semantic_nodes` が `SemanticNodeRef::Strip(track_strip)`・
  `::Processor(processor)` を含み、対応 view node の
  `runtime == Resolved { generation == map.generation }`。

### Rule TS-8-02: viewstate/no-op は revision 軸を動かさず、
   equivalence を stale に正直化する

`viewstate_and_semantic_noop_keep_revision_axes_honest`

- `op_signal_flow_visual(zoom)`: `session.revision`・project
  serialization・`engine.graph_builds`・receipt が不変、
  `view_generation` のみ前進。
- `SetProjectName`（semantic no-op）: revision 前進・
  `graph_builds` +1（通知）だが receipt は同一 `Active`
  generation・`build_key.project_revision` は built-from
  revision のまま（`Equivalent`）。
- view は `status.project_revision` = live・
  `source_map_revision` = built-from・`stale = true`・全 node
  `Stale`・probe binding は要求時 revision を保持。

### Rule TS-8-03: controller bypass → reconcile drift → 収束

`controller_bypass_surfaces_as_reconcile_drift_then_converges`

- `controller_mut().apply` による bypass commit は
  `EnginePort` に通知されず `graph_builds` 不変。
- `engine.reconcile(project, live)` が
  `RoutingReconcileMismatch::ProjectRevision` を含み
  `fault.is_some()`、`plan_key.project_revision` は built-from
  revision。
- view は `stale = true`。次の `session.apply` + `run` で
  receipt/source_map が live revision に収束し `stale = false`。

### Rule TS-8-04: save は activation 証拠ではない

`save_is_persistence_evidence_not_activation`

- `Published`（RT 未観測）状態で `InMemoryStore` に `save`:
  `saved().revision == live revision` だが receipt は
  `Published` のまま・`observed_active_generation != published`・
  `graph_builds` 不変・view status も `Published`。
- `engine.run` 後にのみ `Active` へ遷移し、saved revision と
  build key revision が一致し続ける。

## Feature: workspace same-Revision fixture

### Rule TS-8-05: EnginePort 通知・projection・evidence が同一
   revision 軸

`crates/synvelis-workspace/tests/sf03_revision.rs::
session_engine_projection_and_evidence_share_one_revision`

- track strip → group bus → master の fixture で、
  `session.engine().last_built_revision() == session.revision()`・
  view の `project_revision`/`status.*`・合成 evidence の
  receipt build key・`Active.project_revision`・source map・
  probe binding の `project_revision`/`generation` が一致。
- `semantic_nodes` に記録した `SemanticNodeRef` が対応 node の
  `Unavailable(PluginUnavailable)` に写像され、未記録 node は
  `Unobserved`、probe point の entity が strip node の `probe`
  state に投影される。

### Rule TS-8-06: revision 遷移で旧世代証拠は stale 化・
   undo/redo も通知される

`revision_transitions_stale_prior_generation_evidence`

- apply/undo/redo の各 committed revision で
  `last_built_revision == session.revision()`。
- 旧 revision の evidence は `status.stale == true`・全 node
  `SignalFlowNodeRuntime::Stale`・probe binding は旧 revision
  保持。live revision の evidence で `stale == false` に復帰。

### Rule TS-8-07: save は同一 revision 軸の永続化証拠

`save_is_persistence_evidence_on_the_same_revision_axis`

- `save` は `session.revision`・`graph_builds`・
  `last_built_revision`・projection を不変に保ち、
  `saved().revision == session.revision() ==
  status.source_map_revision`。

## Feature: 非回帰

### Rule TS-8-08: 既存 fixture 非破壊

- `sf03_session_engine.rs`・`sf05_signal_flow.rs`・
  `sf04_probe_wiring.rs`・workspace 既存 `sf_05*.rs` を含む
  `mise run test` 全件 PASS（production 変更なしのため
  振る舞い不変であることの確認）。
