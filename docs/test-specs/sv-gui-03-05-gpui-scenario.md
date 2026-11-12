---
id: "TSPEC-Pn6rTy3wQx8vBs2kLc9mJd"
type: "test-spec"
status: "approved"
title: "SV-GUI-03 slice5 テスト仕様 — GPUI-simulated scenario lane"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
    - "docs/designs/sv-gui-03-05-gpui-scenario-design.md"
    - "docs/tasks/sv-gui-03-05-gpui-scenario.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice5 テスト仕様

## 範囲

`crates/synvelis-app` の test lane。`SemanticGuiTest` port 上の
driver-neutral scenario を `FakeGuiTest`(既定)と `GpuiGuiTest`
(`gpui-test` feature)で実行する。実 Window/実 OS 入力・pixel
golden・製品 Native E2E・実 Plugin Dock は NotRun/Unsupported
(外部 gate: ruviform#313/#314、synvelis#114)。

## ケース

### 共通 scenario(`tests/common/scenario.rs`)

両 lane が共有する scenario 本体。`SemanticGuiTest` port のみで
記述され、adapter 型を含まない。

- `resolve_component`: `TargetQuery::component` pin +
  `SemanticAction` pre-flight。
- `gesture_parity_scenario`: `parity_gesture_scenario`
  (Begin→Preview→Commit)に Applied receipt + `Notify`
  projection + settle + observe を続ける。
- `seek_parity_scenario`/`secondary_seek_parity_scenario`/
  `fader_parity_scenario`: 各 component の commit 値・
  projection `control_id` を固定。
- `invoke_scenario`: `GuiAction::Invoke` + Applied receipt。
- `select_row_scenario`/`tab_cycle_scenario`: view-local
  `select`/`tab-select` Action + Applied receipt。
- `inspector_table_is_read_only`: mounted・selected なし・
  row select reject。
- product oracle `assert_*`: intent 列・phase・observed
  revision/value・display settle を adapter 非依存で検証。
- honesty helper: `assert_honest_capabilities`/
  `assert_unclaimed_capabilities_rejected`/
  `assert_visual_honesty`/`assert_stale_generation_pin_rejected`/
  `gated_scenario_report`。

### Fake lane(`tests/screen_scenario.rs`)

- `seek_gesture_and_projection_follow_the_shared_parity_scenario`:
  seek gesture → `transport/position` intents → Observed・
  revision 1・8.0 秒 display。report は `driver: Fake`・
  verdict Pass・capabilities `{SemanticAction, MultiWindow}`。
- `fader_gesture_and_projection_follow_the_shared_parity_scenario`:
  fader gesture → `mixer_gain_target()` intents → Observed・
  0.4 display。
- `transport_play_invokes_and_inspector_table_is_read_only`:
  play invoke → `transport`/`play` Action 1 件・Applied、
  inspector table は read-only(select reject)。
- `arrangement_select_row_and_tab_cycle_mint_view_state_actions`:
  `arrangement.table` select_row → `select` Action
  `{"row":0}` on `view`、`screen.tabs` 4 tab 巡回 →
  `tab-select` Action `{"index":i}`。
- `secondary_window_runs_the_same_scenarios`: 2 window に
  main+secondary を mount し seek2/transport2 scenario が
  pass(MultiWindow)。
- `fake_target_resolution_and_stale_pins_cover_main_view`:
  resolve pin・未知 id error・generation pin
  `StaleGeneration`・detach 後 stale。
- `fake_unrunnable_items_report_unsupported_not_run_not_pass`:
  未申告 capability `Unsupported`・capture/golden
  `Unsupported`・report verdict `Unsupported`。

### GPUI lane(`tests/scenario_gpui.rs`、`gpui-test` feature)

- `gpui_main_screen_scenarios_pass_with_declared_capabilities`:
  main screen の全 scenario が `driver: GpuiSimulated`・
  verdict Pass・capabilities `{SemanticAction, MultiWindow}`。
- `gpui_two_window_main_and_secondary_scenarios`: 2 window の
  main+secondary scenario が pass。
- `gpui_and_fake_run_the_same_scenarios_with_equal_evidence`:
  同一 scenario 呼び出しを両 adapter で実行し、Action intent
  の `(target, action, params)`・gesture の Commit intent・
  committed plain 値が一致。
- `gpui_target_resolution_and_stale_pins_cover_main_view`:
  Fake lane と同一の resolve/stale 契約(GPUI detach API は
  `(window, reason)`)。
- `gpui_unrunnable_items_report_unsupported_not_run_not_pass`:
  Fake lane と同一の honesty 契約。

## 非対象(NotRun/Unsupported)

- 実 Window mount/close・実 OS pointer/keyboard・IME・DPI・
  pixel capture/golden — ruviform#313/#314、synvelis#114。
- 実 Plugin Dock — 外部 gate。
- 製品 Native E2E — synvelis#114。
- semantic scenario の pass をこれらの証拠へ昇格させることは
  設計上不可能(`driver`/`capabilities` は adapter 実測値)。
