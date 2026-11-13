---
id: "TSPEC-72LEMGMaEAOSiKJDwnu5Sx"
type: "test-spec"
status: "approved"
title: "PC-C4 plugin binding test spec"
created: "2026-11-14"
updated: "2026-11-14"
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
    - "docs/specs/pc-c4-plugin-binding.md"
    - "docs/designs/pc-c4-plugin-binding-design.md"
    - "docs/plans/pc-c4-plugin-binding-plan.md"
    - "docs/tasks/pc-c4-plugin-binding-task.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-plugin-binding.md"
    - "docs/designs/pc-c4-plugin-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
    - "docs/test-specs/pc-c4-02-e2e-plugin-instance-binding.md"
    - "docs/test-specs/pc-c4-05-persisted-instance-id.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-plugin-binding-plan.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c4-plugin-binding.md"
    - "docs/designs/pc-c4-plugin-binding-design.md"
  verified-by: []
  references:
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
    - "docs/test-specs/pc-c4-02-e2e-plugin-instance-binding.md"
    - "docs/test-specs/pc-c4-05-persisted-instance-id.md"
  defers: []
  deferred-by: []
---

# PC-C4 plugin binding test spec

## Feature: persisted instance authority の bind 照合

### Rule: canonical mapping は fail-closed な合成を返す

- **Given** processor が `ProcessorBinding::Resolved(plugin)` で、
  `plugin_instances` が `(processor, plugin)` を保証する
- **When** `plugin_parameter_instance_binding(project, target)` を呼ぶ
- **Then** `semantic` に stage-1 binding、`instance` に persisted
  `PluginInstanceId` が入り、`Some` である

- **Given** store に `(processor, plugin)` の entry が無い、
  PluginRef が drift している、または id が範囲外
- **When** 同関数を呼ぶ
- **Then** semantic 解決が成功する限り `Err` ではなく
  `instance: None` を返す

### Rule: bind は instance authority を照合する

- **Given** session の attach-time pin instance id が project 権威の
  解決結果と一致する
- **When** `session.bind(project, target)` を呼ぶ
- **Then** binding が stage され、dispatch で plugin へ届く

- **Given** store 欠落・id 不一致・同一 PluginRef の id 再発行
  （A→B→A）
- **When** `session.bind` を呼ぶ
- **Then** `BindingError::InstanceMismatch` で拒否され、
  binding は stage されない

- **Given** project の plugin binding が session pin と異なる
- **When** `session.bind` を呼ぶ
- **Then** `BindingError::PluginMismatch` が instance 照合より先に
  報告される（照合順序の pin）

### Rule: 既存契約の非回帰

- pc_c4_binding / pc_c6_oracle_pl / pc_c4_03 / pc_c4_05 /
  pc_c4_07 の全 fixture が persisted identity を populate した上で
  pass し、PLUG-C4-01〜04（typed reject・誤配送なし・世代整合・
  複合制御）を維持する。
- 2 processor・同 raw ParamId の fixture で、session 間の record
  越境は typed reject され、各 plugin instance の delivered log
  に他 instance の event が混入しない。
- session-bound dispatch（bind→eval→dispatch→process）の RT
  fixture が alloc-free / non-blocking を示す。

### Rule: 実機 gate の正直な記録

- 実 VST3/OS backend・plugin GUI は本 slice では NotRun。
  headless fake fixture の成功を実機 Pass に繰り上げない。
