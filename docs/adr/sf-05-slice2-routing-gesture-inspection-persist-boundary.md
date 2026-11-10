---
id: "ADR-guOCD1TmkDjNjrG0pOh2c8"
type: "adr"
status: "accepted"
title: "SF-05 slice2: routing gesture は workspace ViewState の session-only state、inspection focus は link 変換経由、visual state は schema v3 で ViewInstanceId 永続化"
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
    - "docs/adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md"
    - "docs/specs/pc-c5-02-03-gesture-stale-inspector.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
  derived-by:
    - "docs/designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# ADR: SF-05 slice2 — routing gesture / inspection focus / per-view persist の境界

## Context and Problem Statement

Issue #108 の slice-2 は SF-UX-03（Port Drag routing
gesture）・SF-UX-04（link/follow/pinned inspection 追従）・
SF-UX-07（`SignalFlowViewState` の `ViewInstanceId` 単位
persist）を要求する。slice-1 ADR は visual state を app 層
in-memory とし、永続化を本 slice へ `defer` していた。

決めるべきは 4 点: gesture state の所有者、preview/commit の
分界、link event から Signal Flow focus への写像規則、
`ViewState` schema 変更の互換方針。

## Decision

1. **routing gesture は `synvelis-workspace` の `ViewState`
   上の session-only state**（`ViewState::routing_gesture`、
   serde skip）として保持する。fixation は view・source
   `SemanticPortId`・live `ProjectRevision`・port-capability
   generation・window owner（`dock.owner_of`）を Begin 時に
   固定する。gesture family 間の相互排他（既存
   `state.gesture`/`control_edge_gesture` との排他）と
   mid-transfer 拒否は workspace が既に持つ dock/gesture
   権威をそのまま使う — app 層に複写しない。
2. **preview は dry-run、commit は 1 `ProjectOp::Routing`**。
   hover は clone した fixation に対する
   `RoutingPortAssessment`（`Compatible`/`NeedsMapping`/
   `Prohibited`）を計算して preview state に置くだけで
   project・undo・engine に触れない。drop で candidate を
   `dropped_target` として固定し、以後 hover は retarget
   しない（late receipt/target swap なし）。confirm は
   stale 検査（revision・capability generation・window
   ownership/transfer）を先に行い、不一致なら gesture を
   clear して `Stale` を返す。stale でない project 側の
   拒否（`Prohibited`・channel_map 未 consent 等）は gesture
   を生かしたまま error を返し、caller の consent/retry/
   cancel に委ねる。適用は `ProjectSession::apply` 経由の
   1 op = 1 undo 境界。
3. **inspection focus は `SignalFlowInspectionFocus` として
   `SignalFlowViewState` に同居させるが、arrangement の
   `inspection_focus`・`EditingSelection` とは完全に独立**。
   link delivery（`LinkGroupManager::deliver`）は従来どおり
   link context と ordinary Follow inspection を更新した後、
   `signal_flow_entity_from_semantic` の明示変換が `Some` を
   返した場合のみ Signal Flow Follow focus を mode/source
   filter 付きで更新する。変換は現状 `SemanticTarget::Track`
   → `SignalFlowEntityRef::Track` のみで、Lane/Clip/Source は
   `None`（暗黙写像しない）。Explore/Pinned は retarget
   せず、Explore 履歴（back/fwd stack）は serde skip の
   session-only。receiver 側の状態更新のみで再 emit せず、
   `(receiver, event_id)` dedup は既存機構を共有する。
4. **`SignalFlowViewState`（visual + inspection）は
   `ViewState::signal_flow` に additive 追加し、Workspace
   Schema を v2→v3 に上げる**。v2→v3 は additive migration
   （欠落 field は `None`）。`signal_flow` field は tolerant
   deserializer で、payload が壊れていても `None` に degrade
   し snapshot 全体を reject しない。`positions` の
   `BTreeMap<SignalFlowEntityRef, _>` は enum key が JSON
   object key になれないため entry list
   （`[[entity, [x, y]], ...]`）として serialize する。
   app 側は `UiOpsState.signal_flow` map を廃止し、
   `syn.signal_flow.*` の mutation/inspection/routing op が
   wire `view_id` を `ViewInstanceId` に解決して未登録なら
   detached pool に adopt する — 永続化対象を workspace の
   `ViewState` 一本にする。

