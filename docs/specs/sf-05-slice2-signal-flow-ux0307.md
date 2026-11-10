---
id: "SPEC-hhfyzYWYjCwiqdwtsN8Add"
type: "spec"
status: "approved"
title: "SF-05 slice2 Signal Flow — Port Drag gesture + inspection focus + per-view persist（SF-UX-03/04/07）"
created: "2026-11-10"
updated: "2026-11-10"
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
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/pc-c5-02-03-gesture-stale-inspector.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-slice2-signal-flow-ux0307.md"
  references: []
  deferred-by: []
---

# SF-05 slice2: Signal Flow — Port Drag gesture + inspection focus + per-view persist

## What Should Be Built

Issue #108（Signal Flow Editor / Semantic+Visual Hierarchy）の
**slice-1 以降の headless 完結 slice**。slice-1 が確立した
read-only 投影（SF-UX-01/02/05）の上に、編集 gesture・
inspection 追従・view 単位永続化の 3 受入を積む。

- **SF-UX-03**: Port Drag routing gesture。Begin で
  source port・発生 view/window・project revision・
  port-capability generation を fixation し、hover は dry-run
  assessment のみ（project/undo/engine に触れない）、drop で
  candidate を固定、confirm で 1 回の `ProjectOp::Routing`
  transaction として commit（= 1 Undo 境界）。stale
  revision/capability/window transfer・port role 不整合・
  channel layout 不一致を検出し、channel 変換は明示 consent
  （`ExplicitChannelMap`）なしに commit しない。Cancel は
  project revision・undo・runtime activation・graph build・
  PCM を一切変えない。
- **SF-UX-04**: Signal Flow 専用 inspection focus
  （`SignalFlowInspectionFocus`）。Follow/Explore/Pinned の
  3 mode、Explore の back/forward 履歴は session-only
  （永続化しない）。link delivery は `SemanticTarget` →
  `SignalFlowEntityRef` の明示変換が成功した場合のみ
  Follow-mode focus を追従させ、Explore/Pinned は決して
  retarget しない。編集中 selection・arrangement の
  inspection focus とは独立で、link は receiver 側の状態
  更新のみ（再 emit なし、`(receiver, event_id)` dedup）。
- **SF-UX-07**: `SignalFlowViewState`（visual + inspection）を
  `ViewState` 経由で `ViewInstanceId` 単位に永続化。
  Workspace Schema v2→v3 migration（additive）と、壊れた
  `signal_flow` payload が snapshot 全体を reject しない
  safe recovery を含む。

### 提供するもの

1. `synvelis-workspace`:
   - `routing_gesture.rs`（新規）: `RoutingGesture`/
     `RoutingGestureFixation`/`RoutingGesturePreview`/
     `RoutingGestureOutcome`/`RoutingPortAssessment` と
     intent/connection-kind preflight helper。
   - `signal_flow_ui.rs`: `SignalFlowViewState`・
     `SignalFlowInspectionFocus`・serde 派生・
     `signal_flow_entity_from_semantic` 変換 helper。
   - `state.rs`: `ViewState.signal_flow`（tolerant
     deserialize）と session-only `ViewState.routing_gesture`。
   - `workspace.rs`: `routing_gesture_begin/hover/drop/
     confirm/cancel`・`signal_flow_inspect`/
     `set_inspection_mode`/`set_follow_source`/
     `explore_back`/`explore_forward`。
   - `link.rs`: link delivery が Signal Flow Follow focus を
     独立に更新する経路。
   - `persist.rs`: `WORKSPACE_SCHEMA = 3`・readable `[2, 3]`・
     v2→v3 正規化。
2. `synvelis-app`:
   - `syn.signal_flow.inspection` op（`inspect|set_mode|
     set_follow_source|back|forward`）。
   - `syn.signal_flow.routing` op（`begin|hover|drop|confirm|
     cancel|state`）。
   - `syn.signal_flow.visual`/`view` の per-view state を
     workspace `ViewState` へ移管（`UiOpsState.signal_flow`
     廃止）— 初回 mutation で view を detached pool に adopt。
