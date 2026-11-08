---
id: "DESIGN-106SF03Ef3fGh4iJk5lMn6oPq"
type: "design"
status: "approved"
title: "SF-03 slice1 — Lowering/SourceMap/Activation 実装設計"
created: "2026-11-08"
updated: "2026-11-08"
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
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-02-routing-transaction-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
  derived-by:
    - "docs/plans/sf-03-slice1-lowering-activation.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice1-lowering-activation.md"
  references: []
---

# SF-03 slice1 設計 — Lowering・SourceMap・Activation

## 1. 境界と責務

| 層 | crate/module | 責務 |
| --- | --- | --- |
| 契約型 | `synvelis-project::routing_build` | BuildKey/CompileRequest/MappingResolution/SourceMap/Fault/ActivationState/Receipt、fingerprint |
| Lowering | `synvelis-player::routing_lower` | `RoutingModel`→`GraphSpec`+SourceMap+node role 表、`StripProcessor` |
| Playback 計画 | `synvelis-player::lowering`(既存) | clip/comp source window 解決を `plan_playbacks` として抽出・共有 |
| Activation | `synvelis-player::routing_engine` | `RoutingEngine`: prepare→publish→process→receipt、Last-good ledger |

`PreparedRouting` は `CompiledPlan` を所有するため player 側。
`EngineStub`(synvelis-workspace)は変更しない。

## 2. 契約型(`synvelis-project/src/routing_build.rs`)

```rust
pub struct RoutingBuildRequestId(pub u64);
pub struct RuntimeInstanceId(pub u64);
pub struct RuntimeClockEpochRef(pub u64);
pub enum BuildMode { Realtime, Offline }
pub struct BuildBudget { pub max_nodes: usize, pub max_connections: usize, pub max_frames: usize }

pub struct RoutingBuildKey {
    pub project_id: ProjectId,
    pub project_revision: ProjectRevision,
    pub routing_fingerprint: RoutingFingerprint,   // u64 FNV-1a over serde_json projection
    pub capability_generation: u64,                // port_capability_generation
    pub engine_config_generation: u64,
    pub source_format_generation: u64,
    pub lowering_policy_version: u32,              // =1
    pub build_mode: BuildMode,
    pub device_clock_epoch: RuntimeClockEpochRef,
    pub runtime_instance: RuntimeInstanceId,
}
impl RoutingBuildKey {
    /// 全 semantic/capability/engine 項目の同値(project_revision を除く)。
    pub fn semantic_eq(&self, other: &Self) -> bool;
}

pub struct RoutingCompileRequest {
    pub request_id: RoutingBuildRequestId,
    pub expected_project_revision: ProjectRevision,
    pub mode: BuildMode,
    pub resource_budget: BuildBudget,
    pub runtime_instance: RuntimeInstanceId,
    pub device_clock_epoch: RuntimeClockEpochRef,
}

pub enum MappingResolution<T> {
    Resolved(T),
    Unavailable(AvailabilityReason),   // PluginUnavailable | InstanceMissing | CapabilityMismatch
    Unsupported(UnsupportedReason),    // DomainUnsupported | FormatUnsupported | TopologyUnsupported
    NotLowered(NotLoweredReason),      // MutedStrip | DisabledConnection | OutOfScope
}

pub enum SemanticNodeRef { Strip(StripId), Bus(BusId), Processor(ProcessorId) }

pub struct RoutingSourceMap {
    pub project_id: ProjectId,
    pub project_revision: ProjectRevision,
    pub build_key: RoutingBuildKey,
    pub generation: u64,                        // compiled plan generation
    pub semantic_nodes: BTreeMap<SemanticNodeRef, MappingResolution<NodeId>>,
    pub ports: BTreeMap<SemanticPortId, MappingResolution<PortRef>>,
    pub connections: BTreeMap<RoutingConnectionId, MappingResolution<ruviform_graph::ConnectionId>>,
    /// send id → 解決済み tap PortRef(pre=strip dry out / post=strip main out)
    pub send_taps: BTreeMap<SendId, PortRef>,
    /// semantic playback provenance → runtime node/out port
    pub playback_bindings: Vec<RoutingPlaybackRef>,
}

pub struct RoutingPlaybackRef {
    pub content: PlaybackContentRef,  // Clip(ClipId)|CompSegment{..}|CompOverlap{..}
    pub node_id: NodeId,
    pub out_port: PortId,
    /// この playback が feed する semantic port(strip main input)。
    pub destination: SemanticPortId,
}

pub enum RoutingFault {
    StaleProjectRevision, StaleCapability, TemporaryIdMismatch,
    MissingSource, UnsupportedMapping, ChannelMismatch,
    PluginUnavailable, MissingProcessorInstance,
    CompileFailed, ValidationFailed, PublishChannelFull,
    GenerationMismatch, EngineStopped, StaleEvent,
}
// ※ 18 分類の完備は後続 slice。本 slice で発生し得る subset。

pub enum RoutingActivationState {
    Configured { project_revision: ProjectRevision },
    Validated { project_revision: ProjectRevision },
    Prepared { project_revision: ProjectRevision, planned_generation: u64 },
    Published { project_revision: ProjectRevision, generation: u64 },
    Active { project_revision: ProjectRevision, generation: u64, epoch: RuntimeClockEpochRef },
    Blocked { fault: RoutingFault, last_good_generation: Option<u64> },
    Failed { fault: RoutingFault, last_good_generation: Option<u64> },
    Retired,
}

pub struct RoutingActivationReceipt {
    pub request_id: Option<RoutingBuildRequestId>,
    pub build_key: Option<RoutingBuildKey>,
    pub state: RoutingActivationState,
    pub planned_generation: Option<u64>,
    pub published_generation: Option<u64>,
    pub observed_active_generation: Option<u64>,
    pub last_good_generation: Option<u64>,
    pub frames_observed: u64,
    pub fault: Option<RoutingFault>,
}

pub fn routing_fingerprint(project: &Project) -> RoutingFingerprint;
```

