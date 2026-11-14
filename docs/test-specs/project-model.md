---
id: "TSPEC-034X6iT1XRNf2JVURuDjzb"
type: "test-spec"
status: "approved"
title: "Project Model Test Spec — contract case map to synvelis-project tests"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/10"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/project-model.md"
    - "docs/tasks/project-model-task.md"
    - "docs/specs/project-model.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/project-model-design.md"
  derived-by: []
  verifies:
    - "docs/designs/project-model-design.md"
    - "docs/specs/project-model.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Project Model Test Spec — contract case map to synvelis-project tests

## Purpose

Issue #10 の受入条件（PM-AC-01〜13）を、既存 `synvelis-project` の headless
fixture test と設計レビューへ Case ID（PM-T-*）で対応付け、
`evidence/sv10/` 台帳の Expected/Observed/NotRun の根拠を固定する。本 slice
は `crates/` を変更しないため新規 test は追加せず、既存 fixture の照合と
設計契約の静的検証を記録する。実機・実 plugin・実 storage・実 RT の未検証
項目は NotRun/Deferred として明示し、headless 合格を実機合格と混同しない。

## Feature

Project Model の headless 契約検証 — entity 責務・Track+Capabilities・
非破壊編集・時間 domain・stable ID・単一 writer/CAS/undo・snapshot/
migration/VCS・remote asset・失敗分類・Ruviform 境界。

## Rules

- **PM-T-01 Entity 責務と schema**: Track/Lane/Clip/Source/MidiSource/Asset/
  Take/CaptureSegment/Recording registry/Comp registry の責務分界と immutable
  source 契約。Oracle: `id_time_range.rs`・`controller_store.rs`・design
  §Entity 責務分界。
- **PM-T-02 Track+Capabilities・Folder 非干渉**: Audio/MIDI/Instrument/Bus/
  Return/Folder を Track+Capabilities で表現し、Folder の display hierarchy
  が routing edge を追加しない。Oracle: design §Track+Capabilities +
  `routing_sf01.rs`・`sf_02.rs` の connection invariant。
- **PM-T-03 Source 共有・Clip 独立編集**: 1 Source を複数 Clip が参照しても
  clip-local trim/gain/placement は独立。Oracle: `midi_source.rs`・
  `vcs02_diff.rs::clip_duplication_is_added_without_lane_noise`。
- **PM-T-04 Linked vs Independent duplicate**: shared source 編集は linked
  clip に反映、`DetachClipSource` は明示 copy+repoint。暗黙 COW なし。
  Oracle: `midi_source.rs::linked_clips_share_source_edits_and_detach_is_
  explicit`・`undo_redo_cover_note_edits_and_detach`。
- **PM-T-05 TempoMap/PPQN 編集の非破壊性**: tempo/PPQN 変更は musical 位置を
  rational beat で保存し tick を再投影、PCM/raw MIDI/source timestamp を
  変更しない。Oracle: `tim_01_ppqn.rs`・`tempo.rs`・`tim_02_ppqn_smf.rs`。
- **PM-T-06 時間変換境界**: 960 tick→sample・sample rate 変更・tempo 変更
  区間・loop 境界・負 time/pre-roll で欠落/二重発火なし。Oracle:
  `tim_01_ppqn.rs`（mapping/rounding）・`rt_ppqn_beat_to_frame_rounding_
  reproduces`・`md_a.rs` loop regression・`id_time_range.rs` 負値。
- **PM-T-07 MIDI timestamp 非破壊**: source-native `MidiDivision`/`MidiTick`
  を保持し project PPQN へ requantize しない。Oracle: `midi_source.rs::
  native_divisions_preserve_exact_ticks`・`tim_02_smf.rs::smf_import_
  creates_source_with_native_division_and_projection`・`rcd_04_midi_phrase::
  midi_phrase_preserves_native_ticks_without_requantize`。
- **PM-T-08 非破壊編集 ops**: Move/Trim/Slip/Stretch/Loop/Split/Duplicate/
  Mute が source を変更せず reload/undo で再現。Fade は `CompTransition`、
  gain は strip/send `ParameterTarget` domain（clip field ではない —
  design §非破壊編集契約）。Oracle: `md_a.rs`・`controller_store.rs`・
  `vcs02_diff.rs`（clip move/trim/source swap/dup）・design §非破壊編集契約。
