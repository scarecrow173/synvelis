---
id: "TSPEC-sv511fG7hI9jK1lM3nO5pQ7r"
type: "test-spec"
status: "approved"
title: "Issue #511 test spec — CB-07 materialization の検証保証"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/511"
    - "docs/specs/issue-511-cb07-materialization.md"
    - "docs/designs/issue-511-cb07-materialization.md"
    - "docs/adr/issue-511-cb07-materialization.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-511-cb07-materialization.md"
  derived-by:
    - "docs/tasks/issue-511-cb07-materialization.md"
  verifies:
    - "docs/specs/issue-511-cb07-materialization.md"
  verified-by: []
  references:
    - "evidence/sv511/"
  defers: []
  deferred-by: []
---

# Issue #511 test spec — CB-07 materialization

## Feature: scope-removal adopt/render materialization

- Rule R1: `RemoveClip`/`RemoveNote`/`RemoveClipsInRange`/`SplitClip`
  で scope-owned entity が消えた candidate は、adopt で
  `ChangeKind::Removed` write として materialize される。
  - Example: `cb_arr_11_delete_candidate_adopts_and_renders_
    materialized` — RemoveNote+RemoveClip candidate が
    `AdoptOutcome::Applied`・`objects == 2`・revision +1・
    undo_steps 1 で commit され、宣言範囲外の clip/note/lane
    membership が不変。undo 1 step で live が完全復元。
  - Example: `cb_arr_08_mask_gating_clips_only_and_notes_only` —
    notes-only scope の RemoveNote+AddNote candidate が
    `objects == 2`（Removed note + Added note）で adopt され、
    parent clip は不変。
- Rule R2: render 解決は adopt と同一 plan を再利用し、
  削除を含む candidate を半 materialize せず composed view と
  して返す。
  - Example: cb_arr_11 — `resolve_render_project` が composed
    project から clip_a_in/note_in を除き note_foreign/
    clip_a_edge を保持。
- Rule R3: Clip 削除の cascade は `ProjectOp::RemoveClip` と同一
  semantics — lane membership 除去 + clip-local notes 削除。
  clips-only mask でも note は親を失えない（参照整合）。
  - Example: `cb_arr_16_sweep_delete_adopts_and_cascades_notes` —
    clips-only scope で `RemoveClipsInRange` が clip_a_in のみを
    削除し、scope 非所有の note_in も cascade で除去、
    `objects == 1`（clip のみが scope-owned removal）。
- Rule R4: move-out（entity が candidate に実在し scope のみ
  脱退）は引き続き `Unsupported`。非対応 kind の削除も
  `Unsupported`。
  - Example: cb_arr_09（move-out/sweep containment の edit-time
    拒否）が非回帰で pin。

## Feature: SetProjectPpqn × candidate payload

- Rule R5: いずれかの variant set の branch が
  `BranchContentRef::Captured` を保持する間、`SetProjectPpqn` は
  `Error::Validation` で typed 拒否され、live は不変。
  - Example: `cb_arr_14_ppqn_rejects_captured_payloads_then_
    reprojects_scope_time` — fork 直後（candidate = Captured）
    の SetProjectPpqn が拒否され ppqn/scope/live_revision 不変、
    `discard_branch` で全 branch が Live になった後は
    scope `time` が新 grid へ再射影され boundary inventory が
    安定。

## Feature: absolute anchor tempo-map policy

- Rule R6: absolute anchor/extent clip は musical scope 非所有の
  まま — `absolute_anchor: Unsupported` capability と
  `AbsoluteAnchor` boundary reason を維持し、tempo-map 変換
  policy 未確立であることを docs/ADR に記録。
  - Example: `cb_arr_04_boundary_inventory_reports_non_owned_
    lane_clips`・`cb_arr_05_absolute_anchor_reported_and_render_
    pin_identity` が非回帰で pin（本 slice で無変更）。

## Slice 横断不変条件

- Rule R7: cb01〜cb11 系既存 suite に回帰がない
  （`mise run test synvelis-project`）。
- Rule R8: headless fixture lane のみを Pass 判定とし、実 OS/実
  Plugin/実 Render/実 GUI lane は NotRun — `evidence/sv511` に
  lane 別記録。
