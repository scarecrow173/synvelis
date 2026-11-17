---
id: "TSPEC-548StripDomLowr07"
type: "test-spec"
status: "approved"
title: "Issue #548 test-spec — strip-domain lowering の検証仕様"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/548"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-548-strip-domain-lowering.md"
    - "docs/designs/issue-548-strip-domain-lowering.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-548-strip-domain-lowering.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/tasks/issue-548-strip-domain-lowering.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-548-strip-domain-lowering.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/impl/ir/issue-548-strip-domain-lowering.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #548 test-spec — strip-domain lowering の検証仕様

## 検証対象

`cb06_strip_gateway.rs` の positive mask-gated edit 経路と
`tests/prepare.rs` の ordinary prepare 面。

## Rules

- R1: `synvelis.strip.set-mute`/`set-solo`/
  `synvelis.param.set-base`（狭 mask `fader`+`mute`+`solo`
  宣言時）が `branch.edit` で candidate edit として受理され、
  branch content revision が進み live は不変
- R2: `synvelis.strip.insert-processor`/
  `synvelis.processor.set-binding`（全 5 field mask 宣言時）
  が同様に candidate edit として受理される
- R3: mask 未宣言 family（狭 mask での inserts 系）・
  scope 外 target（master strip）・非 strip ordinary op
  （`synvelis.clip.mute`）は `variant-unsupported`/
  `variant-scope-violation` の typed reject を維持し、
  revision を消費しない
- R4: ordinary prepare で新 capability が lower され、
  proposal が retained され、candidate が would-be 状態を
  保持し、committed project は byte-identical
- R5: schema/type 違反・存在しない processor は typed
  reject で proposal を残さない
- R6: 既存 boundary 証拠（scope digest / branch revision
  pin / adopt provenance / lifecycle）は維持

## Examples

- `cb06_strip_fields_mask_gated_edit_wire_lowering` —
  狭 mask で 3 positive edit → compare `verified` →
  mask/scope/非 strip 負側 3 件
- `cb06_strip_fields_insert_ops_wire_lowering` — 全 mask で
  insert/rebind 2 positive edit → live chain 不変 →
  非 strip 負側
- `prepare_strip_bool_ops_lower_and_prepare` —
  set-mute/set-solo の ordinary prepare + candidate 状態 +
  非破壊
- `prepare_param_set_base_and_insert_processor_lower` —
  param.set-base + insert-processor の ordinary prepare
- `prepare_strip_domain_args_fail_typed` — schema/binding/
  存在しない processor の typed reject

## Oracle 種別

headless（`mise run test synvelis-gateway`）。実 PCM/RT/
plugin・実 GUI lane は `NotRun`（外部 gate 責務）。