- **PM-T-09 Split lineage・ID 維持**: split は新 ClipId、undo で復元、
  rename/move は ID 維持。**gap**: 現行 schema に明示 `DerivedFrom` field
  はない（同一 `source_id`・隣接 range の暗黙関係のみ）。Oracle:
  `rcd_04_vcs_stable_id.rs`・`vcs02_diff.rs`・`controller_store.rs::
  save_reload_preserves_identity_ids_and_revision`・design §Stable ID/
  §非破壊編集契約（`ProjectOp::SplitClip`）。
- **PM-T-10 重複評価規則**: 同一 lane の重複は明示評価規則（comp candidate
  選択）のみが解決し、素材削除/上書きしない。Oracle: `md_a.rs`・
  `rcd_04_comp_cas.rs`・`comp_ops_never_mutate_takes_sources_or_assets`。
- **PM-T-11 CAS/atomic apply/全件検証**: stale revision・invalid target・
  複数 clip 同時編集・競合は全件検証で部分適用なし。Oracle:
  `controller_store.rs::stale_revision_and_invalid_target_are_rejected_
  atomically`・`apply_advances_revision_and_is_atomic`・
  `rcd_04_comp_cas.rs::comp_group_cas_rejects_stale_revision_without_
  partial_apply`・`comp_invalid_ops_are_all_or_nothing`・`sf_02` stale/
  cycle reject。
- **PM-T-12 Undo/Redo revision 会計**: undo/redo は新 revision を発行し
  saved を保持、1 gesture=1 undo。Oracle: `controller_store.rs::undo_redo_
  issue_new_revisions_and_keep_saved_intact`・`midi_source.rs::undo_redo_
  cover_note_edits_and_detach`・`tim_01_ppqn.rs::ppqn_change_undo_redo_
  round_trip`・`aut_02.rs::aut_02_one_undo_restores_complete_pre_state`。
- **PM-T-13 stable ID across save/reload/VCS/migration**: ID が save/reload・
  undo/redo・VCS diff/apply/restore・migration をまたいで維持。Oracle:
  `controller_store.rs::save_reload_preserves_identity_ids_and_revision`・
  `comp_ids_survive_undo_redo_save_reload_and_diff`・`comp_stable_ids_
  survive_save_reload_and_diff`・`comp_stable_ids_survive_restore_as_copy`・
  `alib02_v5_real.rs`・`vcs01_snapshot.rs::v5_snapshot_projects_read_only_
  into_current_schema`。
- **PM-T-14 AmbiguousIdentity 拒否**: 同一 number/owner/cross-kind 衝突は
  `AmbiguousIdentity` で拒否、自動 merge なし。Oracle: `vcs02_diff.rs::
  cross_kind_raw_collision_is_ambiguous_identity`・`owner_reference_
  conflict_is_ambiguous_identity`・`monitor_decl_on_ambiguous_track_raw_
  is_ambiguous`・`plugin_instance_persist.rs` oracle reject・`midi_source::
  load_rejects_*`。
- **PM-T-15 HEAD CAS / save_cas**: expected-head CAS・stale lock break・
  concurrent single winner・conflict typed。Oracle: `head_cas.rs` 全件・
  `store_publish.rs`。
- **PM-T-16 snapshot/migration/reload reject**: immutable snapshot・HEAD
  publish・unknown/corrupt/malformed reject・ordered migration read-only。
  Oracle: `store_publish.rs`・`controller_store.rs::unknown_schema_and_
  corrupt_snapshots_are_rejected`・`load_rejects_*`・`vcs01_snapshot.rs`・
  `alib02_digest.rs`・`alib02_v5_real.rs`。
- **PM-T-17 VCS diff/apply/restore**: meaningful diff・apply selected・
  restore as copy・journal replay。Oracle: `vcs02_diff.rs`・`vcs05_apply.rs`・
  `vcs05_restore.rs`・`vcs06_*`・`rcd_04_vcs_stable_id.rs`。
