---
id: "SPEC-034Uw3sFH0Ejg8sJTrDntB"
type: "spec"
status: "approved"
title: "PC-C4 plugin parameter binding — verified frame base・capability gate・generation consistency（fake port slice）"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c6-oracle-rel-tr.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
    - "docs/designs/pc-c3-control-trace-design.md"
  defers: []
  deferred-by: []
---

# PC-C4 plugin parameter binding — verified frame base・capability gate・generation consistency（fake port slice）

## What Should Be Built

Issue #101（PC-C4 実 Plugin Parameter Binding・Verified Frame Base・
Plan Activation/Capability Gate）のうち、`synvelis-player` crate 内で
完結する fake plugin port slice を実装する。既存 PC-B（#43）の
`ParamDispatch`/`ParamTargetTable`/`ParamDispatchLedger` 経路と
PC-C3（#100）の `ControlTrace`/世代・時間・欠落証拠の設計に乗せ、
次を提供する:

- **Verified Frame Base（Ruviform #307 consumer 側）**:
  `PluginNodeControl::frame_base()`/`check_frame_base()`（pin
  `0f5b573` で利用可能）を消費し、`ParamDispatch` 構築時に
  dispatcher の `TimeBase` と attach-time frame base の一致を API 上で
  照合する。異値は Plan Schedule 以前に型付き
  `FrameBaseMismatch` で拒否し、旧 Plan/Node 状態を破壊しない。
- **`PluginParameterBinding`**: `synvelis_target:
  ParameterTargetId`・`instance_id: StablePluginInstanceId`・
  `node: NodeId`・`plugin_param: ParamId`・`capability_generation`・
  `graph_generation`・`attach_clock_context`（verified frame base）を
  持つ非 RT binding record。Project Stable `ParameterTargetId` と
  Plugin Local `ParamId`、runtime `NodeId` 世代、ProjectID を混同
  しない。
- **`PluginBindingSession`**: 1 つの attach 済み plugin instance に
  対する binding table + 検証済み attach context の所有者。
  `bind`（project target → live `ParamInfo` 列挙 + 宣言
  `TargetCapability` の照合、owner=Processor 強制、write means ×
  flags 行列、離散 kind への連続 modulation 拒否）、`revalidate`
  （param removed → defunct）、`dispatch`（binding pin 照合後に
  `ParamDispatch` へ転送 — 全件検証成功まで何も stage しない原子
  batch）を提供する。
- **計算→dispatch→Receipt の結合**: `ControlGraph::compile` →
  `EvalPlan::eval_rt`（automation+manual+LFO/Macro/StepSeq/Depth
  Driver の複合評価、DAG cycle は compile 時に型付き拒否済み）→
  `lower_event` → binding 検証付き dispatch、を headless fixture で
  RT drive と offline `render()` の双方で一致検証する。

## Why Now

Ruviform #307（closed）は attach-time `frame_base` の非 RT 検証済み
context を bridge に実装済みで、Synvelis 側 consumer 導入は本 Issue
（PC-C4）の責務として明示されている。現行 `ParamDispatch::new` は
「AttachConfig と等値必須だが bridge に照合アクセサなし」という
コメント付きで外部 `frame_base` を無検証に受け取っており、誤値は
frame 座標スケール違いとして TimedParamEvent・Seek・Plan/Receipt
整合・Offline 精度を静かに破り得る。#106（SourceMap/ActiveGraph
Generation）・#122（Host/Plugin Parameter 正本）・#127（実測精度）
の統合受入に先立ち、player 層で fail-closed の binding 契約を固定
する必要がある。

## Users and Value

- 後続 slice（#106 SourceMap 接続・#122 Parameter 正本配線・#103
  Plugin Oracle）の実装者が、誤配送 0・世代整合・型付き拒否を
  保証する binding 境界を再利用できる。
- #114 native GUI / #108 UI 系が、Plugin GUI 値変更や re-attach を
  世代不整合として安全に扱える土台を得る。
- Ruviform cross-consumer（AudioLens 等）に対し、#307 契約の
  Synvelis 側消費が型・fixture で検証済みであることを示せる。

## Scope

### In Scope

- `crates/synvelis-player/src/binding.rs`（新規）: `AttachClockContext`・
  `StablePluginInstanceId`・`PluginParameterBinding`・
  `PluginBindingSession`・`BindingError`。
- `crates/synvelis-player/src/dispatch.rs`: `ParamDispatch::new` の
  frame base 照合（`PluginNodeControl::check_frame_base` 経由、
  `DispatchError::FrameBaseMismatch` 追加）。既存 bind/dispatch/
  observe 契約は維持。
- `crates/synvelis-player/src/lib.rs` export（additive）。
- `crates/synvelis-player/src/host.rs` `ProcessorControl` の
  `frame_base` doc を #307 verified context 前提に整合（コメント
  レベル、API 不変）。
- headless テスト `crates/synvelis-player/tests/pc_c4_binding.rs`:
  PLUG-C4-01..05 の fake plugin fixture 実証と PC-C4-PL01〜08 の
  Expected/Observed 照合。
