---
id: "TASK-vcs05-plugin-instance-apply"
type: "task"
status: "in-progress"
title: "VCS-05 PluginInstance apply surface 拡張 task"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/96"
    - "PLAN-vcs05-plugin-instance-apply"
  verified_by:
    - "TSPEC-vcs05-plugin-instance-apply"
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-05 PluginInstance apply surface 拡張 task

## Tasks

- [ ] T1 `apply/entity.rs`: `supported_kind` に `PluginInstance`
  追加、`entity_value` の keyed-synthetic view（`ProcessorId`
  parse → `plugin_instances.instances`）、`write_entity` の
  専用 arm（byte-faithful `PluginInstanceIdentity` 挿入 +
  monotone watermark reseed、`max_new_id` 非寄与）。
- [ ] T2 `apply/plan.rs`: `change_supported` の `Replaced`
  arm に `PluginInstance` 追加。
- [ ] T3 `apply/mod.rs`: module doc の scope-out 文言更新。
- [ ] T4 `tests/vcs02_diff.rs`: apply surface 入りの新契約
  assert へ書換え。
- [ ] T5 `tests/vcs06_apply_matrix.rs`・
  `tests/plugin_instance_persist.rs`: `apply_surface_kind`
  mirror helper を `supported_kind` 全 kind へ同期 + comment
  更新。
- [ ] T6 `tests/vcs06_plugin_instance_matrix.rs`: apply
  fixture helper + apply test 7 本追加。
- [ ] T7 `evidence/reports/vcs-06-fixture-matrix-ledger.md`:
  plugin-instance apply 実測行 + Environment 更新。
- [ ] T8 `docs/designs/plugin-instances-vcs-diff-design.md`:
  Deferred Design Concern を解決済み参照へ更新。
- [ ] T9 doc chain 6+1 文書 + impl-ir/impl-exp + index 再生成。
- [ ] T10 `origin/main` merge + `mise run` canonical 検証 +
  review-protocol + push + PR（Refs #96）。

## Deferred（Issue へ引き継ぎ）

- `Removed` change の cascade apply（全 kind — `PluginInstance`
  entry 除去を含む）→ 別設計
- recording/comping/SMF/clock-epoch 領域の apply → scope-out
- divergent lineage の retired-id 衝突追跡（store 履歴化）→
  schema 変更を伴う別設計
- 実 OS FS durability / 実 Provider / plugin host / GUI / RT /
  cross-consumer → #97/#294
