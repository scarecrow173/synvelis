---
id: "IMPL-034VoSZokxDrVB1U3GAB2Z"
type: "impl"
status: "completed"
title: "SF-03 slice8 実装記録 — app/workspace same-Revision fixture"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/sf03_revision.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/sf03_revision.rs"
      - type: "file"
        path: "docs/specs/sf-03-slice8-revision-fixture.md"
      - type: "file"
        path: "docs/designs/sf-03-slice8-revision-fixture-design.md"
      - type: "file"
        path: "docs/plans/sf-03-slice8-revision-fixture-plan.md"
      - type: "file"
        path: "docs/tasks/sf-03-task1-revision-fixture.md"
      - type: "file"
        path: "docs/test-specs/sf-03-slice8-revision-fixture.md"
      - type: "file"
        path: "docs/impl/ir/sf-03-slice8-revision-fixture-slice.md"
    modified:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice8-revision-fixture.md"
    - "docs/designs/sf-03-slice8-revision-fixture-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-03-slice8-revision-fixture-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-slice8-revision-fixture-design.md"
    - "docs/specs/sf-03-slice8-revision-fixture.md"
  derived-by: []
  verifies:
    - "docs/test-specs/sf-03-slice8-revision-fixture.md"
  verified-by: []
  references:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-03 slice8 実装記録 — app/workspace same-Revision fixture

## Summary

Issue #106 SF-03 の design §8.2-1 が指定する consumer 横断
same-Revision fixture の app/workspace 側残余を、headless test
2 ファイル（計 7 fixture）で固定した。production コード変更は
ゼロ — `SessionRoutingEngine` の receipt/source_map/
probe_bindings/reconcile、`SynvelisApp::signal_flow_view`、
workspace `signal_flow_graph` + 合成 `SignalFlowRuntimeEvidence`
の全公開面をそのまま観測した。

## Context

render 側 same-Revision fixture（slice7 `sf03_render.rs`）は完了
済みで、app/workspace 側は別 dispatch として deferred されて
いた。本 slice は Issue #106 Open のまま、SF-ACT-07/08/09・
GUI gate・SF03-RCP-01/02 を対象外として、revision 整合の
headless 証拠のみを追加する。spec/design/plan/task/test-spec
の docs チェーンに従う。

## Implementation

### Changes

- `crates/synvelis-app/tests/sf03_revision.rs`（新規・4 件）:
  fixture WAV + track strip + clip + chain processor
  （`FreshGainHost` — `StaticProcessorHost` は connect で
  instance を consume するため複数 build を跨ぐ本 fixture では
  使えず、fresh instance を毎回 vend する最小 `ProcessorHost`
  impl を test 内に定義）+ `attach_routing_engine` +
  `PumpPolicy` 駆動。
  - `same_committed_revision_across_receipt_map_probe_and_view`:
    `receipt.build_key.project_revision`・`source_map
    .project_revision`/`generation`・probe binding
    `project_revision`/`active_plan_generation`・
    `signal_flow_view` status が全て `session.revision()` と
    一致。`SemanticNodeRef::Strip/Processor` が source map key
    かつ view node `Resolved { generation }` に写像される
    （processor node は Expanded scope で投影されるため
    `set_scope` を先行実行）。
  - `viewstate_and_semantic_noop_keep_revision_axes_honest`:
    `op_signal_flow_visual(zoom)` は revision/project/
    `graph_builds`/receipt 不変・`view_generation` 前進。
    `SetProjectName` は revision 前進・通知されるが engine は
    `Equivalent`（receipt の `Active` generation・build key は
    built-from revision のまま）、view は `stale = true`・全 node
    `Stale`・probe binding は要求時 revision 保持。
  - `controller_bypass_surfaces_as_reconcile_drift_then_converges`:
    `controller_mut().apply` bypass（`AddClip`）→ `reconcile`
    が `ProjectRevision` mismatch + fault・view `stale` →
    次の `session.apply`+`run` で live revision に収束。
  - `save_is_persistence_evidence_not_activation`: `Published`
    （RT 未観測）で `InMemoryStore` に save → `saved().revision`
    == live だが receipt/view は `Published` のまま・
    `observed_active_generation` 不変。`run` 後にのみ `Active`。
