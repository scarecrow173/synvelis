---
id: "IMPL-8qWeRtYuIoPaSdFgHjKl"
type: "impl"
status: "completed"
title: "TIM-01 PPQN Preview op + New Project PPQN — preview_ppqn + syn.project.preview_ppqn / syn.project.new.ppqn"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/tim01_ppqn_preview.rs"
      - type: "file"
        path: "docs/specs/tim-01-ppqn-preview.md"
      - type: "file"
        path: "docs/designs/tim-01-ppqn-preview-design.md"
      - type: "file"
        path: "docs/plans/tim-01-ppqn-preview-plan.md"
      - type: "file"
        path: "docs/tasks/tim-01-ppqn-preview-task.md"
      - type: "file"
        path: "docs/test-specs/tim-01-ppqn-preview.md"
      - type: "file"
        path: "docs/impl/ir/tim-01-ppqn-preview.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/ppqn.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tim_01_ppqn.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/binding.rs"
  implements:
    - "docs/tasks/tim-01-ppqn-preview-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim-01-ppqn-preview-task.md"
    - "docs/impl/ir/tim-01-app-ppqn-slice.md"
    - "docs/impl/ir/tim-01-configurable-ppqn.md"
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
  references:
    - "docs/specs/tim-01-ppqn-preview.md"
    - "docs/designs/tim-01-ppqn-preview-design.md"
    - "docs/test-specs/tim-01-ppqn-preview.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-01 PPQN Preview op + New Project PPQN

## Summary

Issue #125 の headless 残件 slice を実装した。
`synvelis-project` に非破壊 dry-run `preview_ppqn(&Project,
new_ppqn, policy) -> PpqnChangePreview` を `ppqn.rs` に追加し、
変換対象サイト列挙を `reproject_ppqn_with` に共通化して
mutating command と報告の乖離を構造的に排除した。
`synvelis-app` に read-only op `syn.project.preview_ppqn`
（session gate なし・`session.project()` 経由）と
`syn.project.new` の optional `ppqn` param（domain 検証は
`SetProjectPpqn` 経路に委譲、既定 960）を追加し、
project 層 preview unit 5 件 + `rt_` fixture 1 件・app 層
headless fixture 6 件で証明した。

## Context

`docs/specs/tim-01-ppqn-preview.md` /
`docs/designs/tim-01-ppqn-preview-design.md` /
`docs/plans/tim-01-ppqn-preview-plan.md` /
`docs/tasks/tim-01-ppqn-preview-task.md` /
`docs/test-specs/tim-01-ppqn-preview.md` に従う。
TIM-01 の残件 — Issue 受入条件の「変更前 Preview
（Snap/Grid/SMF export への影響）」と「New Project PPQN
指定」— を headless 範囲で実装する。UI widget・Snap/Grid
追従ポリシー・Presentation Driver 時刻正本移動・実機/製品
Oracle は対象外（NotRun 継続）。

## Implementation

### Changes

- `crates/synvelis-project/src/ppqn.rs`: `PpqnSite`
  （crate-private サイト識別子 kind/instance/part）と
  `TickConversion` callback 型を導入し、`reproject_ppqn` の
  サイト走査を `reproject_ppqn_with(project, convert)` に抽出。
  `reproject_ppqn` は policy 駆動 `reproject_tick` を渡す
  wrapper に縮小（ppqn フィールド更新は wrapper に残す）。
  `PpqnPreviewSite`/`PpqnChangePreview`/`preview_ppqn` を追加
  — 分析 pass（identity 変換で全サイトを floor/ceil 評価し
  exact/inexact/overflow に分類）+ 権威 pass（要求 policy で
  `reproject_ppqn` を clone dry-run → `applies`/`rejection`）。
- `crates/synvelis-project/src/lib.rs`: `PpqnChangePreview`/
  `PpqnPreviewSite`/`preview_ppqn` を re-export。
- `crates/synvelis-project/tests/tim_01_ppqn.rs`: preview unit
  5 件（site 列挙・inexact 報告と policy 可否・overflow・
  範囲外拒否・identity/非破壊）+ `rt_ppqn_beat_to_frame_
  rounding_reproduces`（Beat→Frame 解決が PPQN 変更前後で
  同一 frame endpoint・preview `applies` とコマンド一致）。
