---
id: "DESIGN-034VoSreRK4VhuFxIqvayx"
type: "design"
status: "approved"
title: "SF-03 slice8 — app/workspace same-Revision fixture design"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/sf-03-slice8-revision-fixture.md"
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
  depends-on:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sf-03-slice7-render-consumption-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice8-revision-fixture.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  derived-by:
    - "docs/plans/sf-03-slice8-revision-fixture-plan.md"
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-03-slice7-render-consumption.md"
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
  defers: []
  deferred-by: []
---

# SF-03 slice8 — app/workspace same-Revision fixture design

## Overview

本 slice は test-only。app/workspace 両 consumer が観測する
revision・generation・semantic identity が、engine の
publish/activation 証拠（`RoutingActivationReceipt`・
`RoutingSourceMap`・`SemanticProbeBinding`）と同一 committed
`ProjectRevision` を指すことを、headless fixture で固定する。
production 変更はゼロ — 全ての観測面は既に公開済み。

## Detailed Design

### 1. app fixture（`crates/synvelis-app/tests/sf03_revision.rs`）

`sf05_signal_flow.rs`/`sf04_probe_wiring.rs`/`sf03_session_engine.rs`
の helper パターンを再利用した 4 fixture:

- **fixture 構築** `app_with_engine`: tempdir に stereo fixture WAV
  （`write_fixture` + `import_local`）、track+strip+lane+clip、
  chain processor 1 基（`ProcessorBinding::Resolved` +
  `StaticProcessorHost` の `GainProcessor`）、track strip → master
  strip の `Main` connection。setup は `controller_mut()` 経由で
  engine 観測前に完了させ、`attach_routing_engine` 後の
  `session.apply(SetProjectName)` が初回 build を駆動する。
- **fixture 01 same-Revision**: `engine.run(1, Every(1))` で
  `Active` 化後、`op_probe_control(request)` で probe binding を
  1 件発行し、以下が全て `session.revision()` と一致することを
  assert: `receipt.build_key.project_revision`、
  `source_map.project_revision`/`generation`（=
  `receipt.published_generation`）、`probe_bindings()[0]
  .project_revision`/`active_plan_generation`、
  `signal_flow_view().project_revision`/`status.*`。
  `map.semantic_nodes` が `SemanticNodeRef::Strip(track_strip)`・
  `::Processor(processor)` を key に持ち、対応 view node の
  `runtime == Resolved { generation }`（master strip も同様）。
- **fixture 02 viewstate/no-op**: `op_signal_flow_visual(zoom)` で
  revision・project bytes・`graph_builds`・receipt 不変・
  `view_generation` 前進を確認。続いて `SetProjectName`（semantic
  no-op）で revision 前進 → engine は `Equivalent`（receipt の
  `Active` generation・`build_key.project_revision` は built-from
  revision のまま）だが、view は `status.project_revision` = live、
  `source_map_revision` = built-from、`stale = true`、全 node
  `Stale`、probe binding は要求時 revision を保持 — equivalence
  を新規証拠に見せかけない。
- **fixture 03 bypass→drift→converge**: `controller_mut().apply`
  で session 通知を bypass した commit → `engine.reconcile` が
  `RoutingReconcileMismatch::ProjectRevision` + fault を報告、
  view は `stale = true`。次の `session.apply` で live revision に
  rebuild され `stale = false` に収束。
- **fixture 04 save≠activation**: publish 直後（`Published`、
  `observed_active_generation != published`）に `InMemoryStore`
  へ `save` → `controller().saved().revision == live revision`
  だが receipt は `Published` のまま・`graph_builds` 不変・
  view status も `Published`。`engine.run` 後にのみ `Active` へ
  遷移し、saved revision と build key revision が一致し続ける。

### 2. workspace fixture（`crates/synvelis-workspace/tests/sf03_revision.rs`）

workspace は `synvelis-player`/`synvelis-app` に依存しないため、
`SignalFlowRuntimeEvidence`（receipt/source_map/probes の
`synvelis-project` 型 snapshot）を合成して `signal_flow_graph` に
供給する 3 fixture:

- **fixture 01 same-Revision**: track strip → group bus → master
  の routing fixture で `session.revision()`・
  `EnginePort::last_built_revision()`・view の
  `project_revision`/`status.*`・evidence 内全 revision field
  （receipt build key、`RoutingActivationState::Active
  .project_revision`、source map、probe binding）が一致。
  `semantic_nodes` に `Unavailable(PluginUnavailable)` を記録した
  `SemanticNodeRef` が対応 node の
  `SignalFlowNodeRuntime::Unavailable` に写像され、未記録 node は
  `Unobserved` のまま — map の semantic key 照合が node 単位で
  効いていることを固定（`Resolved` は NodeId 型が workspace の
  依存に無いため合成不可。node 解決の `Resolved` 側は app fixture
  が実 engine map で担保）。
- **fixture 02 transition stale**: apply → `EnginePort` 通知・
  `last_built_revision` 前進・旧 evidence が `stale` + 全 node
  `Stale` + probe binding は旧 revision 保持。undo/redo も新規
  committed revision として通知され、live revision の evidence
  で `stale = false` に復帰。
- **fixture 03 save revision 軸**: `InMemoryStore` への `save` が
  engine 通知・revision・projection を一切動かさず、
  `saved().revision` が evidence の `source_map_revision` と
  一致 — save は同一 revision 軸上の永続化証拠であり別軸でも
  activation 証拠でもない。

### 3. 証拠の対応関係

| 観測面 | app fixture | workspace fixture |
| --- | --- | --- |
| receipt build key revision | 実 engine receipt | 合成 `RoutingActivationReceipt` |
| source map revision/generation | 実 `RoutingSourceMap` | 合成（`Unavailable` resolution のみ） |
| probe binding revision | 実 `op_probe_control` → binding | 合成 `SemanticProbeBinding` |
| view status | `SynvelisApp::signal_flow_view` | `signal_flow_graph` + evidence |
| EnginePort 通知 | `session.apply` 経由の実通知 | `EngineStub` の `graph_builds`/`last_built_revision` |
| save | `InMemoryStore` + `saved()` | 同左 |

## Risks

- `signal_flow_view` の node `Resolved` 判定は lowering が当該
  semantic entity を実際に lowering した場合のみ — fixture は
  strip/processor/master のみを対象とし、未 lowering entity を
  assert しない。
- semantic no-op 後の `stale = true` は「equivalence を stale と
  見なす」現行 projection 仕様の固定であり、policy 変更ではない
  （変更が必要なら別 Issue）。
- 合成 evidence の `RoutingBuildKey` は `routing_fingerprint`
  helper で実 project から計算し、捏造した fingerprint を
  使わない。

## Verification

- `cargo test -p synvelis-app --test sf03_revision` /
  `cargo test -p synvelis-workspace --test sf03_revision`（補助証拠）。
- `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`check-headless-deps`。
