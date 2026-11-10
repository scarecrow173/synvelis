---
id: "PLAN-vcs05-apply-restore-headless"
type: "plan"
status: "approved"
title: "VCS-05 Apply/Restore headless — 実装 plan"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "SPEC-vcs05-apply-restore-headless"
    - "DSGN-vcs05-apply-restore-headless"
  implemented_by:
    - "TASK-vcs05-apply-restore-headless"
  supersedes: []
  superseded_by: []
---

# VCS-05 Apply/Restore headless — 実装 plan

## File map

| file | 内容 |
| --- | --- |
| `crates/synvelis-project/src/apply/mod.rs` | intent/receipt/error/engine/fingerprint API |
| `crates/synvelis-project/src/apply/entity.rs` | entity 抽出・fingerprint・transplant・membership reconcile |
| `crates/synvelis-project/src/apply/plan.rs` | selection→plan・whitelist・preflight・`ApplySelectedOp` |
| `crates/synvelis-project/src/apply/journal.rs` | `ApplyJournal` + memory/dir 実装 |
| `crates/synvelis-project/src/restore/mod.rs` | RestoreRequest/ports/error/receipt/staged publish engine |
| `crates/synvelis-project/src/lib.rs` | `pub mod apply; pub mod restore;` + re-export |
| `crates/synvelis-project/tests/vcs05_apply.rs` | apply fixtures 13 件 |
| `crates/synvelis-project/tests/vcs05_restore.rs` | restore fixtures 7 件 |

`store.rs`/`migration.rs`/`project.rs`/`diff/adapters.rs` は**変更不要**
（bundle layout は既存 `SnapshotEnvelope`/`snapshot_name`/`SCHEMA_VERSION`
を再利用、lineage は receipt のみで schema v11 不要と spec で決定）。

## 順序

1. `apply/` 4 file → `cargo check` → test fixture で CAS/whitelist/
   undo/replay を検証。
2. `restore/` → destination validation → staged materialize →
   readback validate → rename publish → receipt/replay。
3. `lib.rs` 配線 → `mise run fmt-check check lint test build
   test-msrv`。

## Risk register

| risk | 対応 |
| --- | --- |
| fingerprint が Added 対象で live 不在 | absent→`null` 両側符号化 + target_object presence は plan で別途検査 |
| Pending journal + live==after で plan が TargetConflict | fingerprint object set を宣言 selection から構成し reconcile を plan より先に実行 |
| rename 後・manifest Published 前の crash | dest manifest `Validated` を検出し bookkeeping 完了 or replay |
| dir fsync 非対応 platform | `DirSyncEvidence::Unsupported` を receipt に正直記録（gate にしない） |
| `Removed`/unsupported kind | plan 時 `ApplyError::Unsupported` で fail closed |

## Verification matrix

| 受入 | fixture |
| --- | --- |
| apply 成功/1 undo | `apply_added_track_commits_one_undo_step`, `apply_project_rename_only_touches_name` |
| approval/revision/fingerprint/identity | `unapproved_*`, `stale_revision_*`, `fingerprint_drift_*`, `wrong_source_*`, `unknown_change_id_*` |
| fail-closed unsupported/missing/degraded | `removed_change_is_unsupported`, `missing_asset_fails_closed`, `degraded_endpoint_*` |
| idempotent/ACK gap | `second_apply_replays_*`, `ack_gap_reconciles_*`, `pending_without_commit_*` |
| restore publish/replay | `restore_publishes_verified_new_copy` |
| restore fail-closed | `missing_asset_*`, `degraded_policy_*`, `existing_destination_*`, `unsafe_destinations_*`, `low_disk_*`, `cancelled_*` |
