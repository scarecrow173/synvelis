---
id: "TASK-034VmO0u2fPRBj8uO9cGkk"
type: "task"
status: "in-progress"
title: "RCD-04 Task1 product E2E"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-04-product-e2e-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-04-product-e2e-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-04-product-e2e.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 Task1 product E2E

## Work

spec `rcd-04-product-e2e` / design `rcd-04-product-e2e-design`
/ plan `rcd-04-product-e2e-plan` に従い、app 層の product
E2E を `crates/synvelis-app/tests/rcd_04_product_e2e.rs`
単一 file・5 scenario で実装する。`UiRuntime` +
`AppService` 共有 harness で `syn.*` 文字列 dispatch を
駆動し、Comp Transition / MIDI Phrase / Recovery Review /
Recovery guards / Stale+read-only を headless で検証する。
fixture 構築は `AppService::app_mut()` が返す同一
`SynvelisApp` の canonical `ProjectSession::apply` 経路を
使い、第二の project/recording/comp モデルを作らない。
実 Window/OS GUI/device/OS FS/RT sample/AudioLens f64 は
NotRun/Unsupported として記録する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-04-product-e2e-plan.md`
- `docs/specs/rcd-04-product-e2e.md`
- `docs/designs/rcd-04-product-e2e-design.md`

## Dependencies

- crate 層 fixture slice（PR #219 `9798be5b`、main 統合
  済み）: `compile_midi_comp`・comp CAS・MIDI phrase・
  Take/Comp PCM・Recovery fixture。
- `synvelis-app` 既存 op 面: `syn.project.*`・
  `syn.recording.info`・`syn.vcs.*`・`syn.view.*`・
  `syn.control.*`。

## Affected Files

- `crates/synvelis-app/tests/rcd_04_product_e2e.rs`（新規）
- `docs/specs/rcd-04-product-e2e.md`（新規）
- `docs/designs/rcd-04-product-e2e-design.md`（新規）
- `docs/plans/rcd-04-product-e2e-plan.md`（新規）
- `docs/tasks/rcd-04-task1-product-e2e.md`（本 file）
- `docs/test-specs/rcd-04-product-e2e.md`（新規）
- `docs/impl/ir/rcd-04-product-e2e-slice.md`（新規）
- docs 各ディレクトリ `README.md` index（generator 更新）

## Done When

- [x] 5 scenario が `cargo test -p synvelis-app --test
  rcd_04_product_e2e` で pass する。
- [x] `syn.*` 文字列 dispatch 経路のみを検証対象とし、
  第二モデル・第二 dispatch を作っていない。
- [x] Comp/MIDI/Recovery の stable ID・CAS・undo/redo・
  save/reload・VCS diff・read-only・fail-closed が
  wire JSON 面で観測できる。
- [x] 実 GUI/device/OS/RT/AudioLens は NotRun/
  Unsupported として文書化されている。
- [ ] canonical mise 検証（fmt-check/check/lint/test/
  test-rt/test-msrv/check-app-native/lint-app-native）
  が green。

## Verification

- [x] `cargo test -p synvelis-app --test
  rcd_04_product_e2e` — 5 tests pass（補助）。
- [x] `cargo clippy -p synvelis-app --all-targets` —
  warning 0（補助）。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `check-app-native` /
  `lint-app-native`（canonical）。