`routing_fingerprint` は graph-relevant 投影のみを hash する:
`routing` 全体、`control.targets` の `(id, owner, role, base)`、
`tracks`/`lanes`/`clips`(placement/playback/muted/source/lane)、
`comp` registry、`sources`/`assets` は state のみ(source bytes は
`source_format_generation` 側)。project `name`・view state は含まない。

## 3. Lowering(`synvelis-player/src/routing_lower.rs`)

```rust
pub struct SemanticLowered {
    pub spec: GraphSpec,
    pub source_map: RoutingSourceMap,
    pub playbacks: Vec<PlaybackBinding>,       // 既存型を再利用
    pub node_roles: BTreeMap<NodeId, SemanticNodeRole>,
    pub device_in: PortId,                     // master strip out → sink in
    pub skipped: Vec<SkippedPlayback>,
}
pub enum SemanticNodeRole {
    Playback(usize),                     // playbacks index
    StripFader { strip: StripId, gains: [f32; 2] },   // [left,right] 線形 gain(pan 則適用済み)
    SendGain { send: SendId, gain: f32 },
    ChainProcessor { processor: ProcessorId },
}
```

### 3.1 node 構成(決定的順序)

1. `routing.strips` を BTreeMap 順に走査。各 strip につき:
   - **fader node**(`NodeKind::Processor`): ports =
     `[main-in(Input,Main,unbounded(0)), post-out(Output,Main,unbounded(0)),
       pre-out(Output,Main,unbounded(0))]`。
     `pre-out` は入力の dry copy(pre-fader tap 用)。
     fan-out を許すため multiplicity は `unbounded(0)`(routing_tx の
     汎用投影と同一規約)。
   - **chain processor node**(chain 順): ports = semantic ports を
     `port_ids` 順に投影(main-in/out + sidechain inputs)。
2. `routing.connections` を BTreeMap 順に走査し enabled のみ edge 化:
   - `Main`: src strip main-out(post)→ dst strip main-in。
   - `Send(id)`: src = send.tap に応じ strip `pre-out`/`post-out` →
     send gain node → dst strip main-in。
   - `Sidechain`: src strip post-out → processor sidechain in。
   - audibility(`audible_strips`)外の strip を端点に持つ edge は
     emit しない(source map に `NotLowered(MutedStrip)` を記録)。
   - dst semantic main-in PortRef = dst strip の chain 先頭入力
     (chain 空なら fader node の main-in)。
3. `plan_playbacks` の各 playback を content→lane→track→strip で
   所有 strip の main-in PortRef へ接続。
4. master strip の post-out → device `Sink` node の入力
   (`device_in` = output tap)。master strip が無い project は
   `ValidationFailed`。

### 3.2 fader/pan/send gain

- strip fader: `ControlOwner::Strip`+`ParameterRole::Fader` target の
  `base` を線形 gain として採用(capability 無し internal target の
  slice-1 規約。1.0=unity)。target 不在は gain=1.0。
- pan: `ParameterRole::Pan` target の `base` p∈[0,1] を constant-power
  則(`L=cos(p·π/2)`, `R=sin(p·π/2)`、center=等方)で適用。不在は
  center。
- send level: `SendConfig.gain_target` → target `base` を線形 gain
  (1.0=unity、`SendGainSpec` の documented 規約)。target 不在は
  gain node を emit しない(direct edge)。
- mute/solo: `audible_strips` が唯一の authority。processor 内に
  mute 状態を持たない(edge prune が意味論)。

### 3.3 `StripProcessor`

`GraphProcessor` 実装: `in` を `pre_out`/`post_out` へ copy、
`post_out` に gain/pan を適用。fan-in sum は runtime の input
materialize が担う(legacy と同一機構)。

### 3.4 失敗規則

- clip/comp の source 未解決・rate 不一致・mapping 非対応は既存
  `SkippedPlayback` と同一規則で記録(明示 skip、silent success ではない)。
- `SemanticPort.channels` ≠ engine channels、または enabled connection
  endpoint の port 未解決は lowering error(決定的失敗)。
