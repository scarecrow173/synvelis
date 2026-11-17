---
id: "TASK-5AchgRcnLXfPdB4SMDuk5n"
type: "task"
status: "completed"
title: "sv293 semantic→selection integration タスク (synvelis#293 deferred residual slice)"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/293"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv293-semantic-selection-integration.md"
  implemented-by:
    - "docs/impl/ir/sv293-semantic-selection-integration.md"
  depends-on:
    - "docs/plans/sv293-semantic-selection-integration.md"
    - "docs/test-specs/sv293-semantic-selection-integration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcp-04-missing-plugin-resolver.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sv293-semantic-selection-integration.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/impl/ir/sv293-semantic-selection-integration.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
---

# sv293 semantic→selection integration タスク

| # | タスク | 状態 |
| --- | --- | --- |
| T1 | `crates/synvelis-workspace/Cargo.toml` に `ruviform-graph`/`ruviform-audio-core` 追加 + `Cargo.lock` 更新 | done |
| T2 | `src/rcp_selection.rs` producer（snapshot/capability/render index/select）実装 | done |
| T3 | `drive_selection_plan`（reconciler→stale gate→engine→reconciler）実装 | done |
| T4 | `lib.rs` module 登録 + re-export | done |
| T5 | `tests/rcp_selection_integration.rs` oracle（9 tests） | done |
| T6 | doc chain（spec/design/plan/task/test-spec/discovery/impl） | done |
| T7 | `evidence/sv293/` acceptance-ledger + verification | done |
| T8 | fmt/check/lint/test/app/render/prek 検証 | done |
| T9 | review-protocol → merge origin/main → 再検証 → commit/push/PR | done |

## 備考

- `RoutingCompileRequest` は `synvelis_project` の public
  re-export を使用（`routing_engine` 側は private）。
- `StaticProcessorHost::connect` の consume 意味論により、oracle
  の各 drive で host を再武装する（`capability_generation=0`
  のため drift なし）。
- `EdgeKind::Return` は source strip が Return bus 所有の Main
  edge（return bus → mix feed）に付与。
