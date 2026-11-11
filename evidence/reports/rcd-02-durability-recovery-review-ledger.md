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
| Synvelis commit | `feature/116-rcd02-durability` — origin/main `8d925b5` merge 済み (PR #210 `67e0870`・#239 `17fc3de`・TIM-02 #270 を含む) + this change set |
| Cargo.lock Ruviform SHA | `b02942c2f9e6e195d0d752803b077866468fe58e` (SV#28 pin 更新 merge 済み; task3 で `ruviform-io-file` 依存 edge 追加) |
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
| Indeterminate publish | `PublishInject::Indeterminate` → registry 適用済み・marker 未公開/不一致を `PublishIndeterminate` として区別、reconcile で冪等修復・二重 marker/take なし。publish は `StagedFile` pipeline 上で成功後に応答喪失を注入（destination は公開済み） | `rcd02_publish_indeterminate_adopt_recovers`、`rcd02_commit_receipt_lifecycle_and_reconcile` Pass | Pass | `reconcile_commit_receipt` → `CommittedConfirmed` 収束 |
| Publish seam on StagedFile pipeline | `Denied`=create 前（destination/temp 未接触）・`SyncFail`=seal 後 publish 前（temp abort で除去・destination 未公開）・`Indeterminate`=publish 成功後 Err（destination 公開済み）が `publish_atomic_faultable` で決定的に再現される。call 計数は faultable 呼出しのみ | `staged_publish.rs` 7 tests（`publish_faultable_*`・`publish_atomic_*`）Pass | Pass | temp residue は SyncFail/Denied で残らない（旧 `*.tmp` residue は新 pipeline では発生しない） |
|| Unsynced-write 喪失（sync frontier 後の dirty byte 喪失） | checkpoint 後に書き込まれ `sync` されなかった media/journal byte を `set_len` truncate で喪失させたとき、replay は最終 durable frontier のみ intact とし、`review_stream(Adopt)` は verified prefix のみ `FinalizedOutcome::Partial`（3 span/192 frames のみ、喪失 chunk を含まない）で Complete に昇格しない | `rcd02_unsynced_loss_truncates_to_sync_frontier` Pass | Pass | frontier は最後の有効 checkpoint。truncate は power-loss-like な dirty-byte 喪失の fixture モデル（実電源断ではない） |
|| Unsynced-write 非対称喪失（media/journal 片側のみ残存） | media tail 残存+journal tail 喪失 → `StreamClass::Partial` + `unverified_tail_bytes`（journal coverage 外）。journal tail 残存+media tail 喪失 → `StreamClass::Partial` + fault "media shorter than journaled coverage" + `validated_tail_bytes`。両者とも Adopt は verified/物理 prefix のみ `FinalizedOutcome::Partial` で、unverified byte を自動削除・truncate しない | `rcd02_unsynced_loss_asymmetric_tail` Pass | Pass | 非対称 tail は喪失再現後も物理 file を保持し review 判定へ委ねる |

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

## RCD-02-REALFS — Windows/NTFS real-FS 実測

Lane: `recording-poc realfs` 子 process を実 temp FS 上で実行する
opt-in lane（`mise run test-recording-real-fs` →
`cargo test -p recording-poc --locked --test real_fs_probe --
--ignored --test-threads=1`、CI 対象外）。Fixture:
`crates/recording-poc/tests/real_fs_probe.rs`（2 tests）+
`crates/recording-poc/src/main.rs` `realfs` subcommand。
Environment: 本機 Windows (win32) / NTFS / rustc 1.99.0 via mise。
観測値は全て実測（`key=value` stdout）— 真の power-loss（電源断）
は非破壊的に自動化不能のため **NotRun** のままとし、process kill
（TerminateProcess 相当）で代替可能な範囲のみ実測する。

| Case | Expected | Observed | Status | Notes |
| --- | --- | --- | --- | --- |
| Rename 可視性/原子性（missing dest・replace） | rename が原子的に destination を作成/置換し内容が新 source になる | `rename_missing_dest=ok`、`rename_replace=ok`、`rename_replace_content_ok=true` | Pass | NTFS 上の `MoveFileExW` 系挙動の実測 |
| Rename open-handle semantics | open 中 file/destination への rename の可否を観測（環境依存・偽装しない） | `rename_src_open=ok`、`rename_dst_open=ok` | Pass | Rust std は `FILE_SHARE_DELETE` 等を付与するため Windows でも成功 — 観測値が正本 |
| create_new no-replace | 既存 destination への `create_new` open が `AlreadyExists` で拒否される | `create_new_existing=err:AlreadyExists` | Pass | `PublishMode::CreateNew` の基礎 |
| Dir sync capability | directory の open/sync が非対応なら `DirSyncCapability::Unsupported` と正直に検出される | `dir_open=err:PermissionDenied`、`dir_sync=err:PermissionDenied`、`detect_dir_sync=Unsupported` | Unsupported | Windows/NTFS で実測非対応。raw 観測と detector 分類が整合し `Supported` を偽装しない |
| StagedFile publish（replace / create_new / conflict / residue） | `publish_atomic` は content を原子的置換、`publish_atomic_create_new` は新規作成・conflict で既存を保持、publish が temp residue を残さない | `publish_atomic_replace=ok`、`publish_atomic_content_ok=true`、`publish_atomic_create_new=ok`、`publish_create_new_conflict=err:AlreadyExists`、`publish_create_new_content_kept=true`、`publish_residue=0` | Pass | 実 FS 上で StagedFile pipeline の observable semantics を実測 |
| Unsynced byte の in-process 実長 | `sync_data` なしで close した file の実長・内容が write 分を反映 | `file_nosync_len=4096`（write 4096 byte 一致） | Pass | unsynced byte は file system 上に観測可能（durable とは別） |
| Sync frontier への truncate | `set_len` が file を任意長へ truncate し fixture モデルの実 FS 裏付けになる | `set_len=1024`（4096→1024 byte） | Pass | `rcd02_unsynced_loss_*` fixture の truncate が実 FS で再現可能 |
| Process kill と unsynced tail | `write-unsynced` 子 process（synced 1024 byte → `sync_data` → unsynced 2048 byte・ready 後 kill）の post-kill file を別 process から観測 | post-kill len=3072 = synced+unsynced、prefix 0xA5・tail 0x5A 共に intact | Pass | **process kill は OS の dirty page を失わせず unsynced 喪失を再現しない** — よって真の power-loss 行は `NotRun` に留まる |
| 真の power-loss（電源断・OS crash 相当） | 非破壊的に自動化不能 | 未実施 | NotRun | 実機電源断のみ検証可能。fixture truncate はモデル、process kill は dirty page を保持するため代替にならない |

## Explicitly out of scope (never Pass)

- 真の power-loss（電源断・OS crash 相当）の実機 durability —
  **NotRun**（非破壊的に自動化不能。process kill で代替可能な
  範囲は RCD-02-REALFS で実測済みだが、process kill は dirty
  page を保持するため power-loss 喪失を再現しない）。
- 他 OS・他 FS（macOS/Linux 等）の rename/dir sync/unsynced
  挙動 — **NotRun**（Windows/NTFS のみ RCD-02-REALFS で実測）。
- 実 device 録音・disconnect/LowDisk 実 fault（#115）— **NotRun**。
- 製品 Comp/Take/Recovery GUI E2E（#118）— **NotRun**。
- consumer 精度・AudioLens cross-check（#219/#294）— **NotRun**。
- macOS/Linux 等の多 OS FS matrix — **NotRun**（Windows/NTFS のみ
  RCD-02-REALFS で実測。上の行と併せて多 OS 展開は未実測）。
- RT callback 上の実オーディオスレッド noalloc/noblock 計測 —
  `test-rt` の contract test のみ **Pass**、実デバイス計測は **NotRun**。
- #86 verified blob import/Asset 登録・#84 project 側 HEAD CAS 参照進化
  への実接続 — **NotRun**（各 Issue gate。`verified_sha256` は provenance
  記録どまり）。#84 の recording 側 publish 接続は task3 で
  `ruviform_io_file::StagedFile`/`SealedFile::publish` 経由に実装済み
  （`docs/impl/ir/rcd-02-stagedfile-publish-migration.md`）。

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

StagedFile publish migration slice（task3、branch
`feat/116-stagedfile-publish`）: `crates/synvelis-recording/tests/
staged_publish.rs` 新規 7 tests を追加。Windows/NTFS・rustc 1.99.0
で `cargo test -p synvelis-recording` 実行済み（補助証拠; canonical
`mise run test` の結果は PR 本文に記録）。`detect_dir_sync` の
Windows 観測値（`DirSyncCapability::Unsupported`）は manifest
`capabilities.directory_sync` へ従来通り記録され、publish pipeline
の `Durability::FileSync` は dir sync を要求しないため capability
非対応環境でも偽装なく動作する（`staged_publish.rs` の Replace/
CreateNew/seam test が Windows/NTFS 上で Pass）。

Unsynced-loss fixture + real-FS probe slice（本 slice、branch
`impl/116-unsynced-loss-real-fs-probe`）: `recovery_review.rs` +2
tests（`rcd02_unsynced_loss_truncates_to_sync_frontier`・
`rcd02_unsynced_loss_asymmetric_tail`、truncate による dirty-byte
喪失の決定的再現）、`recording-poc` に `realfs` subcommand と
`tests/real_fs_probe.rs` 2 tests を追加し、`mise.toml` に opt-in
manual task `test-recording-real-fs` を 1 件追加（既存
`test-recording-*` 系と同型・CI 対象外）。fixture tests・real-FS
lane ともに Windows/NTFS・rustc 1.99.0 で実行済み — canonical
`mise run test` / `mise run test-recording-real-fs` の結果は PR
本文に記録。