- `crates/synvelis-workspace/tests/sf03_revision.rs`（新規・3 件）:
  `EngineStub`/`EnginePort` 通知 + `evidence_at` で合成
  `SignalFlowRuntimeEvidence`（receipt `Active`・source map・
  probe binding、`semantic_nodes` は `Unavailable
  (PluginUnavailable)` のみ — `NodeId` は workspace 依存に無く
  `Resolved` は合成不可のため、`Resolved` 側は app fixture の
  実 engine map が担保）を `signal_flow_graph` に供給。
  - `session_engine_projection_and_evidence_share_one_revision`:
    `last_built_revision`/`session.revision`/view/evidence 内
    全 revision field が一致、semantic node 単位の resolution
    写像と probe point の node 投影を固定。
  - `revision_transitions_stale_prior_generation_evidence`:
    apply/undo/redo 各 committed revision で通知・旧 evidence
    は `stale`+全 node `Stale`+probe 旧 revision 保持、live
    evidence で復帰。
  - `save_is_persistence_evidence_on_the_same_revision_axis`:
    save は engine 通知・revision・projection を不変に保ち、
    `saved().revision` が evidence revision と一致。
- docs: spec/design/plan/task/test-spec/impl record 新設 +
  managed index（specs/designs/plans/tasks/test-specs README は
  scaffold script が再生成、impl/ir README は手動追記）。

### Decisions During Implementation

- **`FreshGainHost`**: `StaticProcessorHost::connect` は
  `instances.remove` で instance を消費する one-shot 契約
  （sf05 fixture は単一 build 前提）。本 fixture は bypass 後の
  rebuild を含む複数 semantic build を駆動するため、connect 毎に
  fresh `GainProcessor` を vend する最小 `ProcessorHost` impl を
  test 内に定義した。`ProcessorLease::noop` は `pub(crate)` の
  ため `ProcessorLease::new(|| {})` を使用。
- **明示 connection を置かない**: track strip は master bus へ
  暗黙 routing されるため、`AddConnection` を追加すると
  `duplicate connection` validation error になる（実測で検出・
  修正）。fixture は sf03_session_engine と同じ implicit
  routing に揃えた。
- **bypass は `AddClip`**: `AddTrack` 単体の bypass は strip を
  持たず rebuild が `PluginUnavailable` で失敗したため、
  sf03_session_engine と同じ既存 lane+source への `AddClip`
  bypass に変更（semantic drift で rebuild が成功する形）。
- **processor node は Expanded scope**: `SignalFlowNodeId::
  Processor` は Overview では投影されないため、test 01 で
  `set_scope=expanded` を先行実行。
- **workspace の `Resolved` 制約**: `MappingResolution<NodeId>`
  の `Resolved` は `ruviform-graph::NodeId` を名指しする必要が
  あり workspace の依存に無いため、合成 map は
  `Unavailable`/`Unobserved` のみ。node 単位の `Resolved`
  照合は app fixture（実 `RoutingEngine` 産の map）に集約した。

## Related Experiments

なし。

## Validation

- `cargo test -p synvelis-app --test sf03_revision`: 4/4 pass
  （補助証拠）。
- `cargo test -p synvelis-workspace --test sf03_revision`: 3/3
  pass（補助証拠）。
- `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`check-headless-deps`: PR 本文の検証節に実測値を
  記録。

## NotRun（正直な記録）

- SF-ACT-07 Partial Preview vs Export policy: 未規定・別 Issue。
- SF-ACT-08 Ruviform RT/render/AudioLens 受入証拠: upstream gate。
- SF-ACT-09 device config-only compile 判定・性能 baseline: 別
  slice。
- SF-ACT-05 実機・全 GUI gate・SF03-RCP-01/02（#293 未実装）・
  defensive-only fault の live firing: 本 slice 対象外。
- probe の RT-confirmed mode・`BoundActive` への実機昇格:
  headless 範囲外（requested/pending の正直な投影までを固定）。
- Issue #106 の Close: Coordinator が行う（本 slice は Open の
  まま）。

## Risks

- semantic no-op 後の `stale = true` は「generation-scoped map
  が live revision と異なる」現行 projection 仕様の固定であり、
  equivalence を新規証拠と見なす policy 変更ではない（変更が
  必要なら別 Issue）。
- `FreshGainHost` は test 内最小 impl であり、実 plugin host の
  capability/boundary 宣言経路は TIM-03/PC-C6 fixture が担保
  済み。

## Follow-ups

- GUI slice が `signal_flow_view`/`op_signal_flow_*` を消費する
  際の同一 revision 前提の再利用。
- SF-ACT-07/08/09・実機検証・SF03-RCP-01/02（別 slice/Issue）。
