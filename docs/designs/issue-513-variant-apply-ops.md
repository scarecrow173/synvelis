---
id: "DESIGN-513a1b2c3d4e5f6g7h8i9j0k3"
type: "design"
status: "approved"
title: "issue-513-variant-apply-ops — 設計"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/513"]
  derives-from:
    - "docs/specs/issue-513-variant-apply-ops.md"
    - "docs/adr/issue-513-variant-apply-ops.md"
  derived-by:
    - "docs/plans/issue-513-variant-apply-ops.md"
    - "docs/test-specs/issue-513-variant-apply-ops.md"
---

# Design: issue-513-variant-apply-ops

## apply surface 拡張

- `supported_kind` に `TakeGroup`/`Comp`/`CompRegion`/`CompTransition`/
  `VariantSet`/`VariantBranch` を追加。
- `entity_value`: `VariantSet` は `VariantSetId` keyed、
  `VariantBranch` は全 `variant_sets` を走査して所属 set を解決
  （MIDI note ownership 解決と同型）。comping kind は既存 entity
  extraction を利用。
- `VariantSet.branches` は protected field — `entity_value` では
  map payload を default 化し、branch membership は `VariantBranch`
  change 側で reconcile。
- write ordering: `VariantSet` write を `VariantBranch` write より先に
  行い、branch 挿入先 set を after-snapshot から解決（foreign set
  への混入を防ぐ）。
- `plan.rs`: `Replaced` で `VariantSet`/`VariantBranch` を許可。
  `Removed` は全 kind `Unsupported` のまま。

## `ProjectOp::SetProcessorBinding`

- `{ processor_id, binding }`。validate で processor 存在を確認。
- apply: `Processor.binding` を差替え、`plugin_instances.sync_processor`
  で identity を再同期（同一 plugin → 維持、別 plugin → 再採番、
  Unresolved → entry 除去）。ports/chain/routing connections は不変。
- adapter confinement: instrument-slot scope は対象 processor のみ、
  strip-fields scope は `StripField::Inserts` mask 必須。
  routing/send/sidechain/multi-output op は従来どおり拒否。

## `variant::add_branch` / `VariantOp::AddBranch`
