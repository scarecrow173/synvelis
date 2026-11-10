---
id: "PLAN-IXbRrdisj9XdLzGbNo1zWK"
type: "plan"
status: "approved"
title: "PC-C4-02 plugin instance binding plan"
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
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-02-e2e-plugin-instance-binding.md"
  verified-by:
    - "docs/test-specs/pc-c4-02-e2e-plugin-instance-binding.md"
  references:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
  defers: []
  deferred-by: []
---

# PC-C4-02 plugin instance binding plan Implementation Plan

## Goal

第一 slice の binding 層を stage-1 semantic authority（#122）へ接続し、
plugin instance の authored identity（`PluginRef`）を connect-time
証拠として generation 経由で vend、session/binding record に pin
する。`bind`/`dispatch` は `semantic_plugin_binding` 解決と
`PluginRef` 一致を fail-closed で強制し、raw `ParamId`・identity の
誤配送経路を構造的に排除する。

## Scope

- `crates/synvelis-project`: `host_param::plugin_instance_binding` +
  export。
- `crates/synvelis-player`: `host.rs`（`ProcessorControl.plugin`）・
  `routing_engine.rs`（`RetainedConnection.plugin` vend）・
  `binding.rs`（session/binding pin・`PluginMismatch`・stage-1 解決）・
  `routing_control.rs`（`vend_nodes` fail-closed）。
- `crates/synvelis-player/tests/pc_c4_binding.rs`: fixture を semantic
  authority 経由に更新 + 新規 identity 系テスト。
- docs チェーン + impl record。

Ruviform・app/workspace・vcs 系は非接触。

## File Map

| File | Change |
| --- | --- |
| `crates/synvelis-project/src/host_param.rs` | `plugin_instance_binding` 追加 |
| `crates/synvelis-project/src/lib.rs` | export 追加 |
| `crates/synvelis-player/src/host.rs` | `ProcessorControl.plugin: Option<PluginRef>` |
| `crates/synvelis-player/src/routing_engine.rs` | `RetainedConnection.plugin`、`retain_connection`、`take_processor_control` |
| `crates/synvelis-player/src/binding.rs` | `PluginParameterBinding.plugin`、session `plugin` pin、`attach` 引数、`bind` stage-1 解決、`BindingError::PluginMismatch`、`dispatch` pin |
| `crates/synvelis-player/src/routing_control.rs` | `vend_nodes` の `control.plugin` 処理 |
| `crates/synvelis-player/tests/pc_c4_binding.rs` | fixture 更新 + 新規テスト |

## Dependency Graph

```text
plugin_instance_binding (project)
        │
        ▼（bind 内で semantic_plugin_binding が同じ解決を行う）
ProcessorBinding::Resolved ──stage_build──▶ RetainedConnection.plugin
        │                                        │ publish
        │                                        ▼
        │                              take_processor_control
        │                                        │ ProcessorControl.plugin
        │                                        ▼
        │                              vend_nodes → session.attach(plugin)
        │                                        │
        └──────── semantic_plugin_binding ◀──────┤ bind: plugin 一致検査
                                                 ▼
                                        PluginParameterBinding.plugin
                                                 │ dispatch pin
                                                 ▼
                                        ParamDispatch → node channel
```

## Risk Register

- `attach` signature 変更 → call site は crate 内 + tests のみ。
  `open()`/`pc_c4_05` の 2 点 + `vend_nodes` を更新。
- `project_with` fixture が processor を未登録 → bind が
  `Project(NotFound)` で全滅 → fixture に `Resolved` processor 登録を
  追加（正本消費の正しい形）。
- `ProcessorControl` への field 追加 → 構築は engine のみ、外部
  構築経路なし（field は pub だが型は engine vend）。
- 旧 generation last-good binding が plugin rebind 後も配送を続ける
  → 意図動作（その instance に対して正しい pin）。新規 bind は
  `PluginMismatch` で止まる。

## Verification Matrix

| Spec criterion | Evidence |
| --- | --- |
| PC-C4-02-01 | `pc_c4_binding.rs` 新規: Unresolved/missing processor・PluginRef 不一致・dispatch foreign plugin record |
| PC-C4-02-02 | `vend_nodes` の `without_control` evidence（`control.plugin: None` 経路）+ 既存 ctl_02/sf03_l6 fixture green |
| PC-C4-02-03 | 既存 `pc_c4_binding.rs` 全 test green（fixture 更新後） |
| PC-C4-02-04 | 新規: plugin rebind → `PluginMismatch`、capability_revision 差分、record drift |
| PC-C4-02-05 | docs 記録のみ（NotRun 維持） |

Canonical: `mise run fmt-check` / `check` / `lint` / `test` /
`test-rt` / `test-msrv`。
