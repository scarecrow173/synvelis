# RCD-02 Durability / Recovery Review / Commit Receipt — Evidence Ledger (synvelis#116)

Integrated acceptance evidence for the RCD-02 Journal/Staging fault slice
(issue #116 の受入 4 行)。Every row records an independently observed
Expected/Observed pair. `Pass` here means *the stated headless/fixture
lane only* — it never implies real OS/FS durability, real devices,
product GUI, RT-thread measurement, or consumer precision. Those lanes
are `NotRun`/`Unsupported` with reasons, per the vocabulary in
`docs/specs/sf-06-cross-gate-oracle-case-ledger.md`.

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis commit | `feature/116-rcd02-durability` — base `3a9f4ca` (PR #210 `67e0870` + PR #239 `17fc3de` を含む) + this change set |
| Cargo.lock Ruviform SHA | `0f5b57372e9a43f84415f7964b2f9636774174f0` |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.0 (b940084d7) via mise; MSRV rust 1.89 |
| Fixture | temp-dir `StagingArea`/`StreamWriter` on real temp FS + `Inject`/`PublishInject` fault seams (fake/injected fault、実 FS 破壊なし); `ProjectController` + `DirectoryBundleStore` で commit/save/reload/undo を検証 |
| CI/PR | see PR body (Refs synvelis#116) |

Status vocabulary: `Pass` = measured pass in the stated lane;
`Pass*` = persisted/headless-layer pass only; `Fail` = measured failure;
`NotRun` = lane exists but was not measured here; `Unsupported` =
capability genuinely unavailable; `Blocked` = implementation missing.

## RCD-02-DUR-01 — checkpoint durability evidence

Fixture: `crates/synvelis-recording/tests/recovery_review.rs`.
Lane: headless writer + fault injection.

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| Checkpoint evidence per attempt | media sync → checkpoint append → journal sync → dir sync の各段の成否が試行ごとに `DurabilityEvidence` へ記録され、失敗時も観測済み stage を保持してから Err | `rcd02_checkpoint_evidence_records_each_stage` Pass | Pass | evidence は失敗を隠さない |
| DirSync unsupported honesty | `dir_sync=Unsupported` は durable 保証と混同されず `DirSyncState::Unsupported` として記録 | `rcd02_evidence_dir_sync_unsupported_is_honest` Pass | Pass* | Windows では dir sync capability が実際に Unsupported — 偽装しない |

## RCD-02-FAULT — fault matrix（kill/short write/sync/permission/diskfull/indeterminate）

Fixture: `recovery_review.rs` + `vs_c.rs` regression. Lane: injected
fault on real temp FS files（fault seam は `FaultWriter`/`Inject`/
`PublishInject`、実 FS/OS 破壊は行わない）。

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| Kill/power-loss 相当（finalize なし drop） | durable frontier まで検証済み、未確定 stream は `ReviewState::Pending` で自動 finalize しない | `rcd02_fault_matrix_journal_torn_tail_review_adopt`、`rcd02_commit_receipt_adopted_partial_finalize_record_absent`（drop + torn tail 注入）Pass | Pass | `review_stream(Adopt)` で verified prefix のみ Partial finalize |
| Media short write | 部分 write が verified prefix を汚染しない。CRC 不一致 tail は `unverified_tail_bytes` へ分離 | `vsc_partial_write_stream_failed`、`vsc_failed_chunk_preserves_validated_prefix`（vs_c）Pass | Pass | frontier は最後の有効 checkpoint |
| Journal short write | `Inject::ShortWrite` で journal tail torn → replay は intact prefix で停止、`StreamClass::Partial` + `ReviewState::Pending`、Adopt は `FinalizedOutcome::Partial`（`source_frames=128` = verified 2 chunks のみ） | `rcd02_fault_matrix_journal_short_write_torn_append` Pass | Pass | unverified tail（1 chunk 分）は finalize 対象外 |
| Journal torn tail / checksum / sequence gap | intact prefix で replay 停止、`StopReason` 分類（TornTail/BadChecksum/SequenceGap） | `rcd02_fault_matrix_journal_torn_tail_review_adopt`、`vsc_journal_torn_tail_and_checksum_and_gap` Pass | Pass | — |
| Sync failure（media/journal） | `sync_data` 失敗は write 失敗と区別され `checkpoint()` Err、evidence に失敗 stage を記録 | `rcd02_fault_matrix_write_sync_permission_diskfull`、`vsc_sync_failed_distinct_from_write` Pass | Pass | — |
| DirSync unsupported | capability `Unsupported` → `DirSyncState::Unsupported`、durable 宣言はそれ以下の保証に留まる | `rcd02_evidence_dir_sync_unsupported_is_honest` Pass | Pass* | — |
| Permission denied（write/sync/publish） | `PermissionDenied`/`SyncPermissionDenied`/publish 拒否が fail-closed で観測される | `rcd02_fault_matrix_write_sync_permission_diskfull` Pass | Pass | 部分 write は verified prefix を汚染しない |
| Disk full（injected ENOSPC） | `ByteLimit` で write 失敗、失敗までの evidence を保持 | `rcd02_fault_matrix_write_sync_permission_diskfull`、`vsc_disk_full_byte_limit` Pass | Pass | — |
| Indeterminate publish | `PublishInject::Indeterminate` → registry 適用済み・marker 未公開/不一致を `PublishIndeterminate` として区別、reconcile で冪等修復・二重 marker/take なし | `rcd02_publish_indeterminate_adopt_recovers`、`rcd02_commit_receipt_lifecycle_and_reconcile` Pass | Pass | `reconcile_commit_receipt` → `CommittedConfirmed` 収束 |

## RCD-02-REVIEW — Recovery Review 明示判定

Fixture: `recovery_review.rs`。Lane: headless scan/review。

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| Adopt verified prefix | `Adopt` は verified prefix のみ finalize。unverified tail 残存時は `FinalizedOutcome::Partial` で Complete に昇格しない | `rcd02_review_adopt_verified_prefix_and_idempotent` Pass | Pass | 再 Adopt は同一 finalized ref（冪等） |
| Finalize Complete claim + tail | journal Finalize=Complete でも tail が残る/非 CleanEnd なら Partial へ降格 | `rcd02_finalize_record_complete_with_tail_downgrades_to_partial` Pass | Pass | — |
| Hold | media/journal/finalized を一切変更せず判定を durable に永続化 | `rcd02_review_hold_persists_without_touching_files` Pass | Pass | — |
| Discard fail-closed | `MediaReferenceProbe` が `Clear` のみ削除許可。`Referenced`/`Unknown`/probe 不在は拒否・無変更 | `rcd02_review_discard_fail_closed_and_idempotent` Pass | Pass | probe 不在でも fail-closed |
| Review record unreadable | 判定記録が存在するのに読めない/デコード不能 → `Unreadable` で拒否（上書きしない） | `rcd02_review_record_unreadable_is_fail_closed` Pass | Pass | — |

## RCD-02-RECEIPT — commit receipt 分類と冪等再照合

Fixture: `recovery_review.rs` + `project_adapter.rs` + `vs_d.rs`。
Lane: headless commit/receipt/reconcile。

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| Receipt lifecycle | Empty/Reserved/CommittedConfirmed/PublishIndeterminate/Conflict/Invalid を区別。`project_saved` は saved revision 比較で Commit→Save 未完了を区別 | `rcd02_commit_receipt_lifecycle_and_reconcile` Pass | Pass | — |
| finalized.json 喪失 vs journal Finalize record | marker file 不在 + journal Finalize=Complete は別状態として観測（`finalized_present=false` ∧ `finalize_record=Some(Complete)` ∧ `journal_stop=CleanEnd`）、committed sources なし | `rcd02_commit_receipt_finalize_publish_lost_is_distinct` Pass | Pass | — |
| Adopted Partial の finalize record 不在 | Partial adopt は finalized file を残すが journal Finalize record を持たず commit は `NothingCommitted` + recoverable `partial-finalize`、receipt は `finalized_present=true` ∧ `finalize_record=None` ∧ `journal_stop=TornTail` | `rcd02_commit_receipt_adopted_partial_finalize_record_absent` Pass | Pass | — |
| Commit → Save → Reload → Undo chain | Finalized(Complete+verified_sha256) → Commit Applied → Undo で commit 由来 entity 全消去（staging media/manifest 残存）→ Redo で同一 Stable ID 復元 → Save → Reload（別 Start 相当）で project snapshot 一致 → 再 commit は `AlreadyApplied`・entity ID 集合/content hash 不変・receipt `CommittedConfirmed`/`project_saved=true` | `rcd02pa_commit_save_reload_undo_restore_ids` Pass | Pass* | `applied_commits` registry + commit-manifest + commit.json marker の三重冪等 |
| 別 Start 二重 Commit 拒否 | crash 前後の retry・save/reload 後 retry・staging drift 後 retry が `AlreadyApplied` で duplicate take/asset/segment/pass を生成しない | `vs_d_crash_before_commit_retry`、`vs_d_crash_after_commit_retry_no_dup`、`vs_d_retry_after_staging_drift_is_already_applied`、`vs_d_recommit_after_undo_recovers` Pass | Pass* | — |
| Undo/Redo ID 復元 | undo/redo が Stable ID を保持して entity を復元 | `vs_d_undo_redo_restores_ids`、`vs_d_save_reload_integrity` Pass | Pass* | — |
| Commit 失敗の区別 | queue overflow/epoch discontinuity/writer error は commit 対象外または recoverable として分類 | `vs_d_queue_overflow_not_committed`、`vs_d_epoch_discontinuity_not_committed`、`vs_d_writer_error_recoverable`、`vs_d_revision_conflict_recoverable`、`vs_d_integrity_conflict_same_identity` Pass | Pass | — |

## RCD-02-SHA — verified-prefix SHA-256 / digest 分離

Fixture: `project_adapter.rs`。Lane: headless finalize/recovery/commit。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| verified_sha256 は verified prefix のみ | `sha256:<64hex>` が durable frontier の `media_byte_len` までを hash し unverified/journaled tail を含めない。finalize/recover-Adopt/manifest の全経路で記録 | `rcd02pa_finalize_records_verified_prefix_sha256`、`rcd02pa_recovery_adopt_records_verified_prefix_sha256`、`rcd02pa_commit_manifest_carries_verified_sha256` Pass | Pass |
| CRC32 fold との分離 | `IntegrityDigest`（CRC32、integrity/idempotency 専用）と `verified_sha256`（content identity）が型・命名・`digest_scheme` で分離。legacy manifest は serde default で互換 | manifest 互換 test + `DigestScheme` 分離（`rcd02pa_commit_manifest_carries_verified_sha256` 内で検証）Pass | Pass |

## RCD-02-PROBE — MediaReferenceProbe / saved HEAD

Fixture: `project_adapter.rs`。Lane: headless project/library 参照。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| Discard 参照判定 | live project の committed media・open session・undo/redo 履歴・saved HEAD 参照を `Referenced` とし `Discard` を拒否。orphan staging は `Clear` で Discard 可 | `rcd02pa_probe_clear_allows_discard`、`rcd02pa_probe_open_session_blocks_discard`、`rcd02pa_probe_blocks_committed_media_in_live`、`rcd02pa_probe_blocks_via_undo_redo_history`、`rcd02pa_probe_blocks_via_saved_head`、`rcd02pa_probe_blocks_open_state_in_history` Pass | Pass* |
| Fail-closed Unknown | HEAD snapshot の schema 不一致・decode 失敗・project_id 不一致・store error は `Unknown`（推測で読まない）。`CompositeProbe` は Referenced 優先・Unknown 保持 | `rcd02pa_probe_unknown_head_states_fail_closed`、`rcd02pa_composite_probe_semantics` Pass | Pass* |
| saved HEAD revision adapter | `head_revision`/`snapshot_revision` が read-only で saved revision を返し receipt の `project_saved` 判定に使える | `rcd02pa_head_revision_and_commit_receipt_saved` Pass | Pass* |

## Explicitly out of scope (never Pass)

- 実 OS/FS durability 実測（power-loss/kill 実機、rename/sync 原子性、
  dir sync capability の実挙動）— **NotRun**（実機 gate）。
- 実 device 録音・disconnect/LowDisk 実 fault（#115）— **NotRun**。
- 製品 Comp/Take/Recovery GUI E2E（#118）— **NotRun**。
- consumer 精度・AudioLens cross-check（#219/#294）— **NotRun**。
- macOS/Linux 等の多 OS FS matrix — **NotRun**（Windows/NTFS のみ）。
- RT callback 上の実オーディオスレッド noalloc/noblock 計測 —
  `test-rt` の contract test のみ **Pass**、実デバイス計測は **NotRun**。
- #86 verified blob import/Asset 登録・#84 StagedFile/HEAD CAS への
  実接続 — **NotRun**（各 Issue gate。`verified_sha256` は provenance
  記録どまり）。

## New/changed fixture files (this change)

- `crates/synvelis-recording/tests/recovery_review.rs` — +3 tests
  （journal `ShortWrite` torn append、finalize publish lost receipt、
  adopted-partial finalize-record-absent receipt）
- `crates/synvelis-recording/tests/project_adapter.rs` — +1 test
  （commit→save→reload→undo/redo→AlreadyApplied e2e）

Prior slices: PR #210（`67e0870`、recovery_review.rs 初版 +
DurabilityEvidence/Review/Receipt 実装）、PR #239（`17fc3de`、
project_adapter.rs 初版 12 tests + verified_sha256/ProjectMediaProbe/
saved-HEAD adapter 実装）。本 change set で recovery_review.rs +3・
project_adapter.rs +1（各 14/13 tests）。全 27 tests は
Windows/rustc 1.99.0 で `cargo test` 実行済み（補助証拠; canonical
`mise run test` の結果は PR 本文に記録）。
