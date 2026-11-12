---
id: "IMPL-Gh7uIo2pAs5dFg9hJk3lZx"
type: "impl"
status: "in-progress"
title: "SV-GUI-03 slice5: GPUI-simulated scenario lane(Fake/GPUI parity)"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added:
      - type: "file"
        path: "docs/specs/sv-gui-03-05-gpui-scenario.md"
      - type: "file"
        path: "docs/designs/sv-gui-03-05-gpui-scenario-design.md"
      - type: "file"
        path: "docs/plans/sv-gui-03-05-gpui-scenario-plan.md"
      - type: "file"
        path: "docs/tasks/sv-gui-03-05-gpui-scenario.md"
      - type: "file"
        path: "docs/test-specs/sv-gui-03-05-gpui-scenario.md"
      - type: "file"
        path: "docs/adr/sv-gui-03-05-gpui-scenario.md"
      - type: "file"
        path: "crates/synvelis-app/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/common/scenario.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/scenario_gpui.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-app/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-app/tests/screen_scenario.rs"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv-gui-03-05-gpui-scenario.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
    - "docs/designs/sv-gui-03-05-gpui-scenario-design.md"
    - "docs/plans/sv-gui-03-05-gpui-scenario-plan.md"
    - "docs/test-specs/sv-gui-03-05-gpui-scenario.md"
    - "docs/adr/sv-gui-03-05-gpui-scenario.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-GUI-03 slice5: GPUI-simulated scenario lane

## Summary

Issue #112 AC5（Ruviform #464 共通 GUI Scenario を Fake/GPUI 両
adapter で実行）として、slice1 の `tests/screen_scenario.rs` が
Fake adapter のみで回していた scenario を driver-neutral 化し、
ruviform-ui-gpui の `GpuiGuiTest`/`GpuiObserver` adapter
（`gpui-test` feature）で同一 scenario を実行する
GPUI-simulated lane を追加した。scenario 本体は
`tests/common/scenario.rs` の driver-neutral helper に集約し、
`SemanticGuiTest` port のみで記述する。`tests/scenario_gpui.rs`
は `ViewHost` + `AttachEpochAllocator` + `create_view_window` の
production composition で mount する。製品コード（`src/`）の
変更はなく、検証 lane（tests + `gpui-test` feature + mise task）
のみの追加。

## Context

- Spec: `docs/specs/sv-gui-03-05-gpui-scenario.md`（approved）
- Design: `docs/designs/sv-gui-03-05-gpui-scenario-design.md`
- Plan: `docs/plans/sv-gui-03-05-gpui-scenario-plan.md`
- Task: `docs/tasks/sv-gui-03-05-gpui-scenario.md`（implements）
- Test-spec: `docs/test-specs/sv-gui-03-05-gpui-scenario.md`
- ADR: `docs/adr/sv-gui-03-05-gpui-scenario.md`
- Source: synvelis Issue #112（SV-GUI-03 slice5・AC5）、
  Ruviform #464（共通 GUI Scenario）。
- spec depends-on: `docs/specs/sv-gui-03-01-transport-inspector-slice1.md`、
  `docs/specs/sv-gui-03-04-selection-inspector-viz.md`、
  `docs/adr/sv175-native-shell-driver-neutral-gpui.md`。
- slice1–4 で product spec 側の契約（selectable table・tabs
  selection・seek/fader gesture・transport action）は確立済み。
  ruviform pin rev `fd1d4d5` に `GpuiGuiTest`/`GpuiObserver`/
  `gpui-test` feature/`ViewMount`/`create_view_window` が既に
  存在し pin 更新は不要。audio-lens `crates/al-gui` に同一構造の
  先行例（`gpui-test` feature・`tests/common/scenario.rs`・
  `tests/scenario_gpui.rs`・`*-ui-gpui` mise lane）がある。

## Implementation

- `crates/synvelis-app/Cargo.toml`: `gpui-test` feature を追加
  (`native` + `ruviform-ui-gpui/gpui-test`、audio-lens と同形)。
