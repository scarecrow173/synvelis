---
id: "DSGN-vcs05-apply-restore-headless"
type: "design"
status: "approved"
title: "VCS-05 Apply/Restore headless — module 設計"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "SPEC-vcs05-apply-restore-headless"
  verified_by:
    - "TSPEC-vcs05-apply-restore-fixtures"
  implemented_by:
    - "TASK-vcs05-apply-restore-headless"
  supersedes: []
  superseded_by: []
---

# VCS-05 Apply/Restore headless — module 設計

## 構成

`synvelis-project` に 2 module を追加（`lib.rs` に `pub mod` +
re-export）。schema v11 は不要 — `Project` 永続形式は不変。

### `apply/`（Slice A+B）

- `mod.rs`: `ApplyChangeIntent`（change_set_id / source_snapshot_id /
  target_object / expected_project_revision / selected_change_ids /
  expected_target_fingerprint / approval）、`ApplyError`（fail-closed
  typed variants）、`ApplyReceipt`/`ApplyEffects`/`ApplyOutcome`、
  `ChangeSetSource`/`PluginAvailability`/`ApplySources` ports、
  `apply_selected` engine、`target_fingerprint` public API。
- `entity.rs`: `entity_value`（kind → canonical JSON 抽出）、
  `fingerprint`（absent → `null`）、`payload_for`（protected
  membership field 調整）、`write_all`/`write_entity`/
  `reconcile_membership`/`reseed_past`（typed transplant +
  IdGenerator watermark 維持）。
- `plan.rs`: `plan_changes`（selection → `PlannedWrite`、whitelist +
  existence + UnknownChange 検査）、`ApplySelectedOp`
  （`Operation` impl — `validate` は side-effect free、`apply` は
  scratch clone へ write_all）、`preflight_assets`/`plugins`/`ports`。
- `journal.rs`: `ApplyJournal` trait、`InMemoryApplyJournal`、
  `DirectoryApplyJournal`（staged publish per key）。

### `restore/`（Slice C）

- `RestoreRequest`/`MissingAssetsPolicy`/`RestorePorts`（
  `RestoreBlobSource`/`RestorePluginStateSource`/
  `RestoreStorageProbe`）。
- `RestoreError`（Cancelled/InvalidRequest/DestinationExists/
  ParentMissing/StagingConflict/MissingAsset/PluginStateRequired/
  LowDisk/Io/Indeterminate/Validation）。
- `RestoreReceipt`（outcome・source/new project id・snapshot digest・
  locator・revision・asset/plugin/stable-id report・DirSyncEvidence・
  effects）。
- staging manifest `restore.json`（Started→Materialized→Validated→
  Published）による crash recovery。dest rename 後の manifest が
  `Validated` なら bookkeeping を完了、`Published`+receipt なら
  検証付き replay。

## 不変条件

- apply: live/Saved HEAD/VCS index/worktree は receipt effects で
  明示。controller apply 1 回 = undo 1 step。
- restore: source は read のみ。destination は `Absent` 前提、
  publish は rename 一発。staged bytes は publish 前に全件
  digest readback。
- 両者とも `SnapshotCancel` で cooperative cancel、cancelled は
  何も書かない（restore は staging 作成前に判定）。

## エラー写像

- `Error::StaleRevision` → `ApplyError::StaleRevision`
- controller `validate`/`apply` 失敗 → `ApplyError::Rejected`
- journal IO/indeterminate → `ApplyError::Journal`
- ChangeSet/snapshot 解決不可 → `ChangeSetNotFound`/`Source`
