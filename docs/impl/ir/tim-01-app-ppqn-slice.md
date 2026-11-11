---
id: "IMPL-034Vu50fg7S7yCp4bni412"
type: "impl"
status: "completed"
title: "TIM-01 app 層 PPQN 統合 — syn.project.set_ppqn + 前提条件検証"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/tim01_ppqn_app.rs"
      - type: "file"
        path: "docs/specs/tim-01-app-ppqn.md"
      - type: "file"
        path: "docs/designs/tim-01-app-ppqn-design.md"
      - type: "file"
        path: "docs/plans/tim-01-app-ppqn-plan.md"
      - type: "file"
        path: "docs/tasks/tim-01-task2-app-ppqn.md"
      - type: "file"
        path: "docs/test-specs/tim-01-app-ppqn.md"
      - type: "file"
        path: "docs/impl/ir/tim-01-app-ppqn-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements:
    - "docs/tasks/tim-01-task2-app-ppqn.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim-01-task2-app-ppqn.md"
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
    - "docs/specs/tim-01-app-ppqn.md"
    - "docs/designs/tim-01-app-ppqn-design.md"
    - "docs/test-specs/tim-01-app-ppqn.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-01 app 層 PPQN 統合 — syn.project.set_ppqn + 前提条件検証

## Summary

Issue #125 の app 層 slice を実装した。`synvelis-app` の
`syn.*` service 面に `syn.project.set_ppqn` op を追加し、
wire params（`old_ppqn`/`new_ppqn`/`policy`/`rounding`/
`expected_revision`）を `ProjectOp::SetProjectPpqn` へ写像、
app 層前提条件（transport-stopped・open recording transaction
なし・automation record disabled）を public accessor で
fail-closed に検査する。`project_info` に `ppqn` を投影し、
11 件の headless fixture（`UiRuntime::dispatch_op` 経路）で
wire CAS/stale 拒否・前提条件拒否・undo/redo・save/reload・
engine revision 通知を証明した。project 層の検証ロジックは
一切重複実装していない。

## Context

`docs/specs/tim-01-app-ppqn.md` /
`docs/designs/tim-01-app-ppqn-design.md` /
`docs/plans/tim-01-app-ppqn-plan.md` /
`docs/tasks/tim-01-task2-app-ppqn.md` /
`docs/test-specs/tim-01-app-ppqn.md` に従う。
TIM-01 headless Command slice（`docs/impl/ir/
tim-01-configurable-ppqn.md`、merge 済み）の残件 — 製品経路
から PPQN を変更する op の不在と app 層前提条件の未検証 —
を解消する。

## Implementation

### Changes

- `crates/synvelis-app/src/service.rs`: `OPS` に
  `syn.project.set_ppqn` の `OpInfo` を追加、`dispatch` に
  `self.app.project_set_ppqn(params)` を追加（`syn.project.*`
  は `self.app.*` 呼出の既存パターン）。
- `crates/synvelis-app/src/app.rs`: `SynvelisApp::
  project_set_ppqn` を追加 — params 検証・`PpqnConversionPolicy`
  写像・前提条件 3 件検査・`session.apply` 呼出・
  `project_info()` 返却。`project_info` に `"ppqn"` を追加。
  wire helper（`invalid_params`/`ppqn_param`/`ppqn_policy`）を
  module 末尾に追加。
- `crates/synvelis-app/tests/tim01_ppqn_app.rs`（新規）:
  11 tests。`UiRuntime` + 共有 `AppService` fixture
  （`rcd_04_product_e2e.rs` と同型）で実 `syn.*` dispatch を
  駆動し、fixture 変異は同一 composed app の `app_mut()`
  経由 — test-only op 面・第二 project model なし。

### Decisions During Implementation

- 前提条件の判定順は transport → open recording transaction →
  `record_enabled`。open transaction は `record_start` が
  `record_enabled` を要求するため、`record_enabled` を先に
  検査するとより具体的な merge-commit-in-flight 拒否が
  到達不能になる（design 記載どおり）。
- `session.transport().playing` を transport gate に採用。
  `ControlSession::playback()` は automation/control 側の別
  transport で project mutation を生まないため対象外
  （design の accessor 選定根拠どおり）。
- `record_enabled` は fail-closed で拒否対象に含める —
  transport 停止中は write が生じないが、Issue #125 契約の
  「Automation Writeなし」を保守的に解釈（design Risks
  記載）。
- `expected_revision` 省略時は `session.revision()`
  （= `controller.live_revision()`）を使い、指定時は
  controller CAS が stale を拒否 — 両経路を test で検証。
- `policy` は必須（暗黙 `Exact` default なし）、`round` 時のみ
  `rounding` 必須、`exact`+`rounding` 指定は `invalid_params`
  — `PpqnConversionPolicy` の「never silently quantize」
  doc 契約を wire でも維持。
- 全ての失敗（params 不正・前提条件・project 層拒否）は
  `invalid_params` に写像し、message に理由を保持
  （`syn.monitoring.*` の既存 convention。wire に `busy`/
  `requires_stop` code は存在しない）。
- TDD: fixture を先に書き RED（`unknown_op` 11 件失敗）を
  確認後に実装し GREEN 化。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

- `cargo test -p synvelis-app --test tim01_ppqn_app` — 11 passed
  （補助証拠。canonical は `mise run test`）
- `mise run fmt-check` — exit 0
- `mise run check`（`cargo check --workspace --all-targets`）—
  exit 0
- `mise run lint`（`cargo clippy --workspace --all-targets --
  -D warnings`）— exit 0
- `mise run test`（`cargo test --workspace --locked`）— exit 0
  （3 回目で green。1・2 回目は `synvelis-library` webdav
  fixture が os error 10053 で flaky fail — 当該 crate は本
  slice 無変更、単独再実行 42/42 pass・workspace 再実行で
  green を確認。infra flake として PR に記録）
- `mise run test-rt`（`cargo test --workspace --locked rt_`）—
  exit 0
- `mise run test-msrv`（rust 1.89.0 toolchain で
  `cargo test --workspace --locked`）— exit 0、
  `tim01_ppqn_app` 11 tests 含む
- `mise run check-app-native` — exit 0
- `mise run lint-app-native` — exit 0
- OS/toolchain: Windows（worktree
  `sv125-ppqn`）、rust ツールチェーンは mise 管理
  （pin 1.99.0 / MSRV 1.89.0）
- NotRun: PPQN 選択 UI widget・Preview/Confirm、Presentation
  Driver 時刻正本移動、実 OS/実 GUI・製品 Oracle（本 slice
  対象外）

## Risks

- `record_enabled` 拒否は保守的（fail-closed）。transport
  停止中は write が生じないため実害は限定的だが、緩和は
  spec 改訂を要する後続判断。
- `control.playback` Playing を gate しない判断は design で
  明示済み — automation write は `record_enabled` + open
  transaction で gate 済み、評価は project を変更しない。
- PPQN 選択 UI・製品 Oracle は別 slice。本 op の wire 契約
  （param 名・policy/rounding 語彙）は後続 UI slice が消費する。

## Follow-ups

- PPQN 選択 UI widget・Preview/Confirm（Issue #125 の UI 受入、
  別 slice）。
- Presentation Driver 時刻正本移動・実機/製品 Oracle 検証
  （別 slice、NotRun 継続）。
