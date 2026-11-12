---
id: "ADR-Vc4bNm8qWe1rTy6uIo0pAs"
type: "adr"
status: "accepted"
title: "SV-GUI-03 slice5: GPUI-simulated scenario lane は driver-neutral helper + opt-in feature で構成する"
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
    - "docs/adr/sv-gui-03-01-transport-inspector-app-state-push.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
    - "docs/designs/sv-gui-03-05-gpui-scenario-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv-gui-03-05-gpui-scenario-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ADR: SV-GUI-03 slice5 — GPUI-simulated scenario lane の構成

## Context and Problem Statement

Issue #112 AC5 は Ruviform #464 の共通 GUI Scenario を Fake/GPUI
両 adapter で実行することを求める。slice1 の
`tests/screen_scenario.rs` は `FakeGuiTest` 直書きで、scenario
本体が adapter に結合していた。GPUI-simulated adapter
(`GpuiGuiTest`)を追加するにあたり、(1) scenario 本体をどこに
置き adapter 差分をどう閉じるか、(2) `gpui` 依存をどう
opt-in にするか、(3) semantic pass を実機・pixel 証拠へ昇格
させない仕組みをどう保つか、の 3 つの配置決定が必要。

## Decision

- **scenario 本体は `tests/common/scenario.rs` の普通の Rust
  関数**に置き、`SemanticGuiTest` port のみで記述する。
  adapter 型は lane 側の mount helper(Fake:
  `open_window`+`mount_view`、GPUI: `ViewHost`+
  `create_view_window`)に閉じる。product oracle(`assert_*`)と
  honesty helper も共通化し、両 lane で同一の検証を行う。
- **`gpui-test` feature は `native` を内包する opt-in
  feature**: `gpui-test = ["native", "ruviform-ui-gpui/
  gpui-test"]`。audio-lens `crates/al-gui` と同形。製品の
  GPUI feature 名(`native`)を再利用し、headless 既定 lane
  (`check`/`lint`/`test`)は gpui 非依存のまま。専用 mise
  lane(`check-app-gpui`/`lint-app-gpui`/`test-app-gpui`)で
  分離検証する。
- **証拠水準は adapter 実測値のみ**: `ScenarioReport` は
  `driver()`/`capabilities()` をそのまま記録し、semantic
  pass は `{SemanticAction, MultiWindow}` の証拠に留める。
  未申告 capability・pixel capture・golden は
  `Unsupported`/`NotRun` を返す oracle で、強い証拠への
  昇格経路を構造的に排除する。外部 gate(実 Window/実 OS:
  ruviform#313、pixel golden: ruviform#314、製品 Native
  E2E: synvelis#114、実 Plugin Dock)は本 lane の対象外。
- **対象は既存 product spec のみ**: `main_screen_spec`/
  `secondary_screen_spec` を spec-level mount で実行する。
  新規 SV-UI scenario/fixture・独自 runner/DSL・製品コード
  変更は行わない(#114 の責務境界)。

## Consequences

### Positive

- parity が「同じ関数を 2 adapter で呼ぶ」構造になり、
  scenario の adapter 間乖離が検出可能になる。
- `gpui-test` が opt-in のため既存 lane の依存・検証コストは
  不変。`mise.toml` への影響は task 追加のみで rollback が
  容易。
- semantic 証拠と実機証拠の境界が型と oracle の両方で
  明示され、外部 gate への進行条件が明確になる。

### Negative

- `gpui-test` build は gpui 依存の初回 compile が重い。
- `tests/common/` helper は各 test target に個別 compile
  される(audio-lens と同じ構造的重複、許容)。
- `GpuiGuiTest` の adapter 制約(`!Send`、
  `TestAppContext::single()`)は adapter 内部に閉じるが、
  ruviform 側の変更に追従が必要になる可能性がある。

## Considered Options

- **scenario を lane ごとに複写**: parity が構造的に保証
  されず、将来の乖離を検出できない。却下。
- **`gpui-test` を `native` 非依存で定義**: ruviform-ui-gpui
  の `gpui-test` feature が `gpui` を要求するため解決不能。
  `native` 内包が正しい形。却下。
- **製品コードに scenario hook を追加**: spec-level mount で
  十分で、製品境界を広げる理由がない。却下。
- **tab 巡回を 5 tab 全てに拡張**: task 文言は「4 タブ」。
  non-default tab 巡回を採用し spec に明記。全 tab 巡回が
  必要なら定数 1 箇所で対応可能。

## References

- `docs/specs/sv-gui-03-05-gpui-scenario.md`
- `docs/designs/sv-gui-03-05-gpui-scenario-design.md`
- audio-lens `crates/al-gui`(`gpui-test` feature、
  `tests/common/scenario.rs`、`tests/scenario_gpui.rs`)
- ruviform `crates/ruviform-ui-gpui`/`crates/ruviform-ui-test`
