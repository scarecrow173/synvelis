---
id: "DESIGN-CJVcdyUweKy4i6p0xOHdJc"
type: "design"
status: "approved"
title: "SF-05 slice2 設計 — routing gesture fixation/preview/commit + inspection focus + schema v3 persist"
created: "2026-11-10"
updated: "2026-11-10"
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
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-05-slice2-routing-gesture-inspection-persist-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/adr/sf-05-slice2-routing-gesture-inspection-persist-boundary.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-slice2-signal-flow-ux0307.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice2 設計: routing gesture + inspection focus + persist

## Context

slice-1 が `signal_flow_graph` read model・`SignalFlowVisualState`
（in-memory）・`syn.signal_flow.view`/`visual` op を確立した。
`ViewState`（schema v2, `deny_unknown_fields`）には
`editing_selection`/`object_selections`/`inspection_focus` 等が
既にあり、`WorkspaceSnapshot`/`ViewRecord` が view 単位で
`ViewState` を丸ごと往復させる。gesture 系は
`state.gesture`（arrangement）・`control_edge_gesture`・
dock `is_transferring` が workspace に既存する。

## Overview

```
syn.signal_flow.routing ──▶ WorkspaceManager::routing_gesture_*
        │                     Begin: fixation{view,source,revision,
        │                            capability_generation,window}
        │                     Hover: dry-run RoutingPortAssessment
        │                     Drop:  dropped_target 固定（以後 retarget 不可）
        │                     Confirm: stale 検査 → ProjectSession::apply
        │                              (ProjectOp::Routing) = 1 undo
        │                     Cancel:  CancelReason で破棄（project 不変）
        ▼
ViewState::routing_gesture (serde skip, session-only)

syn.signal_flow.inspection ──▶ signal_flow_inspect /
        set_inspection_mode / set_follow_source / explore_back /
        explore_forward ──▶ SignalFlowInspectionFocus
        （Follow/Explore/Pinned・session-only 履歴）

LinkGroupManager::deliver ──▶ SemanticTarget ──▶
        signal_flow_entity_from_semantic ──▶ Some のみ
        SignalFlow Follow focus を更新（Explore/Pinned 不変、
        selection/arrangement focus 不変、再 emit なし）

ViewState::signal_flow: Option<SignalFlowViewState>
        { visual: SignalFlowVisualState, inspection }
        ──▶ WorkspaceSnapshot (schema v3) ──▶ ViewInstanceId 単位
        capture/restore。v2→v3 additive、malformed → None
```

## Detailed Design

### `crates/synvelis-workspace/src/routing_gesture.rs`（新規）

```rust
/// Begin で固定される gesture の不変条件。
pub struct RoutingGestureFixation {
    pub view: ViewInstanceId,
    pub source: SemanticPortId,
    pub revision: ProjectRevision,
    pub capability_generation: u64,
    pub window: Option<WindowId>,   // detached view は None
}

/// Hover/drop の dry-run 状態。project への参照を持たない。
pub struct RoutingGesturePreview {
    pub candidate: Option<SemanticPortId>,
    pub assessment: Option<RoutingPortAssessment>,
    pub dropped_target: Option<SemanticPortId>,
}

pub struct RoutingGesture {
    pub fixation: RoutingGestureFixation,
    pub preview: RoutingGesturePreview,
}

pub enum RoutingPortAssessment {
    Compatible,
    NeedsMapping { source_channels: usize,
                   destination_channels: usize },
    Prohibited { reason: String },
}

pub enum RoutingGestureOutcome {
    Applied { revision: ProjectRevision },
    Cancelled { reason: CancelReason },
    Stale,
}
```

helper: `routing_connection_kind(source, target, model)` で
`ConnectionKind`（Main/Sidechain/Send 系）を導出、
`routing_gesture_intent(fixation, preview, channel_map, model)`
で confirm 時の `ProjectOp::Routing` intent を組み立て、
`port_capability_generation(project)` で plugin port
reconfigure を検出する generation を採る。prohibited pair
（direction/role 不整合）は intent 化前に `Prohibited`
assessment として表に出し、commit は project oracle の拒否に
委ねる（二重 policy を持たない）。

### `signal_flow_ui.rs` 追加型

```rust
pub struct SignalFlowInspectionFocus {
    pub target: Option<SignalFlowEntityRef>,
    pub mode: InspectionMode,               // Follow/Explore/Pinned
    pub follow_source: Option<FollowSource>,
    #[serde(skip)] pub explore_back: Vec<SignalFlowEntityRef>,
    #[serde(skip)] pub explore_fwd: Vec<SignalFlowEntityRef>,
}

pub struct SignalFlowViewState {
    pub visual: SignalFlowVisualState,
    pub inspection: SignalFlowInspectionFocus,
}
```

`signal_flow_entity_from_semantic(SemanticTarget)` — 現状
`Track` のみ `Some`。`positions` map は
`serde_entity_positions`（entry list codec）で永続化する。
serde derive を `SignalFlowEntityRef`/`SignalFlowScope`/
`SignalFlowEdgeRole`/`SignalFlowLayer`/`SignalFlowVisualGroup`/
`SignalFlowVisualState`/`SignalFlowInspectionFocus`/
`SignalFlowViewState` に付与。

### `state.rs`

```rust
pub struct ViewState {
    // ...既存 field...
    #[serde(default, deserialize_with = "deserialize_signal_flow")]
    pub signal_flow: Option<SignalFlowViewState>,
    #[serde(skip)]
    pub routing_gesture: Option<RoutingGesture>,
}
```

`deserialize_signal_flow` は payload を `Option<
SignalFlowViewState>` として試行し、失敗時 `None`（warning
なしの degrade — snapshot 全体を殺さない）。

### `workspace.rs` gesture/inspection API