- **PM-T-18 remote asset fetch/cancel/offline**: fetch 失敗/取消は manifest/
  clip に不完全データを残さず、取得済み asset は offline 再生可能。
  Oracle: `alib02_digest.rs`・`vcs05_restore.rs::missing_asset_fails_and_
  leaves_no_destination`・`restore_as_copy_preserves_*`・design §Remote
  Asset（cancel 実装は ALIB 系別 slice）。
- **PM-T-19 revision 分離**: live/saved/recovery/VCS/execution revision を
  混同しない。Oracle: `controller_store.rs::eval_plan_is_stale_tracks_
  frozen_inputs`・`vcs01_snapshot.rs::recovery_checkpoint_is_isolated_from_
  saved_head`・`live_capture_is_immutable_across_edits_undo_and_save`。
- **PM-T-20 失敗分類**: missing/corrupt/stale/invalid/overflow/unsupported/
  incomplete/malformed/migration/cancel を typed で区別。Oracle:
  `store_publish.rs` failed_*・`vcs05_restore.rs` failure tests・
  `vcs06_restore_matrix.rs`・`vcs01_snapshot.rs::cancelled_token_*`・
  `vcs05_restore.rs::cancelled_restore_writes_nothing`・`tim_01_ppqn.rs::
  ppqn_change_rejects_tick_overflow`。
- **PM-T-21 Branch/Scope/Lineage 契約**: typed VariantScope・versioned
  BranchContentRef・branch-local ID 衝突なし・scope 外非破壊・adopt 単一
  CAS+undo。Oracle: design §Branch/Scope/Lineage（実装は #277/#278 別 slice、
  本 slice は契約のみ → design-review）。
- **PM-T-22 Ruviform 境界・第二正本禁止**: GUI/RT/Plugin に第二 project
  正本を置かず、Ruviform に DAW Track/Clip 型を流入させない。Oracle:
  design §Ruviform/Synvelis 境界 + `docs/designs/workspace-model-design.md`
  の ProjectSession 単一正本契約（design-review）。

## Examples

Case → test 対応表（headless。実機は §NotRun）:

