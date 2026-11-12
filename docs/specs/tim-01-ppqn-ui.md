---
id: "SPEC-TiM01uIxYzAbCdEfGh"
type: "spec"
status: "approved"
title: "TIM-01 PPQN 選択 UI read model + Snap/Grid ポリシー — synvelis#125 UI slice（headless）"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/tim-01-ppqn-preview.md"
    - "docs/specs/tim-01-app-ppqn.md"
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-01-ppqn-preview.md"
    - "docs/specs/tim-01-app-ppqn.md"
    - "docs/designs/tim-01-ppqn-preview-design.md"
    - "docs/plans/tim-01-ppqn-preview-plan.md"
    - "docs/tasks/tim-01-ppqn-preview-task.md"
    - "docs/test-specs/tim-01-ppqn-preview.md"
    - "docs/impl/ir/tim-01-ppqn-preview.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  derived-by:
    - "docs/designs/tim-01-ppqn-ui-design.md"
    - "docs/plans/tim-01-ppqn-ui-plan.md"
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-ui.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN 選択 UI read model + Snap/Grid ポリシー（synvelis#125 UI slice・headless）

## 背景・目的

Synvelis Issue #125 の UI slice。先行 slice で headless 契約は
merge 済み — `ProjectOp::SetProjectPpqn`（二重 CAS・rational
beat 再投影）、`syn.project.set_ppqn`、非破壊
`syn.project.preview_ppqn`、`syn.project.new` の optional
`ppqn`（既定 960）。Issue 受入条件（L34）の UI 契約は
「New Project 既定 960・選択可能範囲表示・変更前 Preview
（Snap/Grid/SMF export への影響）・Explicit Confirm/Cancel・
Undo。UI は SV-GUI-02/03 を消費し別時間モデルを持たない」。

本 slice はその UI 契約を **headless/ViewSpec レベル**で実装する:

1. workspace に PPQN 選択 read model（`PpqnUi`）— live PPQN・
   選択可能範囲 `1..=32767`・推奨値・preview 投影・
   select→preview→confirm/cancel の gesture lifecycle。
2. Snap/Grid の `SnapUnit::Explicit` 量子に対する PPQN 変更
   ポリシーを決定し実装（追従 or explicit 維持）。
3. `synvelis-app` に Project tab（ViewSpec）を追加し、
   gesture を既存 `syn.project.*` op 経路に配線する。

## 要求

- `crates/synvelis-workspace/src/ppqn_ui.rs`（新規）に view
  所有の read model `PpqnUi` を実装する。責務は gesture
  状態機械と表示投影のみ — 時刻変換・サイト列挙・可否判定は
  `synvelis-project` の `preview_ppqn`/`SetProjectPpqn` が
  authoritative であり、第二の時間モデルを持たない。
- read model は current PPQN・選択可能範囲 `1..=32767`・
  推奨値（480/960/1000/1920）・`PpqnChangePreview` の表示
  投影（`PpqnPreviewView`、サイトは `PREVIEW_SITE_CAP` で
  bounded）・pending state を提供する。
- gesture lifecycle: `select`（範囲検証・pending 確立）→
  `preview_params`/`record_preview`（`syn.project.preview_ppqn`
  結果の記録、candidate/live-PPQN 不一致は拒否）→
  `confirm_request`（`preview.applies` のときのみ
  `syn.project.set_ppqn` params を mint、preview 時
  revision を `expected_revision` CAS として同梱）/
  `cancel`（非破壊）。`observe_current`/`committed` で外部
  変化（undo/redo/open/new/別 view）を再同期する。
- Snap/Grid ポリシー: `SnapUnit::Explicit` は gesture domain
  の基底単位（project tick）で固定された量子であり、PPQN
  変更を跨いで **tick 数を verbatim 維持**する — 変わるのは
  beat 間隔のみであり、`snap_unit_ppqn_effect` が preview
  表示用に定量化する。`SnapUnit::Auto` は zoom 由来で固定
  量子を持たない。根拠は design に記録する。
- `synvelis-app` の `main_screen_spec` に Project tab を追加
  し、`PpqnScreenState`（current・pending candidate・
  revision-guarded preview）を `publish_project_length`
  経由で投影する。spec は表示専用 — gesture 駆動は
  `SynvelisApp::ppqn_{select,preview_pending,confirm,cancel}`
  helper と `ViewBinding` の `project_{preview_ppqn,set_ppqn,
  undo,redo}` op mint で行い、fake parameterized widget
  action は導入しない。
- gesture helper は既存 `syn.project.preview_ppqn`/
  `set_ppqn` op を内部で呼び、preview 結果を read model に
  feed back する。新規 `syn.*` op は追加しない。
- headless fixture `crates/synvelis-app/tests/
  tim01_ppqn_ui.rs` で read model 投影・confirm→set_ppqn・
  cancel 非破壊・undo 復帰・範囲外 reject・Snap/Grid 効果を
  証明する。

## 受入基準（本 slice）

- `PpqnUi` が select→preview→confirm/cancel を view-local
  状態として完結し、confirm は `preview.applies` かつ
  preview 時 revision CAS を伴う `syn.project.set_ppqn`
  params のみを mint する。cancel は一切 mutate しない。
- app gesture helper が実 op 経路で select→preview→confirm
  を完走し、live PPQN・`PpqnScreenState`・mounted spec
  （Project tab の status/preview/effects/sites）が追従する。
- undo/redo・project new/open・外部 session 変化で pending
  gesture が陳腐化せず再同期される。
- Snap/Grid explicit 量子が PPQN 変更で tick 数を維持し、
  beat 間隔の変化が preview/screen spec に定量表示される。
- 範囲外 select（0・32768）は view 側で reject され、
  project は不変。
- 実 GUI widget intent・実 OS/実機・製品 Oracle・
  Presentation Driver 時刻正本移動は NotRun 継続であり
  Pass 扱いしない。

## 非目標

- `rvf.ui.command.v1` の parameterized widget intent による
  実 GUI confirm/cancel 配線（shell の Action 経路は params
  を運べない — 後続 slice・外部 gate）。
- SMF export 実装の変更（preview/spec は影響情報の報告のみ）。
- `synvelis-project` の PPQN コマンド意味変更。
- Presentation Driver への時刻正本移動。
- `synvelis-workspace/src/lib.rs` の module/export 配置
  （lease 制約により `state.rs` 経由の暫定 mount — 統合時に
  coordinator が top-level `pub mod ppqn_ui` へ移設）。
- `synvelis-gateway`/`synvelis-player`/`synvelis-library` の変更。
- 実 OS/実 GUI・製品 Oracle 検証。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/125
- 上流 slice: `docs/specs/tim-01-ppqn-preview.md`（非破壊
  preview op）、`docs/specs/tim-01-app-ppqn.md`
  （`syn.project.set_ppqn` wire 語彙）、
  `docs/specs/tim-01-configurable-ppqn-project-command.md`
  （headless Command 契約）。
