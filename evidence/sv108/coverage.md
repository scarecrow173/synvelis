# SF-UX-01..07 coverage map (Issue #108 / synvelis#108)

Signal Flow Editor の headless 受入写像。実装本体は SF-05
slice1..3（全て main 統合済み）。本タスクの新規証拠は
`crates/synvelis-workspace/tests/sf_05_s2.rs` に追加した
7 テスト（ファイル内 "SF-05 residual" セクション。6 つの
headless 欠落に対応 — dock/close retention は move+pool と
close で分割）。
正本文書チェーンは既存 slice1..3 の
spec/design/plan/task/test-spec であり、本台帳はそれらへの
写像のみを記録する（新規 canonical 文書は作成しない —
Coordinator 指示）。

凡例: ✅ headless 証拠あり / ◐ 部分的（残件は外部 Gate）/
⛔ 外部 Gate（NotRun/Deferred、`external-gates.md` 参照）

| AC | 内容 | 既存証拠 | 本タスク追加 | 判定 |
| --- | --- | --- | --- | --- |
| SF-UX-01 | Bass→Music Bus→Master・Kick→Sidechain・Pre/Post Send→Return・Processor order が Overview/Local/Expanded/Collapse/Role Filter で意味を維持、同 Connection/Send の二重 Edge なし | `sf_05.rs`: `overview_projects_every_strip_and_every_connection_exactly_once`・`expanded_exposes_processor_nodes_and_ordered_chain_edges`・`collapse_folds_processor_expansion_without_losing_the_strip`・`local_scope_projects_the_root_and_its_direct_neighborhood`・`local_processor_root_expands_only_that_processor`・`role_filter_narrows_edges_without_dropping_nodes`。`sf05_signal_flow.rs`: `view_op_projects_hierarchy_and_stale_revision_honestly`・`visual_set_scope_local_and_role_filter_round_trip` | — | ✅ |
| SF-UX-02 | Node Move/Visual Group/Fold/Zoom/Link Focus のみで DSP/ProjectRevision/Undo/Graph build/PCM 不変 | `sf_05.rs`: `graph_queries_and_visual_ops_leave_the_session_untouched`・`visual_state_is_a_pure_value_with_monotone_revision`。`sf05_signal_flow.rs`: `visual_ops_leave_project_revision_undo_engine_and_receipt_untouched` | `visual_layout_is_excluded_from_project_semantic_diff`（semantic diff 層での `Equal` 明示証明） | ✅ |
| SF-UX-03 | Port Drag fixation・ChannelMap consent・stale revision/window/capability・plugin port reconfigure・cancel・late receipt・target stability・成功 1 Undo | `sf_05_s2.rs`: `routing_gesture_commit_is_one_atomic_undo`・`routing_gesture_preview_and_cancel_never_touch_the_project`・`routing_gesture_drop_on_empty_canvas_cancels`・`routing_gesture_stale_revision_drops_without_retarget`・`routing_gesture_window_transfer_drops_stale`・`routing_gesture_rejects_direction_mismatch_as_prohibited`・`routing_gesture_channel_mismatch_requires_explicit_consent`・`routing_gesture_mutual_exclusion_with_other_gesture_families`。`sf05_signal_flow.rs`: `signal_flow_routing_gesture_commits_one_atomic_undo`・`signal_flow_routing_cancel_and_stale_revision_are_project_safe`・`signal_flow_routing_rejects_bad_params_and_unknown_actions` | `routing_gesture_concurrent_window_revision_second_commit_is_stale`・`routing_gesture_capability_generation_drift_drops_stale`・`routing_gesture_terminal_state_rejects_late_input` | ✅ headless / ⛔ 実 plugin port 再構成は外部 Gate |
| SF-UX-04 | Follow/Explore/Pinned・selection 独立・link delivery・2 Window/Dock/Close/Restore・Keyboard/IME/Plugin GUI focus で誤転送なし | `sf_05_s2.rs`: `signal_flow_inspect_is_independent_of_selection_and_arrangement_focus`・`signal_flow_link_delivery_retargets_only_follow_mode`・`signal_flow_link_follow_source_filter_and_dedup`・`signal_flow_explore_history_and_unmappable_targets`・`signal_flow_state_stays_view_local_across_views`・`capture_restore_roundtrips_signal_flow_state_per_view`・`inspection_focus_type_roundtrips_through_snapshot`。`sf05_signal_flow.rs`: `signal_flow_inspection_modes_history_and_follow_source`・`signal_flow_state_is_per_view_and_survives_snapshot_round_trip` | `input_context_loss_drops_gesture_without_misdelivering_focus`・`signal_flow_state_survives_dock_moves_and_pool_roundtrip`・`closing_view_mid_gesture_discards_preview_without_touching_project` | ✅ headless / ⛔ 実 OS focus/IME/Plugin GUI は外部 Gate |
| SF-UX-05 | `Configured/Prepared/Published/Active/Observed` と `Unavailable/Unresolved/Stale/Lost/Unsupported` を正しい Revision/Generation で表示。Publish≠Active、probe 未取得を表示 | `sf05_signal_flow.rs`: `published_without_rt_observation_is_not_displayed_as_active`・`absent_engine_reports_unattached_and_unobserved`。`sf_05.rs`: `absent_engine_evidence_reports_unobserved_not_success`・`expected_revision_mismatch_is_stale_with_empty_graph`・`local_unknown_root_is_unavailable`・`history_scope_and_empty_layers_are_unsupported` | — | ✅ |
| SF-UX-06 | Probe/Inspector 同 Point を複数 Window で共有・Capture 重複なし。Graph change で Capture 区間分離、Console/Plugin/PCM leakage なし | `sf05_probe_share.rs`: `two_views_share_one_probe_subscription`・`view_close_releases_only_own_lease`・`capture_is_owner_only_and_retreats_to_session_on_close`・`unsubscribe_releases_view_lease_and_fail_closed`・`subscriber_actions_fail_closed_without_engine`・`signal_flow_screen_shows_probe_subscriptions`。`synvelis-workspace` unit: `probe_trace::tests::sf04_s5_*`（lease/fan-out/owner gate） | — | ✅ headless / ⛔ 実機 capture は外部 Gate |
| SF-UX-07 | Workspace Schema v2→新 View State の安全移行・不明 schema 拒否・被破損部分の明示回復・visual Layout の Project Semantic Diff 除外 | `sf_05_s2.rs`: `snapshot_v2_without_signal_flow_migrates`・`snapshot_malformed_signal_flow_degrades_to_none`・`snapshot_rejects_unreadable_schemas` | `visual_layout_is_excluded_from_project_semantic_diff` | ✅ |

