---
id: "TASK-034UC5rYrk7usxShbL01xG"
type: "task"
status: "done"
title: "TIM-04 plugin state snapshot consistency — implementation tasks"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/tim-04-plugin-state-snapshot.md"
  implemented-by: []
  depends-on:
    - "docs/plans/tim-04-plugin-state-snapshot.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-04-plugin-state-snapshot.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# TIM-04 plugin state snapshot consistency — implementation tasks

## Work

Issue #128 headless/mock slice。`synvelis-project` に
`plugin_state` module（types/port/store/session）を新設し、
`synvelis-player` に `PluginHost`→`PluginStatePort` adapter と
generation-scoped handle vend を、`synvelis-render` に
`PluginStateRecipe` offline gate を追加する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/tim-04-plugin-state-snapshot.md`
- `docs/designs/tim-04-plugin-state-snapshot-consistency-design.md`
- `docs/specs/tim-04-plugin-state-snapshot-consistency.md`
- `docs/test-specs/tim-04-plugin-state-snapshot.md`

## Dependencies

- Ruviform #196 `HostCommand::SaveState`/`RestoreState`（消費のみ）
- VCS-01 `ContentDigest`/`OpaqueStateRef`（再利用・変更しない）
- SYN-IO-02 staged publish（`ruviform-io-file` 再利用）
- TIM-03 process boundary（非 RT policy 先例）

## Affected Files

- `crates/synvelis-project/src/plugin_state.rs`（新設）
- `crates/synvelis-project/src/lib.rs`（additive export）
- `crates/synvelis-project/tests/plugin_state.rs`（新設）
- `crates/synvelis-player/src/plugin_state.rs`（新設）
- `crates/synvelis-player/src/{host,routing_engine,lib}.rs`（additive）
- `crates/synvelis-player/tests/tim04_plugin_state.rs`（新設）
- `crates/synvelis-player/tests/sf03_l3.rs`（`state_handle` field 追従）
- `crates/synvelis-render/src/{spec,preflight,job,error,export,lib}.rs`（additive）
- `crates/synvelis-render/tests/tim04_plugin_state.rs`（新設）
- `crates/synvelis-render/tests/{l2_e2e,x_e2e}.rs`（`plugin_state: None` 追従）
- `docs/impl/ir/tim-04-plugin-state-snapshot.md`（新設）

## Done When

- [x] 全品質（Coordinated/Quiescent/BestEffort/Unsupported）と
  失敗 kind（Crash/Timeout/Permission/Corrupt/Unavailable）の
  typed 記録・Strict fail-closed・indeterminate reconcile・
  copy restore 品質維持が fixture で検証される
- [x] player adapter が実 `PluginHost` + mock plugin の
  SaveState/RestoreState round-trip を非 RT で行う
- [x] render `Strict`+非 exact evidence が preflight で
  拒否され、receipt が evidence を偽らず記録する
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`（存在するもの全て）が pass

## Verification

- [x] `cargo test -p synvelis-project plugin_state`（14 tests — 補助証拠）
- [x] `cargo test -p synvelis-player tim04`（9 tests — 補助証拠）
- [x] `cargo test -p synvelis-render tim04`（5 tests — 補助証拠）
- [x] `mise run fmt-check` / `check` / `lint` / `test` — 全 Pass
- [x] `mise run test-rt` / `test-msrv` / `check-app-native` /
  `lint-app-native` / `build-app-native` — 全 Pass
