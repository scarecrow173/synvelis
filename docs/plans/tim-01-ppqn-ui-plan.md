---
id: "PLAN-TiM01uIxYzAbCdEfGh3"
type: "plan"
status: "approved"
title: "TIM-01 PPQN 選択 UI + Snap/Grid ポリシー — implementation plan"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim-01-ppqn-ui.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-01-ppqn-ui.md"
    - "docs/designs/tim-01-ppqn-ui-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-ui.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN 選択 UI + Snap/Grid ポリシー — implementation plan

## Goal

Issue #125 の UI 契約を headless/ViewSpec レベルで実装する:
workspace `PpqnUi` read model（select→preview→confirm/cancel）、
Snap/Grid explicit-tick-preservation ポリシー、`synvelis-app`
Project tab と `ppqn_*` gesture helper / `ViewBinding` op mint
配線、headless fixture 11 件、doc chain 6 件。

## Scope

`synvelis-workspace` の `ppqn_ui.rs`（新規）/`snap.rs`/
`state.rs`、`synvelis-app` の `screens.rs`/`app.rs`/`ui_ops.rs`/
`binding.rs`/新規 test、docs 6 件に閉じる。
`synvelis-project`・`synvelis-workspace/src/lib.rs`・
`synvelis-app/src/lib.rs`/`Cargo.toml`・禁止 test・
gateway/player/library の変更は対象外。

## Upstream Documents

- `docs/specs/tim-01-ppqn-ui.md`

## Design Inputs

- `docs/designs/tim-01-ppqn-ui-design.md`

## File Map

- `crates/synvelis-workspace/src/ppqn_ui.rs` — 新規:
  `PpqnUi` gesture 状態機械 + `PpqnPreviewView` 投影 +
  wire-name helper + `RECOMMENDED_PPQN`/`SMF_EXPORT_NOTE`/
  `PREVIEW_SITE_CAP`
- `crates/synvelis-workspace/src/snap.rs` —
  `DEFAULT_SNAP_QUANTUM_TICKS`・`SnapPpqnEffect`・
  `snap_unit_ppqn_effect`（ポリシーの報告半分）
- `crates/synvelis-workspace/src/state.rs` — ViewState
  grid/snap のポリシー明記 + `#[path]` 暫定 module mount
  + re-export（`lib.rs` lease のため）
- `crates/synvelis-app/src/screens.rs` — `PpqnScreenState` +
  Project tab（status/preview/effects/sites、全 read-only）
- `crates/synvelis-app/src/app.rs` — `publish_project_length`
  の `observe_current` + pending preview 投影、
  `project_preview_ppqn` の record feed-back、
  `project_set_ppqn`/new/open の `committed` re-sync、
  `ppqn_preview_json` の policy 定量報告
- `crates/synvelis-app/src/ui_ops.rs` — `UiOpsState.ppqn_ui`
  + `ppqn_{select,preview_pending,confirm,cancel}` helper +
  undo/redo re-sync
- `crates/synvelis-app/src/binding.rs` — `project_undo`/
  `project_redo`/`project_preview_ppqn`/`project_set_ppqn`
  intent mint
- `crates/synvelis-app/tests/tim01_ppqn_ui.rs` — 新規
  headless fixture 11 件
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  tim-01-ppqn-ui*` — doc chain 6 件（新規）

## Implementation Sequence

- [x] `ppqn_ui.rs` read model + `snap.rs` 効果報告 +
      `state.rs` mount（workspace、`cargo check` green）
- [x] `screens.rs` `PpqnScreenState` + Project tab
- [x] `app.rs` publish/preview/set/new/open の gesture 統合
- [x] `ui_ops.rs` `UiOpsState.ppqn_ui` + `ppqn_*` helper +
      undo/redo re-sync
- [x] `binding.rs` op mint 4 件
- [x] `tim01_ppqn_ui.rs` headless fixture 11 件（green）
- [x] doc chain 6 件を作成
- [ ] 全検証 lane 実行

## Dependencies and Risks

- `synvelis-project` の `preview_ppqn`/`PpqnChangePreview`/
  `PpqnPreviewSite`/`ppqn_in_range`/`PPQN_{MIN,MAX}`/
  `PpqnConversionPolicy`（merge 済み・変更不要）に依存。
- `ProjectSession::{project, revision}`・`SynvelisApp::
  {workspace, ui}`・`publish_project_length(&self)` の
  既存 signature に依存 — `&self` publish が re-sync 点に
  なるため `PpqnUi` は `RefCell` で保持。
- `rvf.ui.command.v1` Action が params を運べない制約を
  確認済み — spec は display-only、gesture 駆動は typed
  helper + ViewBinding mint に限定（fake 回避）。
- `synvelis-workspace/src/lib.rs` は別 worker lease —
  `#[path]` 暫定 mount で compile を通し、統合時に
  coordinator が top-level module へ移設（文書化済み）。

## Task Breakdown

- `docs/tasks/tim-01-ppqn-ui-task.md` — 実装タスク一式

## Verification

- [x] `cargo check -p synvelis-workspace -p synvelis-app
      --all-targets` — green
- [x] `cargo test -p synvelis-app --test tim01_ppqn_ui` —
      11 件 green
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `check-app-native` / `lint-app-native` /
      `test-msrv` / `build` — 全 pass、結果は impl IR に記録

## Open Questions / Gaps

- 実 GUI widget intent（parameterized confirm/cancel）—
  shell Action 経路の params 拡張が必要、後続 slice・
  外部 gate（NotRun）。
- `synvelis-workspace/src/lib.rs`・`synvelis-app/src/lib.rs`
  の export 整理 — coordinator 統合作業。
- SMF export division 連動・Presentation Driver 時刻正本
  移動・実機/製品 Oracle — 別 slice（NotRun）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（dispatch worker として
  実行済み）
- Independent work streams: none
- User approval required before dispatch: n/a（worker spec で
  直接割当）

## Review Handoff

- Snap/Grid tick-preservation 決定の妥当性（design の根拠
  記述）と、`#[path]` 暫定 mount → lib.rs 移設の統合
  handoff を reviewer/coordinator が確認。
