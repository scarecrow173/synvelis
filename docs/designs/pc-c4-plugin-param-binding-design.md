---
id: "DESIGN-034Uw4SG2SiRKQuoKOZGzL"
type: "design"
status: "approved"
title: "PC-C4 plugin parameter binding design — verified frame base・capability gate・generation consistency"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/ruviform/issues/307"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c4-plugin-param-binding.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
    - "docs/designs/pc-c3-control-trace-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c3-05-rt-trace-adapter-design.md"
    - "docs/designs/sf-03-slice3-plugin-host-connect-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-plugin-param-binding.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
    - "docs/designs/pc-c3-control-trace-design.md"
  defers: []
  deferred-by: []
---

# PC-C4 plugin parameter binding design — verified frame base・capability gate・generation consistency

## Context

`ParamDispatch`（PC-B, `crates/synvelis-player/src/dispatch.rs`）は
`TargetId → (NodeId, ParamId)` の binding table と generation-tagged
`TransportPlan` staging を既に所有するが、(a) `frame_base` を無検証で
外部から受け取る、(b) binding は bridge 層の `(node, param)` 解決までで
project 側 `ParameterTarget` の owner/capability/世代との照合を持たない、
(c) plugin instance・capability generation・graph generation を pin する
record が無い。Ruviform #307（pin `0f5b573`）で
`PluginNodeControl::frame_base()`/`check_frame_base()`/
`plan_builder()`/`InstallPlanError::FrameBaseMismatch` が利用可能に
なったため、Synvelis 側 consumer として検証済み context を消費する層を
player 内に新設する。project 側の `ControlGraph`/`EvalPlan`/
`lower_event`/`dispatch_check`/`modulation_allowed`・`TargetCapability`
は既存 port として消費のみ（他 dispatch 稼働中のため変更しない）。

## Scope

### In Scope

- `crates/synvelis-player/src/binding.rs`（新規 module）
- `crates/synvelis-player/src/dispatch.rs` の `ParamDispatch::new`
  frame base 照合 + `DispatchError::FrameBaseMismatch`
- `crates/synvelis-player/src/lib.rs` additive export
- `crates/synvelis-player/src/host.rs` `ProcessorControl::frame_base`
  doc の #307 前提への整合（comment のみ）
- `crates/synvelis-player/tests/pc_c4_binding.rs`（新規 fixture）
- docs チェーン

### Out of Scope

- spec の Out of Scope と同一（実 VST3/OS、project/app/workspace 配線、
  persisted binding、trace 契約変更、ruviform 変更）。

## Components and Boundaries

### `AttachClockContext`（binding.rs）

1 node の attach-time clock context。`frame_base: TimeBase` のみを持ち、
唯一の生成経路は `AttachClockContext::of(&PluginNodeControl)`（=
`control.frame_base()` の読取）。外部 `TimeBase` を主張する構築子を
置かないため、検証済みでない値が context になることは型上あり得ない。
`check_frame_base(base)` は upstream `FrameBaseMismatch` をそのまま返す。

### `StablePluginInstanceId`（binding.rs）

runtime plugin instance の session 内一意 identity（`u64` newtype）。
発行は composition root（本 slice では test fixture）が attach 毎に行い、
同一 allocator 内で再利用しない。re-attach/restart/graph plan swap は
新 id を発行するため、旧 id を pin した binding は構造的に stale になる。
永続安定性（engine restart 横断）は本 slice の対象外 — persisted
binding は #106/#122 統合 slice の論点として deferred。

### `PluginParameterBinding`（binding.rs）

非 RT の binding record（Issue #101 の形状 + `graph_generation`）:

```text
{ synvelis_target: ParameterTargetId,
  instance_id: StablePluginInstanceId,
  node: NodeId,
  plugin_param: ParamId,
  capability_generation: u64,
  graph_generation: u64,
  attach_clock_context: AttachClockContext }
```

`PartialEq` で record 全体比較可能（dispatch 時の pin 照合に使う）。

### `PluginBindingSession`（binding.rs）

1 attach 済み instance の binding table + 検証済み context。

- `attach(control: &PluginNodeControl, node: NodeId,
  processor: ProcessorId, graph_generation: u64,
  capability_generation: u64, instance_id: StablePluginInstanceId)
  -> Result<Self, BindingError>` — `AttachClockContext::of(control)` で
  verified frame base を取得。`ParamDispatch` 自体は所有せず、各 method
  が `&mut ParamDispatch` を受け取る（`HeadlessDriver::AttachedNode`・
  将来の adapter 経路と所有権を衝突させないため）。
