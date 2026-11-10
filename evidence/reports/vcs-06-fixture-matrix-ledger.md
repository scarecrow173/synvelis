# VCS-06 Fixture-Level Fault Matrix — Evidence Ledger (synvelis#97)

Integrated acceptance evidence for VCS-01..VCS-05. Every row records an
independently observed Expected/Observed pair. `Pass` here means *the
stated headless/fixture lane only* — it never implies real GUI, OS
durability, provider/remote, plugin-host, RT, or consumer acceptance.
Those lanes are `NotRun`/`Unsupported` with reasons, per the vocabulary
in `docs/specs/sf-06-cross-gate-oracle-case-ledger.md`.

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis commit | `792566072e4ab3e64f73498de79ba17e466b19ed` + this change set |
| Cargo.lock Ruviform SHA | `0f5b57372e9a43f84415f7964b2f9636774174f0` |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.0 (b940084d7), git 2.54.0.windows.1 |
| Schema | `SCHEMA_VERSION = 11`; v5 projection fixture |
| Fixture | temp-dir `ProjectController` + `ImmutableSnapshot`; real temp git repos via `git` CLI; `DirectoryApplyJournal`/`DirectoryPluginStateStore` on real FS |
| CI/PR | see PR body (Refs synvelis#97) |

Status vocabulary: `Pass` = measured pass in the stated lane;
`Pass*` = persisted/headless-layer pass only; `Fail` = measured failure;
`NotRun` = lane exists but was not measured here; `Unsupported` =
capability genuinely unavailable; `Blocked` = implementation missing.

## VCS-SNAP-01..06 — immutable snapshot resolver

Fixture: `crates/synvelis-project/tests/vcs01_snapshot.rs` (+
`vcs06_*` cross-checks). Lane: headless capture/resolve only.

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| VCS-SNAP-01 Captured Live | live capture pins revision+digest, immutable across edits/undo/save | `live_capture_is_immutable_across_edits_undo_and_save` Pass | Pass* | n/a (read-only) |
| VCS-SNAP-02 Saved HEAD | saved resolve pins HEAD; mid-window HEAD movement → HeadConflict | `saved_head_movement_inside_window_is_head_conflict`, `saved_head_ref_pin_*`, `saved_resolve_without_head_is_not_found` Pass | Pass* | conflict is explicit error, no retry |
| VCS-SNAP-03 Recovery | recovery checkpoint isolated from saved HEAD | `recovery_checkpoint_is_isolated_from_saved_head` Pass | Pass* | n/a |
| VCS-SNAP-04 VCS fixed revision | staged/committed locators resolve via port; generation/index drift → IndexChanged; partial stage explicit | `vcs_staged_*`, `vcs_committed_*`, `wrong_locator_variant_is_invalid_locator` Pass | Pass* | stale generation rejected, never silent |
| VCS-SNAP-05 old-v5 projection | v5 envelope projects read-only into current schema | `v5_snapshot_projects_read_only_into_current_schema` Pass | Pass* | projection only, no source rewrite |
| VCS-SNAP-06 unknown/corrupt/missing | unknown schema rejected; corrupt → explicit failure; missing/unknown assets → Degraded not silent | `unknown_and_too_old_schema_versions_are_rejected`, `corrupted_and_invalid_snapshots_are_explicit_failures`, `missing_unknown_and_unresolved_regions_are_degraded`, `absent_probe_marks_every_asset_unknown_not_present` Pass | Pass* | degraded completeness recorded |
| VCS-SNAP read-only oracle | resolution never mutates controller/store/VCS port | `resolution_never_mutates_controller_store_or_vcs_port`, `stale_live_revision_is_explicit`, `cancelled_token_fails_explicitly_at_phase_boundaries`, `oversized_snapshot_is_budget_exceeded` Pass | Pass* | — |
| VCS-SNAP-REAL-* (live GUI session, RT thread, real provider E2E) | — | not measured | **NotRun** | needs #114 real GUI + device/RT lanes |

## VCS-DIFF-01..08 — semantic diff / ChangeSet

Fixture: `vcs02_diff.rs` + `vcs06_diff_matrix.rs`. Lane: headless compare only.

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| VCS-DIFF-01 Track/Lane/Clip | add/remove/modify/move classified semantically | `track_name_change_is_modified`, `ab_ba_reverses_directional_kinds`, `aa_is_equal_and_comparison_is_deterministic` Pass | Pass | deterministic AB/BA |
| VCS-DIFF-02 Note propagation | clip-local note edit/add → typed Note changes | `clip_note_edit_and_addition_are_typed_changes` (vcs06) Pass | Pass | — |
| VCS-DIFF-03 lane ordering | same-set reorder → `Reordered`, no entity churn | `lane_order_change_is_reordered_not_modified` (vcs06) Pass | Pass | — |
| VCS-DIFF-04 FX/routing | processor reorder → `Reordered`; connection rewire/add/remove typed | `processor_reorder_is_reordered`, `existing_connection_rewire_is_reconnected`, `connection_add_remove_is_added_removed` Pass | Pass | — |
| VCS-DIFF-05 Automation | region point edit → `Modified`; interpolation shape change detected | `automation_region_edit_is_modified`, `automation_interpolation_change_is_modified` (vcs06) Pass | Pass | — |
| VCS-DIFF-06 Asset | content swap → `Replaced`; addition → `Added` | `asset_content_swap_is_replaced`, `asset_added_is_typed_add_not_reorder` (vcs06) Pass | Pass | — |
| VCS-DIFF-07 MIDI note payload | velocity edit → `Modified`, add → `Added` | `midi_note_edit_and_addition_are_typed_changes` (vcs06) Pass | Pass | — |
| VCS-DIFF-08 Unknown/Ambiguous | missing/unknown asset, opaque state, cross-kind collision, owner conflict, cross-project → `Unknown`/`Ambiguous`, never `Equal` | `missing_asset_blocks_equal_promotion`, `unknown_asset_status_is_unresolved_source`, `unknown_opaque_state_blocks_equal`, `cross_kind_raw_collision_is_ambiguous_identity`, `owner_reference_conflict_is_ambiguous_identity`, `different_project_ids_are_unknown_not_diffed` Pass | Pass | `Partial` completeness propagates |
| VCS-DIFF determinism/cache/budget | AA equal; cache identity; budget/cancel explicit | `change_set_cache_returns_identical_result`, `budget_exceeded_and_cancel_are_explicit_errors`, `pre_v6_snapshot_marks_routing_partial_projection` Pass | Pass | — |
| VCS-DIFF real plugin/consumer diff view | — | not measured | **NotRun** | consumer/precision lanes #219/#294 |

## VCS-GIT-01..06 — git read provider

Fixture: `vcs03_git_provider.rs` + `vcs06_git_matrix.rs`. Lane: real local
temp git repos, read-only oracle on index/HEAD/worktree.

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| VCS-GIT-01 fixed Commit/Tree/Parent | fixed revisions immune to branch/tag moves; merge keeps all parents | `linear_history_and_fixed_revision_survive_branch_move`, `branch_and_tag_moves_do_not_change_fixed_reads`, `merge_commit_preserves_all_parents`, `rename_and_delete_are_visible_across_fixed_revisions` Pass | Pass | — |
| VCS-GIT-02 Index/WorkTree/Commit separation | staged snapshot pins index generation; drift → IndexChanged; worktree status separate | `index_generation_tracks_staging_changes`, `staged_snapshot_reads_index_and_detects_partial_stage`, `staged_snapshot_and_working_status_report_index_state`, `merge_conflict_marks_staged_snapshot_incomplete` (vcs03) + vcs01 staged cases Pass | Pass | stale generation rejected |
| VCS-GIT-03 Partial stage / missing assets | partial stage explicit; untracked asset Missing never silent-equal; missing LFS degraded not pointer bytes | `untracked_asset_is_missing_never_silent_equal`, `missing_lfs_object_is_degraded_never_pointer_bytes`, `vcs_staged_partial_stage_is_explicit_failure`, `vcs_staged_absent_project_file_is_partial_stage` Pass | Pass | degraded recorded |
| VCS-GIT-04 identity | repository_id vs project path separation; SHA-1/SHA-256 object formats distinct from ContentDigest | `repository_identity_separates_repos_and_project_paths`, `detect_workspace_reports_identity_and_format`, `sha256_object_format_is_distinct_from_content_digest` Pass | Pass | — |
| VCS-GIT-05 symlinks/gitlinks/submodule | symlink mode preserved, link-target bytes never followed; gitlink reported not fetched | `gitlink_entries_are_reported_not_fetched` (vcs03), `symlink_index_entry_keeps_link_target_bytes` (vcs06, `update-index --cacheinfo 120000`) Pass | Pass | — |
| VCS-GIT-06 cancel/secret/budget | cancel aborts every op; no credential material in provider surface; history page clamped, limit=0 rejected | `cancelled_token_aborts_every_operation` (vcs03), `remote_credentials_never_surface_in_provider_output`, `history_limit_is_clamped_and_zero_rejected` (vcs06) Pass | Pass | — |
| VCS-GIT read-only oracle | provider never mutates checkout/index/worktree/HEAD or project session | `exercise_provider`/`repo_oracle` invariance checks (vcs03) Pass | Pass | oracle compares full repo state |
| VCS-GIT-REAL-* (real remote/auth/force-push/GC/OS perms) | — | not measured | **NotRun** | needs real remote + OS matrix |

## VCS-UI-01..05 — history/diff UI

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| VCS-UI-01..05 | native history visual diff, two-window, IME | no headless lane exists; requires app/workspace + real GPUI (#114) | **NotRun** (external gate) |

## VCS-APPLY-01..06 — apply selected change

Fixture: `vcs05_apply.rs` + `vcs06_apply_matrix.rs`. Lane: headless
`apply_selected` + real `DirectoryApplyJournal` on temp FS.

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| VCS-APPLY-01 selected apply + undo | apply commits once; exactly one undo step | `apply_added_track_commits_one_undo_step`, `apply_project_rename_only_touches_name`, `multi_change_apply_commits_as_single_undo_step` (vcs06, 2-change selection → 1 undo) Pass | Pass | undo/redo verified |
| VCS-APPLY-02 approval + CAS | empty ticket → Unapproved; stale revision → StaleRevision; fingerprint drift → FingerprintMismatch; wrong source identity → ChangeSetMismatch; unknown change id → UnknownChange | `unapproved_intent_fails_closed`, `stale_revision_fails_closed`, `fingerprint_drift_fails_closed`, `wrong_source_snapshot_identity_fails`, `unknown_change_id_fails_closed` Pass | Pass | live unchanged on every rejection |
| VCS-APPLY-03 fail-closed content | Removed → Unsupported; missing/unknown asset → MissingAsset/AssetUnknown; unsupported singleton field (ppqn) → UnsupportedField; absent target → TargetMissing; live ID collision → TargetConflict; unavailable plugin → MissingPlugin; Unknown/Partial changeset → NotApplicable | `removed_change_is_unsupported`, `missing_asset_fails_closed`, `unsupported_singleton_field_fails_closed`, `modified_change_on_absent_target_is_target_missing`, `added_change_with_live_id_collision_is_target_conflict`, `unavailable_plugin_binding_fails_closed`, `processor_changeset_with_unknown_opaque_state_is_not_applicable`, `degraded_endpoint_snapshot_fails_closed` (vcs05) Pass | Pass | all rejections leave live untouched |
| VCS-APPLY-04 replay/crash | same intent → Replayed, no double mutation; Pending-without-commit retries; ACK gap reconciles; **on-disk journal survives restart** | `second_apply_replays_receipt_without_double_mutation`, `pending_without_commit_retries_normally`, `ack_gap_reconciles_to_replayed` (vcs05), `directory_journal_survives_restart_and_replays` (vcs06, fresh `DirectoryApplyJournal` over same dir → Replayed) Pass | Pass | journal is the recovery record |
| VCS-APPLY-05 cancel | cooperative cancel → `Cancelled`, no mutation | `cancelled_intent_fails_closed_before_mutation` (vcs06) Pass | Pass | — |
| VCS-APPLY-06 independence | Saved HEAD / VCS index / git worktree untouched; no write before explicit apply | receipt `effects.untouched.*` asserted in every apply test; `directory_journal_*` confirms persisted receipt only Pass | Pass* | engine-apply flag stays false |
| VCS-APPLY-REAL-* (real engine hot-swap, GUI approval UX) | — | not measured | **NotRun** | engine/GUI lanes external |

## VCS-RESTORE-01..06 — restore as new copy

Fixture: `vcs05_restore.rs` + `vcs06_restore_matrix.rs`. Lane: real temp
FS staging→manifest→readback→atomic rename; `DirSync` honestly reported.

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| VCS-RESTORE-01 publish+readback+replay | verified bundle publishes; envelope/HEAD/blobs/receipt readable; replay idempotent; source project/Git untouched | `restore_publishes_verified_new_copy` Pass | Pass | source invariants asserted |
| VCS-RESTORE-02 missing/degraded assets | MissingAsset fails closed, no dest; AllowDegraded publishes marked copy | `missing_asset_fails_and_leaves_no_destination`, `degraded_policy_publishes_marked_copy` Pass | Pass | degraded list in receipt |
| VCS-RESTORE-03 crash/resume/readback | interrupt mid-materialize → staging kept; resume → Published; tampered staged envelope → Validation, no dest; foreign manifest → StagingConflict; orphan staging → StagingConflict | `interrupted_staging_resumes_to_published`, `tampered_staging_fails_resume_and_keeps_destination_absent`, `foreign_staging_manifest_conflicts`, `orphan_staging_dir_conflicts` (vcs06) Pass | Pass | staging is the recovery record; `directory_sync: Unsupported` observed on Windows (honest, recorded) |
| VCS-RESTORE-04 plugin state | processors require plugin-state source (`PluginStateRequired`); manifest+blobs copied verbatim into `plugin-state/` | `processor_project_requires_plugin_state_source`, `plugin_state_manifest_and_blobs_are_copied` (vcs06, real `DirectoryPluginStateStore` round-trip) Pass | Pass* | real plugin *host* restore is NotRun |
| VCS-RESTORE-05 destination safety | existing dest → DestinationExists; relative/`CON`/`..` leaf → InvalidRequest; case-conflicting sibling → DestinationExists (lexical scan, FS-independent); low disk → LowDisk; cancel → nothing written | `existing_destination_fails_closed`, `unsafe_destinations_fail_before_any_write`, `low_disk_fails_closed`, `cancelled_restore_writes_nothing` (vcs05), `dotdot_leaf_is_rejected_before_any_write`, `case_conflicting_sibling_is_rejected` (vcs06) Pass | Pass | pre-write rejection verified |
| VCS-RESTORE-06 independent identity | new ProjectId ≠ source; stable IDs preserved; source Saved HEAD/Git index/worktree untouched | asserted in `restore_publishes_verified_new_copy` receipt effects Pass | Pass* | — |
| VCS-RESTORE-REAL-* (OS crash durability, AV/locking, real plugin host, consumer open) | — | not measured | **NotRun** | OS matrix + #219/#294 |

## VCS-COMPAT-01..04 — cross-gate compatibility

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| VCS-COMPAT-01 schema compat (v5→current) | old snapshots project read-only; unknown/too-old rejected; no source rewrite | SNAP-05/06 rows Pass | Pass* |
| VCS-COMPAT-02 Gate A / consumer precision | AudioLens/consumer cross-check | not measured | **NotRun** (#219/#294) |
| VCS-COMPAT-03 CI evidence (#83) | CI-run evidence rows | local `mise` verification only; CI lane not executed here | **NotRun** |
| VCS-COMPAT-04 other OS (macOS/Linux) | same matrix on POSIX FS | Windows-only run; `directory_sync` already reports `Unsupported` here | **NotRun** |

## Explicitly out of scope (never Pass)

- VCS Writer / 3-way semantic merge — unimplemented, **Blocked**.
- Additional VCS providers (non-git) — **Unsupported** in this slice.
- Real GUI/IME/two-window (#114), native plugin dock (#176), RT/device,
  consumer/precision (#219/#294), CI gate (#83) — **NotRun**.

## New fixture files (this change)

- `crates/synvelis-project/tests/vcs06_diff_matrix.rs` — 5 tests
- `crates/synvelis-project/tests/vcs06_apply_matrix.rs` — 8 tests
- `crates/synvelis-project/tests/vcs06_restore_matrix.rs` — 8 tests
- `crates/synvelis-library/tests/vcs06_git_matrix.rs` — 3 tests

All 24 pass on Windows/rustc 1.99.0 via `cargo test` (supplemental;
canonical `mise run test` result recorded in the PR).
