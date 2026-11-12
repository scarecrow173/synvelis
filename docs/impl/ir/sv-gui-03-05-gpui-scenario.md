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

## 実装内容

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

## 設計上の判断

- tab 巡回は `screen.tabs` の non-default 4 tab(`[1,2,3,4]`)。
  task の「4 タブ巡回」はこの解釈で spec/design に明記。
- `ScenarioReport` は adapter 実測の `driver()`/`capabilities()`
  をそのまま記録し、semantic pass を強い証拠へ昇格させない。
- detach stale は adapter ごとの API(Fake: view 引数あり、
  GPUI: window+reason)で lane 側に記述し、stale 系 error を
  要求。
- `Cargo.lock`/vendor/docs index は変更なし(pin 更新不要)。

## 検証

(実走結果を追記)
