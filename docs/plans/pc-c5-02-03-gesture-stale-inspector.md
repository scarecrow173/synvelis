---
id: "PLAN-034UxXVFIkzqs2eO4wcXo0"
type: "plan"
status: "approved"
title: "PC-C5 slice2 実施計画 — Gesture 固定・stale drop・Inspector mode（UI-C5-02/03・headless）"
created: "2026-11-09"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c5-02-03-gesture-stale-inspector.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c5-02-03-gesture-stale-inspector-design.md"
    - "docs/tasks/pc-c5-02-03-gesture-stale-inspector.md"
    - "docs/test-specs/pc-c5-02-03-gesture-stale-inspector.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c5-02-03-gesture-stale-inspector.md"
    - "docs/designs/pc-c5-02-03-gesture-stale-inspector-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c5-02-03-gesture-stale-inspector.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice2 実施計画

## 前提確認

- `InspectionMode`/`InspectionFocus`/`LinkGroupManager::deliver` は
  `synvelis-workspace`（ws-a 由来）に存在し、`InspectionMode` は
  Follow/Pinned のみ。`object_selections`（per-domain editing selection）
  は inspect 経路と型レベルで分離済み。
- `ProjectSession::apply(op, expected_revision)` は CAS 付きで
  `Error::StaleRevision` を返し、revision/inbox/undo を原子的に更新する
  （synvelis-project は変更せず消費のみ）。
- Audio Routing gesture は `gesture.rs`/`edit.rs`/`tool.rs` の
  `SemanticTarget` 系で独立しており、Control Layer edge とは型を分ける。
- `ControlsUi` の parameter gesture は generation 照合済みの既存機構で、
  本 slice では再利用も変更もしない。

## Steps

1. `state.rs`: `InspectionMode::Explore`・`FollowSource`・
   `InspectionFocus.follow_source`/`explore_back`/`explore_fwd`・
   `ViewState.control_edge_gesture` を追加（session-only field は
   serde skip、preset 非永続化方針を維持）。
2. `control_edge.rs` を新規作成: `ControlEdgeFixation`（view・typed
   両 endpoint・`ProjectRevision`・window epoch）・`ControlEdgeGesture`・
   `ControlEdgePreview`・`ControlEdgeOutcome`（`Applied`/`Cancelled`/
   `Stale`）。
3. `workspace.rs`: inspector API（`inspect`/`set_inspection_mode`/
   `set_follow_source`/`explore_back`/`explore_forward`）と control edge
   API（`control_edge_begin`/`preview`/`commit`/`cancel`）を追加。
   commit は window epoch（`dock.owner_of` + `is_transferring`）と
   fixated revision の CAS で stale を `Stale` として drop し、成功時のみ
   `session.apply(AddModulationAssignment)` → `distribute_revision`。
   `set_input_context` の gesture 破棄対象に control edge を追加。
4. `link.rs`: `deliver` が Follow mode かつ `follow_source` 一致時のみ
   `inspection_focus.target` を更新するよう拡張。
5. `preset.rs`/`lib.rs`: `InspectionFocus` 再構築の新 field 追従と
   `control_edge` module・型の re-export。
6. `tests/uic5.rs` を新規作成し、UI-C5-02/03 を網羅する 12 fixture を実装。
7. `mise run fmt-check` → `check` → `lint` → `test` を実行し、失敗は
   実修正で潰す。`git fetch origin main && git merge origin/main` の後、
   最終検証を再実行。docs index を再生成。
8. impl record（`docs/impl/ir/`）を作成し、task status を `done` に更新。
   論理単位で commit → `feature/102-uic5-gesture-stale` を push →
   PR 作成（`Refs #102`、Close しない）。

## Verification

| 項目 | 方法 |
| --- | --- |
| fmt | `mise run fmt-check` |
| 型 | `mise run check` |
| lint | `mise run lint`（`-D warnings`） |
| test | `mise run test`（`uic5.rs` を含む workspace 全体） |
| 実 GUI/OS | **NotRun**（UI-C5-05/native gate、別 Issue） |

## Risks

- GPUI ViewSpec/Binding への inspector mode・control edge gesture 配線は
  未実施（headless slice の範囲外、#112/#114 native gate）。
- `FollowSource::View` は link event の publisher view のみ照合する。
  group を跨ぐ follow は deliver の group 制約上到達不能 — 仕様上の
  制約として許容。
- `follow_source: None` は従来挙動（view の属する group の全 event に
  追従）を維持する後方互換設計。既存 ws_a fixture が依存。

## Sources and Relations

- [spec](../specs/pc-c5-02-03-gesture-stale-inspector.md) /
  [design](../designs/pc-c5-02-03-gesture-stale-inspector-design.md) /
  [task](../tasks/pc-c5-02-03-gesture-stale-inspector.md) /
  [test-spec](../test-specs/pc-c5-02-03-gesture-stale-inspector.md)
