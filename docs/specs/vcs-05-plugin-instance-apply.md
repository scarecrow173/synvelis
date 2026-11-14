---
id: "SPEC-vcs05-plugin-instance-apply"
type: "spec"
status: "approved"
title: "VCS-05 PluginInstance apply surface 拡張 slice — synvelis#96 Slice B"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "ISSUE-96"
    - "SPEC-vcs05-apply-restore-headless"
    - "SPEC-vcs05-monitoring-apply-surface"
    - "DESIGN-034VG5a2ikhPyz9RCyjDDT"
  verified_by:
    - "TSPEC-vcs05-plugin-instance-apply"
  implemented_by:
    - "TASK-vcs05-plugin-instance-apply"
  supersedes: []
  superseded_by: []
---

# VCS-05 PluginInstance apply surface 拡張 slice — synvelis#96 Slice B

## 目的

`DIFF_POLICY_VERSION = 3`（synvelis#324 / PR merge 済み）で VCS
semantic diff の分類対象に追加された persisted plugin-instance
identity（`ObjectKind::PluginInstance`、
`ComparisonDomain::Routing`、keyed-synthetic collection
`Project::plugin_instances.instances`）を、VCS-05 Safe Apply
Selected Change の apply whitelist に追加する。現行は whitelist
外のため `PluginInstance` change を含む selection は fingerprint
CAS 段で `ApplyError::Unsupported` に fail-closed する
（`tests/vcs02_diff.rs` の
`plugin_instance_change_is_outside_apply_surface` が旧契約を
pin 済み）。本 slice は `Added`/`Replaced` の typed entity
transplant を可能にし、byte-faithful な persisted `id` 復元と
monotonic never-reissued watermark 統合を確定する。

## Why Now

`plugin_instances` は schema v13 で `Project` に永続化される
authored state であり、restore-as-copy では verbatim
round-trip 済み（VCS-RESTORE-08 Pass）。diff 分類も
`DIFF_POLICY_VERSION = 3` で merge 済み。一方 apply surface
だけが未対応で、plugin-instance identity のみが異なる snapshot
対を部分適用できない。`docs/designs/
plugin-instances-vcs-diff-design.md` の Deferred Design
Concern が「要求が出た時点で別 spec/design に切り出す」と
記録した apply 側設計を、本 slice で確定する。Issue #96
コメントが記録する残存 DoD の PluginInstance 項目に相当する。

## スコープ

### In Scope

- `crates/synvelis-project/src/apply/entity.rs`:
  `supported_kind` への `PluginInstance` 追加、`entity_value`
  の keyed-synthetic view（`stable_id` が借用する `ProcessorId`
  空間 → `plugin_instances.instances` 参照）、`write_entity`
  の専用 arm（byte-faithful `PluginInstanceIdentity` 挿入 +
  instance watermark の monotone reseed。`max_new_id` 非寄与）。
- `crates/synvelis-project/src/apply/plan.rs`:
  `change_supported` の `Replaced` arm への `PluginInstance`
  追加（`id`/`plugin` field swap が `ChangeKind::Replaced`
  として分類されるため）。
- `crates/synvelis-project/src/apply/mod.rs`: module doc の
  scope-out 文言更新。
- `crates/synvelis-project/tests/vcs02_diff.rs`:
  `plugin_instance_change_is_outside_apply_surface` を
  apply surface 入りの新契約 assert へ書換え。
- `crates/synvelis-project/tests/vcs06_apply_matrix.rs`・
  `tests/plugin_instance_persist.rs`: `apply_surface_kind`
  mirror helper を `supported_kind` 全 kind へ同期
  （`TrackMonitorDecl`/`PunchMarks`/`PluginInstance` 追加）、
  stale comment 更新。