- `ProcessorBinding::Unresolved` または instance 未提供の processor
  node は emit するが factory が失敗 → `PluginUnavailable` で
  fail-closed(bypass も fake silence もしない)。
- `SendTap`/`channel_map` は semantic model の値をそのまま下ろす。

## 4. `RoutingEngine`(`synvelis-player/src/routing_engine.rs`)

```rust
pub struct RoutingEngine {
    engine: EngineConfig,
    project_rt: GraphProject,
    compiler: Compiler,
    runtime: GraphRuntime,
    rt: RtRuntime,
    ctx: RtContext,
    ledger: ActivationLedger,
    instance: RuntimeInstanceId,
    epoch: RuntimeClockEpochRef,
}
pub struct RoutingDeps<'a> {
    pub canonical_of: &'a dyn Fn(SourceId) -> Option<AudioFormat>,
    pub prepared_clips: Vec<PreparedClip>,   // playbacks と同順
    pub processors: BTreeMap<ProcessorId, Box<dyn GraphProcessor>>,
    pub clock: Arc<AtomicU64>,                // ClipSync 用
    pub seek_nonces/drain_nonces…             // playback 毎(既存 driver と同型)
}
```

`PlaybackBinding` 毎に `ClipPlayer`+`ClipSync` を構成するため、
deps は `Vec<PlaybackRuntime>`(consumer 組み立て済み)を受ける形に
する(driver の activate が行う prepare→ClipPlayer 構築を関数化して
共有するか、RoutingEngine が同じ手順を踏む)。

### 4.1 遷移

- `prepare(request, project, deps) -> Result<PreparedRouting, RoutingFault>`:
  1. `routing_fingerprint`+`port_capability_generation`+engine 世代で
     `RoutingBuildKey` を構成(request の revision/mode/epoch/instance)。
  2. `lower_semantic` → spec+source_map(失敗は fault を返す)。
  3. `GraphProject::prepare` → `Validator` 合格で `Validated`。
  4. `Compiler::compile`(generation = runtime.generation()+1)→
     `PreparedRouting{key, spec, source_map, compiled, device_in,
     playbacks, node_roles}`。`Prepared` 状態へ。
- `publish(prepared, project, deps) -> RoutingActivationReceipt`:
  1. live project から key を再計算。revision/fingerprint/
     capability が一致しない場合は `Blocked{StaleProjectRevision|
     StaleCapability, last_good}` で publish 0。
     `semantic_eq` による NoRebuild 証拠経路は後続 slice
     (本 slice では stale は一律 reject)。
  2. `GraphRuntime::publish(compiled)` → `CommandChannelFull` 等は
     `Failed{PublishChannelFull, last_good}`(旧 plan は runtime が
     維持)。成功で `Published{rev, gen}`。
- `process(frames)` / `observe()`:
  - `rt.process(frames, ctx)` 後に `status().active_generation` を読み、
    `Published` 世代と一致すれば `Active{rev, gen, epoch}` receipt を
    確定(`frames_observed += frames`)。`frames==0`/停止中は
    `Published` のまま。
  - `last_good_generation` = 最後に `Active` 観測した generation。
- `send_event`/`collect` 等の runtime passthrough は driver と同型で
  公開するが、本 slice の receipt には含めない。

### 4.2 既存 driver との関係

`HeadlessDriver` は変更しない。`RoutingEngine` は独立した runtime
instance を持ち、clip playback 機構(`ClipPlayer`/`Prefetch`/
`PlaybackStats`)は同じ型を再利用する。将来の統合は app 層の選択。

## 5. plan_playbacks 抽出(lowering.rs リファクタ)

`lower_with_inserts` の clip/comp 解決部を `PlaybackPlan` 収集関数へ
抽出し、legacy path(node emit+fan-in 接続)と semantic path(node emit
+所有 strip 接続)の双方が同一の解決結果を使う。`emit_node`/`LowerEnv`
は legacy 側に残し、semantic 側は `routing_lower` が独自 emit を持つ。

## 6. 失敗と Last-good

- compile/validation 失敗: publish 未到達、旧 plan 維持、
  `Failed{CompileFailed|ValidationFailed, last_good}`。
- publish channel full: `Failed{PublishChannelFull, last_good}`。
- stale key: publish 前に `Blocked{Stale*, last_good}`、publish 0。
- RT が新 generation を観測しない限り `Active` にならない
  (delayed activation は `Published` のまま Last-good と新 plan が
  queue 上で待機する — ruviform runtime の世代 semantics に従う)。
- Project は常に commit 済みのまま(revision 巻き戻し無し)。

## 7. テスト方針(test-spec 参照)

- `tests/sf03_lowering.rs`(player): golden PCM 単一/複数 track、
  send tap、mute/solo prune、sidechain 順序、sourcemap↔revision、
  missing source、stale publish 拒否、activation 証拠分離。
- `tests/routing_build.rs`(project): fingerprint 同値/差分、
  MappingResolution、receipt 型の決定性。
