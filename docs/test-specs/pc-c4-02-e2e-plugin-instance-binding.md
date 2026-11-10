---
id: "TSPEC-otlsBc81CdrVkhwlcp4umO"
type: "test-spec"
status: "approved"
title: "PC-C4-02 plugin instance binding e2e — stage-1 authority・plugin identity pin"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
  verified-by: []
  references:
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  defers: []
  deferred-by: []
---

# PC-C4-02 plugin instance binding e2e — stage-1 authority・plugin identity pin

## Purpose

Issue #101 第二 slice が、stage-1 semantic binding 正本
（`semantic_plugin_binding`/`plugin_instance_binding`）を消費し、
plugin instance の authored identity（`PluginRef`）を connect-time
証拠として pin することを Fixture+Expected/Observed で固定する。
第一 slice の PLUG-C4-01..05 fixture は semantic authority 経由の
bind に更新して green を維持する。実 VST3/OS は対象外（NotRun）。

## Feature

Semantic plugin instance binding — stage-1 authority consumption・
plugin identity pin（headless）

## Rules

### PC-C4-02-01 — stage-1 authority consumption（bind）

- **Given**: `Resolved(PluginRef)` processor 登録済み project +
  vended `PluginRef` pin の session + live `ParamInfo` 列挙。
- **When**: `session.bind(dispatch, project, target, params)`。
- **Then**: `semantic_plugin_binding` が解決され、record の
  `plugin`/`plugin_param`/`param_id` が semantic binding と一致。

### PC-C4-02-02 — fail-closed identity gate

- **Given**: session pin `pluginA`。
- **When**: project の processor binding が `pluginB`
  （descriptor/version/capability_revision のいずれか差分）または
  `Unresolved`/未登録の場合に `bind`。
- **Then**: `PluginMismatch`（pluginB）または `Project(_)`
  （Unresolved/未登録）の typed reject。delivery 0。

### PC-C4-02-03 — dispatch pin

- **Given**: 記録済み binding を clone し `plugin` を差し替えた
  record、または `PluginRef` 不一致のまま発行された record。
- **When**: `session.dispatch(dispatch, &[(record, event)])`。
- **Then**: `PluginMismatch`（record 比較前の pin 検査）で batch 全体
  reject — staging 0。

### PC-C4-02-04 — generation/既存契約の維持

- 第一 slice の PLUG-C4-01..05 全 fixture が semantic authority 経由の
  bind で green（RT/offline parity・generation drift・queue full・
  stale plan・re-attach・transport/seek）。
- `ProcessorControl.plugin` は connect-time `Resolved` のみ `Some`;
  `None` は `without_control` evidence（`RoutingControlDispatch`
  fixture または engine unit で確認）。

### PC-C4-02-05 — NotRun 宣言

- 実 VST3/OS backend・実 GUI・AudioLens f64・Ruviform cross-consumer
  CI は NotRun。`ProcessorControl.plugin` の vend は
  `PluginHostProcessors`（fake port 経由の plugin host）で実証し、
  実 plugin adapter では未検証。

## Fixtures

- `crates/synvelis-player/tests/pc_c4_binding.rs`:
  - `plugin_ref()` helper + `project_with` の `Resolved` processor
    登録 + `open()`/attach call site の plugin pin。
  - 新規 test:
    - `pc_c4_02_bind_requires_resolved_processor` — processor 未登録
      → `Project(NotFound)`、`Unresolved` → `Project(Validation)`。
    - `pc_c4_02_plugin_identity_mismatch_rejects_bind` — project
      rebind（descriptor 差分 / capability_revision 差分）→
      `PluginMismatch`。
    - `pc_c4_02_dispatch_rejects_foreign_plugin_record` — record の
      `plugin` 差し替え → `PluginMismatch`、batch atomic。
    - `pc_c4_02_rebind_keeps_old_generation_binding_valid` —
      rebind 後も旧 session の既存 binding は dispatch 可能
      （last-good）、新規 bind は `PluginMismatch`。
- 既存 `pc_c4_binding.rs` 全 test: fixture 更新後 green。

## Evidence

- `mise run test`（`cargo test --workspace --locked`、canonical）。
- `mise run test-rt` で RT fixture 非回帰。
- 実 plugin backend: NotRun（follow-up gate）。
