---
id: "TASK-552SendGainSeal06"
type: "task"
status: "done"
title: "Issue #552 tasks — SetSendGainTarget seal 解放 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/552"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-552-sendgain-seal.md"
  implemented-by:
    - "docs/impl/ir/issue-552-sendgain-seal.md"
  depends-on:
    - "docs/plans/issue-552-sendgain-seal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-550-strip-routing-ops.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-552-sendgain-seal.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #552 tasks — SetSendGainTarget seal 解放 slice

- [x] T1: `ops.rs` — `SetSendGainTarget` apply arm で旧
      `SendLevel` seal の release（target + lane +
      assignment 閉包の削除）と新 seal を 1 apply で
      不可分適用。`release_send_level_target` helper 追加。
- [x] T2: `variant/adapter.rs` — `SetSendGainTarget` arm
      の typed `Unsupported` を除去し positive 受理化
      （scope 検査は維持）。
- [x] T3: `tests/ctl_02.rs` — Some→Some rebind
      （閉包カスケード・singleton 維持・undo 復元）・
      Some→None→Some・`RoutingEdit` 経路の 3 試験追加。
- [x] T4: `tests/cb06_strip_fields.rs` — rebind の
      typed `Unsupported` 実測を positive candidate
      edit 検証へ置換。
- [x] T5: `docs/` chain（discovery/specs/designs/
      plans/tasks/test-specs/impl-ir/impl-exp）+
      `evidence/sv552/` 作成。
