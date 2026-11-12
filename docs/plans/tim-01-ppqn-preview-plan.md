---
id: "PLAN-7hJkMnBvCxZqWeRtYuIo"
type: "plan"
status: "approved"
title: "TIM-01 PPQN Preview op + New Project PPQN — implementation plan"
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
    - "docs/specs/tim-01-ppqn-preview.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-01-ppqn-preview.md"
    - "docs/designs/tim-01-ppqn-preview-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-preview.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN Preview op + New Project PPQN — implementation plan

## Goal

Issue #125 の headless 残件として、`syn.project.preview_ppqn`
（非破壊 dry-run op）と `syn.project.new` の optional `ppqn`
param を実装し、project 層 `preview_ppqn` 自由関数・headless
fixture・doc chain で受入条件の「変更前 Preview」と
「New Project PPQN 指定」を headless 範囲で証明する。

## Scope

`synvelis-project` の `ppqn.rs`/`lib.rs`/tests、`synvelis-app`
の `service.rs`/`app.rs`/`binding.rs`/新規 test、docs 6 件に
閉じる。workspace/gateway/player/library/controller/store の
変更・UI widget・実機検証は対象外。

## Upstream Documents

- `docs/specs/tim-01-ppqn-preview.md`

## Design Inputs

- `docs/designs/tim-01-ppqn-preview-design.md`

## File Map

- `crates/synvelis-project/src/ppqn.rs` — `PpqnSite`/
  `TickConversion`/`reproject_ppqn_with` へのサイト列挙共通化、
  `PpqnPreviewSite`/`PpqnChangePreview`/`preview_ppqn` 追加
- `crates/synvelis-project/src/lib.rs` — preview 型・関数の
  re-export
- `crates/synvelis-project/tests/tim_01_ppqn.rs` — preview unit
  6 件 + `rt_ppqn_beat_to_frame_rounding_reproduces`（追記のみ）
- `crates/synvelis-app/src/app.rs` — `project_preview_ppqn`、
  `project_new_with_ppqn`、`ppqn_preview_json` 等の wire helper
- `crates/synvelis-app/src/service.rs` — `OPS`/`dispatch`/
  `optional_u32_param`
- `crates/synvelis-app/src/binding.rs` — `project_new` の
  `ppqn: Option<u32>` 追加
- `crates/synvelis-app/tests/tim01_ppqn_preview.rs` — 新規
  headless fixture 6 件
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  tim-01-ppqn-preview*` — doc chain 6 件（新規）

## Implementation Sequence

- [x] `ppqn.rs` のサイト走査を `reproject_ppqn_with` に抽出し
      `preview_ppqn`/`PpqnChangePreview`/`PpqnPreviewSite` を実装
- [x] `lib.rs` re-export 追加
- [x] `tim_01_ppqn.rs` に preview unit test + `rt_` fixture を
      追記（project 層で green 確認）
- [x] `app.rs`/`service.rs`/`binding.rs` に preview op と
      `project.new.ppqn` を実装
- [x] `tim01_ppqn_preview.rs` に headless fixture を実装
      （green 確認）
- [x] doc chain 6 件を作成
- [x] 全検証 lane 実行

## Dependencies and Risks

- `ProjectSession::project()`（`&Project` accessor）・
  `ProjectSession::apply`・`session.revision()` に依存
  （完備済み・変更不要）。
- `reproject_ppqn` の refactor は振る舞い不変が前提 —
  `tim_01_ppqn.rs` 既存 14 件の green で回帰を担保。
- preview の clone dry-run は project サイズに比例するが、
  non-RT 境界（UI/host thread）の read-only op であり
  許容範囲。

## Task Breakdown

- `docs/tasks/tim-01-ppqn-preview-task.md` — 実装タスク一式

## Verification

- [x] `cargo test -p synvelis-project --test tim_01_ppqn` —
      20 件 green
- [x] `cargo test -p synvelis-app --test tim01_ppqn_preview` —
      6 件 green
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `check-app-native` / `lint-app-native` /
      `test-msrv` / `build` — 結果は impl IR に記録

## Open Questions / Gaps

- Snap/Grid 既定量子の PPQN 追従・PPQN 選択 UI・SMF export
  division 連動・実機/製品 Oracle は別 slice（NotRun）。

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

- `reproject_ppqn_with` への refactor が既存挙動を変えていない
  か、`preview_ppqn` の 2-pass 構造（分析 + 権威 dry-run）が
  妥当か reviewer が確認。