- `routing_gesture_begin(view, source)` — 既存 gesture family
  3 系（`gesture`/`control_edge_gesture`/`routing_gesture`）の
  いずれかがあれば `GestureActive`、`dock.is_transferring` で
  `ViewBusy`、source が `routing.ports` に無ければ
  `InvalidGesture`。fixation を確定して `ViewState::
  routing_gesture` に格納。
- `routing_gesture_hover(view, Option<port>)` — preview の
  candidate/assessment を更新。drop 後は `InvalidGesture`。
- `routing_gesture_drop(view)` — preview.candidate を
  `dropped_target` に固定して返す。candidate 無し（空 canvas
  drop）なら gesture を終了。
- `routing_gesture_confirm(view, channel_map)` —
  1) `fixation.window` と現 `dock.owner_of` の一致、
  2) live revision == fixation.revision、
  3) capability generation 一致、を順に検査 → 不一致は
  gesture clear + `Stale`。4) `NeedsMapping` で
  `channel_map=None` は project が拒否 → gesture 生存のまま
  error。5) `routing_gesture_intent` → `session.apply` →
  `Applied{revision}`。
- `routing_gesture_cancel(view, reason)` — gesture を捨てて
  `Cancelled{reason}`。project 不変。
- `signal_flow_inspect` — target が変わるとき旧 target を
  `explore_back` に push・`explore_fwd` clear、mode=Explore。
- `signal_flow_set_inspection_mode` — Explore 以外へ移ると
  履歴 stack を clear。
- `signal_flow_explore_back/forward` — stack 間を 1 段移動、
  mode は変更しない。

### `link.rs`

`deliver` の末尾で、ordinary inspection 更新の後に
`signal_flow_entity_from_semantic(target)` が `Some(entity)`
かつ当該 view の Signal Flow focus が `Follow` mode・
`follow_source` filter 一致なら `inspection.target` を更新。
`None`（Lane/Clip/Source）や Explore/Pinned では何もしない。
receiver 側のみ・再 emit なし・dedup は既存の
`(receiver, event_id)` 機構。

### `persist.rs`

`WORKSPACE_SCHEMA = 3`、`READABLE_SCHEMAS = [2, 3]`。
`from_json` は parse 後 `schema` を 3 に正規化 — v2 snapshot
は `signal_flow` 欠落 → `None` のまま読める（additive）。
`capture`/`restore` は変更不要（`ViewRecord.state` が
`ViewState` 丸ごと運ぶ）。

### `synvelis-app`

- `ui_ops.rs`: `UiOpsState.signal_flow` map を削除し、
  `sf_view_instance(view)` で wire `view-{{n}}` →
  `ViewInstanceId` を解決（未登録なら `dock_view_info` の
  kind/title、無ければ `ViewType::SignalFlow` で detached
  adopt）。`op_signal_flow_visual` は `state_mut().signal_flow`
  の `visual` を、`op_signal_flow_inspection`/`routing` は
  workspace API を呼ぶ。`op_signal_flow_view` と
  `app.rs::signal_flow_view` は `ViewState.signal_flow.visual`
  から options を復元（未登録/未作成は default）。
- wire 形: entity は `{"kind","id"}`、port は tagged
  `SemanticPortId` 文字列、inspection/routing の ack は
  focus/outcome/fixation/preview を JSON 化。

## File Map

- `crates/synvelis-workspace/src/routing_gesture.rs` - 新規
- `crates/synvelis-workspace/src/signal_flow_ui.rs` - serde +
  `SignalFlowViewState`/`SignalFlowInspectionFocus`/変換 helper
- `crates/synvelis-workspace/src/state.rs` - `signal_flow`/
  `routing_gesture` field + tolerant deserializer
- `crates/synvelis-workspace/src/workspace.rs` - gesture +
  inspection API
- `crates/synvelis-workspace/src/link.rs` - Signal Flow 追従
- `crates/synvelis-workspace/src/persist.rs` - schema v3
- `crates/synvelis-workspace/src/lib.rs` - export
- `crates/synvelis-workspace/tests/sf_05_s2.rs` - 新規
- `crates/synvelis-app/src/ui_ops.rs` - ops + helpers
- `crates/synvelis-app/src/app.rs` - view options 読み替え
- `crates/synvelis-app/src/service.rs` - op 登録
- `crates/synvelis-app/tests/sf05_signal_flow.rs` - 追加 test
- `docs/` チェーン一式

## Risk / Rollback

- `ViewState` への additive field は `#[serde(default)]` で
  既存 v2 payload と両立 — `deny_unknown_fields` は維持。
- gesture commit が `session.apply` 失敗（stale 以外）で
  gesture を残す設計は、cancel で必ず抜けられることを test
  で固定する。
- link 追従の追加分岐は Follow のみ — Explore/Pinned/selection
  への波及は test で否定形まで検証する。
- rollback: 新規 module/ops は additive、`ViewState` field は
  default で安全 — revert は file 削除 + schema 定数戻しで
  完結する。

## Verification

- `mise run fmt-check`/`check`/`lint`/`test`。
- `mise run test-rt`/`check-app-native`/`build-app-native`/
  `lint-app-native` — 存在範囲で実行、不能は NotRun 記録。
- `mise run test-msrv`。
- 実 OS/GPUI は NotRun（#114 gate）。

## Sources and Relations

- [spec](../specs/sf-05-slice2-signal-flow-ux0307.md) /
  [ADR](../adr/sf-05-slice2-routing-gesture-inspection-persist-boundary.md) /
  [plan](../plans/sf-05-slice2-signal-flow-ux0307.md) /
  [task](../tasks/sf-05-slice2-signal-flow-ux0307.md) /
  [test-spec](../test-specs/sf-05-slice2-signal-flow-ux0307.md)