- `crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs`:
  apply fixture 追加（Added/Replaced 適用、Removed
  fail-closed、oracle reject、TargetMissing/TargetConflict、
  watermark monotonicity、ACK-gap replay）。
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`:
  plugin-instance apply の実測行追加・Environment ブロック更新。
- `docs/designs/plugin-instances-vcs-diff-design.md`:
  Deferred Design Concern を本 slice の design への参照へ更新。
- 本 slice の doc chain（spec/design/plan/task/test-spec/impl-ir/
  impl-exp）。

### Out of Scope

- `Removed` change の apply — cascade design deferred のまま
  全 kind で fail-closed 維持（`PluginInstance` の `Removed`
  も許可しない。本 Spec で確定）。
- `src/diff/` の変更、`ObjectKind` の追加、apply surface への
  他 kind 追加。
- plugin-state blob の内容転送（opaque state は VCS-01 snapshot
  の completeness が既に gate — apply は project projection の
  みを動かし、plugin state store は触らない）。
- `src/ops.rs`・`src/project.rs`・`src/plugin_instance.rs` の
  変更（既存 `sync_processor`/`validate_against` がそのまま
  権威 oracle として効く）。
- `tests/vcs05_apply.rs`・`tests/vcs05_restore.rs` 等の他
  fixture、`docs/*/README.md` index（再生成のみ）。
- 実 OS FS durability・実 Provider/plugin E2E・GUI/RT・
  #97 REAL lanes・ruviform#294 — NotRun 維持。

## Requirements

- `PluginInstance` の entity identity は `stable_id` が借用する
  `ProcessorId` 空間（diff の keyed-synthetic 規約
  `diff_map(plugin_instances.instances)` と同一）。entity 値は
  `PluginInstanceIdentity { id, plugin }` の全体で、id を持つ
  entity ではないが keyed map の value として serialize される。
  `keyed!`/`keyed_insert!` macro は使わず、`stable_id` から
  `ProcessorId` を parse して `project.plugin_instances.
  instances` を直接参照・挿入する。
- `write_entity` は after-snapshot の `PluginInstanceIdentity`
  を **byte-faithful に** 挿入する（persisted `StablePlugin
  InstanceId` は authored state であり、remap すると fingerprint
  CAS と ACK-gap replay reconcile（`live_fp == after_fp`）が
  破綻する — 選択 object の post-apply live 値は after と
  一致しなければならない）。
- instance `watermark` は `max(live, after.plugin_instances.
  watermark)` で reseed する — 単調増加のみ、決して後退
  しない（live 側で retire 済みの id を将来の `allocate` が
  再発行しないことを保証する）。transplant される `id` は
  一貫した after store で `id < after.watermark` を満たすため
  range 内に収まる。entity `id_watermark`（u128）とは別
  ドメインであり `max_new_id` には寄与しない。
- `Added`/`Replaced` のみ apply 可能。`Removed` は
  `ApplyError::Unsupported{change: Removed, "removal changes are
  deferred (cascade design)"}` で fail-closed。`Modified` は
  diff が生成しない（`id`/`plugin` はともに `Replaced`
  分類）が、kind gate 上は `Added|Modified => Ok` の既存
  分岐をそのまま通る。`Reordered`/`Reconnected`/`Moved` は
  既存の kind 別 gate が拒否する。
- plugin-instance entry と owning processor の binding 整合は
  apply 層で再検査しない — `write_entity` 後の
  `Project::validate_references` →
  `PluginInstanceIdentities::validate_against` が最終権威
  （entry.plugin == processor.binding.plugin、entry 存在 ⇔
  Resolved、id range/一意性、processor 実在）。binding を
  伴わない `PluginInstance` 単独変更は oracle が `Rejected`
  で fail-closed する。
- plugin preflight は既存 `preflight_plugins`（`Processor`
  write の after-binding を検査）のまま — `PluginInstance`
  payload は digest/port/plugin-availability の新規依存を
  導入しない（plugin field の変更は必ず対応する `Processor`
  change とセットでのみ成立し、そちらが preflight を担う）。
- apply は typed entity transplant の既存契約を維持する:
  whole-entity payload、fingerprint CAS、preflight、
  `ProjectController` 単一 writer 経由 1 undo step、journal
  receipt。JSON path patch は導入しない。
- 失敗経路（fingerprint mismatch・plan reject・oracle
  reject・journal Failed）で live `project.plugin_instances`・
  revision・undo 履歴が不変であることを fixture で固定する。

## Acceptance Criteria

- [ ] `PluginInstance` の `Added`/`Replaced` selection が
  typed entity transplant として適用され、live
  `project.plugin_instances.instances` が after snapshot の
  entry と **byte-faithful に**一致する（after 側で live
  採番と異なる `id`/`watermark` を仕込んだ fixture で、
  live-mint 値ではなく after 値が採用されることを実測）。
  1 apply = 1 undo step、undo/redo で identity store が
  往復する。
- [ ] watermark が単調非減少: after.watermark < live.
  watermark のとき live 値を保持し、after.watermark > live
  のとき after 値を採用する。transplant 後も
  `validate_against` が全不変条件を通す。
- [ ] `Removed` selection は `Unsupported` fail-closed、
  journal `Failed`、live 不変。
- [ ] binding を伴わない `PluginInstance` 単独変更
  （plugin field が live binding と不一致になる `Replaced`）
  は oracle `Rejected` で fail-closed、live/journal が honest。
- [ ] `TargetMissing`（live に entry 不在の `Replaced`）と
  `TargetConflict`（live に entry 存在の `Added`）が
  fail-closed。
- [ ] ACK-gap replay: Pending journal + live==after で
  `Replayed` が返り二重 mutation しない（`PluginInstance`
  entity が fingerprint/reconcile 対象として機能することを
  実測）。
- [ ] `target_fingerprint` が `PluginInstance` で解決する
  （旧来の `Unsupported` を返さない）。
- [ ] `vcs02_diff.rs`・`vcs06_apply_matrix.rs`・
  `plugin_instance_persist.rs`・`vcs05_apply.rs`・
  `vcs06_plugin_instance_matrix.rs` 既存 fixture が非回帰で
  pass。`mise run fmt-check`/`check`/`lint`/`test` が全 pass。
- [ ] ledger の plugin-instance apply 行が実測 verdict
  （Pass）と環境情報へ更新される。

## Deferred Design Concerns

- `Removed` change の cascade apply — 全 kind 共通の別設計
  （`PluginInstance` の entry 除去を含む）。
- recording/comping/SMF/clock-epoch 領域の apply — scope-out
  のまま。
- divergent lineage 間での retired-id 衝突: byte-faithful
  transplant は after 系列の id 割当を権威として採用するため、
  live 系列で retire 済みの id が after 側の別 entry に
  割当済みの場合、transplant はその id を「再発行」しうる。
  store は retired-id 履歴を watermark 以外に持たず検出
  不能であり、restore-as-copy / project file load と同じ
  「外部系列の identity 採用」semantics として受容する
  （`max()` reseed により将来の `allocate` による再発行は
  防がれる）。design に詳細を記録。
- engine/runtime への実適用、GUI approval UX、実 plugin host
  での state 復元 — app 層・#97。

## Sources and Relations

- `docs/specs/vcs-05-apply-restore-headless.md`（apply/restore
  headless slice の正本 spec — 本 slice はその surface 拡張）。
- `docs/specs/vcs-05-monitoring-apply-surface.md`（直前の
  apply surface 拡張 slice — 本 slice はその続き）。
- `docs/designs/plugin-instances-vcs-diff-design.md`
  （diff 分類の正本 — keyed-synthetic 規約・Deferred Design
  Concern の出典）。
- `crates/synvelis-project/src/plugin_instance.rs`
  （identity store の不変条件正本 — `validate_against` oracle）。
- GitHub issue `scarecrow173/synvelis#96`（Refs。本 spec は
  Issue を Close しない）。
