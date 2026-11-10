---
id: "TASK-gH5lLpWnYI2KFNM99SswOf"
type: "task"
status: "in-progress"
title: "PC-C4-02 task1 — plugin instance binding 実装"
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
    - "docs/plans/pc-c4-02-plugin-instance-binding.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-02-plugin-instance-binding.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-02-e2e-plugin-instance-binding.md"
  verified-by:
    - "docs/test-specs/pc-c4-02-e2e-plugin-instance-binding.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C4-02 task1 — plugin instance binding 実装

## Work

- T1 — `synvelis-project`: `host_param::plugin_instance_binding` +
  `lib.rs` export。
- T2 — `synvelis-player`: `ProcessorControl.plugin`（host.rs）+
  `RetainedConnection.plugin`/`take_processor_control` vend
  （routing_engine.rs）。
- T3 — `binding.rs`: `PluginParameterBinding.plugin`・session pin・
  `attach` 引数・`bind` の `semantic_plugin_binding` 解決 +
  `PluginMismatch`・`dispatch` pin 検査。
- T4 — `routing_control.rs`: `vend_nodes` の `control.plugin`
  fail-closed。
- T5 — `tests/pc_c4_binding.rs`: fixture を semantic authority 経由に
  更新（`project_with` に `Resolved` processor 登録、`open`/attach
  call site に plugin pin）+ 新規 identity テスト
  （Unresolved/missing processor・PluginRef 不一致・foreign record
  dispatch・rebind 後 bind 拒否）。
- T6 — 最終検証（merge origin/main → mise tasks）+ impl record。

## Classification

- feature / synvelis-project+player 層 plugin instance binding slice
  （Issue #101 第二 slice）

## Upstream Documents

- plan: `docs/plans/pc-c4-02-plugin-instance-binding.md`
- spec: `docs/specs/pc-c4-02-plugin-instance-binding.md`
- design: `docs/designs/pc-c4-02-plugin-instance-binding-design.md`