| Case | 検証内容 | test（crate::file::fn） |
| --- | --- | --- |
| PM-T-01 | entity 責務・schema・id/time/range | `synvelis-project::id_time_range`（all_ids_roundtrip/id_kind_mismatch/session_and_source_frames/musical_tick_and_project_time_allow_negative/half_open_ranges/media_span）; `controller_store`（apply/save/load 系） |
| PM-T-02 | Track+Capabilities・Folder 非干渉・routing invariant | `routing_sf01`（master_cannot_be_added_or_removed/oracle_rejects_orphaned_and_doubly_referenced_sends/the_strip_graph_cannot_cycle）; `sf_02`（a_feedback_cycle_rejects_as_a_typed_cycle）; design §Track+Capabilities（Folder=display のみ） |
| PM-T-03 | Source 共有・Clip 独立 | `midi_source::linked_clips_share_source_edits_and_detach_is_explicit`; `vcs02_diff::clip_duplication_is_added_without_lane_noise`; `md_a` clip placement/playback fixtures |
| PM-T-04 | Linked vs Independent duplicate・DetachClipSource | `midi_source::linked_clips_share_source_edits_and_detach_is_explicit`; `midi_source::undo_redo_cover_note_edits_and_detach`; `tim_02_smf::smf_detach_clip_source_creates_editable_copy_without_provenance` |
| PM-T-05 | TempoMap/PPQN 非破壊 | `tim_01_ppqn::ppqn_change_preserves_beat_positions_and_reprojects_ticks`; `ppqn_change_preserves_absolute_media_and_source_native_data`; `ppqn_exact_policy_rejects_inexact_positions`; `tempo::tempo_map_mutation_bumps_revision_and_refs_point_at_it`; `tim_02_ppqn_smf::ppqn_exact_change_preserves_smf_chain_end_to_end` |
| PM-T-06 | 時間変換境界 | `tim_01_ppqn::rt_ppqn_beat_to_frame_rounding_reproduces`; `tim_01_ppqn::ppqn_change_handles_long_arrangements`; `md_a::regression_loop_window_is_the_repeat_unit`; `regression_loop_play_from_outside_handle_rejected`; `id_time_range::musical_tick_and_project_time_allow_negative`; `tim_01_ppqn::ppqn_denominator_boundary_is_explicit` |
| PM-T-07 | MIDI timestamp 非破壊 | `midi_source::native_divisions_preserve_exact_ticks`; `tim_02_smf::smf_import_creates_source_with_native_division_and_projection`; `rcd_04_midi_phrase::midi_phrase_preserves_native_ticks_without_requantize` |
| PM-T-08 | 非破壊編集 ops | `md_a`（clip placement/playback/comp fixtures）; `controller_store::save_reload_preserves_identity_ids_and_revision`; `vcs02_diff::clip_move_is_moved_and_trim_is_modified`; `clip_move_plus_trim_is_two_changes`; `clip_source_swap_is_replaced`; design §非破壊編集契約 |
| PM-T-09 | Split lineage・ID 維持 | `rcd_04_vcs_stable_id::comp_stable_ids_survive_save_reload_and_diff`; `vcs02_diff` clip 系; `controller_store::save_reload_preserves_identity_ids_and_revision`; design §Stable ID（split op は `ProjectOp::SplitClip`） |
| PM-T-10 | 重複評価規則 | `md_a`; `rcd_04_comp_cas::comp_ops_never_mutate_takes_sources_or_assets`; `comp_switch_is_single_cas_undo_and_leaves_raw_immutable`; `project_comp_is_deterministic_and_preserves_provenance` |
| PM-T-11 | CAS/atomic apply | `controller_store::stale_revision_and_invalid_target_are_rejected_atomically`; `apply_advances_revision_and_is_atomic`; `rcd_04_comp_cas::comp_group_cas_rejects_stale_revision_without_partial_apply`; `comp_invalid_ops_are_all_or_nothing`; `sf_02` stale/cycle reject |
| PM-T-12 | undo/redo revision | `controller_store::undo_redo_issue_new_revisions_and_keep_saved_intact`; `midi_source::undo_redo_cover_note_edits_and_detach`; `tim_01_ppqn::ppqn_change_undo_redo_round_trip`; `aut_02::aut_02_one_undo_restores_complete_pre_state` |
| PM-T-13 | stable ID across save/reload/VCS/migration | `controller_store::save_reload_preserves_identity_ids_and_revision`; `comp_ids_survive_undo_redo_save_reload_and_diff`; `comp_stable_ids_survive_save_reload_and_diff`; `comp_stable_ids_survive_restore_as_copy`; `alib02_v5_real`; `vcs01_snapshot::v5_snapshot_projects_read_only_into_current_schema` |
| PM-T-14 | AmbiguousIdentity 拒否 | `vcs02_diff::cross_kind_raw_collision_is_ambiguous_identity`; `owner_reference_conflict_is_ambiguous_identity`; `monitor_decl_on_ambiguous_track_raw_is_ambiguous`; `plugin_instance_persist` oracle reject 系; `midi_source::load_rejects_*` |
| PM-T-15 | HEAD CAS | `head_cas` 全件（save_cas_commits_only_when_head_matches/concurrent_save_cas_admits_single_winner/controller_save_cas_rejects_moved_head/two_process_save_cas_single_winner_typed_conflict/stale_head_lock_is_broken 等）; `store_publish` |
| PM-T-16 | snapshot/migration/reload | `store_publish` 全件; `controller_store::unknown_schema_and_corrupt_snapshots_are_rejected`; `load_rejects_malformed_tempo_map`/`load_rejects_invalid_clip_contract`/`load_still_accepts_valid_snapshot`; `vcs01_snapshot`（immutable/degraded/cancel/budget）; `alib02_digest`; `alib02_v5_real` |
| PM-T-17 | VCS diff/apply/restore | `vcs02_diff`（aa/ab/clip/automation/asset/monitor/plugin/midi typed changes）; `vcs05_apply`（unapproved/stale/fingerprint/unknown/missing/replay/journal）; `vcs05_restore`（verified copy/missing/degraded/existing/unsafe/low-disk/cancel）; `vcs06_*`; `rcd_04_vcs_stable_id` |
| PM-T-18 | remote asset fetch/cancel/offline | `alib02_digest`（digest_kind/receipt/live entities/empty digest）; `vcs05_restore::missing_asset_fails_and_leaves_no_destination`; `restore_as_copy_preserves_*`; `alib02_v5_real::missing_blob_classifies_degraded_not_rejected`; design §Remote Asset |
| PM-T-19 | revision 分離 | `controller_store::eval_plan_is_stale_tracks_frozen_inputs`; `vcs01_snapshot::recovery_checkpoint_is_isolated_from_saved_head`; `live_capture_is_immutable_across_edits_undo_and_save`; `live_and_saved_captures_share_digest_but_not_identity` |
| PM-T-20 | 失敗分類 | `store_publish::failed_snapshot_publish_keeps_head_and_destination`/`failed_head_publish_keeps_snapshot_orphan_and_old_head`/`failed_stage_create_leaves_no_head`; `vcs05_restore` failure 系; `vcs06_restore_matrix`（interrupt/tampered/foreign/locked/readonly）; `vcs01_snapshot::cancelled_token_fails_explicitly_at_phase_boundaries`; `tim_01_ppqn::ppqn_change_rejects_tick_overflow`; `migration` typed error（`alib02_v5_real::unknown_schema_is_rejected_not_degraded`） |
| PM-T-21 | Branch/Scope/Lineage 契約 | design §Branch/Scope/Lineage（design-review のみ。実装は #277/#278 別 slice） |
| PM-T-22 | Ruviform 境界・第二正本禁止 | design §Ruviform/Synvelis 境界 + `workspace-model-design` の ProjectSession 単一正本契約（design-review） |

