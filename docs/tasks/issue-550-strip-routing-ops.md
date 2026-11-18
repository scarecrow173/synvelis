---
id: "TASK-550StripRtOps06"
type: "task"
status: "done"
title: "Issue #550 tasks — strip routing ops・entity removal slice"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/550"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-550-strip-routing-ops.md"
  implemented-by:
    - "docs/impl/ir/issue-550-strip-routing-ops.md"
  depends-on:
    - "docs/plans/issue-550-strip-routing-ops.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-548-strip-domain-lowering.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-550-strip-routing-ops.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #550 tasks — strip routing ops・entity removal slice

- [x] T1: `variant/types.rs` — `StripField` に
      `Sends`/`MainRoute`/`Sidechain`/`Bus` 追加
- [x] T2: `variant/scope.rs` — routing entity の
      `scope_objects` 導出（source-side 所有規則）
- [x] T3: `variant/adapter.rs` — routing op 受理・
      port confinement・scope-entity removal check・
      helper（`require_strip_field`/`strip_routing_source`）
- [x] T4: `variant/strip.rs` — capabilities per-field・
      boundary_dependencies・edit_candidate・PreEditIds
- [x] T5: `apply/entity.rs` — `removal_supported_kind`・
      `remove_entity` arms（ParameterTarget/
      AutomationLane/ModulationSource/
      ModulationAssignment/RoutingConnection/SendConfig）
- [x] T6: `variant/engine.rs` — Removed write payload に
      削除 entity serialization（preflight_ports 対応）
- [x] T7: `cb06_strip_fields.rs` — #550 節（positive/
      negative/adopt/boundary）+ 既存 stale assertion 修正
- [x] T8: `prepare_engine.rs` — send 4 op descriptor 登録
- [x] T9: `prepare.rs` — lower fn・lower_for・op_facts
- [x] T10: `cb06_strip_gateway.rs` + `tests/rig/mod.rs` —
      send fixture・wire test 2 件・capability/boundary
      更新・`tagged()` 全 kind 対応
- [x] T11: doc chain（本ファイル群）+ cb-06 doc 3 件追従
      更新 + `evidence/sv550/` + `evidence/sv283/` 追記
- [x] T12: `review-protocol` レビュー・blocker 0 確認
- [x] T13: 検証コマンド実行・commit・push・PR・worker_done
