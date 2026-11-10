---
id: "TSPEC-vcs05-apply-restore-fixtures"
type: "test-spec"
status: "approved"
title: "VCS-05 apply/restore fixtures"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "SPEC-vcs05-apply-restore-headless"
    - "DSGN-vcs05-apply-restore-headless"
  verifies:
    - "TASK-vcs05-apply-restore-headless"
  supersedes: []
  superseded_by: []
---

# VCS-05 apply/restore fixtures

## Feature: Safe Apply Selected Change

- **Rule VCS-APPLY-01** 選択 change のみ適用、成功は 1 undo step、
  receipt effects が live/saved/vcs/git/engine を正直に報告。
  Examples: `apply_added_track_commits_one_undo_step`,
  `apply_project_rename_only_touches_name`.
- **Rule VCS-APPLY-02** approval 空・stale revision・fingerprint
  drift・誤 source_snapshot_id・未知 change id は全て fail closed で
  live 不変。Examples: `unapproved_*`, `stale_revision_*`,
  `fingerprint_drift_*`, `wrong_source_*`, `unknown_change_id_*`.
- **Rule VCS-APPLY-03** `Removed`/unsupported kind・missing/unknown
  asset・degraded/partial endpoint は fail closed。
  Examples: `removed_change_is_unsupported`,
  `missing_asset_fails_closed`, `degraded_endpoint_*`.
- **Rule VCS-APPLY-04** 二重 apply は `Replayed` で二重実行なし。
  Pending journal + live==after は reconcile、Pending + before は
  正常再試行。Examples: `second_apply_*`, `ack_gap_*`,
  `pending_without_commit_*`.

## Feature: Restore As New Copy

- **Rule VCS-RESTORE-01** verified bundle を新 project id で publish、
  HEAD/snapshot/blob/receipt/manifest を readback、source 不変、
  同一 request は `Replayed`。Example:
  `restore_publishes_verified_new_copy`.
- **Rule VCS-RESTORE-02** missing asset(Fail)・degraded 許容・
  existing destination・unsafe leaf・low disk・cancel は fail closed。
  Examples: `missing_asset_*`, `degraded_policy_*`,
  `existing_destination_*`, `unsafe_destinations_*`, `low_disk_*`,
  `cancelled_*`.

## NotRun（後続 Issue）

- 実 OS filesystem durability（Windows/macOS/Linux 個別）→ #97
- 実 Git/Provider/plugin host E2E → #97
- GUI・RT・AudioLens cross-consumer → #294