3. headless fixture test:
   - `crates/synvelis-workspace/tests/sf_05_s2.rs`（gesture
     lifecycle・stale・consent・cancel・inspection mode・
     link 追従・snapshot migration/recovery）。
   - `crates/synvelis-app/tests/sf05_signal_flow.rs` 追加
     （wire op lifecycle・per-view 分離・snapshot 往復）。

## Why Now

slice-1（PR 済み）が read model・visual state・engine evidence
合成を確立し、SF-02 の `ProjectOp::Routing` transaction・
SF-03 の session/engine 接続・PC-C5-02 の gesture fixation /
Inspector Follow/Explore/Pinned 先例が main に揃った。
本 slice はそれらを Signal Flow 固有の意味論で結合し、
Issue #108 の編集・追従・永続化 slice を headless で完結
させる。実 GUI 受入（SF-UX-08・#114 gate）と Probe 共有
（SF-UX-06・#107 配線待ち）の前提となる。

## Scope Boundary

- **In scope**: `crates/synvelis-workspace/`・
  `crates/synvelis-app/`（ui_ops/app/service + tests）・
  `docs/` チェーン。
- **Out of scope（後続 slice / NotRun）**:
  - SF-UX-06 Probe ON/OFF・2 Window 同一 Semantic Point・
    capture dedup — **NotRun**（#107 app/session wiring 待ち）。
  - SF-UX-08・実 GUI（High DPI/Multi Window/Keyboard/IME/
    a11y/Plugin focus）— **NotRun**（実 OS gate）。
  - SF-UX-01/02/05 の残り実 GUI 側受入 — **NotRun**。
  - #109 独立 Oracle・#114 GPUI harness・#103・AudioLens UI
    互換 — **NotRun**。
- **触らない**: `crates/synvelis-project/`・
  `crates/synvelis-library/`・`crates/synvelis-recording/`・
  `crates/synvelis-player/`・`crates/synvelis-render/`
  （`ProjectSession::apply`/`preview_routing_edit`/
  `routing_tx::preflight`/`Project::routing`/`probe.rs`/
  `routing_build.rs` は既存 public API の read/consume のみ）、
  Ruviform 側、`Cargo.toml`/`mise.toml`/`.github/workflows/`。

## Acceptance

- [ ] SF-UX-03: Port Drag/ChannelMap incompatibility/explicit
  consent/Cancel/Concurrent window revision/Plugin port
  reconfigure/late receipt で Target swap なし、成功は 1 Undo
  （headless fixture）。
- [ ] SF-UX-02 の一部: Node Move/Visual Group/Fold/Zoom/
  Link Focus のみで DSP/ProjectRevision/Undo/Graph build/PCM
  不変（fixture 層 — slice-1 検証の回帰込み）。
- [ ] SF-UX-04: link delivery が Follow mode の Signal Flow
  focus のみを追従させ、Explore/Pinned・selection・
  arrangement focus を変えない。FollowSource filter・
  dedup・no re-emit を維持する（headless fixture）。
- [ ] SF-UX-07: Visual state が `ViewInstanceId` 単位で保存・
  復元され、v2 snapshot が v3 へ migrate し、malformed
  `signal_flow` payload が `None` に degrade して snapshot
  全体を reject しない（headless fixture）。

## Dependencies / Triggers

- 先行: SF-05 slice-1（read model/visual state/ops）・
  SF-01/SF-02（routing model・transaction）・SF-03 slice5
  （session engine 接続）・PC-C5-02（gesture fixation・
  inspector mode 先例）— 全て main 済み。
- 後続へ送る: SF-UX-06 probe 共有/capture dedup（#107 配線）、
  SF-UX-08 実 GUI 受入（#114 gate）、#109 oracle 結合。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/108>
- [ADR](../adr/sf-05-slice2-routing-gesture-inspection-persist-boundary.md) /
  [design](../designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md) /
  [plan](../plans/sf-05-slice2-signal-flow-ux0307.md) /
  [task](../tasks/sf-05-slice2-signal-flow-ux0307.md) /
  [test-spec](../test-specs/sf-05-slice2-signal-flow-ux0307.md)
