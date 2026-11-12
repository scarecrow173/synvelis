---
id: "TASK-Km2wQp9xZr5vNt8yLc4sDh"
type: "task"
status: "in-progress"
title: "SV-GUI-03 slice5 タスク分解 — GPUI-simulated scenario lane"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv-gui-03-05-gpui-scenario-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv-gui-03-05-gpui-scenario-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
    - "docs/designs/sv-gui-03-05-gpui-scenario-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-05-gpui-scenario.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice5 タスク

- [x] 前提確認: ruviform pin rev `fd1d4d5` に `gpui-test` feature・
      `GpuiGuiTest`/`GpuiObserver`/`ViewMount`/`create_view_window`
      が存在(pen 更新不要)
- [x] `crates/synvelis-app/Cargo.toml`: `gpui-test` feature 追加
      (`native` + `ruviform-ui-gpui/gpui-test`)
- [x] `tests/common/mod.rs` + `tests/common/scenario.rs`: 新規
      (driver-neutral scenario・product oracle・honesty helper・
      `ScenarioReport` 骨格)
- [x] `tests/screen_scenario.rs`: 共通 helper 利用へリファクタ
      (Fake lane 維持 + fader/select/tab/secondary/honesty 追加)
- [x] `tests/scenario_gpui.rs`: 新規(`gpui-test` gate、
      `ViewHost`+`create_view_window` mount)
- [x] `mise.toml`: `check-app-gpui`/`lint-app-gpui`/
      `test-app-gpui` 追加
- [x] docs chain 作成(spec/design/plan/task/test-spec/ir/ADR)
- [ ] 検証一式実走(fmt-check/check/lint/test/check-app-gpui/
      lint-app-gpui/test-app-gpui/check-app-native/lint-app-native/
      test-msrv/build)
- [ ] Conventional Commits → push → PR(日本語 `Refs #112`)→
      Issue #112 コメント → worker_done

## 境界メモ

- 編集範囲は `Cargo.toml`・`tests/`(common/scenario_gpui/
  screen_scenario)・`mise.toml`・`docs/` のみ。
- `crates/synvelis-app/src/`・`synvelis-workspace`・
  `synvelis-theme`・`Cargo.lock`・docs index は変更しない。
- 製品側に変更が必要になった場合は変更せず coordinator へ
  question で問い合わせる。
- `mise.toml` は共有ファイル — PR に対象・理由・rollback を明記。