- `docs/` 文書チェーン（spec/design/plan/task/test-spec/impl record）。

### Out of Scope

- 実 VST3/OS Backend・実 plugin version/rate/block — **NotRun**
  （Ruviform #196/#307/#253 の別 Owner gate）。
- OS MIDI 翻訳 — Ruviform #253 の責務。
- `synvelis-project`/`synvelis-app`/`synvelis-workspace`/
  `synvelis-recording`/`synvelis-library` 等他 crate の変更
  （project 側接続は既存 port の消費のみ）。
- project/workspace/app 層への実配線（後続 slice）。
- `ControlTrace`/`DispatchTraceAdapter`/`ProbeTraceBridge` の公開
  契約変更（消費のみ）。
- ruviform pin 変更・Ruviform 側実装。
- persisted binding の保存形式（runtime object のみ — 永続化は
  #106/#122 統合 slice）。

## Requirements

- 同一 AttachContext・一致 TimeBase で `eval_rt` → `lower_event` →
  dispatch → Frame/Value Receipt が RT drive と offline render で
  一致する（fake plugin fixture、絶対 frame 比較）。
- 異 TimeBase・同 RawId 別 plugin instance・param removed・
  readonly・unsupported means・queue full はすべて型付き Reject で
  あり、別 target への誤配送は 0（batch は全件検証後にのみ stage）。
- LFO/Macro/StepSeq/Depth Driver/automation+manual の複合制御を
  `EvalPlan` 経路で評価し、DAG cycle は `ControlGraph::compile` の
  `ControlCycle` で拒否される。離散 `ValueKind` への連続 modulation
  は `modulation_allowed`（Continuous のみ）により bind 時に型付き
  拒否し、黙って有効化しない。
- Plugin restart/re-attach/GraphPlan swap/Loop/Seek/Transport
  change/Plugin GUI 値変更に対し fail-closed: instance id・node・
  capability/graph generation・verified frame base の pin が外れた
  binding は型付き拒否。frame base は attach 時不変（upstream 契約）
  であり、ClockEpoch/Transport/PlanGeneration と混同しない。
  無断の offline fidelity 低下をしない（offline render は同一
  dispatch 経路・test-scoped `OfflineRenderSupport::Verified`
  宣言は PC-B 先例どおり fixture 限定）。
- 実 VST3/OS Backend は未検証時 **NotRun** と記録し、Ruviform
  cross-consumer CI/f64/RT no-alloc/nonblocking の非回帰を
  `mise run test-rt` 等の既存計測で確認する。

## Acceptance Criteria

- [ ] PLUG-C4-01〜04 の fixture 実証（fake plugin port・headless）。
- [ ] PLUG-C4-05 のうち Ruviform #307 attach-time FrameBase 契約との
  型/fixture 互換（実 VST3 は NotRun）。
- [ ] PC-C4-PL01〜08 を Fixture+Expected/Observed で照合
  （PluginInstance/CapabilityGen/GraphGen/ClockEpoch/Verified
  FrameBase、ParameterTargetId/ParamId、QueueFull/Reload/Graph
  Swap/Retire、min-process Frame Offset、stale event 拒否）。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`check-headless-deps`（存在するもの全て）が green。

## Deferred Design Concerns

- 実 VST3/OS plugin・version/rate/block・RT/Offline 精度の実測
  | fake port slice の範囲外、Upstream gate | Ruviform #196/#253
  Owner gate 通過後 | Mock と実機の差異は未検証のまま残る。
- project/workspace/app 層への binding 配線（SourceMap 世代照合・
  Parameter 正本接続・session 所有権）| 他 dispatch 稼働中・別 slice
  | #106/#122 統合 slice | port 契約は本 slice で固定するが実配線は
  後続。
- persisted binding / `StablePluginInstanceId` の長期安定性
  （engine restart 横断）| runtime object のみが対象 | 永続化 slice
  | session 内一意性のみ保証し、永続 id の再検証は未対応。
- `DispatchTraceAdapter`/`ControlTrace` への binding stage 証拠の
  接続 | 既存契約の消費に限定 | trace 統合 slice | binding 段階の
  trace slot は現状 `ParamDispatch` ledger の stage のみ。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/101>
  （PLUG-C4-01..05・PC-C4-PL01〜08 検証方法）
- Upstream: <https://github.com/scarecrow173/ruviform/issues/307>
  （attach-time frame_base 検証済み context — closed、pin `0f5b573`
  で `frame_base()`/`check_frame_base()`/`plan_builder()`/
  `InstallPlanError::FrameBaseMismatch` 利用可能）
- 前提: PC-B #43（`ParamDispatch`/binding table/ledger・
  RT+Offline parity 手法）、PC-C3 #100（`ControlTrace`・世代・
  時刻・欠落証拠）、SF-03 slice3（`ProcessorHost`/`PluginNodeControl`
  vend 経路）、CTL-01/02 合意（`ParameterTargetId` 単一正本・
  2 段 binding）。
- 本 spec は Issue #101 のうち `synvelis-player` 内完結の fake port
  slice のみを対象とし、Issue は Open 継続が前提。
