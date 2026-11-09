---
id: "IMPL-034UyQi9tCjjAPwfMy7pSX"
type: "impl"
status: "in-progress"
title: "PC-C5 UI-C5-02/03 Gesture 固定・stale drop・Inspector mode（headless slice）"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
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
    - "docs/tasks/pc-c5-02-03-gesture-stale-inspector.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C5 UI-C5-02/03 Gesture 固定・stale drop・Inspector mode（headless slice）

## Summary

`synvelis-workspace` に Control Layer edge gesture（`ModulationSource →
ParameterTarget` の connect、Begin/Preview/Commit/Cancel 固定 semantics・
window epoch + revision CAS による stale drop）と Inspector
Follow/Explore/Pinned semantics（`InspectionMode::Explore`・`FollowSource`・
explore 履歴・link deliver の mode/source 照合・inspect/mode/back/forward API）
を実装し、`tests/uic5.rs` 12 fixture で UI-C5-02/03 を実証した。
`synvelis-app`・`synvelis-project` は変更なし。

## Context

- Task: `docs/tasks/pc-c5-02-03-gesture-stale-inspector.md`
- Plan: `docs/plans/pc-c5-02-03-gesture-stale-inspector.md`
- Spec: `docs/specs/pc-c5-02-03-gesture-stale-inspector.md`
- Design: `docs/designs/pc-c5-02-03-gesture-stale-inspector-design.md`
- Test spec: `docs/test-specs/pc-c5-02-03-gesture-stale-inspector.md`
- Upstream: synvelis#102（UI-C5-02/03）、synvelis#5（Inspector semantics 正本）、
  PC-C5-01（`docs/specs/pc-c5-01-relation-inspector.md`、main 入り済み）

## Implementation

### Changes

- `crates/synvelis-workspace/src/state.rs`: `InspectionMode::Explore` 追加、
  `FollowSource`（`View`/`Group`）新設、`InspectionFocus` に
  `follow_source`（serde 永続化）と `explore_back`/`explore_fwd`
  （`#[serde(skip)]` session-only）を追加。`ViewState` に
  `control_edge_gesture`（`#[serde(skip)]`）を追加。
- `crates/synvelis-workspace/src/control_edge.rs`（新規）:
  `ControlEdgeFixation`（view・typed 両 endpoint・`ProjectRevision`・
  `Option<WindowId>` epoch）・`ControlEdgeGesture`・`ControlEdgePreview`・
  `ControlEdgeOutcome`（`Applied`/`Cancelled`/`Stale`）。
- `crates/synvelis-workspace/src/workspace.rs`: inspector API
  （`inspect`/`set_inspection_mode`/`set_follow_source`/`explore_back`/
  `explore_forward`）と control edge API（`control_edge_begin`/
  `preview`/`commit`/`cancel`）を追加。commit は window epoch
  （`dock.owner_of` + `is_transferring`）と fixated revision の CAS で
  stale を `ControlEdgeOutcome::Stale` として drop し、成功時のみ
  `session.apply(AddModulationAssignment)` → `distribute_revision`。
  `set_input_context` が control edge gesture も破棄するよう拡張。
- `crates/synvelis-workspace/src/link.rs`: `deliver` が Follow mode かつ
  `follow_source` 一致（`None` = 従来通り自 group の全 event）時のみ
  `inspection_focus.target` を更新。`linked_context` と editing
  selection の扱いは不変。
- `crates/synvelis-workspace/src/preset.rs`: `into_view_state` の
  `InspectionFocus` 再構築に新 field を追従（preset は mode のみ永続化の
  既存方針を維持）。
- `crates/synvelis-workspace/src/lib.rs`: `control_edge` module と
  型の re-export、`FollowSource` の re-export。
- `crates/synvelis-workspace/tests/uic5.rs`（新規）: 12 fixture。
- `docs/`: spec/design/plan/task/test-spec/impl の文書チェーン。

### Decisions During Implementation

- **Control edge gesture は workspace 層の fixation として実装**。
  `ControlsUi` の `CommandIntent`（`LogicalTarget` 単一）では 2 endpoint の
  connect を表現できないため、`gesture.rs`（Audio Routing）とも別型の
  `control_edge.rs` に分離。`ControlEntityRef`（UI anchor）は Begin で 1 回だけ
  typed id に解決し、これが「UI selection ≠ Command Target」の実装点。
- **`follow_source: None` は従来挙動を維持**（view の属する group が deliver
  する全 event に追従）。既存 ws_a fixture が依存しており、`Some` でのみ
  source 照合を追加する後方互換設計。
- **explore 履歴は session-only**（`#[serde(skip)]`）。Issue #5 の
  「Explore の履歴は既定でリセット」と `PresetViewState` の
  entity-state 非永続化方針に一致。`set_inspection_mode` で Explore 以外へ
  遷移した際に両 stack を clear。
- **commit の stale 判定は 2 段**: まず `is_transferring`/`owner_of` の
  window epoch 照合、次に `session.apply` の expected revision CAS
  （`Error::StaleRevision` → `Stale`）。gesture は stale 時に破棄し、
  非 stale の project error では保持（caller が cancel 可能）。
- **`inspect` は常に Explore へ遷移**（Follow/Pinned からの明示 navigate は
  pin を解除する）。履歴は「異なる target への遷移時のみ」push。

## Related Experiments

### Adopted

- なし（実験ログなしで実装）。

### Rejected

- なし。

## Validation

<!-- canonical = mise。merge origin/main 後に実施予定の結果を記録。 -->

## Risks

- GPUI ViewSpec/Binding への inspector mode・control edge gesture 配線は
  未実施（headless slice の範囲外、#112/#114 native gate）。
- `FollowSource::View` は link event の publisher view のみ照合する。
  group を跨ぐ follow（異 group の view を source にする）は deliver の
  group 制約上到達不能 — 仕様上の制約として許容。

## Follow-ups

- UI-C5-04（Recording Conflict/Retry/Rebase/Abort）— 後続 slice。
- UI-C5-05（実 OS GUI/Plugin window/high-DPI/IME/a11y）— NotRun。
- Audio Routing edge gesture 自体の固定 semantics 検証 — 別 slice。
