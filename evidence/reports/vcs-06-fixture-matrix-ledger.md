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
| Synvelis commit | `1093530a89499fae210aa3529ef2666012c7548f` (origin/main post-#332) + this change set — VCS-APPLY-07 apply rows measured on the monitoring-apply slice merged with origin/main @ `1093530`; earlier rows measured under `7822857`/`7925660`/`099e7ea` |
| Cargo.lock Ruviform SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` (VCS-APPLY-07 apply rows; earlier rows measured under `9865cd57`/`b02942c2`/`0f5b5737`) |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.0 (b940084d7), git 2.54.0.windows.1 |
| Schema | `SCHEMA_VERSION = 13` (monitoring persisted since v12); v5 projection fixture |
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

## VCS-DIFF-01..10 — semantic diff / ChangeSet

Fixture: `vcs02_diff.rs` + `vcs06_diff_matrix.rs` +
`vcs06_monitoring_matrix.rs`. Lane: headless compare only.

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
| VCS-DIFF-09 TrackMonitorDecl | decl upsert → `Added` keyed on `TrackId`; field edit → `Modified` + field evidence; clear → `Removed`; no `Track` duplicate; `Recording` domain; AA = `Equal`; Recording 除外で monitor-only 差分 `Equal` / Recording のみで `Different`; 異 `project_id` → `Unknown` + `AmbiguousIdentity{CrossProject}` | `track_monitor_decl_add_modify_clear_are_typed_changes` (vcs06, `a218944b` policy v2 / schema v13) Pass | Pass | — |
| VCS-DIFF-10 PunchMarks | `{None,None}`→declared → `Added` + `SemanticTimeRange::Musical [in,out)`; mark edit → `Modified` + `punch_in` evidence; declared→`{None,None}` → `Removed` + covered span; partial `{in,None}` → `Added` + range 無し; inverted `{out<in}` → `Modified` + range 無し; `Recording` domain | `punch_marks_presence_transitions_are_typed_changes` (vcs06, policy v2 / schema v13) Pass | Pass | — |
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

Headless lane: `syn.vcs.view`/`syn.vcs.visual` ops on a real
`SynvelisApp` over an in-memory `VcsHistoryBackend` fixture
(`crates/synvelis-app/tests/vcs04_history.rs`,
`crates/synvelis-app/tests/vcs06_ui_matrix.rs`), plus the
workspace-layer `vcs_history_view`/`history_screen_spec` projections
(`crates/synvelis-workspace/tests/vcs04_history.rs`). Lane: headless
JSON/screen-spec projection only — never real GPUI, OS windows, IME,
or keyboard.

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| VCS-UI-01 fixed A/B + parent selection | pinned endpoints produce one shared ChangeSet; merge `after` records chosen parent | `ab_pin_produces_diff_and_merge_parent_choice` (app), `ab_pin_committed_nodes_produces_shared_changeset`, `merge_after_endpoint_records_parent_choice` (workspace) Pass | Pass* | read-only pins |
| VCS-UI-02 SideBySide/Focus view modes | `view_mode` switch is display-only over the same ChangeSet/Target | `view_mode_and_clear_selection_are_display_only` (workspace) Pass | Pass* | display-only, no requery |
| VCS-UI-03 two-window/dock | dock move / window detach preserves VCS view state headlessly; real OS two-window not measured | `dock_moves_and_two_window_detach_preserve_vcs_state` (workspace), `view_close_drops_vcs_state_and_ops_stay_read_only` (app) Pass; real OS window lane **NotRun** | Pass* | headless dock only |
| VCS-UI-04 Unknown/Stale/Partial never zero-change | cross-project committed pair → `overall:"unknown"` + issues, never `equal`/0-change; `Absent` asset bytes → `completeness:"Partial"` + `missing-asset`/`unresolved-source` issue rows mirrored into the read-only `vcs.issues` screen-spec table; staged pin with failed `index_generation` probe or absent backend → `freshness:"unknown"` + `freshness_reason`; live revision advance → `stale` | `cross_project_committed_endpoints_report_unknown_not_equal`, `absent_asset_bytes_mark_degraded_partial_with_issue_rows`, `staged_pin_freshness_is_unknown_when_index_probe_fails`, `staged_pin_freshness_is_unknown_without_backend` (`vcs06_ui_matrix.rs`, app) + `live_pin_reports_stale_after_revision_advances` (app), `live_endpoint_goes_stale_when_revision_advances`, `staged_endpoint_pins_and_detects_index_drift` (workspace), `missing_asset_blocks_equal_promotion`, `unknown_asset_status_is_unresolved_source` (vcs02) Pass | Pass* | fail-closed; issues verbatim |
| VCS-UI-05 keyboard/IME/a11y/real GUI | real GPUI input, IME composition, accessibility, native window evidence | no headless lane exists; requires real GPUI (#114) | **NotRun** (external gate) | — |
| VCS-UI lifecycle/read-only | kind-scoped open/close, foreign repo/bad params honest errors, no-backend status, screen spec read-only, ops never touch project/engine/transport | `history_projection_reports_backend_and_rows`, `staged_and_saved_endpoints_report_honestly`, `foreign_repository_and_bad_params_fail_honestly`, `no_backend_reports_status_and_refuses_history`, `open_close_lifecycle_is_kind_scoped`, `screen_spec_is_a_read_only_projection` (app) + `history_pages_and_node_selection`, `committed_endpoint_selector_resolves_once_and_rejects_foreign`, `no_backend_reports_status_but_live_still_pins`, `cancelled_ops_surface_cancelled_not_success`, `history_bound_refuses_oversized_page`, `remove_view_and_release_views_clean_vcs_state`, `vcs_ops_never_touch_project_engine_or_transport` (workspace) Pass | Pass* | — |

## VCS-APPLY-01..07 — apply selected change

Fixture: `vcs05_apply.rs` + `vcs06_apply_matrix.rs` +
`vcs06_monitoring_matrix.rs`. Lane: headless
`apply_selected` + real `DirectoryApplyJournal` on temp FS.

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| VCS-APPLY-01 selected apply + undo | apply commits once; exactly one undo step | `apply_added_track_commits_one_undo_step`, `apply_project_rename_only_touches_name`, `multi_change_apply_commits_as_single_undo_step` (vcs06, 2-change selection → 1 undo) Pass | Pass | undo/redo verified |
| VCS-APPLY-02 approval + CAS | empty ticket → Unapproved; stale revision → StaleRevision; fingerprint drift → FingerprintMismatch; wrong source identity → ChangeSetMismatch; unknown change id → UnknownChange | `unapproved_intent_fails_closed`, `stale_revision_fails_closed`, `fingerprint_drift_fails_closed`, `wrong_source_snapshot_identity_fails`, `unknown_change_id_fails_closed` Pass | Pass | live unchanged on every rejection |
| VCS-APPLY-03 fail-closed content | Removed → Unsupported; missing/unknown asset → MissingAsset/AssetUnknown; unsupported singleton field (ppqn) → UnsupportedField; absent target → TargetMissing; live ID collision → TargetConflict; unavailable plugin → MissingPlugin; Unknown/Partial changeset → NotApplicable | `removed_change_is_unsupported`, `missing_asset_fails_closed`, `unsupported_singleton_field_fails_closed`, `modified_change_on_absent_target_is_target_missing`, `added_change_with_live_id_collision_is_target_conflict`, `unavailable_plugin_binding_fails_closed`, `processor_changeset_with_unknown_opaque_state_is_not_applicable`, `degraded_endpoint_snapshot_fails_closed` (vcs05) Pass | Pass | all rejections leave live untouched |
| VCS-APPLY-04 replay/crash | same intent → Replayed, no double mutation; Pending-without-commit retries; ACK gap reconciles; **on-disk journal survives restart** | `second_apply_replays_receipt_without_double_mutation`, `pending_without_commit_retries_normally`, `ack_gap_reconciles_to_replayed` (vcs05), `directory_journal_survives_restart_and_replays` (vcs06, fresh `DirectoryApplyJournal` over same dir → Replayed) Pass | Pass | journal is the recovery record |
| VCS-APPLY-05 cancel | cooperative cancel → `Cancelled`, no mutation | `cancelled_intent_fails_closed_before_mutation` (vcs06) Pass | Pass | — |
| VCS-APPLY-06 independence | Saved HEAD / VCS index / git worktree untouched; no write before explicit apply | receipt `effects.untouched.*` asserted in every apply test; `directory_journal_*` confirms persisted receipt only Pass | Pass* | engine-apply flag stays false |
| VCS-APPLY-07 monitoring kinds | `TrackMonitorDecl`/`PunchMarks` の `Added`/`Modified` selection が typed entity transplant で適用され、`Removed` は fail-closed | `target_fingerprint` が両 kind で解決（旧来の `Unsupported` を返さない）; `Added` 2-change selection → `Applied`、live `monitoring.tracks`/`punch` が after と一致、1 apply = 1 undo step で undo/redo が monitoring 状態を往復 — `monitor_kind_added_selection_applies_typed_entities` (vcs06)・`monitor_decl_and_punch_apply_as_one_undo_step` (vcs05) Pass; `Modified` 適用 + 連続 undo が S1→S0 へ 1 step ずつ復帰 — `monitor_kind_modified_selection_applies_typed_entities` Pass; `Removed` → `ApplyError::Unsupported{kind, change: Removed, "removal changes are deferred (cascade design)"}`、journal `Failed`、live `monitoring` 不変（両 kind）— `monitor_kind_removed_selection_stays_fail_closed` Pass | **Pass** | `Removed` cascade は全 kind deferred のまま; `PluginInstance` apply は #324 系の別 slice; 実 OS/GUI/RT lanes は NotRun |
| VCS-APPLY-REAL-* (real engine hot-swap, GUI approval UX) | — | not measured | **NotRun** | engine/GUI lanes external |

## VCS-RESTORE-01..08 — restore as new copy

Fixture: `vcs05_restore.rs` + `vcs06_restore_matrix.rs` +
`vcs06_monitoring_matrix.rs` + `vcs06_plugin_instance_matrix.rs`. Lane: real temp
FS staging→manifest→readback→atomic rename; `DirSync` honestly reported.

| Case | Expected | Observed | Status | Recovery/Rollback |
| --- | --- | --- | --- | --- |
| VCS-RESTORE-01 publish+readback+replay | verified bundle publishes; envelope/HEAD/blobs/receipt readable; replay idempotent; source project/Git untouched | `restore_publishes_verified_new_copy` Pass | Pass | source invariants asserted |
| VCS-RESTORE-02 missing/degraded assets | MissingAsset fails closed, no dest; AllowDegraded publishes marked copy | `missing_asset_fails_and_leaves_no_destination`, `degraded_policy_publishes_marked_copy` Pass | Pass | degraded list in receipt |
| VCS-RESTORE-03 crash/resume/readback | interrupt mid-materialize → staging kept; resume → Published; tampered staged envelope → Validation, no dest; foreign manifest → StagingConflict; orphan staging → StagingConflict | `interrupted_staging_resumes_to_published`, `tampered_staging_fails_resume_and_keeps_destination_absent`, `foreign_staging_manifest_conflicts`, `orphan_staging_dir_conflicts` (vcs06) Pass | Pass | staging is the recovery record; `directory_sync: Unsupported` observed on Windows (honest, recorded) |
| VCS-RESTORE-04 plugin state | processors require plugin-state source (`PluginStateRequired`); manifest+blobs copied verbatim into `plugin-state/` | `processor_project_requires_plugin_state_source`, `plugin_state_manifest_and_blobs_are_copied` (vcs06, real `DirectoryPluginStateStore` round-trip) Pass | Pass* | real plugin *host* restore is NotRun |
| VCS-RESTORE-05 destination safety | existing dest → DestinationExists; relative/`CON`/`..` leaf → InvalidRequest; case-conflicting sibling → DestinationExists (lexical scan, FS-independent); low disk → LowDisk; cancel → nothing written | `existing_destination_fails_closed`, `unsafe_destinations_fail_before_any_write`, `low_disk_fails_closed`, `cancelled_restore_writes_nothing` (vcs05), `dotdot_leaf_is_rejected_before_any_write`, `case_conflicting_sibling_is_rejected` (vcs06) Pass | Pass | pre-write rejection verified |
| VCS-RESTORE-06 independent identity | new ProjectId ≠ source; stable IDs preserved; source Saved HEAD/Git index/worktree untouched | asserted in `restore_publishes_verified_new_copy` receipt effects Pass | Pass* | — |
| VCS-RESTORE-07 monitoring verbatim | decl (lane/arm/monitor/channel_map) + punch marks survive restore-as-copy verbatim; fork `project_id` only | `envelope.project.monitoring == source`（`monitoring.tracks` + `monitoring.punch` 全 field一致）— `restore_as_copy_preserves_monitoring_verbatim` (vcs06) Pass | Pass | — |
| VCS-RESTORE-08 plugin_instances verbatim | issued `PluginInstanceId`s + `project.plugin_instances` survive restore-as-copy verbatim (envelope readback pin) | `restore_as_copy_preserves_plugin_instances_verbatim` (vcs06_plugin_instance_matrix) Pass | Pass | — |
| VCS-RESTORE-REAL-* (OS crash durability, AV/locking, real plugin host, consumer open) | — | not measured | **NotRun** | OS matrix + #219/#294 |

## VCS-COMPAT-01..04 — cross-gate compatibility

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| VCS-COMPAT-01 schema compat (v5→current) | old snapshots project read-only; unknown/too-old rejected; no source rewrite | SNAP-05/06 rows Pass | Pass* |
| VCS-COMPAT-02 Gate A / consumer precision | AudioLens/consumer cross-check | not measured | **NotRun** (#219/#294) |
| VCS-COMPAT-03 CI evidence (#83) | CI-run evidence rows | ci.yml run [42938180676](https://github.com/scarecrow173/synvelis/actions/runs/42938180676)（head `13e65189`・daily-ci dispatch）: ubuntu verify **success**（vcs06 全 pass）・macOS success・windows verify **failure**（非関連 `synvelis-library --test webdav` TransientIo・記録済み）・両 gate failure。#323 covering run は収集時点（2026-11-12 再検証）で未実走 = NotRun pending。証跡: `vcs-06-compat-evidence.md` | measured（**非一様 green**） |
| VCS-COMPAT-04 other OS (macOS/Linux) | same matrix on POSIX FS | hosted macOS verify leg は run 42938180676 で success 観測済みだが、製品 OS matrix 実機計測ではない。Windows-only run; `directory_sync` already reports `Unsupported` here | **NotRun** |

## Explicitly out of scope (never Pass)

- VCS Writer / 3-way semantic merge — unimplemented, **Blocked**.
- Additional VCS providers (non-git) — **Unsupported** in this slice.
- Real GUI/IME/two-window (#114), native plugin dock (#176), RT/device,
  consumer/precision (#219/#294), CI gate (#83) — **NotRun**.

## New fixture files (this change)

- `crates/synvelis-project/tests/vcs06_diff_matrix.rs` — 5 tests
- `crates/synvelis-project/tests/vcs06_apply_matrix.rs` — 8 tests
- `crates/synvelis-project/tests/vcs06_restore_matrix.rs` — 8 tests
- `crates/synvelis-project/tests/vcs06_monitoring_matrix.rs` — 6 tests
  (monitoring-domain: VCS-DIFF-09/10・VCS-APPLY-07・VCS-RESTORE-07;
  VCS-APPLY-07 は apply surface 拡張 slice で supported-apply +
  Removed fail-closed の 3 test へ再構成)
- `crates/synvelis-library/tests/vcs06_git_matrix.rs` — 3 tests
- `crates/synvelis-app/tests/vcs06_ui_matrix.rs` — 4 tests (VCS-UI-04 lane)

All 34 pass on Windows/rustc 1.99.0 via `cargo test` (supplemental;
canonical `mise run test` result recorded in the PR).
