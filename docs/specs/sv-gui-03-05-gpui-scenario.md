---
id: "SPEC-Gp5sQm2vNx8wRt4yLc7jKf"
type: "spec"
status: "approved"
title: "SV-GUI-03 slice5: GPUI-simulated scenario lane(Fake/GPUI parity)"
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
  depends-on:
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
    - "docs/specs/sv-gui-03-04-selection-inspector-viz.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-03-05-gpui-scenario-design.md"
    - "docs/tasks/sv-gui-03-05-gpui-scenario.md"
    - "docs/test-specs/sv-gui-03-05-gpui-scenario.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice5 仕様: GPUI-simulated scenario lane(Fake/GPUI parity)

## What Should Be Built

Issue #112 AC5(Ruviform #464 共通 GUI Scenario を Fake/GPUI 両
adapter で実行)として、slice1 の `tests/screen_scenario.rs` が
Fake adapter のみで回していた scenario を driver-neutral 化し、
ruviform-ui-gpui の `GpuiGuiTest`/`GpuiObserver` adapter
(`gpui-test` feature)で同一 scenario を実行する GPUI-simulated
lane を追加する。

- `crates/synvelis-app/Cargo.toml` に `gpui-test` feature を追加
  する(audio-lens `crates/al-gui` と同形:
  `gpui-test = ["native", "ruviform-ui-gpui/gpui-test"]`)。
  `native` feature を内包するため headless 既定 lane
  (`cargo check --workspace` 等)は gpui 非依存のまま。
- `tests/common/scenario.rs` に driver-neutral scenario helper を
  集約する。scenario 本体は `SemanticGuiTest` port のみで記述し、
  adapter 型(Fake/GPUI)は lane 側の mount helper に閉じる。
  対象 scenario は既存 product spec(`main_screen_spec` /
  `secondary_screen_spec`)に対して:
  - `parity_gesture_scenario` on `seek.position`(commit 8.0 秒、
    projection `transport/position`)と `fader.gain`(commit 0.4、
    projection は `mixer_gain_target()` の tagged param id)。
  - `transport.play`/`transport2.play` の `Invoke`(discrete
    `Action` intent on `transport` target)。
  - Arrangement `arrangement.table` の `select_row`(view-local
    `select` Action `{"row": n}` on `VIEW_STATE_TARGET`)。
  - `screen.tabs` の `select_tab` 巡回(初期 active 0 を除く
    4 tab、`tab-select` Action `{"index": i}`)。
  - secondary window の `seek2.position`/`transport2.play`
    (MultiWindow 証拠)。
  - receipt(`Event::Result` Applied)+ projection(`Event::Notify`)
    の post と settle 後 snapshot の product oracle。
- `tests/scenario_gpui.rs`(`#![cfg(feature = "gpui-test")]`)を
  追加し、`ViewHost::new(view_instance, AttachEpochAllocator::
  mint())` → `host.mount(spec)` → `create_view_window(
  &WindowSpec::default(), ViewMount { host, theme: None })` の
  production composition と同じ形で mount する。
- `tests/screen_scenario.rs` は共通 helper 呼び出しへ最小限
  リファクタし、Fake lane の既存検証(gesture parity・play
  invoke・inspector read-only)を維持する。
- `ScenarioReport` は adapter 実測の `driver()`/`capabilities()`
  をそのまま記録する。`GpuiSimulated`/`Fake` の pass は
  `{SemanticAction, MultiWindow}` の semantic 証拠に留め、
  simulated pointer・real OS・IME・pixel capture・native plugin
  dock は `Unsupported`/`NotRun` のまま記録し Pass 扱いしない。
- stale pin 検証: generation pin の `StaleGeneration` reject と
  detach 後の stale target を両 lane で確認する。
- `mise.toml` に `check-app-gpui`/`lint-app-gpui`/`test-app-gpui`
  の 3 task を追加する(webview/native lane と同形。
  `mise.toml` は共有ファイルのため PR に対象・理由・rollback を
  明記する)。

## Why Now

- Issue #112 の AC5 は「共通 GUI Scenario を Fake/GPUI で実行」
  を求めており、slice1-4 で product spec 側の契約
  (selectable table・tabs selection・seek/fader gesture・
  transport action)は確立済み。残るのは adapter 追加と
  driver-neutral 化のみ。
