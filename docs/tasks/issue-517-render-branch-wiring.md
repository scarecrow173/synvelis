---
id: "TASK-sv517hI0jK1lM2nO3pQ4rS5"
type: "task"
status: "done"
title: "Issue #517 tasks — render/branch pipeline 本番配線"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/plans/issue-517-render-branch-wiring.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-517-render-branch-wiring.md"
  derived-by:
    - "docs/test-specs/issue-517-render-branch-wiring.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #517 tasks

- [x] T1: `DirectoryBranchContentStore` 実装（store.rs）+ re-export
- [x] T2: `JournaledRuntimePublish` 実装（runtime.rs 新規）+
      re-export
- [x] T3: `VariantPorts::standard(Option<&Path>)` + host.rs /
      mcp-poc main.rs 配線（durable project root / discovery None）
- [x] T4: `synvelis.render.*` — RenderAction/RenderIntent/
      RenderDispatch/UnavailableRenderDispatch/render_descriptors
      （variant.rs）+ prepare_engine render_ops + lower_render +
      prepare.rs Render arm + commit.rs GateDeps.render dispatch
- [x] T5: VCS-RESTORE-10 — RestorePorts.branch_content +
      RestoreBranchContentSource（BranchContentStore blanket impl）+
      materialize_branch_content + receipt report + ledger 更新
- [x] T6: テスト — sv517_render_wiring.rs（project 6 件）+
      sv517_render_actions.rs（gateway 6 件）+ 既存 literal 更新
- [x] T7: doc chain + evidence/sv517 + mise 検証 + prek +
      review-protocol + PR
