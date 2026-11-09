---
id: "SPEC-034Uuc3q3FZYT5JjxQtDv9"
type: "spec"
status: "approved"
title: "SF-05 slice1 Signal Flow headless — hierarchy 投影 + visual-only 不変 + 状態表示"
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
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/pc-c5-01-relation-inspector.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  deferred-by: []
---

# SF-05 slice1: Signal Flow headless — hierarchy 投影 + visual-only 不変 + 状態表示

## What Should Be Built

Issue #108（Signal Flow Editor / Semantic+Visual Hierarchy）の
**headless 完結 slice**。3 つの受入を headless fixture で検証する。

- **SF-UX-01**: `RoutingModel`（SF-01 正本）を read-only 投影し、
  Bass→Music Bus→Master・Kick→Sidechain・Pre/Post Send→Return・
  Processor order の意味を `Overview`/`Local`/`Expanded`/`Collapse`/
  `Role Filter` の各表示モードで維持する。同一 `RoutingConnection`/
  `Send` を二重 Edge として出さない。
- **SF-UX-02**: Node Move / Visual Group / Fold / Zoom / Link Focus
  等の visual-only 操作は `ViewInstanceId` 単位の非 DSP 状態のみを
  変更し、`ProjectRevision`/Undo/Graph build/PCM を不変に保つ。
- **SF-UX-05**: `Configured`/`Prepared`/`Published`/`Active`/
  `Observed` と `Unavailable`/`Unresolved`/`Stale`/`Lost`/
  `Unsupported` を正しい Revision/Generation で表示する。
  `Publish != Active`（停止中に publish 成功を active と見せない）
  と Probe 未取得（requested/queued と RT-observed の区別）を
  潰さない。

### 提供するもの

1. `synvelis-workspace::signal_flow_ui`（新規 additive module）:
   - `SignalFlowEntityRef`（Track/Bus/Strip/Processor/Port/
     Connection/Send の型付き ref — 既存 `SelectionDomain` を
     広げない独立型）
   - `SignalFlowViewOptions`/`SignalFlowScope`/`SignalFlowLayer`/
     `SignalFlowEdgeRole`/`SignalFlowCoverage`
   - `SignalFlowNode`/`SignalFlowEdge`/`SignalFlowView`
   - `signal_flow_view(session, options, evidence)` —
     `RelationIndex`/`arrangement_ui` と同じ「session を読んで
     DTO を返す」read model。第二 domain model を作らない。
   - `SignalFlowRuntimeEvidence` — app が engine から採った
     receipt/source_map/probe bindings の snapshot。
   - `SignalFlowVisualState` + visual-only mutation
     （`visual_move_node`/`visual_set_group`/`visual_fold`/
     `visual_set_zoom`/`visual_set_link_focus`/`visual_set_scope`）。
     純粋値型で session/project を参照しない。
2. `synvelis-app`:
   - `SignalFlowScreenState` + main view の "Signal Flow" tab
     （status Label + nodes Table + edges Table、全て read-only）。
   - `publish_project_length` に signal flow refresh を additive
     追加。
   - `SessionRoutingEngine` に `probe_bindings()`/`probe_segments()`
     の read-only passthrough を追加（player crate は不変）。
   - ops: `syn.signal_flow.view`（投影+状態の照会）と
     `syn.signal_flow.visual`（visual-only mutation、ack +
     `view_generation` を返す）。
3. headless fixture test:
   - `crates/synvelis-workspace/tests/sf_05.rs`（投影・scope/
     collapse/role filter・dedup・非破壊・stale/unsupported）。
   - `crates/synvelis-app/tests/sf05_signal_flow.rs`（SF-UX-02 の
     revision/undo/graph-build 不変、SF-UX-05 の Publish≠Active・
     Probe 未取得区別を engine 実体で検証）。

## Why Now

Issue #108 の開始依存（#104 Semantic Routing Schema・#105/#106
lowering/activation・#107 probe adapter 基盤・#102 Relation Index・
headless view/window 基盤）は全て main 済み。read-only の Signal
Flow 投影は Probe/編集 UI の完成を待たず先行できる（Issue の
依存正規化どおり）。本 slice は後続の編集 slice（Port Drag/FX
reorder/SF-UX-03）と GPUI 実機 slice（SF-UX-08, #114 gate）の土台
となる view model を確立する。

## Scope Boundary

- **In scope**: 上記 3 ファイル群 + docs チェーン。
  `crates/synvelis-app/`・`crates/synvelis-workspace/`（additive）
  ・`docs/`。
- **Out of scope（後続 slice / NotRun）**:
  - SF-UX-03 Port Drag 編集・consent/late receipt
  - SF-UX-04 Follow/Explore/Pinned/2Window/Dock/Close/Focus 連携
  - SF-UX-05 の Probe *request* 発行（`probe_request` 等の mutation
    op — 本 slice は binding 状態の**表示**のみ消費）
  - SF-UX-06 Probe 共有/Capture dedup
  - SF-UX-07 Workspace Schema v2→v3 migration
    （`SignalFlowVisualState` は本 slice では **in-memory のみ**、
    `WorkspaceSnapshot`/`ViewState` serde には触れない）
  - SF-UX-08 実 OS/GPUI lifecycle/HighDPI/A11y/IME → **NotRun**
    （#114 gate）
  - #109/#103/#114 との Case ID 結合 → **NotRun**（oracle/gate 段階）
- **触らない**: `crates/synvelis-recording/`、
  `crates/synvelis-player/`、`crates/synvelis-project/` の
  control ledger 本体（`RelationIndex`/`ControlTrace`/
  `RoutingSourceMap` は read-only 消費のみ）、Ruviform。

## Acceptance

- [ ] SF-UX-01: Overview/Local/Expanded/Collapse/Role Filter の
  全モードで Bass→Music Bus→Master・Kick→Sidechain・
  Pre/Post Send→Return・Processor order の意味を維持し、
  同一 Connection/Send の二重 Edge がない（headless fixture）。
- [ ] SF-UX-02: `syn.signal_flow.visual` の全 action で
  `session.revision()`・undo 深さ・`graph_builds`・engine receipt
  が不変（headless fixture、revision tag 検証）。
- [ ] SF-UX-05: 状態表示が正しい `project_revision` と
  plan/published/observed generation を伴い、`Published`（未
  RT-observed）と `Active` を区別し、Probe binding の
  requested/queued と RT-confirmed を区別する（headless fixture）。

## Dependencies / Triggers

- 先行: SF-01（routing model/schema v6）・SF-03（lowering/
  SourceMap/activation receipt）・SF-04（probe adapter 基盤）・
  PC-C5（relation_ui read model 先例）— 全て main 済み。
- 後続へ送る: Port Drag/編集 op（SF-UX-03）、Follow/Pinned/2Window
  連携（SF-UX-04）、Probe request op と共有（SF-UX-06）、
  Workspace Schema v3（SF-UX-07）、GPUI 実機受入（SF-UX-08）。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/108>
- [ADR](../adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md) /
  [design](../designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md) /
  [plan](../plans/sf-05-slice1-signal-flow-headless.md) /
  [task](../tasks/sf-05-slice1-signal-flow-headless.md) /
  [test-spec](../test-specs/sf-05-slice1-signal-flow-headless.md)