- `bind(&mut self, dispatch, project, target, params)
  -> Result<PluginParameterBinding, BindingError>`:
  1. `project.control.targets[target]` 解決（無ければ typed reject）。
  2. `owner == ControlOwner::Processor(self.processor)` を強制
     （Strip/Send/Clip target を plugin session に bind させない —
     誤配送の構造的防止）。
  3. `target.capability` 必須 → `ParamId(cap.param_id)`。
  4. live `params` 列挙に `param_id` が存在すること（param removed →
     `BindingError::ParamRemoved`）。
  5. 宣言 `TargetCapability`（flags_bits/min/max/default）と live
     `ParamInfo` の照合 → drift は `CapabilityMismatch`。
  6. live `check_param_write(info, TimedEvent)` + 宣言側
     `dispatch_check(target, TimedEvent)` → readonly /
     not_automatable / unsupported means を typed reject。
  7. `target` への enabled `ModulationAssignment` があり
     `!modulation_allowed(target.kind)` なら `UnsupportedModulation`
     （連続 modulation の離散 kind への黙った有効化を防ぐ）。
  8. `dispatch.bind(target, param, params)` → `PluginParameterBinding`
     を記録（`param_owners` の `(node,param)` 一意性は dispatch 側契約）。
- `revalidate(&mut self, dispatch, target, params) -> bool` —
  `dispatch.revalidate` に委譲。false（param 消失等）なら session の
  binding record も除去し、以後の当該 record による dispatch を
  `BindingDefunct` で拒否。
- `invalidate(&mut self)` — node 退役/re-attach 時に全 binding を
  stale 化（`dispatch.invalidate_node` と対で呼ぶ）。
- `dispatch(&self, dispatch, writes: &[(PluginParameterBinding,
  TimedParamEvent)]) -> Result<Arc<TransportPlan<EventKind>>,
  BindingError>`:
  1. `dispatch.node() == self.node` かつ `dispatch.frame_base() ==
     attach_clock.frame_base()`（context pin — `ParamDispatch::new`
     の構築時照合に対する dispatch 時の再確認）。
  2. 全 `(binding, event)` を検証: `binding == self.bindings[target]`
     の record 一致（instance/node/param/gens/clock context の pin）、
     `binding.plugin_param == event.event.param_id`、
     `binding.capability_generation == self.capability_generation`、
     `binding.graph_generation == self.graph_generation`。
     1 件でも不一致なら stage 前に typed reject — batch は原子的。
  3. `(binding.synvelis_target, event)` を `dispatch.dispatch` へ転送し
     `DispatchError` を `BindingError::Dispatch` に写像。
- `eval_event(&self, project, plan, scratch, target, pos, manual,
  position, means) -> Result<TimedParamEvent, BindingError>` —
  `plan.eval_rt` → `lower_event` の結合。`self.bindings` に target が
  無ければ `TargetNotBound`（compute→lower 段階で fail-closed）。
  automation+manual 複合は `eval_rt` の既存 semantics に委譲。
- `check_plan(&self, plan, project) -> Result<(), BindingError>` —
  `plan.is_stale(project)` なら `PlanStale`（GraphPlan swap 検出）。
- `is_current(capability_generation, graph_generation) -> bool` と
  accessor 群（instance/node/processor/frame_base/generations/
  binding 参照）。

### `ParamDispatch::new` の frame base 照合（dispatch.rs）

署名を `-> Result<Self, DispatchError>` に変更し、構築時に
`control.check_frame_base(frame_base)` を実行。`Err` は新 variant
`DispatchError::FrameBaseMismatch { expected, actual }`（upstream
`FrameBaseMismatch` の値を保持）。doc の「must equal … no accessor」
コメントを #307 verified context 前提に更新。`frame_base()` accessor・
`control()`・既存 bind/dispatch/observe 契約は不変。全 call site
（`host.rs` doc・`driver.rs`・tests）を `Result` 対応に更新する。

### `BindingError`（binding.rs）

`#[non_exhaustive]` enum。variant:

- `FrameBaseMismatch(ruviform_audio_plugin_bridge::FrameBaseMismatch)`
- `InstanceMismatch` / `NodeMismatch`（foreign binding・node pin）
- `CapabilityGeneration { expected, actual }` /
  `GraphGeneration { expected, actual }`