- `crates/synvelis-app/src/app.rs`: `project_preview_ppqn`
  （`ppqn_param`/`ppqn_policy` 再利用・`session.project()` の
  `&Project` に `preview_ppqn` 適用・session gate なし）と
  `ppqn_preview_json`/`ppqn_site_json`/`ppqn_policy_name`/
  `rounding_name` を追加。`project_new_with_ppqn(path, name,
  ppqn)` を追加し `project_new` は `None` 委譲の後方互換
  wrapper。ppqn は bundle 作成前に `session.apply(
  SetProjectPpqn{Exact})` で適用 — 範囲検証は project 層
  authoritative、拒否時に stray directory を残さない。
- `crates/synvelis-app/src/service.rs`: `OPS` に
  `syn.project.preview_ppqn` 追加、`syn.project.new` dispatch
  で `optional_u32_param("ppqn")` を解析、
  `project_new_with_ppqn` へ委譲。
- `crates/synvelis-app/src/binding.rs`: `project_new` に
  `ppqn: Option<u32>` を追加（`json!` null 許容で emit 形状
  維持）。
- `crates/synvelis-app/tests/tim01_ppqn_preview.rs`（新規）:
  6 tests。`UiRuntime` + 共有 `AppService` fixture
  （`tim01_ppqn_app.rs` と同型）で実 `syn.*` dispatch を駆動。

### Decisions During Implementation

- サイト列挙の共通化を `reproject_ppqn_with(project,
  &mut dyn FnMut(&PpqnSite, i64) -> Result<i64, Error>)` の
  変換コールタックに決定 — mutating command は
  `reproject_tick` を、preview は identity+分析評価を渡す。
  `PpqnSite`（kind/instance/part）は `pub(crate)` で
  `Display` により既存 rejection message 形式を維持しつつ
  site 識別を付与。
- `preview_ppqn` は 2 pass 構造: 分析 pass でサイト分類
  （exact/inexact/overflow・`projected` は要求 policy の
  rounding 結果）、権威 pass で実 `reproject_ppqn` を clone
  dry-run し `applies`/`rejection` を確定 — 報告と実
  コマンドの乖離を構造的に排除。
- `overflow_sites` は floor/ceil envelope で保守的に分類。
  i64 境界で個別 rounding が成功し得る理論上の差は
  `applies`（権威）との分離で文書化済み。
- `PpqnChangePreview.effects` は informational string
  （Snap/Grid 既定量子 960 tick の musical 意味変化・
  SMF export division は明示 option で同一 1..=32767 範囲）
  に留め、structural 契約は site/feasibility のみ。
- `syn.project.new` の ppqn は `ppqn != 960` のときのみ
  `SetProjectPpqn{Exact}` を適用 — 明示 960 は identity で
  revision churn なし。fresh project は空で変換は常に exact
  のため `Round` param は不要。
- domain 範囲外の `syn.project.new.ppqn` は
  `AppError::store` → `service_error` で surface
  （project 層 `Error::Validation` を既存 store error 経路で
  通知、message に理由を保持）。wire 形状不正のみ
  `invalid_params`。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

- `cargo test -p synvelis-project --test tim_01_ppqn` — 20
  passed（既存 14 + preview 5 + rt_ 1。補助証拠。canonical
  は `mise run test`）
- `cargo test -p synvelis-app --test tim01_ppqn_preview` — 6
  passed（補助証拠。canonical は `mise run test`）
- `mise run` 全検証 lane の結果は worker 報告に記録
  （fmt-check/check/lint/test/test-rt/check-app-native/
  lint-app-native/test-msrv/build）
- OS/toolchain: Windows（worktree `sv125-tim01-preview`）、
  rust ツールチェーンは mise 管理（pin 1.99.0 / MSRV 1.89.0）
- NotRun: PPQN 選択 UI widget・Preview/Confirm、Snap/Grid
  追従ポリシー、SMF export division 連動、Presentation
  Driver 時刻正本移動、実 OS/実 GUI・製品 Oracle（本 slice
  対象外）

## Risks

- `reproject_ppqn_with` への refactor は全サイト走査を
  callback 化したのみで、既存 `tim_01_ppqn.rs` 14 件の green
  で回帰なしを確認済み。
- preview の clone dry-run は project サイズに比例するが
  non-RT 境界の read-only op であり許容範囲。
- `effects` の informational string は将来の Snap/Grid/SMF
  変更で陳腐化し得る — structural 契約に含めない判断を
  design に記録済み。

## Follow-ups

- PPQN 選択 UI widget・Preview/Confirm ダイアログ（本 op の
  wire 契約を消費、別 slice）。
- Snap/Grid 既定量子の PPQN 追従ポリシー・SMF export
  division 連動（別 slice）。
- Presentation Driver 時刻正本移動・実機/製品 Oracle 検証
  （別 slice、NotRun 継続）。