### NotRun / Deferred（headless 範囲外）

| Case | 内容 | 状態 | 後続 gate |
| --- | --- | --- | --- |
| PM-T-23 | 実 GUI Clip/Track 編集・hit-test・gesture | NotRun | synvelis#112/#114（GUI-L2）・#11（timeline tools） |
| PM-T-24 | 実 plugin state/instance save/restore・実 plugin GUI | NotRun | synvelis#176・tim-04・ruviform plugin host（EXT-L3） |
| PM-T-25 | 実 RT no-alloc/no-block・execution graph publish・sample 一致 | NotRun | synvelis#294・ruviform#219/#294 consumer gate・#18 |
| PM-T-26 | 実 storage durability・電源断・stale lock 実測・real FS fault | NotRun | synvelis#84/#330・ruviform-io-file real FS・rcd-02 durability |
| PM-T-27 | macOS/Linux lane | Deferred | POLICIES.md — 使用要件未発生 |
| PM-T-28 | Creative Branch 実装（VariantScope/BranchContentRef/Adopt） | NotRun | synvelis#277/#278 別 slice（本書は契約のみ） |
| PM-T-29 | remote provider fetch/cancel 実装・offline 再生の実 fetch | NotRun | ALIB 系別 slice（alib-04/06/07）・synvelis#9 |
| PM-T-30 | cross-consumer/RT gate（他 consumer での project model 検証） | NotRun | ruviform#219/#294・synvelis consumer gate |

## Guarantees

- headless test は「意味論 oracle」であり、OS storage/実 plugin/実 RT/
  実 GUI を保証しない。実機項目は必ず NotRun/Deferred で記録する。
- `synvelis-project` の test は `ProjectController`/`InMemoryStore`/
  `DirectoryBundleStore`（temp dir）上の契約を検証し、実 durability・
  実 device・実 plugin を含まない。
- 本 slice は新規 test を追加しない。既存 fixture が PM-T oracle を
  満たさない場合は gap として記録し、実装は別 Issue に委譲する。

## Non-goals

- `crates/synvelis-project` の実装変更・新規 test 追加・schema version
  bump。
- 実 GUI/実 plugin/実 RT/実 storage/実 OS の検証。
- Creative Branch・remote provider・execution graph の実装。
- Ruviform API の変更。

## Risk

- 既存 test が PM-T oracle を完全には網羅しない領域（Folder capability
  の routing 非干渉・Branch/Scope・remote cancel）は design-review と
  NotRun/Deferred で明示し、実装 gap を隠さない。
- `md_a.rs`/`rcd_04_*` は comp/recording fixture であり、汎用 clip
  move/trim/slip op の直接 test が薄い場合は design-review + 関連 diff
  test で補完し、gap は evidence に記録する。
