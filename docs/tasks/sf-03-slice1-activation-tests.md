---
id: "TASK-106SF03Hi6iJk7lMn8oPq9rSt"
type: "task"
title: "SF-03 slice1 T2 — RoutingEngine activation ledger・stale 防止・SF-ACT-03/04 テスト"
status: "done"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-03-slice1-lowering-activation.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/sf-03-slice1-contract-lowering.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice1-lowering-activation.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice1 T2 — activation + テスト

- `routing_engine.rs`: `RoutingEngine`(prepare/publish/process/
  observe)、`PreparedRouting`、activation ledger、Last-good、
  stale key 再照合(publish 0 保証)。
- `synvelis-player/tests/sf03_l1.rs`(fingerprint/契約含む全受入を集約)。
- `docs/test-specs/sf-03-slice1-lowering-activation.md`。
- 受入: SF-ACT-03/04(stale publish 0、証拠分離、Last-good、
  stub false positive 無し)。
