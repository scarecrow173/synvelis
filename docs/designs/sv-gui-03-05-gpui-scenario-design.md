---
id: "DESIGN-Ht8wEr4xQz2nBs6vYp9dMc"
type: "design"
status: "approved"
title: "SV-GUI-03 slice5 設計 — driver-neutral scenario 抽出 + GpuiGuiTest lane"
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
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice5 設計: driver-neutral scenario + GPUI-simulated lane

## Context

slice1 の `tests/screen_scenario.rs` は `FakeGuiTest` 上で
`parity_gesture_scenario`(seek)と `transport.play` invoke を
直接記述していた。AC5 は同じ product scenario を
`GpuiGuiTest`(ruviform-ui-gpui `gpui-test` feature)でも実行
することを求める。ruviform-ui-test の `SemanticGuiTest` port
は両 adapter が実装する driver-neutral 契約であり、scenario
本体を port の上に置けば adapter 差分は mount/stale API の
みに閉じる。audio-lens `crates/al-gui` に同一構造の先行例が
ある(`tests/common/scenario.rs` + `tests/scenario_gpui.rs` +
`gpui-test` feature + `*-ui-gpui` mise lane)。

## Decision

- **scenario 本体は `tests/common/scenario.rs` の普通の Rust
  関数**: `SemanticGuiTest` port のみを引数に取り、adapter 型は
  現れない。`resolve_component`(component pin + `SemanticAction`
  pre-flight)、gesture parity(Apply receipt + `Notify`
  projection + settle + observe)、invoke/select_row/tab_cycle
  (Applied receipt + observe)、product oracle
  (`assert_*`)、honesty helper(`assert_unclaimed_capabilities_
  rejected`/`assert_visual_honesty`/`assert_honest_capabilities`/
  `assert_stale_generation_pin_rejected`)、`ScenarioReport` 骨格
  をここに集約する。`tests/common/mod.rs` は
  `#![allow(dead_code)]`(片 lane でしか使わない helper が
  feature 無し build で dead_code にならないようにするため、
  audio-lens と同じ措置)。
- **mount は lane 側**: Fake は `open_window` + `mount_view`、
  GPUI は `ViewHost::new(view_instance, AttachEpochAllocator::
  new().mint())` → `mount` → `drain_events` →
  `create_view_window(&WindowSpec::default(), ViewMount { host,
  theme: None })` → `settle`。これは `src/bin/synvelis-native.rs`
  の production composition と同じ形。
- **`gpui-test` feature は `native` を内包**:
  `gpui-test = ["native", "ruviform-ui-gpui/gpui-test"]`。
  audio-lens と同形で、製品の GPUI feature 名(`native`)を
  再利用しつつ headless 既定 lane を汚さない。
- **scenario の対象は既存 product spec のみ**:
  `main_screen_spec`/`secondary_screen_spec` を
  `MainScreenState` fixture(project_length 120s、
  arrangement 2 行、inspector 1 行)で mount する。新規
  SV-UI scenario/fixture・独自 runner/DSL は作らない。
- **tab 巡回は 4 tab**: `screen.tabs` は 5 tab
  (Arrangement/Transport/Mixer/Inspector/Signal Flow)で初期
  active は 0。巡回対象は non-default の `[1,2,3,4]` — task の
  「4 タブ巡回」はこの 4 tab を指すと解釈し、spec/design に
  明記する。
- **capability/証拠の正直さ**: `ScenarioReport` は
  `gui.driver()`/`gui.capabilities()` をそのまま記録。
  `HONESTY_ABSENT`(SimulatedPointerOrKeyboard/
  RealOsPointerOrKeyboard/Ime/VisualCapture/NativePluginDock)
  は全 adapter で `require_capabilities` が `Unsupported` を
  返すことを oracle で確認。`capture_frame`/`golden_check` は
  両 adapter とも `Unsupported` verdict — pixel 証拠を捏造
  しない。
- **stale pin**: `resolve_target` の generation pin
  (`generation + 1`)が `StaleGeneration` で reject される
  ことを共通 helper で検証。detach 後の stale target は
  adapter ごとの detach API(Fake: `(window, view, reason)`、
  GPUI: `(window, reason)`)で lane 側に記述し、stale 系
  error(`StaleGeneration`/`StaleEpoch`)を要求する。
- **mise lane**: `check-app-gpui`/`lint-app-gpui`/
  `test-app-gpui` を webview/native lane と同形で追加。
  `mise.toml` は共有ファイル — PR に対象・理由・rollback
  (task 削除で戻せる)を明記する。

## Consequences

### Positive

- scenario 本体が 1 箇所になり、Fake/GPUI の parity が
  「同じ関数を 2 adapter で呼ぶ」形で構造的に保証される。
  intent の差異が出た場合は adapter 側の bug として顕在化する。
- GPUI-simulated の semantic 証拠が `driver:
  GpuiSimulated` + `{SemanticAction, MultiWindow}` のまま
  記録され、実機・pixel 証拠への昇格経路が存在しない。
- `gpui-test` feature が opt-in のため、既存の headless
  check/lint/test lane と native lane の検証コスト・依存は
  不変。

### Negative

- `gpui-test` build は gpui 依存を引くため初回 compile が
  重い(mise task として分離済み)。
- `tests/common/` の helper は両 lane の test target に
  個別 compile される(重複は許容、audio-lens と同じ構造)。
- `screen.tabs` の 5 tab 目(Signal Flow)は巡回対象外 —
  spec の「4 タブ」は non-default tab の意と解釈。もし
  全 tab 巡回が意図なら scenario 定数 1 箇所の変更で対応可能。

## Considered Options

- **scenario を `screen_scenario.rs` に残し GPUI 側で複写**:
  parity が「たまたま同じ」形になり、将来の乖離を検出できない。
  却下。
- **`gpui-test` feature を `native` 非依存で定義**: ruviform-ui-
  gpui の `gpui-test` feature が `gpui` を要求するため、
  `native`(=`dep:ruviform-ui-gpui` + `gpui`)なしでは解決できない。
  audio-lens と同じく `native` 内包が正しい形。却下。
- **製品コード側に scenario hook を追加**: spec-level mount で
  十分に検証でき、製品コード変更は slice の境界外。
  却下(必要なら coordinator へ question)。
- **5 tab 全てを巡回**: task 文言は「4 タブ」。初期 active を
  除く 4 tab 巡回が自然な解釈であり、Signal Flow を含めると
  tab 内容の非対称性(5 番目)を暗黙に含む。4 tab を採用し、
  spec に明記。

## References

- `docs/specs/sv-gui-03-05-gpui-scenario.md`
- audio-lens `crates/al-gui/tests/common/scenario.rs` /
  `tests/scenario_gpui.rs` / `Cargo.toml`(`gpui-test` feature)
- ruviform `crates/ruviform-ui-gpui/src/gpui_driver/`(
  `GpuiGuiTest`/`GpuiObserver`/`create_view_window`)
- ruviform `crates/ruviform-ui-test/src/scenario.rs`(
  `SemanticGuiTest`/`parity_gesture_scenario`/`ScenarioReport`)
- `crates/synvelis-app/src/bin/synvelis-native.rs`
  (production composition)
