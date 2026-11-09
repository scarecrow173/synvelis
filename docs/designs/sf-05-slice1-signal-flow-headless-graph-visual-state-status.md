---
id: "DESIGN-034UudJz8OJsiidXryTNTU"
type: "design"
status: "approved"
title: "SF-05 slice1 Signal Flow headless 設計 — graph 投影 + visual state + status 合成"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-slice1-signal-flow-headless.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice1 設計: Signal Flow headless 投影

## Context

`ProjectSession::project()`/`revision()` で `Project::routing`
（`RoutingModel`）が読める。`RoutingActivationReceipt`/
`RoutingSourceMap`/`MappingResolution`/`SemanticProbeBinding`/
`ProbeResolveState` は全て `synvelis-project` 型で、workspace が
そのまま消費できる。`SessionRoutingEngine`（app）は
`receipt()`/`source_map()`/`rt_status()` を公開済み、player の
`probe_bindings()`/`probe_segments()` は read-only で app 側に
passthrough を足せる。

## Overview

```
ProjectSession ──project()/revision()──▶ RoutingModel
        │                                    │ signal_flow_graph()
        │                                    ▼
SessionRoutingEngine ──receipt()/source_map()/probe_bindings()──▶
        │ SignalFlowRuntimeEvidence           SignalFlowView
        │ (app が snapshot)                   {revision, coverage,
        ▼                                      nodes, edges, status}
   synvelis-app screens/app.rs ──▶ SignalFlowScreenState ──▶
        main view "Signal Flow" tab (read-only Label+Tables)

syn.signal_flow.visual ──▶ SignalFlowVisualState (UiOpsState,
        in-memory, ViewInstanceId 単位) ──▶ 次回 view query の
        options へ畳み込む。session/project には一切触れない。
```

## Detailed Design

### `crates/synvelis-workspace/src/signal_flow_ui.rs`（新規）