- `crates/synvelis-app/tests/common/mod.rs` + `scenario.rs`:
  driver-neutral scenario helper を新規作成。`SemanticGuiTest`
  port のみで記述され、`resolve_component`/gesture parity/
  invoke/select_row/tab_cycle/inspector read-only/product oracle
  (`assert_*`)/honesty helper(`assert_honest_capabilities`/
  `assert_unclaimed_capabilities_rejected`/
  `assert_visual_honesty`/`assert_stale_generation_pin_rejected`/
  `gated_scenario_report`)を集約。
- `crates/synvelis-app/tests/screen_scenario.rs`: 共通 helper
  利用へリファクタ。既存 2 test を維持しつつ fader gesture・
  arrangement select_row・4-tab 巡回・secondary window・
  stale pin・unsupported honesty の test を追加(Fake lane)。
- `crates/synvelis-app/tests/scenario_gpui.rs`: 新規
  (`#![cfg(feature = "gpui-test")]`)。`ViewHost` +
  `AttachEpochAllocator` + `create_view_window` の production
  composition で mount し、main scenario・2 window・Fake/GPUI
  parity・stale pin・unsupported honesty の 5 test。
- `mise.toml`: `check-app-gpui`/`lint-app-gpui`/
  `test-app-gpui` を native lane の後に追加。

### 設計上の判断

- tab 巡回は `screen.tabs` の non-default 4 tab(`[1,2,3,4]`)。
  task の「4 タブ巡回」はこの解釈で spec/design に明記。
- `ScenarioReport` は adapter 実測の `driver()`/`capabilities()`
  をそのまま記録し、semantic pass を強い証拠へ昇格させない。
- detach stale は adapter ごとの API(Fake: view 引数あり、
  GPUI: window+reason)で lane 側に記述し、stale 系 error を
  要求。
- `Cargo.lock`/vendor/docs index は変更なし(pin 更新不要)。

## Related Experiments

実験ログなし — `metadata.experiments` の adopted/rejected は空で、
関連する `docs/impl/exp/` の実験ファイルもない。採否は spec/design
で事前確定済みの確定実装。

## Validation

(実走結果を追記)

task の未完了項・spec Acceptance Criteria が要求する検証 lane:

- `mise run fmt-check` / `check` / `lint` / `test`
- `mise run check-app-gpui` / `lint-app-gpui` / `test-app-gpui`
- `mise run check-app-native` / `lint-app-native` / `test-msrv` /
  `build`

## Risks

- `gpui-test` build は gpui 依存を引くため初回 compile が重い
  （design Negative。mise task として分離済み）。
- `tests/common/` の helper は両 lane の test target に個別
  compile される（重複は許容、audio-lens と同じ構造）。
- tab 巡回は `screen.tabs` の non-default 4 tab（`[1,2,3,4]`）の
  解釈 — task の「4 タブ巡回」はこの解釈で spec/design に明記
  済み。5 tab 全巡回が意図なら scenario 定数 1 箇所の変更で対応
  可能。
- `mise.toml` は共有ファイル — PR に対象・理由・rollback を明記
  する必要がある。
- 実 Window/実 OS 入力・pixel golden・製品 Native E2E・実 Plugin
  Dock は `NotRun`/`Unsupported` のまま — semantic scenario の
  pass をこれらの証拠へ昇格させることは設計上不可能
  （`driver`/`capabilities` は adapter 実測値）。

## Follow-ups

- 検証一式の実走（task 未完了項）: `fmt-check`/`check`/`lint`/
  `test`/`check-app-gpui`/`lint-app-gpui`/`test-app-gpui`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build` の結果を
  本節へ追記。
- Conventional Commits → push → PR（日本語 `Refs #112`）→
  Issue #112 コメント → worker_done。
- 実 Window/実 OS Driver（ruviform#313）・Pixel Golden
  （ruviform#314）・製品 Native E2E（synvelis#114）・実 Plugin
  Dock は外部 gate として後続 slice。
