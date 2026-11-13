---
id: "IMPL-1p5mZ4InpO2EeDDwvz9TSj"
type: "impl"
status: "completed"
title: "PC-C4 plugin binding — persisted instance authority の bind 照合 implementation record"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added:
      - type: "file"
        path: "docs/specs/pc-c4-plugin-binding.md"
      - type: "file"
        path: "docs/designs/pc-c4-plugin-binding-design.md"
      - type: "file"
        path: "docs/plans/pc-c4-plugin-binding-plan.md"
      - type: "file"
        path: "docs/tasks/pc-c4-plugin-binding-task.md"
      - type: "file"
        path: "docs/test-specs/pc-c4-plugin-binding.md"
      - type: "file"
        path: "docs/impl/ir/pc-c4-plugin-binding.md"
      - type: "file"
        path: "docs/impl/exp/pc-c4-plugin-binding.md"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c4_binding_rt.rs"
      - type: "file"
        path: "evidence/sv101/verification.md"
      - type: "file"
        path: "evidence/sv101/external-gates.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/host_param.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/ctl_02.rs"
      - type: "file"
        path: "crates/synvelis-player/src/binding.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c4_binding.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c6_oracle_pl.rs"
  implements:
    - "docs/plans/pc-c4-plugin-binding-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-plugin-binding-plan.md"
    - "docs/specs/pc-c4-plugin-binding.md"
    - "docs/designs/pc-c4-plugin-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
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

# PC-C4 plugin binding — implementation record

## Summary

synvelis#101 残件 slice: persisted `PluginInstanceIdentities` を
stage-1.5 の instance authority として `PluginBindingSession::bind`
へ接続した。`synvelis-project` に canonical mapping
`plugin_parameter_instance_binding`（stage-1 semantic binding +
persisted instance 解決の合成、`instance: Option`）を追加し、
`synvelis-player` の `bind` が plugin identity 照合の後に attach-time
pin の `StablePluginInstanceId` を照合する。store 欠落・PluginRef
drift・同一 PluginRef の id 再発行（rebind A→B→A）は全て
`BindingError::InstanceMismatch` で fail-closed。fixture は
persisted id 前提へ整合させ、「同 raw `ParamId`・別 instance」は
別 `ProcessorId` + 別 instance id で model 化した。

## Context

- PC-C4-05（schema-v13 `PluginInstanceIdentities`）は store と
  hydration を導入済みだったが、`bind` は stage-1 semantic binding
  のみを解決し persisted instance を照合していなかった。同一
  `(ProcessorId, PluginRef)` が新 id で再発行されても旧 pin の
  session が bind を受け付け得る gap があった。
- `vend_nodes`（routing_control）は attach 時に同じ store を
  resolve するため、正常系では session pin と bind 時解決が構造的に
  一致する。本 slice はその一致を bind 経路で明示的に検査する。

## Implementation

- `crates/synvelis-project/src/host_param.rs`:
  - `PluginParameterInstanceBinding { semantic, instance }` —
    stage-1.5 canonical mapping の結果型。`instance` は
    `Option<PluginInstanceId>`。
  - `plugin_parameter_instance_binding(project, target)` —
    `semantic_plugin_binding` + `plugin_instances.resolve(processor,
    plugin)` の合成。semantic 失敗は `Err`、store が vouch できない
    場合は `instance: None`（fail-closed は consumer 側）。
- `crates/synvelis-project/src/lib.rs`: 新規型・関数を export。
- `crates/synvelis-project/tests/ctl_02.rs`:
  `plugin_parameter_instance_binding_resolves_persisted_instance` —
  解決成功・2 processor の非 aliasing・store 欠落/drift で
  `None`・semantic 失敗は `Err` を pin。
- `crates/synvelis-player/src/binding.rs`: `bind` が
  `plugin_parameter_instance_binding` を使い、照合順序を
  semantic `Err` → `PluginMismatch` → `InstanceMismatch`
  （`None` または pin 不一致）→ capability/live enumeration に固定。
- `crates/synvelis-player/tests/pc_c4_binding.rs`:
  - `project_with_instances`/`project_with_two`/`open_as`/
    `instance_id_of`/`processor_b`/`plugin_b` helper 追加。
    fixture project は `plugin_instances.ensure` で store を
    populate し、session pin は store 発行 id と一致。
  - `pc_c4_02_typed_rejects_no_misdelivery`: 2 processor モデルへ
    更新。session 間越境は `TargetNotBound`、偽造 instance id は
    `InstanceMismatch`。
  - `pc_c4_04_generation_consistency`: 「再 attach で新 instance」
    の旧 model を plugin rebind（`plugin_b` + 新 id）へ修正。
  - `pc_c4_02_instance_authority_bind_gate`（新規）: store 欠落・
    store plugin drift・A→B→A 再発行・`PluginMismatch` 優先順位・
    現行 authority での再 bind 成功を pin。
- `crates/synvelis-player/tests/pc_c6_oracle_pl.rs`:
  `project_with` が store を sync。`pc_c6_pl04` は「同一 authored
  plugin の再 attach = 同一 persisted id・新世代」model へ修正し、
  旧 record は `CapabilityGeneration` で死ぬことを pin
  （PC-C4-05 不変条件の明示）。
- `crates/synvelis-player/tests/pc_c4_binding_rt.rs`（新規）:
  session-bound dispatch（canonical bind → `eval_event` →
  `session.dispatch` → `PluginNode` delivery）の RT
  alloc-free/non-blocking fixture（`stats_alloc`、headless）。

## Related Experiments

`docs/impl/exp/pc-c4-plugin-binding.md` — `instance` を `Option`
にする設計判断と、fail-closed `Result` 案を退けた経緯。

## Validation

- `mise run fmt-check` — exit 0。
- `mise run check synvelis-project` / `synvelis-player` — exit 0。
- `mise run lint synvelis-project` / `synvelis-player` — exit 0
  （clippy `-D warnings`）。
- `mise run test synvelis-project` / `synvelis-player` — exit 0
  （全 test target green。`pc_c4_02_instance_authority_bind_gate`・
  `pc_c4_binding_rt` 含む）。
- `mise run test-rt` — 結果は `evidence/sv101/verification.md` と
  PR 本文に実測を記載。

## Risks

- `bind` の照合順序: instance 解決を `Err` 化すると plugin drift
  時に `PluginMismatch` の報告が後退する — `Option` 設計で回避し、
  順序を `pc_c4_02_instance_authority_bind_gate` が pin。
- fixture の instance pin を persisted id へ揃える変更は既存
  「再 attach = 新 id」model と意味論が異なる — PC-C4-05 の
  不変条件（同一 `(processor, plugin)` は同一 id）に合わせて
  pc_c4_04/pl04 を rebind/世代 model へ修正した。

## Follow-ups

- 実 VST3/AU/CLAP backend・OS device・plugin GUI・AudioLens・
  wire-schema/L1・oracle — synvelis#101 残件、外部 gate
  （`evidence/sv101/external-gates.md`）。
- `HeadlessDriver::next_instance` 等 runtime 境界 provenance の
  別経路との整合は後続 slice で検討。