```rust
// --- 型付き entity ref（SelectionDomain とは独立） ---
pub enum SignalFlowEntityRef {
    Track(TrackId), Bus(BusId), Strip(StripId),
    Processor(ProcessorId), Port(SemanticPortId),
    Connection(RoutingConnectionId), Send(SendId),
}   // Copy/Clone/Ord — BTreeSet/BTreeMap key に使う

// --- node identity ---
pub enum SignalFlowNodeId { Strip(StripId), Processor(ProcessorId) }

pub enum SignalFlowNodeKind {
    TrackStrip { track: TrackId, name: String },
    BusStrip { bus: BusId, kind: BusKind, name: String },
    Processor { strip: StripId, index: usize },
}

pub enum SignalFlowNodeState { Ready, Unresolved }
pub enum SignalFlowNodeRuntime {           // evidence 由来
    Unobserved,                            // engine/map なし
    Resolved { generation: u64 },
    Unavailable(AvailabilityReason),
    Unsupported(UnsupportedReason),
    NotLowered(NotLoweredReason),
    Stale,                                 // map revision < live
}

pub struct SignalFlowNode {
    pub id: SignalFlowNodeId,
    pub kind: SignalFlowNodeKind,
    pub label: String,
    pub state: SignalFlowNodeState,        // semantic（binding 解決）
    pub runtime: SignalFlowNodeRuntime,    // runtime/probe 合成
    pub muted: bool, pub soloed: bool,     // strip のみ有意
    pub processor_count: usize,
}

// --- edge（RoutingConnection 1:1 + Expanded の Chain 合成） ---
pub enum SignalFlowEdgeId {
    Connection(RoutingConnectionId),
    Chain { strip: StripId, index: usize },
}
pub enum SignalFlowEdgeRole { Main, Sidechain, Send, Chain }
pub struct SignalFlowEndpoint {
    pub node: SignalFlowNodeId,
    pub port: Option<SemanticPortId>,      // inspect 用
    pub port_role: PortRole,
}
pub struct SignalFlowEdge {
    pub id: SignalFlowEdgeId,
    pub role: SignalFlowEdgeRole,
    pub source: SignalFlowEndpoint,
    pub target: SignalFlowEndpoint,
    pub enabled: bool,
    pub tap: Option<SendTap>,
}

// --- query / result ---
pub enum SignalFlowScope {
    Overview,
    Local { root: SignalFlowEntityRef },
    Expanded,
    History,      // snapshot infra 未搭載 → coverage Unsupported
}
pub enum SignalFlowLayer { Audio }         // #[non_exhaustive]
pub enum SignalFlowCoverage { Complete, Truncated, Stale, Unavailable, Unsupported }

pub struct SignalFlowViewOptions {
    pub scope: SignalFlowScope,
    pub layers: BTreeSet<SignalFlowLayer>,
    pub roles: BTreeSet<SignalFlowEdgeRole>,   // Role Filter
    pub collapsed: BTreeSet<SignalFlowEntityRef>, // Collapse
    pub max_nodes: usize, pub max_edges: usize,
    pub expected_revision: Option<ProjectRevision>,
    pub view_generation: u64,                  // echo 用
}
impl Default — Overview/Audio/全 role/空 collapsed/64 nodes/128 edges

pub struct SignalFlowRuntimeEvidence {
    pub receipt: Option<RoutingActivationReceipt>,
    pub source_map: Option<RoutingSourceMap>,
    pub probes: Vec<SemanticProbeBinding>,
}

pub struct SignalFlowStatus {
    pub project_revision: ProjectRevision,
    pub engine_attached: bool,
    pub activation: Option<RoutingActivationState>,   // verbatim
    pub planned_generation: Option<u64>,
    pub published_generation: Option<u64>,
    pub observed_active_generation: Option<u64>,
    pub last_good_generation: Option<u64>,
    pub fault: Option<RoutingFault>,
    pub source_map_revision: Option<ProjectRevision>,
    pub source_map_generation: Option<u64>,
    pub stale: bool,   // map 存在し revision が live より古い
    pub probes: Vec<SignalFlowProbeStatus>,
}
pub struct SignalFlowProbeStatus {
    pub point: SemanticProbePoint,
    pub state: ProbeResolveState,
    pub requested_mode: ProbeMode,
    pub confirmed_mode: Option<ProbeMode>,   // None = RT 未確認
    pub generation: u64,
    pub project_revision: ProjectRevision,
}

pub struct SignalFlowView {
    pub project_id: ProjectId,
    pub project_revision: ProjectRevision,
    pub view_generation: u64,
    pub coverage: SignalFlowCoverage,
    pub scope: SignalFlowScope,
    pub nodes: Vec<SignalFlowNode>,
    pub edges: Vec<SignalFlowEdge>,
    pub status: SignalFlowStatus,
}

pub fn signal_flow_graph(session, options, evidence)
    -> SignalFlowView            // = signal_flow_graph_with
```

投影規則:

- node = 全 strip（`routing.strips`、BTreeMap 順）。
  `StripOwner::Track(t)` → `TrackStrip`（`project.tracks[t].name`）、
  `StripOwner::Bus(b)` → `BusStrip`（bus kind/name、Master 含む）。
  `processor_count` = chain 長。
- `Strip`/`Bus`/`Track` の `SignalFlowNodeState::Unresolved`:
  所有 port の `PortBinding::Unresolved` または（processor node
  の場合）`ProcessorBinding::Unresolved` を含む場合。
- `muted`/`soloed` は `routing.strip_muted`/`strip_soloed`
  （`&project.control` 経由、read-only）。
- edge = `routing.connections` を BTreeMap 順に 1:1。`kind`:
  `Main`→role Main、`Sidechain`→role Sidechain、
  `Send(id)`→role Send + `sends[id].tap`（send config が無い
  異常系は `tap: None` で edge は出す — connection が正本）。
- port→node 解決: `PortOwner::Strip(s)` → Strip node、
  `PortOwner::Processor(p)` → Processor node（Expanded でのみ
  node 実在）。processor node 非表示時は `strip_of_processor(p)`
  の strip node へ畳み、`port_role` は保持。
