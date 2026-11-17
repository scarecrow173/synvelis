---
id: "PLAN-513a1b2c3d4e5f6g7h8i9j0k4"
type: "plan"
status: "approved"
title: "issue-513-variant-apply-ops — 実装 plan"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/513"]
  derives-from: ["docs/designs/issue-513-variant-apply-ops.md"]
  derived-by:
    - "docs/tasks/issue-513-variant-apply-ops.md"
    - "docs/test-specs/issue-513-variant-apply-ops.md"
---

# Plan: issue-513-variant-apply-ops

## File map

- `src/apply/entity.rs` — supported_kind/entity_value/write ordering
- `src/apply/plan.rs` — Replaced 許可（Removed は不変）
- `src/ops.rs` — `ProjectOp::SetProcessorBinding`（enum/validate/apply）
- `src/variant/types.rs` — `AddBranchReceipt` 等
- `src/variant/engine.rs` — `VariantOp::AddBranch` + `add_branch`
- `src/variant/mod.rs` — export
- `src/variant/adapter.rs` — `SetProcessorBinding` confinement
- `src/variant/{instrument,processing_range,strip}.rs` — capability
- `tests/vcs06_apply_matrix.rs`, `tests/plugin_instance_persist.rs` —
  apply surface mirror
- `tests/vcs07_variant_diff.rs` — variant replay pin 更新
- `tests/rcd_04_comp_cas.rs` — comping replay test
- `tests/cb03_midi_clip.rs` — CB-MIDI-01 literal lane（3 candidates）
- `tests/cb04_instrument_slot.rs` — `SetProcessorBinding` test
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` — VCS-APPLY-10 行
- `evidence/sv513/` — acceptance ledger + lane 簿記

## 順序

1. apply surface（entity/plan）→ 2. ops.rs binding op → 3. variant
   engine add_branch → 4. adapter/capability → 5. tests → 6. ledger/
   evidence/docs → 7. `mise run fmt-check` / `check` / `lint` / `test`
   synvelis-project → 8. review-protocol → 9. commit/push/PR。

## 検証

`mise run fmt-check`、`mise run check synvelis-project`、
`mise run lint synvelis-project`、`mise run test synvelis-project`
（canonical）。headless fixture lane のみ — 実 OS/Plugin/Render/GUI は
NotRun。
