---
id: "TASK-034VvIOJTIiu4YPdKizKEx"
type: "task"
status: "done"
title: "PC-C4-05 task1 — persisted StablePluginInstanceId 実装"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/plans/pc-c4-05-persisted-instance-id-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-05-persisted-instance-id-plan.md"
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
    - "docs/designs/pc-c4-05-persisted-instance-id-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-05-persisted-instance-id-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-05-persisted-instance-id.md"
  verified-by:
    - "docs/test-specs/pc-c4-05-persisted-instance-id.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C4-05 task1 — persisted StablePluginInstanceId 実装

## Work

- T1 — `crates/synvelis-project/tests/plugin_instance_persist.rs`
  （新規・red）: save→load round-trip・採番単調（Remove→Insert で
  id 不回復・undo で復元）・oracle reject（dangling/plugin 不一致・
  id 重複・watermark 範囲外・Resolved entry 欠落・Unresolved entry）・
  v12→v13 migration backfill・VCS apply 回帰。
- T2 — `crates/synvelis-project`: `plugin_instance.rs` 新設、
  `project.rs`（field/oracle）、`store.rs`（v13）、`migration.rs`
  （v12→v13 step）、`ops.rs`（Insert/Remove 維持）、`apply/entity.rs`
  （Processor arm `sync_processor` — Coordinator 承認済み拡張）、
  `lib.rs` export。T1 green 化。
- T3 — `crates/synvelis-player/tests/pc_c4_05_persisted_instance.rs`
  （新規・red）: hydration で persisted id が session/binding
  provenance に流れる・reload 後同一 id・entry 欠落/plugin 不一致で
  `without_control`。
- T4 — `crates/synvelis-player`: `routing_control.rs` の `vend_nodes`
  を `plugin_instances.resolve` hydration に置換・`next_instance`
  撤去、`binding.rs` の `StablePluginInstanceId` doc 更新。T3 green 化。
- T5 — `SCHEMA_VERSION` pin を持つ既存 test（`md_a.rs`/`tim_02_smf.rs`
  等）の literal を 13 へ更新（存在すれば）。

## Classification

- feature（schema v13 追加 + runtime hydration — bug-fix ではない）

## Done When

- [x] `Project::plugin_instances` が schema v13 で永続化され、
  save→load・v12→v13 migration round-trip で同一 id が復元される
- [x] Resolved⇔entry 全単射 oracle が `validate_references` で
  load/apply 双方に効き、違反は typed reject
- [x] `RoutingControlDispatch` が persisted id を hydration し、
  missing/stale は `without_control` で fail-closed
- [x] VCS apply の Resolved processor 追加が reject されず新規 id が
  採番される（回帰 test）

## Verification

- [x] `mise run test` exits 0（新規 test + 既存全 test green）
- [x] `mise run check` exits 0
- [x] `mise run lint` exits 0
- [x] `mise run fmt-check` exits 0
- [x] `mise run test-rt` exits 0
- [x] `mise run test-msrv` exits 0
- [x] `mise run check-headless-deps` exits 0

## Notes

- `apply/entity.rs` は Coordinator 承認済みの範囲拡張（ask
  `msg_dc85e87b3616` 回答）。承認条件: Added/Resolved 新規採番・
  plugin 不変 Modified は既存 id 維持・Unresolved は entry 除去・
  ensure 意味論。`write_all`/`write_entity` の他入口は
  `ApplySelectedOp::apply` のみであることを確認済み。
- `driver.rs`（clip-insert FX の ephemeral id）は別 domain のため
  変更しない。
