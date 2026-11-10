---
id: "TSPEC-vcs06-fixture-matrix"
type: "test-spec"
status: "approved"
title: "VCS-06 fixture matrix tests"
created: "2026-11-13"
updated: "2026-11-13"
owners: []
relations:
  source:
    - "SPEC-vcs06-fixture-fault-matrix"
  verifies:
    - "TASK-vcs06-fixture-fault-matrix"
  supersedes: []
  superseded_by: []
---

# VCS-06 fixture matrix tests

## Feature: uncovered fault rows

- **Rule VCS-DIFF-NOTE** クリップローカル Note の編集/追加が型付き
  `Modified`/`Added` として出る。lane 順序変更は `Reordered`。
  automation 補間形状・asset 追加・MIDI note payload も型付き。
  Examples: `clip_note_edit_and_addition_are_typed_changes`,
  `lane_order_change_is_reordered_not_modified`,
  `automation_interpolation_change_is_modified`,
  `asset_added_is_typed_add_not_reorder`,
  `midi_note_edit_and_addition_are_typed_changes`.
- **Rule VCS-APPLY-N** 複数 change は 1 undo step。cancel・
  UnsupportedField・TargetMissing・TargetConflict・MissingPlugin・
  Unknown/Partial 拒否・ディスク上 journal の再起動 replay を検証。
  Examples: `multi_change_apply_commits_as_single_undo_step`,
  `cancelled_intent_fails_closed_before_mutation`,
  `unsupported_singleton_field_fails_closed`,
  `modified_change_on_absent_target_is_target_missing`,
  `added_change_with_live_id_collision_is_target_conflict`,
  `unavailable_plugin_binding_fails_closed`,
  `processor_changeset_with_unknown_opaque_state_is_not_applicable`,
  `directory_journal_survives_restart_and_replays`.
- **Rule VCS-RESTORE-N** 中断 staging の resume・改竄 envelope の
  readback 拒否・foreign/orphan manifest の StagingConflict・
  plugin-state 必須/複写・`..` leaf・大小文字衝突 sibling。
  Examples: `interrupted_staging_resumes_to_published`,
  `tampered_staging_fails_resume_and_keeps_destination_absent`,
  `foreign_staging_manifest_conflicts`, `orphan_staging_dir_conflicts`,
  `processor_project_requires_plugin_state_source`,
  `plugin_state_manifest_and_blobs_are_copied`,
  `dotdot_leaf_is_rejected_before_any_write`,
  `case_conflicting_sibling_is_rejected`.
- **Rule VCS-GIT-N** symlink index エントリは target bytes を保持、
  remote credential は provider surface に出ない、history page は
  clamp・limit=0 は拒否。Examples:
  `symlink_index_entry_keeps_link_target_bytes`,
  `remote_credentials_never_surface_in_provider_output`,
  `history_limit_is_clamped_and_zero_rejected`.