- ruviform pin rev(`fd1d4d5`)には `GpuiGuiTest`/`GpuiObserver`/
  `gpui-test` feature/`ViewMount`/`create_view_window` が既に
  存在し、pin 更新は不要。
- audio-lens `crates/al-gui` に完全な先行例
  (`gpui-test` feature、`tests/common/scenario.rs`、
  `tests/scenario_gpui.rs`、`*-ui-gpui` mise lane)があり、
  同一構造を synvelis の product spec へ適用できる。

## Users and Value

- 開発者は `mise run test-app-gpui` 一発で、製品画面 scenario が
  Fake と GPUI-simulated の両 adapter で同一契約を満たすことを
  検証できる。semantic intent の parity が確認できるため、
  実 Window/実 OS 層(ruviform#313)・pixel golden(#314)・製品
  Native E2E(synvelis#114)の外部 gate に進む前の adapter 間
  一貫性が担保される。

## Scope

- `crates/synvelis-app/` の tests/feature のみ:
  - `Cargo.toml`: `gpui-test` feature 追加。
  - `tests/common/mod.rs` + `tests/common/scenario.rs`: 新規。
  - `tests/screen_scenario.rs`: 共通 helper 利用への最小リファクタ。
  - `tests/scenario_gpui.rs`: 新規(`gpui-test` feature gate)。
- `mise.toml`: `check-app-gpui`/`lint-app-gpui`/`test-app-gpui`
  task 追加(共有ファイル・task のみ)。
- docs チェーン(spec/design/plan/task/test-spec/ir/ADR)。

## Non-Goals

- `crates/synvelis-app/src/`(screens.rs 等の製品コード)、
  `synvelis-workspace`、`synvelis-theme` の変更。
- 新規 SV-UI 製品 Scenario/Fixture 定義、独自 Runner/DSL
  (synvelis#114 の責務)。
- 実 Window/実 OS Driver(ruviform#313)・Pixel Golden
  (ruviform#314)・製品 Native E2E(synvelis#114)・実 Plugin Dock
  の証拠 — `NotRun`/`Unsupported` 記録に留める。
- `Cargo.lock`・`ui/vendor/ruviform`・docs index の変更
  (pin 更新不要のため。統合時に Coordinator が処理)。
- `app.rs` の stale `"multi_window"` 記述修正(#113 領域)。

## Acceptance Criteria

- `gpui-test` feature が `native` + `ruviform-ui-gpui/gpui-test`
  を有効化し、`cargo check --workspace`(feature 無し)は gpui
  非依存のまま成功する。
- `tests/common/scenario.rs` の scenario helper が
  `SemanticGuiTest` port のみで記述され、`FakeGuiTest` と
  `GpuiGuiTest` の両方で実行できる。
- `tests/scenario_gpui.rs` が `ViewHost` + `create_view_window`
  の production composition で `main_screen_spec`/
  `secondary_screen_spec` を mount し、seek/fader gesture
  parity・transport play invoke・arrangement select_row・
  4-tab 巡回・secondary window scenario が pass する。
- Fake/GPUI parity テストで、同一 scenario 呼び出しが両
  adapter で一致する `CommandIntent`(Action は
  target/action/params、gesture は Commit + settle 値)を
  発行する。
- `ScenarioReport` が `driver`/`capabilities` を adapter 実測の
  まま記録し、`{SemanticAction, MultiWindow}` 以外を申告しない。
  pixel capture・golden check・未申告 capability pre-flight は
  `Unsupported`/`NotRun` を返し、semantic pass を強い証拠へ
  昇格させない。
- generation pin の `StaleGeneration` reject と detach 後の
  stale target が両 lane で確認される。
- `mise run fmt-check`/`check`/`lint`/`test`/`check-app-gpui`/
  `lint-app-gpui`/`test-app-gpui`/`check-app-native`/
  `lint-app-native`/`test-msrv`/`build` が成功する。

## Open Questions

- なし(product spec は既存のまま scenario を実行可能。
  製品側変更が必要になった場合は本 slice の範囲外として
  coordinator へ escalate する方針を task に明記)。
