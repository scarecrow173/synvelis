---
id: "SPEC-9xQpPvK2mN7rT4wE8jH3d"
type: "spec"
status: "approved"
title: "TIM-01 PPQN Preview op + New Project PPQN — syn.project.preview_ppqn / syn.project.new.ppqn（synvelis#125 preview slice）"
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
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/specs/tim-01-app-ppqn.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/specs/tim-01-app-ppqn.md"
    - "docs/designs/tim-01-configurable-ppqn-design.md"
    - "docs/designs/tim-01-app-ppqn-design.md"
    - "docs/plans/tim-01-app-ppqn-plan.md"
    - "docs/tasks/tim-01-task2-app-ppqn.md"
    - "docs/test-specs/tim-01-app-ppqn.md"
    - "docs/impl/ir/tim-01-app-ppqn-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  derived-by:
    - "docs/designs/tim-01-ppqn-preview-design.md"
    - "docs/plans/tim-01-ppqn-preview-plan.md"
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-preview.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN Preview op + New Project PPQN（synvelis#125 preview slice）

## 背景・目的

Synvelis Issue #125 の headless 残件 slice。TIM-01 headless
Command slice（`ProjectOp::SetProjectPpqn`・rational beat 再投影・
二重 CAS）と app 層 `syn.project.set_ppqn` は merge 済みだが、
Issue 受入条件の「変更前 Preview（Snap/Grid/SMF export への影響）」
と「New Project PPQN 指定」が未実装だった。

本 slice は 2 つの headless 契約を追加する:

1. `syn.project.preview_ppqn` — `SetProjectPpqn` の**非破壊
   dry-run**。変換対象件数・inexact になる位置の一覧・overflow
   の有無・policy（Exact/Round）別の可否を返し、Project を
   一切変更しない。read-only op であり session gate を要求しない。
2. `syn.project.new` の optional `ppqn` param — 新規 project の
   初期 PPQN を `1..=32767` で選択可能にする（省略時は既定 960）。
   検証は project 層に委譲し、app 層で範囲ルールを重複実装しない。

## 要求

- `synvelis-project` に `preview_ppqn(&Project, new_ppqn, policy)
  -> Result<PpqnChangePreview, Error>` の**非破壊自由関数**を
  `ppqn.rs` に追加する。controller メソッドにはしない
  （`&Project` を受け、live state を変更しない）。
- preview は変換対象の musical-tick サイト全件を列挙し、
  Exact/Round policy に対する可否・inexact 位置一覧・overflow
  有無を報告する。`reproject_ppqn` と同一のサイト列挙を共有し、
  報告と実コマンドが乖離しない構造にする。
- `PpqnChangePreview`/`PpqnPreviewSite` を `lib.rs` から
  re-export する。
- `syn.project.preview_ppqn` op を `service.rs`/`app.rs` に
  追加する。`new_ppqn`/`policy`/`rounding` の wire 語彙は
  `set_ppqn` と同一。`session.project()` 経由で `&Project` を
  取得し、**read-only**（transport/recording/automation の
  session gate を要求しない）。
- `syn.project.new` に optional `ppqn` param（u64→u32 形状検査は
  service 層、`1..=32767` の domain 検証は project 層の
  `SetProjectPpqn` 経路に委譲）を追加する。`binding.rs` の
  `project_new` は ppqn 指定時のみ emit し、既存 caller を
  破壊しない。
- Snap/Grid 既定量子・SMF export への影響は preview の
  informational 報告に留め、ポリシー変更・UI widget・
  Presentation Driver 時刻正本移動は本 slice の対象外。

## 受入基準（本 slice）

- `preview_ppqn` が Project を変更せず（ppqn・tempo map・
  musical 位置・revision 不変）、サイト件数・inexact 一覧・
  overflow・policy 別可否を正確に報告する。
- `syn.project.preview_ppqn` が headless（`UiRuntime::
  dispatch_op`）で動作し、session gate 状態（transport playing・
  record_enabled）でも回答する。params 不正は `invalid_params`。
- `syn.project.new` の `ppqn` param が明示選択・省略時 960
  既定・範囲外拒否を project 層委譲で実現する。
- headless fixture `crates/synvelis-app/tests/
  tim01_ppqn_preview.rs` と project 層追記
  `crates/synvelis-project/tests/tim_01_ppqn.rs` が全て green。
- UI widget・実機/製品 Oracle 行は NotRun 継続であり Pass
  扱いしない。

## 非目標

- PPQN 選択 UI widget・Preview/Confirm ダイアログの UI 実装。
- Snap/Grid `SnapUnit::Explicit(960)` の PPQN 追従ポリシー
  （UI slice で決定）。
- SMF export 挙動の変更（preview は影響情報の報告のみ）。
- Presentation Driver への時刻正本移動。
- `synvelis-workspace`/`synvelis-gateway`/`synvelis-player`/
  `synvelis-library` の変更、controller/store の変更。
- 実 OS/実 GUI・製品 Oracle 検証。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/125
- 上流 slice: `docs/specs/tim-01-configurable-ppqn-project-command.md`
  （headless Command 契約）、`docs/specs/tim-01-app-ppqn.md`
  （`syn.project.set_ppqn` wire 語彙・前提条件境界）。
