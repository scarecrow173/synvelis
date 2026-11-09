---
id: "TASK-034Uw8yD0eUlCZJ2VqMd2P"
type: "task"
status: "done"
title: "PC-C4 task1 — plugin param binding 実装"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/plans/pc-c4-plugin-param-binding.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-plugin-param-binding.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  verified-by:
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C4 task1 — plugin param binding 実装

## Work

- T1 — `dispatch.rs`: `ParamDispatch::new` → `Result` 化 +
  `DispatchError::FrameBaseMismatch`（`control.check_frame_base` 経由）。
- T2 — `binding.rs`: `AttachClockContext`/`StablePluginInstanceId`/
  `PluginParameterBinding`/`PluginBindingSession`/`BindingError`。
- T3 — `lib.rs` export + `host.rs` doc 整合 + 既存 test call site 更新。
- T4 — `tests/pc_c4_binding.rs`: PLUG-C4-01..05・PC-C4-PL01..08 fixture。
- T5 — 最終検証（merge origin/main → mise tasks）+ impl record。

## Classification

- feature / synvelis-player 層 plugin param binding slice（Issue #101）

## Upstream Documents

- plan: `docs/plans/pc-c4-plugin-param-binding.md`
- spec: `docs/specs/pc-c4-plugin-param-binding.md`
- design: `docs/designs/pc-c4-plugin-param-binding-design.md`
