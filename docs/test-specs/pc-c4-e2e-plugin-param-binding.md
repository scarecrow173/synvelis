---
id: "TSPEC-034Uw6FJykHzgki5MvwCPZ"
type: "test-spec"
status: "approved"
title: "PC-C4 plugin parameter binding e2e — PLUG-C4-01..05 / PC-C4-PL01..08"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
    - "docs/test-specs/pc-b-e2e-parameter-dispatch.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-plugin-param-binding.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
  verified-by: []
  references:
    - "docs/test-specs/pc-b-e2e-parameter-dispatch.md"
  defers: []
  deferred-by: []
---

# PC-C4 plugin parameter binding e2e — PLUG-C4-01..05 / PC-C4-PL01..08

## Purpose

Issue #101 の fake plugin port slice が、Verified Frame Base・
capability gate・generation pin を伴う plugin parameter binding を
headless で正しく実証したことを Fixture+Expected/Observed で固定する。
実 VST3/OS は対象外（NotRun）。

## Feature

Plugin parameter binding — verified frame base・capability gate・
generation consistency（fake plugin port）

## Rules

- PL01 Verified Frame Base: `ParamDispatch::new` は attach-time
  `frame_base` と一致する `TimeBase` のみ受理し、異値は
  `DispatchError::FrameBaseMismatch` で Plan Schedule 前に拒否。
  `AttachClockContext` は `PluginNodeControl::frame_base()` のみから
  mint される。
- PL02 Instance/Param pin: 同 `ParamId` raw id を持つ別 plugin
  instance・node・generation・clock context の binding record は
  `session.dispatch` で型付き拒否。別 target への誤配送は 0。
- PL03 Capability gate: param removed・readonly・not automatable・
  unsupported means・宣言↔live drift は bind/dispatch で型付き
  Reject。離散 `ValueKind` への連続 modulation は
  `UnsupportedModulation`。
- PL04 Generation consistency: re-attach（新 instance id）・node
  invalidate・capability/graph generation drift・stale EvalPlan は
  fail-closed。Loop/Seek/Transport change/Plugin GUI 値変更は
  frame base を変えず binding を不当に stale 化しない。
- PL05 Queue/stale event: queue full は `QueueFull`、batch は全件
  検証後のみ stage、stale/未解決 event は `param_defunct`/
  `param_unresolved`/`PositionUnrepresentable` で拒否。
- PL06 RT/Offline parity: 同一 AttachContext・一致 TimeBase の
  eval→lower→dispatch→delivered event が RT drive と offline
  `render()` で絶対 frame/value 一致（in-block offset 含む）。
- PL07 Composite control: automation+manual+LFO/Macro/StepSeq+
  Depth Driver の複合評価値が `eval_rt` semantics どおり dispatch
  される。DAG cycle は `ControlGraph::compile` の `ControlCycle`。
- PL08 Min-process frame offset: block 内 offset の event が
  `frame_offset` 精度で絶対 engine frame に配送される。

## Examples

- Given: fake plugin（`ParamInfo` 列挙・delivered log 付き）に
  attach した `PluginNodeControl` と、`ControlOwner::Processor` の
  `ParameterTarget`（`TargetCapability` 付き）。
  When: `PluginBindingSession::attach` → `bind` → `eval_event` →
  `session.dispatch` → RT block 駆動。
  Then: plugin endpoint が絶対 frame で `ParamValue` を受け取り、
  ledger stage が `Dispatched` に到達。同じ手順を offline
  `render()` で行うと delivered event 列が完全一致する。
- Given: `TimeBase::sample_rate(44_100)` を主張する
  `ParamDispatch::new`（attach は 48_000）。
  When: 構築を試みる。
  Then: `DispatchError::FrameBaseMismatch` が返り、node の
  plan/queue 状態は不変。

## Guarantees

- binding record は instance/node/param/capability gen/graph gen/
  verified clock context を pin し、1 件でも不一致の batch は
  stage しない。
- 全 reject は型付き（`BindingError`/`DispatchError`）で upstream
  `ParamWriteRejection` token を保存し、success に変換しない。
- `session.dispatch` は `ParamDispatch` を所有せず `&mut` で受け取る
  ため `HeadlessDriver`/`DispatchTraceAdapter` の所有権と衝突しない。

## Non-goals

- 実 VST3/CLAP plugin・OS backend・実機計測（NotRun）。
- project/workspace/app 層への配線・永続化・SourceMap 接続。
- `ControlTrace` slot への binding 段階証拠の追加。
