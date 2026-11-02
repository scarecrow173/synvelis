---
id: "DESIGN-034PCA03Nj8xRs4yAbOcDe"
type: "design"
status: "approved"
title: "PC-A Parameter Control Model 設計 — Scope/Breakdown/StateMachine headless PoC"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/40"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md"
    - "docs/designs/ws-a-workspace-foundation-design.md"
    - "docs/designs/ed-a-editing-model-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/185"
  defers: []
  deferred-by: []
---

# PC-A Parameter Control Model 設計 — Scope/Breakdown/StateMachine headless PoC

## Context

Issue #40: #6 合意 Parameter Control Model を `synvelis-project`
(永続モデル + 評価) + `synvelis-workspace` (session 状態機械) 上に
headless PoC として実装する詳細設計。ADR Option 1 に従い、永続
データと評価規則は project 側、Mode/Arm/Gesture/Recording/
Playback/Observation は workspace 側の `ControlSession` に置く。

前提:
- `ProjectController::apply` は validate → scratch clone apply →
  atomic swap → 1 undo push を提供する。
- ruviform 契約: `ruviform-plugin-core` の `ParamId`/`ParamFlags`/
  `ParamValue`/`ParamInfo`/`ParamWriteMeans`/`ParamWriteRejection`/
  `check_param_write`/`check_param_write_for`/`ParamEvent`/
  `TimedParamEvent`/`Event`/`EventKind`/`EventList` (rf#186);
  `ruviform-media-core` の `BeatPosition`/`Ppqn`/`Tick`/
  `TickPosition`/`TempoMap`/`MediaTime`/`TimeBase`/`Rounding` (rf#189),
  `InputStamp`/`ClockAnchor`/`ClockDomainId`/`map_stamp_to_frame`/
  `EngineFrame`/`ClockEpochId` (rf#188), `TickTimestamp`/
  `MediaTimestamp`/`TimestampProvenance` (rf#185)。
- 時間演算は checked API のみ (`BeatPosition::checked_add`/
  `checked_sub`、ruviform `Tick`/`TickDuration` の unchecked 演算子
  不使用)。

## Scope

### In Scope

- `synvelis-project/src/control.rs`: 永続モデル
  (ParameterTarget/AutomationLane/ModulationSource/
  ModulationAssignment/ControlScope/BeatPoint) + 評価
  (reference 解決/modulation 合成/clamp/量子化) + control graph
  compile + capability/配送契約 + 競合検出。
- `synvelis-project` 変更: 新 ID (param/automation-lane/mod-source/
  mod-assign/note)、`Project::control`、`ProjectOp` 追加、
  `validate_references` 拡張、`SCHEMA_VERSION` 1→2。
- `synvelis-workspace/src/control.rs`: `ControlSession`
  (mode/arm/gesture/recording/playback/observation/projection)。
- `tests/pc_a.rs` 受入 test (PC-A-01..08)。

### Out of Scope

- 実 GUI、実 plugin adapter、Bake、Take multi-pass、Feedback graph、
  MIDI Control Mapping 実装、Note entity、sample-accurate 配送。

## Design

### 1. 永続モデル (`synvelis-project::control`)

Wire DTO:

```text
BeatPoint {num: i64, den: u32}        // ruviform BeatPosition の serde
                                      // DTO。den==0 は construction 拒否。
type BeatRange = Range<BeatPoint>;    // 既存 Range<F> を再利用 (end>=start)
```

`BeatPoint` は `Ord` を ruviform `BeatPosition` の全順序経由で実装
し、`Range<BeatPoint>` で区間検証を再利用する。tick↔beat は
`BeatPosition::new(tick.value(), ppqn)` (正確な有理数) と
`TickPosition::from_beat(beat, ppqn, Rounding)` で相互変換 —
PPQN 格子への黙示 quantize なし。

ID (全て `define_id!`):

```text
ParameterTargetId "param", AutomationLaneId "automation-lane",
ModulationSourceId "mod-source", ModulationAssignmentId "mod-assign",
NoteId "note"
```

Entities:

```text
ControlScope = Track(TrackId) | Clip(ClipId) | Note(NoteId) | Project

ValueKind = Continuous | Stepped{steps:u32(>=2)} | Boolean | Choice{options:u32(>=2)}
ParamMapping = Linear | Exponential      // Exponential: 非線形写像 fixture
TargetCapability {param_id: u32, flags_bits: u32, min_value, max_value,
                  default_value}
  // ParamFlags::from_bits で厳密復元 (未知 bit 拒否)、check_param_write
  // に渡す ParamInfo を構築 (name/unit は契約に無関係なため空)。
ParameterTarget {id, owner: ControlScope, name: String,
                 kind: ValueKind, mapping: ParamMapping,
                 capability: Option<TargetCapability>, base: f64}
  // base は normalized [0,1] (構築時検証)。
AutomationPoint {pos: BeatPoint, value: f64}   // normalized [0,1]
Interpolation = Step | Linear
AutomationRegion {range: BeatRange, interpolation, points: Vec<AutomationPoint>}
  // points は pos 昇順・range 内。
AutomationLane {id, target: ParameterTargetId, regions: Vec<AutomationRegion>}
LfoShape = Sine | Triangle | Square
SourceKind = Lfo{rate: BeatPoint (周期、>0), shape, bipolar: bool}
           | Macro{value: f64 ∈ [-1,1]}
           | StepSeq{values: Vec<f64>, step: BeatPoint (>0)}
ModulationSource {id, owner: ControlScope, kind: SourceKind}
Polarity = Bipolar | Unipolar
Transform = Identity | Invert | Abs | Square
ModulationAssignment {id, source, target, depth: f64,
                      depth_source: Option<ParameterTargetId>,
                      polarity, transform, enabled: bool}
ControlModel {targets, lanes, sources, assignments: BTreeMap<…>}
Project::control: ControlModel        // #[serde(default)]
```

`SourceKind::value(&self, at: BeatPosition) -> f64`: LFO は位相
`at / rate` の決定的評価 (Sine/Triangle/Square)、Macro は定数、
StepSeq は `floor(at/step) % len` の index 評価。bipolar は [-1,1]、
unipolar は [0,1]。

### 2. 評価 (`control::eval`)

```text
ReferenceSource = Base | Automation | Manual
lane_value(project, target, pos) -> Option<f64>
  // target の全 lane の全 region の point を統合 (region は非重複が
  // 不変条件)。pos で chase 評価: 最新 point <= pos と次 point で
  // interpolation。最初の point より前は None (base fallback)。
resolve_scope_pos(project, target, pos) -> ScopeResolution
  // Track/Project: pos をそのまま使用。
  // Clip(clip): clip.anchor が Musical(range) なら
  //   local = pos - range.start を BeatPosition checked_sub で評価。
  //   clip range 外 → LaneOutside (envelope は clip 存在区間のみ)。
  //   Media anchor → Unsupported (clip-local 評価は Musical anchor のみ)。
  // Note: entity 未実装 → Unavailable。
evaluate_control(project, target, pos, manual: Option<f64>)
    -> Result<ControlEvaluation, Error>
  // 1. reference = manual or lane_value or base (ReferenceSource 記録)
  // 2. target 向き enabled assignments を id 昇順で:
  //    raw = source.value(pos); transformed = transform.apply(raw);
  //    polarity Unipolar → (x+1)/2; depth_eff = depth * depth_source解決値
  //    contribution = depth_eff * transformed
  // 3. raw_total = reference + Σ contributions
  // 4. clamp [0,1] → clamped/saturated flag; ValueKind で量子化
  //    (Stepped: round(v*(steps-1))/(steps-1), Boolean: v>=0.5,
  //     Choice: round(v*(options-1))/(options-1))
  // 5. host_resolved = ParamValue::new(v) (rf#186 契約の正規化/NaN
  //    規則に準拠), plain = mapping.denormalize(v)
ControlEvaluation {position, base, automation: Option<f64>,
                   reference, reference_source, contributions: Vec<
                     Contribution{assignment_id, source_id, raw,
                     transformed, depth, value}>,
                   raw_total, clamped, saturated, quantized: bool,
                   host_normalized, host_plain}
```

加算は assignment id 昇順固定 (表示順・node 配置に非依存の決定的
順序、#6 決定③)。Clip scope 評価は clip の anchor を毎回解決する
ため、clip の MoveClip は point 書換えなしで envelope 評価を移動
させる (PC-A-02/08)。

### 3. Control Graph (`control::graph`)

```text
ControlGraph::compile(project) -> Result<EvalPlan, Error>
  // node: 全 source + 全 target。
  // edge: assignment.enabled → source→target;
  //       assignment.depth_source → depth_source(target)→target。
  // DFS で topo 順生成、back edge → Error::ControlCycle {path}
  // (無遅延循環の明示拒否)。
EvalPlan {order: Vec<ParameterTargetId>}
  // RT 側評価は固定 order の有界走査のみ。prepare/validate は
  // compile (非 RT) が担い、構築失敗は既存実行を壊さない
  // (plan は immutable value)。
```

### 4. 競合検出 (`control::conflict`)

```text
AutomationConflict {target, lane_a, lane_b, overlap: BeatRange}
detect_conflicts(project) -> Vec<AutomationConflict>
  // target ごとに全 lane の region range を走査し pairwise overlap。
  // 同一 lane 内の重複も検出 (不正 snapshot 防御)。
ConflictPolicy = Reject | Replace
ProjectOp::WriteAutomationRegion {lane, region, policy}
  // validate: lane 存在、region 整合 (points 昇順・range 内・値域)。
  //   Reject: detect した overlap があれば Error::AutomationConflict
  //     {target, overlapping: Vec<(lane, range)>} を返す — silent
  //     merge/priority 解決なし。
  //   Replace: 明示上書き — 既存 region から重複範囲の point を除去
  //     し新 region を挿入 (caller が明示した overwrite なので可)。
ProjectOp::EraseAutomationRange {lane, range}
  // 範囲内の point 除去 (recording commit の erase+write で併用)。
Project::validate_references
  // lane→target、assignment→source/target/depth_source、scope 所有
  // 参照 (Track/Clip) を検証。同一 lane 内の region 重複は
  // Validation、cross-lane の重複は detect_conflicts が報告する
  // (snapshot は複数 lane が同一 target を指すことを禁止しない —
  //  重複の存在自体は正当な検出対象であり load 拒否は lane 内重複
  //  のみ)。
```

`Error` に `AutomationConflict {target: String, detail: String}` と
`ControlCycle {path: String}` を追加。

### 5. Capability / 配送 (`control::dispatch`)

```text
dispatch_check(target, means: ParamWriteMeans)
    -> Result<(), ParamWriteRejection>
  // capability なし (fixture 外の純 internal param) → Ok
  // capability あり → check_param_write(&param_info, means)
  //   (rf#186 の手段×flags 行列をそのまま消費)
lower_event(project, target, eval_value, position: MediaTime,
            means: ParamWriteMeans) -> Result<TimedParamEvent, Error>
  // dispatch_check → ParamEvent{param_id, ParamValue} →
  // TimedParamEvent{position, event}。means の選択は caller —
  // TimedEvent が NotAutomatable なら caller は BlockBoundary を
  // 明示的に選び degraded を記録できる。
quantize_value(kind, v) -> f64      // Stepped/Boolean/Choice の明示規則
modulation_allowed(kind) -> bool    // Continuous のみ true —
  // Boolean/Choice/Stepped への連続 modulation は quantum 規則が
  // あっても depth 連続合成は許可しない (量子化は最終出力の規則)。
  // PoC: assignment 作成時に target.kind != Continuous → reject。
```

### 6. Session 状態機械 (`synvelis-workspace::control`)

```text
AutomationMode = Off | Read | Touch | Latch | Write   // UI mode (既定 Read)
WriteArm = Armed | Disarmed                            // 既定 Disarmed
PlaybackStatus = Stopped | Playing
PlaybackState {status, position: BeatPoint,
               loop_range: Option<BeatRange>,
               epoch: ClockEpochId, frame: EngineFrame}
GestureState (per target): {started_at: BeatPoint,
  points: Vec<(BeatPoint, f64)>}                       // ControlGesture
RecordingTransaction {started_at, fixed_targets: Vec<ParameterTargetId>,
  fixed_revision: ProjectRevision,
  segments: BTreeMap<AutomationLaneId, Vec<AutomationPoint>>,
  sealed: Option<SealReason>}                          // SealReason = LoopWrap
ControlObservation {plugin_reported: Option<(f64, MediaTimestamp)>,
  probe_value: Option<(f64, MediaTimestamp)>,
  state: ObservationState}
ObservationState = AtPlayhead | Live | Captured | Stale | Unavailable
ControlSession {
  modes: BTreeMap<ParameterTargetId, AutomationMode>,
  arms: BTreeMap<ParameterTargetId, WriteArm>,
  record_enabled: bool,
  playback: PlaybackState,
  gestures: BTreeMap<ParameterTargetId, ControlGesture>,
  manual_overrides: BTreeMap<ParameterTargetId, f64>,
  latch_held: BTreeMap<ParameterTargetId, f64>,
  recording: Option<RecordingTransaction>,
  observations: BTreeMap<ParameterTargetId, ControlObservation>,
  input_anchor: Option<ClockAnchor>,
}
```

全て session scoped — ViewState には入れず、`ProjectSession` の
`control` フィールドとして保持 (transport/engine stub と同列)。
永続化しない (WorkspaceSnapshot も Project も無関係)。

API (WorkspaceManager 経由で呼び出し、プロジェクト mutation は
`session.apply` → `distribute_revision` の既存経路):

```text
set_mode(view-or-ctx なし、target, mode)   // session のみ → revision 不変
  // mode 変更時: latch_held から当該 target を除去 (Latch 終了規則)。
set_arm(target, armed) / set_record_enabled(bool)
transport_play() / transport_stop() / transport_seek(pos) /
transport_advance(to) -> TransportEffect {wrapped: bool, sealed: bool}
  // advance は loop_range 越えで wrap (position = loop.start) し、
  // recording 中なら transaction を LoopWrap で seal (以後の書込み
  // 拒否、commit/abort のみ受付)。seek は position 更新のみ
  // (chase = 次回 evaluate が新 pos で行われるため特別処理不要)。
gesture_begin(target, at) -> Result<(), WorkspaceError>
  // fixation: {target, mode snapshot, arm snapshot, record_enabled,
  //   revision, started_at}。Off → BaseEdit、他は ValueGesture。
  // capability READONLY → Validation 拒否。
gesture_update(target, pos, value)         // manual_overrides 更新 +
                                           // gesture.points 追記
gesture_end(target) -> GestureEnd
  // Off → SetParameterBase op (1 undo、Base 編集 = 通常編集)
  // Read → override 破棄 (復帰)、無書込み
  // Touch → write 条件成立時のみ gesture 区間を recording segment へ
  //         + override 破棄 (復帰)
  // Latch → 同上 + latch_held[target] = last value (継続書込み)
  // Write → write 条件成立時のみ書込み継続 (override 維持)
record_start() -> Result<(), WorkspaceError>
  // 条件: record_enabled && playing && ≥1 armed target。
  // fixation: {targets: armed∩write-mode 対象, revision, started_at}。
record_write(target, pos, value)           // segment 追記
  // (Touch/Latch/Write の gesture 書込みと transport_advance 中の
  //   latched/continuous 書込みが呼ぶ)
record_commit() -> Result<RecordingOutcome, WorkspaceError>
  // segments を lane ごと Composite{EraseAutomationRange(span)+
  //   WriteAutomationRegion(region, Replace)} として 1 session.apply
  //   = 1 undo。sealed でなくても明示呼出しで確定。
record_abort()                              // transaction 破棄
evaluate(target, ctx) -> Result<ControlEvaluation, Error>
  // ctx = AtPlayhead (停止) | Live (再生)。manual = override or
  //   latch_held or write 継続値 → manual 引数で eval へ注入。
lower(target, position_beats, means) -> Result<TimedParamEvent|Degraded>
  // beat → TempoMap.beat_to_media → dispatch_check → lower_event。
  // NotAutomatable → BlockBoundary で再試行した場合は Degraded として
  // 明示 (UnsupportedPrecision 相当を報告)。
feed_plugin_report / feed_probe (target, value, stamp: MediaTimestamp)
  // provenance は ruviform TickTimestamp/MediaTimestamp 契約。
observation(target) -> ControlObservation   // state 判定:
  // データなし → Unavailable。stamp の provenance Estimated → Stale
  // 扱いではなく、state は playback 文脈 (AtPlayhead/Live) と
  // 観測の存在/陳腐 (revision/frame 差) で決定。
stamp_gesture(target, stamp: InputStamp, value) -> Result<(), WorkspaceError>
  // rf#188: map_stamp_to_frame(stamp, anchor, frame_base, now) →
  // EngineFrame → media_to_tick → BeatPosition で gesture_update。
  // MapRejection は defined rejection として返す。
projections (読み取りのみ):
  lane_view(target) -> LaneView {regions 読み取り}
  signal_flow_view(target) -> Vec<AssignmentView{source, depth, enabled}>
  inspector_breakdown(target, ctx) -> ControlBreakdown {evaluate +
    observation + mode + arm + capability}
```

Mode × write 条件 (`write_allowed(target) = arm==Armed &&
record_enabled && playback.status==Playing && recording open 可能`):

| Mode | Gesture 中 reference | 書込み | 終了 |
| --- | --- | --- | --- |
| Off | manual (表示のみ) | gesture_end → SetParameterBase | — |
| Read | manual | なし | override 破棄で復帰 |
| Touch | manual | write_allowed なら gesture 区間 | override 破棄 |
| Latch | manual | write_allowed なら gesture 区間 + 継続 | latch_held 保持 |
| Write | manual | write_allowed なら継続 | override 維持 (stop まで) |

記録値は gesture/manual の normalized 値 = **Modulation 合成前の
Manual/Reference 値**。`evaluate` の host_resolved は書き込みに
使われない (二重適用防止、PC-A-05)。

### 7. Crate 依存

```text
root Cargo.toml [workspace.dependencies]: ruviform-plugin-core = git 追加
synvelis-project: + ruviform-plugin-core
synvelis-workspace: dep 変更なし (ruviform-media-core 型は project 経由
  で再 export された DTO を使う…必要なら direct でも可だが最小化の
  ため project の DTO のみ)
```

実装上 `TimedParamEvent`/`MediaTime` 等は project 側 API が返すので
workspace は plugin-core を直接触れない。

### 8. 不変条件

- Automation/Modulation の点列・寄与は `Project::control` のみ —
  投影・session は複製を持たない。
- Mode 選択・arm・gesture・observation feed は `session.apply` を
  呼ばない (revision 不変)。
- `Reject` policy で overlap する region は apply されない
  (controller の validate-total で保証)。
- Recording commit は必ず 1 `apply`。
- 全ての eval 入力は finite; `ParamValue::new` が NaN を拒否する
  契約に依存し、計算経路は f64 finite を維持する。
- Clip/Note scope と Track/Project scope の座標は型で分離
  (`ScopeResolution` が local 座標を返す)。

## Testing Strategy

`crates/synvelis-workspace/tests/pc_a.rs` — fixture は
`CounterIdGenerator` + `FakeWindowHost` (完全決定論)。
PC-A-01..08 を個別 test に対応付ける (test-spec 参照)。

## Risks

- `ruviform-plugin-core` 追加による依存増 — 同一 rev の ruviform
  crate のみ。`cargo tree` で確認。
- `SCHEMA_VERSION` 2 で旧 snapshot が読めない — PoC 間の意図的変更。
- `Project::control` 追加で `validate_references` が長くなる —
  既存パターン (entity ごとの検証列) に従う。
- `Region` 重複検出は target ごと O(n²) — PoC 規模では許容、数を
  正直に記録。
- LFO 評価は `to_f64` 経由 (beat→f64)。正確な正本は BeatPosition
  であり f64 は評価専用の投影 (documented lossy)。