- scope:
  - `Overview`: strip node のみ。processor node/Chain edge なし。
  - `Expanded`: 全 strip の processor を node 化
    （`chain.processors` 順、`index` 付き）し、隣接 processor 間に
    `role=Chain` の合成 edge（`strip→proc[0]`、`proc[i]→proc[i+1]`、
    `proc[last]→strip` は出さず、processor 間のみ — 境界配線は
    lowering の暗黙規則で persisted connection ではないため、
    意味を偽装しない）。processor port 宛の connection は
    processor node へ直接繋ぐ。
  - `Local{root}`: root を strip node に解決
    （Track→`strip_of_track`、Bus→`bus.strip`、Processor→所有
    strip＋その processor のみ展開）。focus strip と edge で
    直接繋がる node のみ残す induced subgraph。`History` は
    `coverage=Unsupported`。
  - `collapsed`（options）: 指定 entity（Strip/Bus/Track）の
    processor 展開を畳む — Expanded でもその strip は Overview
    相当に戻る。node/edge 自体は消さない。
  - `roles`（Role Filter）: edge role で filter。`Chain` を外すと
    processor 順表示だけ消え node は残る。
  - `layers`: `Audio` 以外は未定義 — 現状 `Audio` のみ有効。
    `layers` が空なら `coverage=Unsupported`。
- `expected_revision` ≠ live → `coverage=Stale`、nodes/edges 空、
  status は live revision で構成（relation_ui と同じ規則）。
- node/edge が `max_*` 超過 → 決定的 prefix で `Truncated`。
- `status`: `evidence=None` → `engine_attached=false`、
  `activation=None`、`stale=false`、probes 空、各 node
  `runtime=Unobserved`。`evidence` あり → receipt の state/
  generations を verbatim、`source_map.project_revision` と
  `session.revision()` の比較で `stale`、node runtime は
  `source_map.semantic_nodes` の `SemanticNodeRef::Strip`/
  `Bus`/`Processor` を node id へ写像し、`MappingResolution`
  variant を `SignalFlowNodeRuntime` へ写す
  （`Resolved` は map の `generation` を伴う）。
  probe は `point.entity` を node へ写像し `ProbeResolveState`
  を node の probe 状態として保持（`SignalFlowNode` に
  `probe: Option<ProbeResolveState>` 追加）。

### `SignalFlowVisualState`（同 module、純粋値型）

```rust
pub struct SignalFlowVisualGroup {
    pub id: u64, pub label: String,
    pub members: BTreeSet<SignalFlowEntityRef>,
    pub collapsed: bool,
}
pub struct SignalFlowVisualState {
    pub positions: BTreeMap<SignalFlowEntityRef, [f64; 2]>,
    pub groups: Vec<SignalFlowVisualGroup>,
    pub collapsed: BTreeSet<SignalFlowEntityRef>,
    pub zoom: f64,                 // default 1.0, >0
    pub pan: [f64; 2],
    pub link_focus: Option<SignalFlowEntityRef>,
    pub scope: SignalFlowScope,
    pub visual_revision: u64,      // 全 mutation で +1
}
impl Default
// 自由関数（&mut state のみ触る。session 不要）:
pub fn visual_move_node(state, entity, [x,y])
pub fn visual_set_group(state, group_id, label, members, collapsed)
pub fn visual_fold(state, entity, folded: bool)   // collapsed set 操作
pub fn visual_set_zoom(state, zoom)               // zoom<=0 は無視
pub fn visual_set_pan(state, [x,y])
pub fn visual_set_link_focus(state, Option<entity>)
pub fn visual_set_scope(state, scope)
// visual_state → SignalFlowViewOptions への畳み込み:
impl SignalFlowVisualState {
    pub fn view_options(&self, max_nodes, max_edges) -> SignalFlowViewOptions
}
```

### `crates/synvelis-app`

- `engine.rs`: `SessionRoutingEngine::probe_bindings()` /
  `probe_segments()` passthrough（`inner.borrow().engine.*`）。
