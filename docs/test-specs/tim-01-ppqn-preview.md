---
id: "TSPEC-5nMbVcXzKlJhGfDsApOi"
type: "test-spec"
status: "approved"
title: "TIM-01 PPQN Preview + New Project PPQN — test spec"
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
  implemented-by:
    - "crates/synvelis-app/tests/tim01_ppqn_preview.rs"
    - "crates/synvelis-project/tests/tim_01_ppqn.rs"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim-01-app-ppqn.md"
    - "docs/test-specs/tim-01-configurable-ppqn.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/tim-01-ppqn-preview-design.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-01-ppqn-preview.md"
  verified-by: []
  references:
    - "docs/tasks/tim-01-ppqn-preview-task.md"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN Preview + New Project PPQN — test spec

## Purpose

Issue #125 preview slice の `preview_ppqn` 自由関数と
`syn.project.preview_ppqn`/`syn.project.new.ppqn` が spec の
受入基準を満たすことを、
`crates/synvelis-project/tests/tim_01_ppqn.rs`（unit 追記）と
`crates/synvelis-app/tests/tim01_ppqn_preview.rs`（headless
`UiRuntime::dispatch_op` 経路）で固定する。mutating Command
契約の網羅は `docs/test-specs/tim-01-configurable-ppqn.md` /
`docs/test-specs/tim-01-app-ppqn.md` が担う。製品 UI・実
OS/実 GUI・製品 Oracle は対象外のため、本 spec が退役するのは
preview/new-ppqn が上位 slice に統合された時点。

## Feature

`preview_ppqn` が `&Project` を変更せずに PPQN 変更の影響
（サイト件数・inexact 位置・overflow・policy 別可否・
要求 policy の applies/rejection）を報告し、
`syn.project.preview_ppqn` が read-only op としてそれを wire
公開し、`syn.project.new` が optional `ppqn` で初期 PPQN を
選択可能にする（省略時 960、範囲検証は project 層委譲）。

## Rules

- `preview_ppqn` は `&Project` のみを受け、ppqn・tempo map・
  musical 位置・revision を一切変更しない。
- サイト列挙は `reproject_ppqn` と同一経路
  （`reproject_ppqn_with`）を共有し、報告は実コマンドが
  触れるサイト集合と一致する。
- inexact site は Exact で `projected: null`・Round で
  宣言 rounding の投影 tick を持つ。overflow site は全
  policy で失敗し `projected: null`。
- `syn.project.preview_ppqn` は session gate
  （transport/recording/automation）を要求せず、params 不正・
  `new_ppqn` 範囲外は `invalid_params`。
- `syn.project.new` の `ppqn` は省略/null で 960 既定、
  `1..=32767` 外は project 層 `SetProjectPpqn` 検証で拒否
  （wire 形状不正のみ service 層 `invalid_params`）。

## Examples

- `preview_ppqn_reports_sites_and_confirms_exact_change`:
  Given seeded project（11 サイト）、When `preview_ppqn(480,
  Exact)`、Then `site_count == 11`・inexact/overflow 空・
  `exact_ok`/`round_ok`/`applies` true・project clone 不変。
- `preview_ppqn_lists_inexact_sites_and_policy_feasibility`:
  Given anchor tick 1 の project、When `preview_ppqn(1000,
  Exact)`、Then inexact に `clip_anchor` site（tick 1・
  projected null）・`applies` false・rejection あり。
  `Round(Floor)` では `projected == 1`・`applies` true。
- `preview_ppqn_reports_overflow_sites`: Given 10^15 beats の
  tempo event、When `preview_ppqn(32767, Exact)`、Then
  `overflow_sites` に `tempo_event`・`round_ok` false・
  `applies` false。480 では feasible。
- `preview_ppqn_rejects_out_of_range_ppqn`: `new_ppqn`
  0/32768/u32::MAX は Err。
- `preview_ppqn_identity_reports_all_sites_exact`: identity
  preview は全サイト exact・`applies` true。
- `rt_ppqn_beat_to_frame_rounding_reproduces`: musical clip の
  `resolve_span` が PPQN 変更前後で同一 frame endpoint を
  返し、preview の `applies` がコマンド結果と一致。
- `preview_ppqn_reports_without_mutating`（app）: dispatch
  経路で report 返却・project JSON/revision/engine builds
  不変。
- `preview_ppqn_ignores_session_gates`（app）: transport
  playing + record_enabled で `set_ppqn` は拒否されるが
  preview は回答する。
- `preview_ppqn_reports_inexact_sites_and_policy_feasibility`
  （app）: tempo event tick 1 で exact は `applies` false・
  inexact 報告、`round`+`floor` は `projected == 1` で
  `applies` true。
- `preview_ppqn_rejects_malformed_params`（app）: param 欠落・
  未知 policy/rounding・`new_ppqn` 0/32768/非数値は
  `invalid_params`。
- `project_new_ppqn_defaults_and_explicit_selection`（app）:
  ppqn 省略 → 960、明示 960 → 960 で revision churn なし、
  明示 480 → 480 で save/reload 後も保持。
- `project_new_rejects_bad_ppqn`（app）: wire 形状不正
  （string・u32 超過）は `invalid_params`、domain 範囲外
  （0・32768）は project 層委譲で `service_error`。

## Guarantees

- preview は live Project・revision・engine notification を
  変更しない（read-only 契約が構造で保証される）。
- 報告内容は `reproject_ppqn` と同一サイト列挙に基づき、
  `applies`/`rejection` は実コマンド経路の clone dry-run が
  権威。
- `syn.project.new` の ppqn domain 検証は project 層
  authoritative であり、app 層は wire 形状のみ検査する。

## Non-goals

- PPQN 選択 UI・Preview/Confirm ダイアログ・Snap/Grid
  追従ポリシー・SMF export 連動。
- Presentation Driver 時刻正本移動・実 OS/実 GUI・製品 Oracle。
- `synvelis-workspace`/`synvelis-gateway`/`synvelis-player`/
  `synvelis-library`/`controller.rs`/`store.rs` の変更。

## Risk

- 分析 pass の inexact/overflow 分類と要求 policy の実結果が
  乖離する経路（i64 境界の個別 rounding）は `applies` を権威
  として分離済み — fixture は通常 musical 値で検証し、境界差は
  design に明記。
- `project_new` の ppqn 適用順序（bundle 作成前）を誤ると
  stray directory が残る — fixture で拒否後の project 状態を
  確認。
