---
id: "TASK-sv511eF6gH8iJ0kL1mN3oP5q"
type: "task"
status: "done"
title: "Issue #511 tasks — CB-07 materialization 実装タスク"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/511"
    - "docs/plans/issue-511-cb07-materialization.md"
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
  related:
    - "docs/test-specs/issue-511-cb07-materialization.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-511-cb07-materialization.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-511-cb07-materialization.md"
  references:
    - "evidence/sv511/verification.md"
  defers: []
  deferred-by: []
---

# Issue #511 tasks — CB-07 materialization

- [x] T1 `apply/entity.rs`: `removal_supported_kind`（Clip|Note）と
  `remove_entity`（Clip: clips map + 全 lane `clip_ids` + notes
  cascade / Note: notes map 除去・冪等）を追加し、`write_entity`
  に `Removed` 分岐を実装。
- [x] T2 `variant/engine.rs`: `plan_adopt_writes` の removal 判定を
  entity-presence 3 分岐化 — move-out `Unsupported`、非対応 kind
  `Unsupported`、真の削除は `Removed` write（payload `Null`）。
- [x] T3 `ppqn.rs`: `reproject_ppqn` に `Captured` payload 存在時の
  `Error::Validation` 拒否を追加（validate/apply/preview 共通経路）。
- [x] T4 tests: `cb_arr_11_delete_candidate_adopts_and_renders_
  materialized`（1 CAS/undo・render materialize・confinement）、
  `cb_arr_08` notes-only adopt pin 更新、`cb_arr_14_ppqn_rejects_
  captured_payloads_then_reprojects_scope_time`、`cb_arr_16_sweep_
  delete_adopts_and_cascades_notes` 新規。
- [x] T5 doc chain（discovery/spec/design/plan/task/test-spec/
  adr/impl-ir/impl-exp + exp.jsonl）と `evidence/sv511` ledger。
- [x] T6 正規検証: `mise run fmt-check` / `check` / `lint` /
  `test synvelis-project` / `test-rt` / `prek` /
  `check-headless-deps`。
- [x] T7 review-protocol 1 cycle 以上・blocker 0 確認、commit・
  push・PR（`Refs #511`）・`worker_done`。