- `app.rs`:
  - `pub fn signal_flow_view(&self, view: &str) -> SignalFlowView`
    — `self.ui.signal_flow` の visual state（view id 単位、
    未登録は default）から options を作り、
    `signal_flow_runtime_evidence()` で engine snapshot を採り、
    `synvelis_workspace::signal_flow_graph(session, options,
    evidence.as_ref())` を返す。
  - `fn signal_flow_runtime_evidence(&self)` — engine handle が
    あれば `SignalFlowRuntimeEvidence` を構成、なければ None。
    engine handle の保持方法は `attach_routing_engine` の既存
    フィールドに従う（`Option<SessionRoutingEngine>` clone）。
  - `publish_project_length` に
    `screen.signal_flow = SignalFlowScreenState::from_view(...)`
    を additive 追加（main view = `main_view_id()` の visual
    state を使う）。
- `screens.rs`: `SignalFlowScreenState`（status text・revision・
  generations・coverage・nodes/edges rows）+ `signal_flow_status_label`/
  `signal_flow_nodes_table`/`signal_flow_edges_table` + main view に
  "Signal Flow" tab を additive 追加。status/coverage/generation は
  `{:?}`/数値で verbatim 表示。全 Table `selection: None`。
- `ui_ops.rs` + `service.rs`:
  - `syn.signal_flow.view` params `{view?, scope?, root?, roles?,
    collapsed?, max_nodes?, max_edges?}` → `SignalFlowView` の
    JSON（nodes/edges/status を wire 化）。options 未指定分は
    その view の `SignalFlowVisualState` から補完。
  - `syn.signal_flow.visual` params `{view?, action, ...}` —
    action ∈ `move_node|set_group|fold|unfold|zoom|pan|
    link_focus|set_scope`。`self.ui.signal_flow`
    （`BTreeMap<String, SignalFlowVisualState>`）だけを変更し、
    ack `{ok:true, view, view_generation}` を返す。
    session/project/engine には触れない。
  - `UiOpsState` に `signal_flow:
    BTreeMap<String, SignalFlowVisualState>` を additive 追加。

## File Map

| file | 変更 |
| --- | --- |
| `crates/synvelis-workspace/src/signal_flow_ui.rs` | 新規 |
| `crates/synvelis-workspace/src/lib.rs` | module + re-export（additive） |
| `crates/synvelis-workspace/tests/sf_05.rs` | 新規 |
| `crates/synvelis-app/src/engine.rs` | probe passthrough 2 関数 |
| `crates/synvelis-app/src/app.rs` | `signal_flow_view`/evidence helper/publish 行 |
| `crates/synvelis-app/src/screens.rs` | `SignalFlowScreenState` + tab + tables |
| `crates/synvelis-app/src/ui_ops.rs` | `UiOpsState.signal_flow` + op handlers |
| `crates/synvelis-app/src/service.rs` | OPS 登録 + dispatch 2 件 |
| `crates/synvelis-app/tests/sf05_signal_flow.rs` | 新規 |

## Risk / Rollback

- additive のみ。`MainScreenState`/`UiOpsState` への field 追加は
  `Default` derive で既存 test を壊さない。
- player/project/recording crate・`ViewState` serde・wire 既存 op
  には触れない。rollback は commit revert で完結。
- `signal_flow_view` の engine evidence は `Option` — engine 未
  接続の app（既存 test fixture）でも `Unobserved` で動く。

## Verification

- `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test`（canonical）。
- `mise run check-app-native` / `lint-app-native` /
  `build-app-native` は存在すれば実行、不能なら NotRun 記録。
- 実 OS/GPUI 描画・SF-UX-08 は **NotRun**（#114 gate）。

## Sources and Relations

- [spec](../specs/sf-05-signal-flow-headless-hierarchy.md) /
  [ADR](../adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md) /
  [plan](../plans/sf-05-slice1-signal-flow-headless.md) /
  [task](../tasks/sf-05-slice1-signal-flow-headless.md) /
  [test-spec](../test-specs/sf-05-slice1-signal-flow-headless.md)
