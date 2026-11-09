---
id: "ADR-034UudJ57tbwiodGimQ1ab"
type: "adr"
status: "accepted"
title: "SF-05 slice1: Signal Flow 投影は workspace read model + engine evidence 合成、visual state は app 層 in-memory"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/adr/pc-c5-01-relation-readmodel-coverage.md"
    - "docs/adr/sv-gui-03-02-arrangement-readmodel-undo-ops.md"
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md"
  verifies: []
  verified-by: []
  references: []
  defers:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# ADR: SF-05 slice1 — Signal Flow read model の配置・visual state の所有・status 合成

## Context and Problem Statement

Issue #108 SF-05 は `RoutingModel`（SF-01 正本）の Signal Flow
投影・visual-only 操作の非破壊性・Revision/Generation を伴う
状態表示を要求する。投影をどこに置き、visual state をどこに保持し、
runtime/Probe 状態をどう合成するかを決める。

## Decision

1. **graph 投影は `synvelis-workspace` の新規 read model**
   （`signal_flow_ui` module）に置く。`relation_ui`/`arrangement_ui`
   と同じ「`ProjectSession` を読み表示 DTO を返すだけ」の形。
   `ControlSession::signal_flow_view`（modulation assignment 投影）
   とは別物であり、名前衝突を避けるため module/関数名を
   `signal_flow_graph` 系にする。`RoutingModel` を第二 domain
   model として複写しない。
2. **visual state は `UiOpsState` 内の in-memory 値**として
   `ViewInstanceId`（wire `view_id`）単位で保持する。Issue の
   `SignalFlowViewState`（positions/visual_groups/collapsed/
   visible scope/visual_revision）に対応する純粋値型
   `SignalFlowVisualState` を workspace module に定義し、
   mutation は session/project を参照しない自由関数とする。
   `WorkspaceSnapshot`/`ViewState`（schema v2,
   `deny_unknown_fields`）には **本 slice では追加しない** —
   永続化は SF-UX-07（schema migration）の範囲。
3. **runtime/probe 状態は `SignalFlowRuntimeEvidence` として
   app が採取して workspace 関数へ渡す**。`SessionRoutingEngine`
   の `receipt()`/`source_map()`/`rt_status()` と、player の
   read-only `probe_bindings()`/`probe_segments()`（app 側
   `SessionRoutingEngine` に passthrough 追加 — player crate
   自体は不変）を snapshot し、`RoutingActivationState`・
   `MappingResolution`・`ProbeResolveState` を verbatim に近い形で
   `SignalFlowStatus`/node status へ写す。`Publish`（非 RT
   publish 成功）と `Active`（RT 観測世代）は receipt の別
   generation field のまま運び、UI 側で潰さない。engine 未接続は
   `engine_attached: false` + runtime 状態 `Unobserved` として
   正直に表示する。
4. **edge は `RoutingConnection` 1 件 = 1 edge**。`kind = Send(id)`
   の connection は `sends` から tap を引き `role = Send` の
   1 edge のみ出す（connection と send config の二重 edge なし）。
   chain 内 processor 順は persisted connection ではなく
   `chain.processors` の順序から `role = Chain` の合成 edge を
   Expanded scope でのみ出す — connection id とは別 id 名前空間
   なので dedup 規則と衝突しない。端点が processor port の
   connection は、processor node が非表示の scope では所有 strip
   node へ畳み込み、edge role（Main/Sidechain/Send）は保持する
   — 異 role の edge を 1 本に潰さない。

## Consequences

### Positive

- domain 正本（`project.routing`）を複写せず、SF-03/04 の
  semantics（receipt の Published/Active 区別、source map の
  MappingResolution、probe の requested/confirmed 区別）を UI 側で
  再解釈しない。
- headless fixture が workspace/app 両 crate で完結し、
  `revision()`/`graph_builds`/receipt の不変を直接検証できる。
- visual state が純粋値型なので、永続化・undo・DSP への混入経路が
  型として存在しない。

### Negative

- `Expanded` は全 strip の processor を展開するため大規模
  project では `max_nodes`/`max_edges` による `Truncated` が
  常態化し得る（bounded 表示は Issue の LOD/clip 方針どおり。
  本 slice は pagination/LOD を実装しない）。
- visual state は in-memory のため view 間・再起動を跨がない
  （SF-UX-07 で schema v3 と共に扱う）。
- `Local{root}` は 1-hop ego network のみ — 深い探索は後続 slice。

## Considered Options

### Option A: workspace read model + app 合成（採用）

`relation_ui` の先例通り session→DTO の投影を workspace に置き、
engine evidence は app が構造体で渡す。

-利点: domain 境界が既存パターンと一致。player 非依存で
workspace test が軽い。runtime 型（receipt/map/binding）は全て
project 型なので evidence struct も project 型のみで構成できる。
- 欠点: evidence の組み立てが app に 1 段必要（薄い写像で許容）。

### Option B: app 層で `RoutingModel` を直接走査

`SynvelisApp` が `project.routing` を直接読み DTO を組む。

-利点: workspace 変更ゼロ。
- 欠点: read model の置き場として workspace を外し、
`arrangement_ui`/`relation_ui` と不整合。headless 検証が app
crate の重い依存に引きずられる。採用しない。

### Option C: visual state を `ViewState`/`WorkspaceSnapshot` に追加

Issue の `SignalFlowViewState` をそのまま serde 化する。

-利点: view 間・再起動で layout が残る。
- 欠点: schema v2 `deny_unknown_fields` のため schema v3 migration
（SF-UX-07）が必須になり本 slice の範囲を超える。永続化なしの
in-memory で要件（SF-UX-02 の不変性検証）は満たせる。採用しない。

### Option D: `ControlsUi`/`ControlSession` projection に追加

既存 control projection 経路に signal flow を載せる。

-利点: ui_ops capture にそのまま乗る。
- 欠点: routing graph は control session の gesture/observation
状態ではなく project 派生の read model。責務が濁る。採用しない。

## References

- `docs/specs/sf-05-signal-flow-headless-hierarchy.md`
- `docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md`
- `docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md`
- `docs/specs/sf-04-slice1-semantic-probe-adapter.md`
- `docs/adr/pc-c5-01-relation-readmodel-coverage.md`

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/108>
- [spec](../specs/sf-05-signal-flow-headless-hierarchy.md) /
  [design](../designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md) /
  [plan](../plans/sf-05-slice1-signal-flow-headless.md) /
  [task](../tasks/sf-05-slice1-signal-flow-headless.md) /
  [test-spec](../test-specs/sf-05-slice1-signal-flow-headless.md)
