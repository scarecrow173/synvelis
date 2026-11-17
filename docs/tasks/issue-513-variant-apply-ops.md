---
id: "TASK-513a1b2c3d4e5f6g7h8i9j0k5"
type: "task"
status: "completed"
title: "issue-513-variant-apply-ops — task 分解"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/513"]
  derives-from: ["docs/plans/issue-513-variant-apply-ops.md"]
  derived-by: ["docs/test-specs/issue-513-variant-apply-ops.md"]
---

# Tasks: issue-513-variant-apply-ops

- [x] T1 apply surface: comping + variant kinds の supported_kind/
  entity_value/write ordering 拡張
- [x] T2 `ProjectOp::SetProcessorBinding`（validate/apply/plugin_instances
  sync）
- [x] T3 `VariantOp::AddBranch` + `variant::add_branch`（N>2 candidate）
- [x] T4 adapter confinement + capability 報告更新（3 adapter）
- [x] T5 apply surface mirror 更新（vcs06/plugin_instance_persist）
- [x] T6 vcs07 pin 更新（variant replay + partial fail-closed）
- [x] T7 rcd_04 comping replay test
- [x] T8 cb03 CB-MIDI-01 literal lane（同時 3 candidate）
- [x] T9 cb04 `SetProcessorBinding` scoped/typed/identity test
- [x] T10 ledger VCS-APPLY-10 + evidence/sv513 + doc chain
- [x] T11 fmt/check/lint/test synvelis-project + review-protocol