- `PlanStale`（`EvalPlan::is_stale`）
- `TargetNotBound` / `BindingDefunct` / `ParamRemoved`
- `OwnerMismatch`（target.owner が session の Processor でない）
- `CapabilityMismatch { param: ParamId }`（宣言↔live drift）
- `UnsupportedModulation { target: ParameterTargetId }`
- `WriteRejected { means, rejection }`（upstream token 保存）
- `Dispatch(DispatchError)` / `Project(synvelis_project::Error)`

rejection を success に変換しない — 全失敗は型付き。

## Data and Control Flow

```text
Project (ParameterTarget/TargetCapability/assignments)
  + EvalPlan::eval_rt (automation+manual+modulation, DAG cycle safe)
  + lower_event (means×flags, quantize)
  + live ParamInfo enumeration (plugin params())
        │
        ▼  non-RT only
PluginBindingSession::bind → PluginParameterBinding (pins)
        │
        ▼  session.dispatch: pin 照合（instance/node/param/gens/clock）
ParamDispatch::dispatch → TransportPlanBuilder(frame_base=verified)
        │  Play + install_plan (upstream FrameBaseMismatch 再検証)
        ▼
PluginNode scheduler → ParamEvent @ absolute engine frame
        │
        ▼  RT drive / offline render()（同一経路）
plugin endpoint delivered events + AppliedReceipt/ParamValueObservation
```

RT/Offline の一致は dispatch 経路が同一であること + fixture の
絶対 frame/value 比較で検証する。frame base は attach 不変、
ClockEpoch/Transport/PlanGeneration は frame base と別軸
（upstream 契約）— seek/loop/transport change は binding の有効性を
変えない。Plugin GUI 値変更は report channel の `record_reported`
観測であり、列挙/flags が変わる場合のみ `revalidate` で defunct 化。

## Non-Functional Constraints

- RT 契約: binding/session/dispatch はすべて非 RT orchestration 側。
  `eval_rt`/`lower_event`/`check_param_write` は upstream の
  allocation-free 契約をそのまま消費し、本層は plan 構築・検証のみ
  （RT callback・Mutex・blocking なし）。
- f64 正直性: `ParamValue`/`quantize_value`/`eval_rt` の semantics を
  変更しない。NaN/非有限は upstream で reject 済み。
- fail-closed: 未知・不一致・stale を成功に変えない。binding pin の
  一部欠落 record は受理しない。
- 後方互換: `ParamDispatch` の公開 API は `new` の `Result` 化を除き
  不変（破壊的変更は crate 内 call site のみ）。

## Risks and Trade-offs

- `ParamDispatch::new` の `Result` 化は既存 test/driver call site の
  更新を伴う — 機械的で影響は crate 内に限定。
- `StablePluginInstanceId` の session 内一意性は composition root の
  発行規律に依存 — doc で「attach 毎に新規発行・再利用禁止」を明記し、
  同一 id の再利用を検出する機構は持たない（YAGNI: 永続化は deferred）。
- `bind` 時の live `params` 照合は bind 呼出し側が最新列挙を渡す前提
  — `revalidate` を plugin 再列挙時に呼ぶ契約を doc に明記。
- `eval_event` は `eval_rt`+`lower_event` の薄い結合であり、独自の
  時間写像を持たない（position は caller 供給の `MediaTime` —
  beat→frame 変換は tempo map を持つ composition root の責務）。

## Deferred Design Concerns

- 実 VST3/OS backend・実 plugin capability | fake port のみ | Upstream
  gate | 実機差異は未検証。
- `DispatchTraceAdapter`/`ControlTrace` への binding 段階証拠
  | 既存 stage ledger で代替可能 | trace 統合 slice | binding 固有の
  slot が必要になった時点で設計。
- project/workspace/app 配線・SourceMap 世代照合 | 他 dispatch 範囲 |
  #106/#122 slice | port 契約のみ固定。
- persisted `StablePluginInstanceId`・binding snapshot | runtime
  object のみ | 永続化 slice | restart 横断の stale 検出は未対応。

## Verification Notes

- [x] `ParamDispatch::new` frame base 照合は upstream
  `check_frame_base` の `FrameBaseMismatch` を型として保存する設計で
  #307 契約と整合（fixture で異値拒否を実証）。
- [x] 誤配送 0 は「session.dispatch が全 binding を stage 前に検証」
  +「`ParamDispatch` の `param_mismatch`/`param_owners`」+「node 専用
  channel」の 3 層で構成的に保証。
- [x] PLUG-C4-01 の RT/Offline 一致は PC-B-04 と同じ
  `render()` parity 手法を binding 経路に適用して検証。