## Consequences

### Positive

- gesture の fixation/stale 判定が dock window ownership・
  project revision・capability generation を workspace の
  権威から直接読み、app が状態を二重管理しない。
- preview が型上 project を触れない（`RoutingGesturePreview`
  は値のみ）ので、hover 中の undo/PCM/graph build 混入経路が
  存在しない。
- link→Signal Flow 追従が明示変換 1 関数に閉じ、
  `SemanticTarget` の非 routing 対象を暗黙に Signal Flow
  entity へ写す経路がない。
- schema v3 は additive で v2 snapshot をそのまま読め、
  per-view state が `ViewRecord`/`ViewState` の既存往復に
  そのまま乗る。

### Negative

- `ViewState::routing_gesture` は serde skip のため
  snapshot 復元で gesture は復活しない — session-only の
  意図どおりだが、crash/再起動を跨ぐ drag は再開しない。
- `SemanticTarget` → `SignalFlowEntityRef` の変換が Track
  のみのため、link event が Bus/Strip/Processor を指しても
  Signal Flow focus は追従しない（拡張は後続 slice の
  判断）。
- app の `syn.signal_flow.*` mutation op が view を detached
  adopt するため、headless/未 attach の `view-N` でも state
  が生える — `syn.view.attach` 側は既存の
  registry/move_view 経路で整合する。

## Considered Options

### Option A: workspace ViewState gesture + session apply commit（採用）

PC-C5-02 の gesture fixation 先例と同じく workspace が
fixation/stale を所有し、commit は `ProjectSession::apply`
の 1 op に畳む。

-利点: 単一 writer・undo 境界・dock 権威を再利用。
  stale 判定の根拠（revision/capability/window）が 1 箇所。
- 欠点: workspace API が gesture lifecycle 分だけ増える
  （`routing_gesture_*` 6 関数 + inspection 5 関数）。

### Option B: app 層で gesture state を保持

`UiOpsState` に fixation/preview を置き、confirm 時に
`ProjectOp::Routing` を組み立てる。

-利点: workspace 変更が小さい。
- 欠点: window ownership・gesture 相互排他・stale revision
  の権威が workspace/dock にあり、app がそれらを読み集めて
  再判定する二重化になる。採用しない。

### Option C: inspection focus を ordinary `inspection_focus`
  に相乗り

`SemanticTarget` のまま共有 focus を使い、Signal Flow 側で
表示変換する。

-利点: state が 1 系統。
- 欠点: Issue が要求する「Signal Flow 専用の
  Follow/Explore/Pinned・独立履歴・明示 entity 変換」を
  表せず、arrangement inspect/link context と意味が混ざる。
  採用しない。

### Option D: schema v2 のまま visual state を app 側に残す

`UiOpsState.signal_flow` を維持し persistence だけ後回し。

-利点: schema 変更ゼロ。
- 欠点: SF-UX-07 の `ViewInstanceId` 単位 persist を満たせ
  ず、view-1/view-2 の state 分離も workspace snapshot に
  乗らない。採用しない。

## References

- `docs/specs/sf-05-slice2-signal-flow-ux0307.md`
- `docs/specs/sf-05-signal-flow-headless-hierarchy.md`
- `docs/specs/pc-c5-02-03-gesture-stale-inspector.md`
- `docs/adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md`

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/108>
- [spec](../specs/sf-05-slice2-signal-flow-ux0307.md) /
  [design](../designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md) /
  [plan](../plans/sf-05-slice2-signal-flow-ux0307.md) /
  [task](../tasks/sf-05-slice2-signal-flow-ux0307.md) /
  [test-spec](../test-specs/sf-05-slice2-signal-flow-ux0307.md)
