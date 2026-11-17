---
id: "TSPEC-513a1b2c3d4e5f6g7h8i9j0k6"
type: "test-spec"
status: "approved"
title: "issue-513-variant-apply-ops — test spec"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/513"]
  derives-from:
    - "docs/specs/issue-513-variant-apply-ops.md"
    - "docs/designs/issue-513-variant-apply-ops.md"
  verifies:
    - "docs/plans/issue-513-variant-apply-ops.md"
  verified-by:
    - "docs/impl/ir/issue-513-variant-apply-ops.md"
---

# Test spec: issue-513-variant-apply-ops

## Feature: variant/VCS apply op surface

### Rule: comping/variant kinds replay through apply_selected

- Example: `comping_changes_replay_through_apply_selected`（rcd_04）—
  comp registry 追加の 6-change selection が `Applied`、registry 完全、
  region-only partial selection は fail-closed（dangling `comp_id` を
  reference oracle が reject、live/revision 不変）。
- Example: `variant_changes_replay_through_apply_selected`（vcs07）—
  `VariantSet` Added + `VariantBranch`×2 selection が `Applied`、
  set/branch/adopted membership が after と一致。
- Example: `variant_set_without_branch_changes_fails_closed`（vcs07）—
  branch を伴わない set-only selection は post-apply validation で
  reject、rollback、project 不変。

### Rule: SetProcessorBinding は scoped・typed・identity 同期

- Example: `cb_inst_plugin_binding_swap_is_scoped_typed_and_identity_synced`
  （cb04）— resolved→resolved で instance id 再採番、live/sibling 不変、
  scope 外 processor は typed reject、resolved→unresolved で entry 除去、
  未知 processor id は validate で reject。

### Rule: set 内 branch 追加で N>2 candidate を同時保持

- Example: `cb_midi_01_literal_three_simultaneous_candidates`（cb03）—
  fork + `add_branch`×2 で adopted+3 candidates、B の編集が A/C/live/
  raw native source を変更せず、B adopt で sibling は frozen のまま。

### Rule: Removed/preset/routing は Unsupported 維持

- 既存 pin `removed_change_is_unsupported`・capability 報告の
  `Unsupported` エントリが非回帰であること。
