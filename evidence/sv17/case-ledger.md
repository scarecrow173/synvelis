# SV17 — Project / Asset / Persistence 設計契約 受入証拠台帳 (synvelis#17)

Issue #17「Project / Asset / Persistence — Immutable
Snapshot・Media Lifecycle・Safe Save・Recovery」の受入条件
13 項目（`PROJ-RCP-01` + PAP 系 12 項目）を、既存
fixture の被覆状況・先行 slice の実測証拠・本 slice の
未実行項目へ写像した台帳。**本 slice は docs/evidence
のみであり、実装 crate の `check`/`lint`/`test` は再実行
していない** — fixture の存在はソース確認で pin し、
先行 slice の実測は SHA・日時付きで参照する。本 slice で
実行した検証は `mise run fmt-check` のみ（結果は
`docs/impl/ir/project-persistence.md` に記録）。

`Pass`/`Pass*` は **先行 slice が記録した実測の参照** で
あり、本 slice で再測したことを意味しない。「fixture 設定
済み・本 slice 未実走」と「構成済みテストなし=NotRun」
を厳密に区別する（`docs/test-specs/project-persistence.md`
Rule 14）。Mock FS・injected fault・`PublishInject`/`Inject`
seam の PASS は fault *classification* の証拠であり、実 FS
durability・物理電断の証明ではない。

## Environment

| Field | Value |
| --- | --- |
| Synvelis branch | `scarecrow173/sv17-project-persistence`（task 指定ブランチ名は `docs/17-project-persistence`） |
| Synvelis base SHA | `98ae4eeb9d4236cec63ba1c8d5cda5521c4e8fe4` + merge `origin/main` `93690e37291769b6364e7861e8d17ade27ec16c7`（PR #398 synvelis#10 Project Model docs chain） |
| OS（本 slice 実行環境） | Windows (win32) |
| Toolchain | docs/markdown のみ — `mise run fmt-check` が正規検証 |
| Fixture lane | headless `cargo test`（temp-dir real FS + injected fault seam）。実機 GUI/Device/Plugin/実 FS 電断は対象外 |
| 先行実測の参照先 | `evidence/sv84/case-ledger.md`（StagedFile/CAS・Windows NTFS 実測）・`evidence/reports/rcd-02-durability-recovery-review-ledger.md`（Recording fault matrix・Windows 実測）・`evidence/reports/vcs-06-fixture-matrix-ledger.md`（VCS fixture matrix・Windows 実測）・`evidence/reports/syn-io-02-os-l1-evidence.md`（CI leg 照合） |

Status vocabulary（`POLICIES.md` §3 準拠）:

- `Pass` = 先行 slice で実測 pass が記録されている
  （参照 SHA/環境を明記）。
- `Pass*` = persisted/headless 層のみの pass（実 FS
  durability・実機 GUI/Device/Plugin を含まない）。
- `NotRun` = fixture が存在するが本 slice で実行していない
  （先行実測を参照する場合はその旨を明記）、または構成
  済みテストがない契約。
- `Unsupported` = capability が環境上利用不可（例: dir
  sync `Unsupported` の正直分類）。
- `Deferred` = POLICIES.md §2 の別 Owner/将来 lane へ委譲
  （実 GUI/実 OS/実 Plugin/実機・物理電断・Network/Cloud
  FS・RCP 系 Artifact attach）。

## Case Ledger — Issue #17 受入条件 13 項目

