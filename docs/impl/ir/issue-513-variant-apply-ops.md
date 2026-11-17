---
id: "IMPL-513a1b2c3d4e5f6g7h8i9j0k7"
type: "impl"
status: "completed"
title: "issue-513-variant-apply-ops"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/513"]
  derives-from:
    - "docs/plans/issue-513-variant-apply-ops.md"
    - "docs/tasks/issue-513-variant-apply-ops.md"
  derived-by: ["docs/impl/exp/issue-513-variant-apply-ops.md"]
---

# IR: issue-513-variant-apply-ops

## 実装内容

- `apply/entity.rs`: `supported_kind` に comping 4 kind +
  `VariantSet`/`VariantBranch` を追加。`entity_value` に variant
  entity extraction（set keyed、branch は全 set 走査で owner 解決）。
  `VariantSet.branches` protected handling、added entity の map
  default、write ordering（set → branch）。
- `apply/plan.rs`: `Replaced` で variant 2 kind を許可。
  `Removed` → `Unsupported` 不変。
- `ops.rs`: `ProjectOp::SetProcessorBinding` — validate で processor
  存在確認、apply で binding 差替 + `plugin_instances.sync_processor`。
- `variant/types.rs`/`engine.rs`/`mod.rs`: `VariantOp::AddBranch` +
  `add_branch` entry point + receipt 型 + export。
- `variant/adapter.rs`: `SetProcessorBinding` を instrument-slot
  accepted ops + processor scope check + strip-fields `Inserts` mask
  に追加。
- `variant/{instrument,processing_range,strip}.rs`: capability 報告を
  plugin rebind 対応に更新（preset/routing/mid-chain end は
  `Unsupported` 維持）。
- tests: vcs06/plugin_instance_persist mirror、vcs07 pin 書き換え、
  rcd_04 comping replay、cb03 3-candidate lane、cb04 binding test。
- ledger: VCS-APPLY-10 Blocked→Pass、out-of-scope bullet 整合。
- evidence/sv513 + doc chain 10 ファイル。

## 検証

`cargo test -p synvelis-project --test vcs07_variant_diff` 8/8 Pass、
`--test rcd_04_comp_cas` 7/7 Pass（comping replay 含む）、
`--test cb03_midi_clip` 13/13 Pass（3-candidate lane 含む）、
`--test cb04_instrument_slot cb_inst_plugin` 1/1 Pass。
canonical `mise run` 検証は PR 本文に記録。
