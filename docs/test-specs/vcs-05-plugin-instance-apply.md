---
id: "TSPEC-vcs05-plugin-instance-apply"
type: "test-spec"
status: "approved"
title: "VCS-05 PluginInstance apply surface fixtures"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "SPEC-vcs05-plugin-instance-apply"
    - "DSGN-vcs05-plugin-instance-apply"
  verifies:
    - "TASK-vcs05-plugin-instance-apply"
  supersedes: []
  superseded_by: []
---

# VCS-05 PluginInstance apply surface fixtures

## Feature: plugin-instance identity on the apply surface

`ObjectKind::PluginInstance` の `Added`/`Replaced` change が
`apply_selected` の typed entity transplant として適用され、
byte-faithful な persisted `PluginInstanceId` と monotone
watermark reseed が行われ、`Removed` は fail-closed を維持する
ことを headless fixture で固定する。実 OS FS・実 Provider/
plugin host・GUI/RT は対象外（NotRun）。

## Rules

- **T1 Added selection transplants identity verbatim**:
  `PluginInstance(Added)` を含む processor 追加 selection
  （`Processor(Added)` + `SemanticPort(Added)` +
  `PluginInstance(Added)`）が `ApplyOutcome::Applied` で
  commit され、live `plugin_instances.instances[proc]` が
  after snapshot の entry と byte-faithful に一致する。
  after 側で live 採番と異なる `id`/`watermark`（例: id=7,
  watermark=8）を仕込んだ fixture で、live-mint 値（id=1）
  ではなく after 値が採用されることを実測する。1 apply =
  1 undo step で undo は entry 除去 + watermark 復帰、redo で
  復元する。`target_fingerprint` は `PluginInstance` で解決
  する（`Unsupported` を返さない）。
  Example: `plugin_instance_added_selection_transplants_
  identity_verbatim`（vcs06_plugin_instance_matrix）。
- **T2 Replaced selection transplants new identity**:
  processor rebind（`Processor(Modified)` + `PluginInstance
  (Replaced)`）の selection が適用され、live entry が after
  の `{id, plugin}` に一致する。after 側 id=9/watermark=10
  （live が mint するはずの id=2 とは異なる値）で、
  byte-faithful 採用と watermark 前進採用を実測。undo で
  `{id=1, plugin=A}`/watermark=2、redo で after 状態へ復帰。
  `validate_references` が適用後・undo 後ともに通る。
  Example: `plugin_instance_replaced_selection_transplants_
  new_identity`。
- **T3 Removed stays fail-closed**: entry を持つ live（S1）
  に対する S1→S0（processor+entry 除去）の `PluginInstance
  (Removed)` selection は `ApplyError::Unsupported{kind:
  PluginInstance, change: Removed, "removal changes are
  deferred (cascade design)"}` で拒否され、live
  `plugin_instances` と journal `Failed` record が honest
  に保たれる。
  Example: `plugin_instance_removed_selection_stays_fail_
  closed`。
- **T4 binding-less transplant fails closed at the oracle**:
  plugin field が live binding と異なる `PluginInstance
  (Replaced)` を `Processor` change なしで単独選択すると、
  write は走るが `validate_against` の「pins a different
  plugin」で `ApplyError::Rejected` — live
  `plugin_instances`・processor binding・revision・undo が
  不変、journal `Failed`。
  Example: `plugin_instance_rebind_without_processor_change_
  fails_closed`。
- **T5 presence rules mirror the classifier**: `Added` は
  live entry 不在、`Replaced` は live entry 存在を要求。
  `Replaced` on absent → `TargetMissing`、`Added` on present
  → `TargetConflict` で fail-closed（live 不変）。
  Example: `plugin_instance_target_missing_and_conflict_
  fail_closed`。
- **T6 watermark monotonicity**: after.watermark < live.
  watermark の id-only `Replaced` 適用で、transplant される
  `id` は after 値を採用しつつ watermark は live 値を保持
  する（never backward）。`validate_against` が range/一意性/
  bijection を通す。undo で entry・watermark が復帰。
  Example: `plugin_instance_watermark_never_moves_backward`。
- **T7 ACK-gap replay covers the new kind**: `PluginInstance`
  適用後に Applied receipt を落とした journal（LossyJournal
  相当）で同一 intent を retry すると、`live_fp == after_fp`
  の reconcile により `Replayed` が返り、store は再採番・
  二重 mutation されない。
  Example: `plugin_instance_ack_gap_replays_without_double_
  mutation`。
- **T8 plugin availability gates the selection**: `PluginInstance
  (Added)` を含む processor 追加 selection が
  `PluginAvailability::plugin_available == false` の環境で
  `ApplyError::MissingPlugin` により preflight reject され、
  processor・identity entry・undo step のいずれも commit され
  ない。PluginInstance write 自体は plugin preflight を追加
  要求しない — entry↔binding 整合は `validate_against` oracle
  の責務。
  Example: `plugin_instance_added_selection_rejects_
  unavailable_plugin`。

## Guarantees

- 失敗経路（Unsupported/TargetMissing/TargetConflict/
  Rejected/FingerprintMismatch）で live `plugin_instances`・
  `live_revision`・undo 履歴・journal state が改竄されない。
- apply は `ProjectController` 単一 writer 経由で 1 undo step —
  receipt の `effects.single_undo_step`/`untouched.*` が真を
  報告する。
- `PluginInstance` を含まない既存 selection の apply 挙動・
  fingerprint・plan は不変（`vcs05_apply.rs`・
  `plugin_instance_persist.rs`・`vcs06_apply_matrix.rs` 既存
  fixture が非回帰で pass）。
- watermark は `max(live, after)` で単調非減少 — live 側の
  採番履歴を後退させない。
- `Removed` は全 kind で `Unsupported` のまま（`PluginInstance`
  を含む）。

## Non-goals

- `Removed` の cascade apply（全 kind deferred）。
- plugin-state blob の転送・実 plugin host での state 復元
  （opaque state は snapshot completeness gate の責務）。
- divergent lineage の retired-id 衝突追跡（store 履歴化は
  別設計）。
- session 観測値・実 device monitor・実 GUI — 非永続/外部 gate。