| Case | 受入条件（Issue #17 転記） | 根拠 fixture（crate::test / test file） | Expected | Observed（先行実測/本 slice） | Status | 備考 |
| --- | --- | --- | --- | --- | --- | --- |
| SV17-C01 | `PROJ-RCP-01`: Save durabilityとRender Jobを別Receipt化し、DiskFull/Crash/late Artifact attachでもHEAD・Raw Asset・Undoの原子性が保護される。 | `crates/synvelis-project/tests/{store_publish,head_cas}.rs`・`crates/synvelis-recording/tests/recovery_review.rs`（publish/CAS/receipt 分類）＋ **Artifact attach 統合 fixture は未設計** | Saved HEAD 確定は Render Job 完了を待たない。DiskFull/Crash/late attach で HEAD・Raw Asset・Undo の原子性が保護される。 | publish/CAS/receipt 分類は先行実測で Pass（`sv84` C01〜C13・`rcd-02` ledger）。Artifact attach 固有の統合 fixture・実測は **構成済みテストなし**。 | **NotRun**（部分被覆: publish/CAS/receipt 分類は Pass 参照済み、Artifact attach 統合は RCP 系 #290〜#297 へ Deferred） | Project 側境界は design D5 で固定。Render Job Receipt 実装は RCP Issue の責務。 |
| SV17-C02 | Live R123のままSaved R120を非同期保存でき、未保存変更を保持。 | `crates/synvelis-project/tests/controller_store.rs`（`save_reload_preserves_identity_ids_and_revision`・`undo_redo_issue_new_revisions_and_keep_saved_intact`）・`head_cas.rs`（`save_cas_commits_only_when_head_matches`・`controller_save_cas_rejects_moved_head`） | live revision を進めず saved HEAD のみを target に save でき、未保存変更（live-only edits）が保持される。 | 先行実測: `sv84` case-ledger・`syn-io-02-os-l1-evidence.md` で store fixture Pass（Windows NTFS）。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行実測 Pass 参照・本 slice 未実走） | Save Job scheduler 本体は #92 系。contract は `save()`/`save_cas()` で表現済み。 |
| SV17-C03 | 2つのSave Jobが逆順に完了しても新しいSaved HEADを旧Jobが覆さない。 | `crates/synvelis-project/tests/head_cas.rs`（`concurrent_save_cas_admits_single_winner`・`two_process_save_cas_single_winner_typed_conflict`・`stale_head_lock_is_broken`・`cas_failure_releases_the_lock`） | 同 expected-head の協調 writer は exactly one winner。逆順完了でも古い Job の expected-head は stale → `Conflict`。 | 先行実測: `sv84` case-ledger + `syn-io-02-os-l1-evidence.md`（2-process CAS e2e・PR #382）で Pass。 | **NotRun**（fixture 設定済み・先行実測 Pass 参照・本 slice 未実走） | Save Job queue/cancel/UI receipt は #92 系。CAS 契約レベルの保証のみ。 |
| SV17-C04 | Snapshot/HEAD/Directory Syncの各時点のCrashを注入し有効なRootを検証。 | `crates/synvelis-project/tests/store_publish.rs`（`saves_publish_snapshots_then_repoint_head`・`failed_snapshot_publish_keeps_head_and_destination`・`failed_head_publish_keeps_snapshot_orphan_and_old_head`・`leftover_temp_siblings_do_not_disturb_the_store`・`reopen_reproduces_head_and_snapshots`・`concurrent_same_name_publish_admits_one_winner`）・`head_cas.rs`（lock 喪失 fencing） | snapshot publish→HEAD repoint→dir sync の各 crash window で `head()`/`read_snapshot()`/再 Open が有効な Root を正直に報告。orphan 許容・HEAD 逆行なし。 | 先行実測: `sv84` case-ledger C01〜C13 で Pass（Windows NTFS・injected/process-kill 相当）。物理電断は fixture 範囲外。 | **NotRun**（fixture 設定済み・先行実測 Pass 参照・本 slice 未実走。物理電断・真 power-loss は全 OS で Deferred） | crash window は決定的模擬。実 FS 電断は `test-recording-real-fs` 型の別 lane。 |
| SV17-C05 | Import転送・検証・Project Commitの失敗で未検証Asset/Clipを公開しない。 | `crates/synvelis-library/tests/import.rs`（`import_streams_verifies_publishes_and_commits`・`digest_mismatch_is_integrity_error`・`byte_budget_is_enforced_during_staging`・`stale_revision_rejected_then_explicit_rebase_converges`・`publish_failure_resumes_from_manifest`・`indeterminate_publish_is_reconciled_not_misreported`・`directory_sync_failure_is_recorded_not_hidden`・`missing_lane_target_is_rejected_without_clip`・`wrong_track_lane_target_is_rejected`）・`import_recovery.rs`（`save_failure_preserves_commit_and_recovers`・`stale_then_target_gone_rejects_and_orphan_protected`） | transfer→verify→commit の各失敗で未検証 Asset/Clip を公開しない。staged→atomic publish→controller commit の順序。`StaleRevision`/`IntegrityConflict`/`NotPublished`/`Indeterminate` を正直分類。 | 先行実測: ALIB-02 slice の `import.rs`/`import_recovery.rs` fixture（15+ tests）・`issue-86` wiring。個別 run 証拠は ALIB-02 ledger/CI に委譲。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行 slice で実装済み・本 slice 未実走） | Scripted publisher は fault classification のみ — 実 durability 証拠ではない。 |
| SV17-C06 | Content Blobの同一Byte列と論理Asset/Sourceの異なるID/来歴が両立する。 | `crates/synvelis-library/tests/import.rs`（`same_bytes_across_providers_share_one_verified_blob`）・`crates/synvelis-project/tests/alib02_digest.rs`（`crc_digest_collision_never_becomes_content_addressed`・`digest_kind_classifies_persisted_forms`）・`alib02_v5_real.rs`（`crc_collision_assets_stay_separate_and_weak`・`legacy_crc_blob_coexists_with_sha256_namespace`） | 同一 byte 列の Content Digest 共有（dedup）と論理 Asset/Source ID・Provenance の分離が両立。CRC 衝突・provider 跨ぎでも論理 ID は独立。 | 先行実測: ALIB-02 fixture で Pass（SHA-256 content-addressed namespace・CRC は weak/opaque のまま）。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行 slice で実装済み・本 slice 未実走） | `LocalBlobDir::object_rel` の `sha256:<hex>` namespace と legacy path の分離。 |
| SV17-C07 | Offline再生がRemote ServerやPreview Cacheに依存しない。 | `crates/synvelis-render/tests/x_e2e.rs`（`x_01_recorded_playback_export_bit_exact`・`x_03_identity_revision_undo_save_reload`・`x_04_finalized_only_visibility`・`x_05_fault_classes_separate`）・`crates/synvelis-render/tests/l2_e2e.rs`（`l2_01_master_export_reopen_pcm_match` 他）・`crates/synvelis-player/tests/l1_e2e.rs`（`l1_01_import_verifies_and_materializes`・`l1_03_headless_clip_playback_exact`・`l1_04_failure_classes_are_typed_and_atomic`・`l1_04_import_idempotency_and_orphan`） | 再生/export が project blob/media root のみ依存し、Remote Server・Preview Cache に依存しない。 | 先行実測: X/L1/L2 slice の E2E fixture で Pass（recording→playback→export bit-exact）。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行 slice で実装済み・本 slice 未実走） | Remote provider 再取得経路・Preview Cache は PAP arch で Deferred。Offline playback は local blob/media root のみ。 |
| SV17-C08 | Recording Commit後Save前CrashのMedia RecoveryとProject Recoveryを別の根拠として表示。 | `crates/synvelis-recording/tests/recovery_review.rs`（`rcd02_commit_receipt_lifecycle_and_reconcile`・`rcd02_commit_receipt_finalize_publish_lost_is_distinct`・`rcd02_commit_receipt_adopted_partial_finalize_record_absent`）・`project_adapter.rs`（`rcd02pa_commit_save_reload_undo_restore_ids`・`rcd02pa_head_revision_and_commit_receipt_saved`）・`vs_d.rs`（`vs_d_crash_before_commit_retry`・`vs_d_crash_after_commit_retry_no_dup`） | media 側（journal/StagingArea verified prefix）と project 側（controller/store）を別 receipt・別根拠で表示。`project_saved` は saved revision 比較で Commit→Save 未完了を区別。 | 先行実測: `evidence/reports/rcd-02-durability-recovery-review-ledger.md` で Pass（Windows・injected fault）。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行実測 Pass 参照・本 slice 未実走） | `MediaReferenceProbe` + commit receipt `project_saved` 分離。 |
| SV17-C09 | CommitIdentityの再試行でSource/Take/Clipの二重登録を防止。 | `crates/synvelis-recording/tests/vs_d.rs`（`vs_d_crash_after_commit_retry_no_dup`・`vs_d_retry_after_staging_drift_is_already_applied`・`vs_d_recommit_after_undo_recovers`・`vs_d_integrity_conflict_same_identity`・`vs_d_revision_conflict_recoverable`）・`recovery_review.rs`（`rcd02_review_adopt_verified_prefix_and_idempotent`）・`project_adapter.rs`（applied_commits registry） | `CommitRecordingCommand`+`RecordingCommitIdentity` の再試行で `AlreadyApplied` となり Source/Take/Clip 二重登録を防ぐ。三重冪等（registry+manifest+marker）。 | 先行実測: `rcd-02` ledger・`vs_d.rs` fixture で Pass。`synvelis-gateway` commit_gate keyed retry も整合。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行実測 Pass 参照・本 slice 未実走） | `AlreadyApplied`/`IntegrityConflict`/`StaleRevision` の typed 分類。 |
| SV17-C10 | Undo/VCS/Recoveryに保護されたMediaをGC対象としない。 | `crates/synvelis-recording/tests/project_adapter.rs`（`rcd02pa_probe_blocks_committed_media_in_live`・`rcd02pa_probe_blocks_via_undo_redo_history`・`rcd02pa_probe_blocks_via_saved_head`・`rcd02pa_probe_blocks_open_state_in_history`・`rcd02pa_probe_open_session_blocks_discard`・`rcd02pa_probe_unknown_head_states_fail_closed`・`rcd02pa_composite_probe_semantics`・`rcd02pa_probe_clear_allows_discard`）・`crates/synvelis-library/tests/import_recovery.rs`（`orphan_gc_protects_reachable_and_deletes_only_orphans`・`incomplete_job_and_blob_are_protected_from_gc`）・`store_publish.rs`（orphan snapshot 許容） | Undo/VCS/Recovery/Active Job/Pin に保護された Media/Asset を GC 対象にしない。orphan snapshot/media の誤削除を防ぐ。 | 先行実測: `rcd-02` ledger（probe fail-closed）・ALIB-02 `import_recovery.rs`・`sv84` orphan snapshot で Pass。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行実測 Pass 参照・本 slice 未実走） | 初期は非破壊的 GC（候補検出+明示 Cleanup）。Active Job/Pin Root の完全実装は #92 系で段階的。 |
| SV17-C11 | Schema Migration後も旧Snapshotを保持し、Plugin欠落状態でもOpaque Stateを保存。 | `crates/synvelis-project/tests/controller_store.rs`（`unknown_schema_and_corrupt_snapshots_are_rejected`）・`vcs01_snapshot.rs`（`v5_snapshot_projects_read_only_into_current_schema`・`unknown_and_too_old_schema_versions_are_rejected`・opaque_state Missing 分類）・`alib02_v5_real.rs`（`real_v5_snapshot_decodes_migrates_and_reloads`・`real_v5_read_only_projection_is_deterministic`・`migrated_save_preserves_original_and_sibling_blobs`・`unknown_schema_is_rejected_not_degraded`・`corrupt_or_foreign_state_is_rejected`）・`alib02_digest.rs`（`v5_snapshot_with_crc_digests_migrates_losslessly`）・`plugin_state.rs`（capture/restore quality）・`monitoring_persist.rs`（`v11_snapshot_migrates_to_v12_with_empty_monitoring`） | 旧 Immutable Snapshot を非破壊で保持。Versioned Reader/Migration Registry 単一経路。未知必須 schema を明示 Reject。Plugin 欠落時も Opaque State を保存。v5 projection は read-only。 | 先行実測: ALIB-02 v5 compat・VCS-01・plugin_state fixture で Pass（v5→現行 schema lossless migration・opaque plugin-state bytes 保存・unknown schema reject）。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行 slice で実装済み・本 slice 未実走） | `alib02_v5_real.rs` の `migrated_save_preserves_original_and_sibling_blobs` が旧 snapshot+blob+opaque plugin-state の保存を pin。 |
| SV17-C12 | VCSの過去Revision閲覧で現行Project/Engine/Working Treeを変更しない。 | `crates/synvelis-project/tests/vcs01_snapshot.rs`（`live_capture_is_immutable_across_edits_undo_and_save`・`saved_head_movement_inside_window_is_head_conflict`・`recovery_checkpoint_is_isolated_from_saved_head`・`vcs_staged_*`・`vcs_committed_*`・`resolution_never_mutates_controller_store_or_vcs_port`・`stale_live_revision_is_explicit`・`cancelled_token_fails_explicitly_at_phase_boundaries`・`oversized_snapshot_is_budget_exceeded`）・`vcs02_diff.rs`・`vcs05_restore.rs`・`vcs06_*` | 過去 Revision 閲覧で現行 Project/Engine/Working Tree を変更しない。`SnapshotIdentity` 解決は捕捉後に mutate せず、generation/index drift は `IndexChanged`、Restore-as-Copy 既定。 | 先行実測: `evidence/reports/vcs-06-fixture-matrix-ledger.md` で Pass（VCS-SNAP-01..06・DIFF・RESTORE・APPLY 行）。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行実測 Pass 参照・本 slice 未実走） | GUI 履歴閲覧は #114 系（NotRun）。real-git provider は `vcs03_git_provider.rs`/`vcs06_git_matrix.rs`。 |
| SV17-C13 | 外部編集による保存競合を検出し、破壊的上書きを回避する。 | `crates/synvelis-project/tests/head_cas.rs`（`controller_save_cas_rejects_moved_head`・`external_head_rewrite` 系・`concurrent_save_cas_admits_single_winner`）・`store_publish.rs`（`external_head_rewrite_is_reported_honestly`） | `save_cas` expected-head 照合で外部 HEAD 書換えを `Conflict` で検出し、破壊的上書きを防ぐ。非協調 writer・外部プロセスの保証限界を明示。 | 先行実測: `sv84` case-ledger + `syn-io-02-os-l1-evidence.md` で Pass（`HEAD.lock` + expected-head CAS・fencing）。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行実測 Pass 参照・本 slice 未実走） | advisory lock は協調 `save_cas` caller のみを直列化。非協調 writer/外部プロセスは検出限界を明示（design D4）。 |