## 新規テスト詳細（`sf_05_s2.rs` residual セクション）

### `routing_gesture_concurrent_window_revision_second_commit_is_stale`

SF-UX-03 concurrent window revision。2 window・2 view が同一
project revision に fixate した gesture を並行実行。先着
`Applied` が 1 route を commit した後、後着は `Stale` で drop —
previewed target を新 revision へ retarget せず、undo 1 回で
committed route のみが戻る（成功 1 Undo 境界）。

### `routing_gesture_capability_generation_drift_drops_stale`

SF-UX-03 plugin port reconfigure 相当。gesture 中に resolved
processor binding を `capability_revision` bump で差し替え、
`port_capability_generation` が fixation 値から動くことを確認。
confirm は `Stale` を返し project/connections は不変。

### `routing_gesture_terminal_state_rejects_late_input`

SF-UX-03 late receipt。Applied 終端後の late hover/drop/confirm/
cancel が全て `WorkspaceError::NoGesture` で fail-closed。
project/revision 不変、gesture slot は解放され新規 gesture が
開始可能。

### `input_context_loss_drops_gesture_without_misdelivering_focus`

SF-UX-04 Keyboard/IME/Plugin GUI focus analogue。
`InputContext::{TextInput,Ime,PluginGui,Modal}` への遷移で
View→非 View の度に進行中 gesture が drop。late confirm も
`NoGesture`。editing selection・arrangement inspection・
Signal Flow inspection の 3 focus channel が誤転送されない。

### `signal_flow_state_survives_dock_moves_and_pool_roundtrip`

SF-UX-04 Dock/Close/Restore。`ViewInstanceId` key の
Signal Flow state（inspection target + visual position）が
dock move・`detach_to_pool`・`reattach` を越えて保持され、
sibling view の state に漏れない。

### `closing_view_mid_gesture_discards_preview_without_touching_project`

SF-UX-04 Close。dropped-target 状態で `remove_view` すると
preview は view と共に破棄。project/revision/sibling inspection
は不変、生存 view の gesture は継続可能。

### `visual_layout_is_excluded_from_project_semantic_diff`

SF-UX-07 semantic diff 除外。visual mutation 全 surface +
inspection 変更の前後で `compare_snapshots` が
`ComparisonEquality::Equal`・total 0・changes 空。直後の
`SetProjectName` で diff が出る陽性対照で oracle の非空虚性を
保証。