## 補足 — fixture が存在しない/未設計の契約

以下は Issue #17 の横断契約のうち、**構成済みテストがない
または実装が未着手**のため `NotRun`/`Deferred` とする
（`docs/designs/project-persistence-design.md` の契約マップ
「残 gap」列と対応）:

| 契約 | 状態 | 委譲先 |
| --- | --- | --- |
| Active Execution Plan revision の永続化接続 | 設計上は Revision 分離に含まれるが、実装・fixture なし | #23/RCP 系 |
| Render Job Receipt・Artifact attach 統合 | `PROJ-RCP-01` の Render Job 側は未実装 | RCP 系 #290〜#297 |
| Save Job scheduler（queue・cancel・UI receipt） | contract は `save_cas` で保証、scheduler 本体なし | #92 系 |
| Autosave/Recovery Root 物理配置・Backup/Save As・Retention 期間・GC UI | PAP arch §15 PoC 保留 | #92〜#97 系 |
| 実 FS durability（物理電断・OS crash）・Network/Cloud FS | fixture 範囲外・実機 lane 未整備 | 別 lane（`test-recording-real-fs` 型） |
| 実 GUI/実 OS/実 Plugin/実機 L2/L3 | POLICIES.md §2 の別 Owner 区分 | #114 系等 |
| macOS/Linux の File Sync/DirSync/Atomic Publish/Indeterminate 実機証拠 | fixture 設定済みだが実機 run なし | OS-L1 実機 lane |

## 検証実行記録（本 slice）

| Command | OS | Toolchain | Exit | Result | 備考 |
| --- | --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows (win32) | rustc 1.99.1 (58cf8f9c3)・cargo 1.99.1（mise 管理） | 0 | Pass | docs/設計スライスの正規検証（`cargo fmt --all -- --check`）。実装 crate の `check`/`lint`/`test` は対象外。 |
| `doc_status.js lint`（specs/designs/plans/tasks/test-specs/impl/ir/impl/exp 各 dir） | Windows (win32) | node（doc-status skill script） | 0 | Pass | `project-persistence` 関連 file の finding なし（初回の broken-relation/invalid-id 指摘を修正後）。 |
| `doc_maintenance.js index apply`（specs/designs/plans/tasks/test-specs/impl/ir/impl/exp） | Windows (win32) | node（doc-maintenance skill script） | 0 | Pass | 影響 dir の index を再生成。`docs/impl/exp` は `.jsonl` のみ管理のため `.md` 追加で差分なし。 |

※ 実装 fixture の fresh run は本 slice の対象外（タスク指定の
正規検証は `mise run fmt-check`）。先行実測は上記参照先の
SHA・日時付き記録を正本とする。
