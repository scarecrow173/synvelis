# PC-C6 acceptance cross-gate headless fixture 被覆棚卸し (Issue #103 / synvelis#103)

Issue #103 固定 Test Matrix 全 53 行（PC-C1-R01〜R08・
PC-C2-REL01〜REL07・PC-C3-TR01〜TR08・PC-C4-PL01〜PL08・
PC-C5-UI01〜UI07・PC-C6-COMP01〜COMP05・PC-X1〜X6・
GUI-PC-01〜04）を、統合済み fixture/docs へ写像した棚卸し
結果。行→証拠の正本対応表は `acceptance-ledger.md`。

凡例: ✅ headless 証拠あり（本 slice で `mise run test` 実測
green）/ ◐ 部分的 / ⛔ 外部 Gate（NotRun/Unsupported —
`external-gates.md` 参照）

## 計測環境（2026-11-14）

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | `eac70aca419508c9837fa18a03f4f75223e52749`（origin/main HEAD = 本 slice base） |
| Ruviform resolved SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock` git dep） |
| OS / CPU / FS | Windows 11 Pro 10.0.26300 / Intel Core i9-9900K / NTFS（C:） |
| Device + Plugin | なし（headless — audio device・native plugin host 未接続） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024 |

---

## PC-C1-R01〜R08（Recording Safety）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| PC-C1-R01 | `synvelis-workspace/tests/pc_c6_oracle_rec.rs::pc_c6_r01_record_start_fixes_context`（独立 oracle）・`pc_c1.rs::rec_c1_05_lifecycle_gates`・`aut_03.rs::aut_03_prepared_commit_pins_fixed_context` | ✅ T0 headless / ⛔ 実機録音は T3 |
| PC-C1-R02 | `pc_c6_oracle_rec.rs::pc_c6_r02_foreign_change_conflict_retains_points`・`pc_c1.rs::rec_c1_02_*`・`aut_03.rs::aut_03_conflict_retains_points_until_explicit_rebase` | ✅ T0 |
| PC-C1-R03 | `pc_c6_oracle_rec.rs::pc_c6_r03_structural_rejection_retains_columns`・`pc_c1.rs::rec_c1_03_*` | ✅ T0 |
| PC-C1-R04 | `pc_c6_oracle_rec.rs::pc_c6_r04_commit_one_undo_exactly_once`・`pc_c1.rs::rec_c1_01_*`・`rec_c1_04_duplicate_ack_is_exactly_once` | ✅ T0 |
| PC-C1-R05 | `pc_c6_oracle_rec.rs::pc_c6_r05_explicit_rebase_repreview_then_commit`・`uic5_04.rs::uic5_04_foreign_change_conflicts_then_explicit_rebase_pending` | ✅ T0 |
| PC-C1-R06 | `pc_c6_oracle_rec.rs::pc_c6_r06_loop_wrap_seals_and_pass_advances`・`aut_03.rs::aut_03_wrap_*`・`aut_03_later_passes_write_nothing_until_explicit_rearm`・`aut_03_pass_lifecycle`・`aut_03_multi_lane_pass_is_one_undo` | ✅ T0 |
| PC-C1-R07 | `pc_c6_oracle_rec.rs::pc_c6_r07_mode_gating_oracle`・`aut_01.rs` mode 系（read/touch/latch/write/boundary）・`pc_a.rs::pc_a_05_recording_gates_undo_seal_fixation` | ✅ T0 |
| PC-C1-R08 | `pc_c6_oracle_rec.rs::pc_c6_r08_fixed_scope_survives_project_drift`（tempo/clip drift）・`pc_c1.rs::rec_c1_06_*`・`synvelis-project/tests/ctl_01.rs` scope 行列・`synvelis-workspace/tests/pc_b.rs::pc_b_02_note_scope_records_note_relative` | ◐ note-scope は eval 層+record transaction 単一経路までの被覆（R08 の Note/Clip/Tempo×drift 全組合せ fixture 無し）/ ⛔ 実 RT 不変は T3 |

## PC-C2-REL01〜REL07（Relation Index）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| PC-C2-REL01 | `synvelis-project/tests/pc_c6_oracle_rel.rs::rel01_directions_agree_with_oracle`・`pc_c2_relation.rs::rel_c2_01_*`・`rel_c5.rs` | ✅ T0 |
| PC-C2-REL02 | `pc_c6_oracle_rel.rs::rel02_source_kinds_and_depth_driver`・`pc_c2_relation.rs::rel_c2_02_*`・`rel_c5.rs::assignment_and_depth_hops_*` | ✅ T0 |
| PC-C2-REL03 | `pc_c6_oracle_rel.rs::rel03_duplicate_assignments_and_disabled`・`rel_c5.rs::include_disabled_gates_disabled_edges` | ✅ T0 |
| PC-C2-REL04 | `pc_c6_oracle_rel.rs::rel04_cycles_dangling_and_unsupported`・`pc_c2_relation.rs::rel_c2_04_*`・`rel_c5.rs::cycle_edges_*` | ✅ T0 |
| PC-C2-REL05 | `pc_c6_oracle_rel.rs::rel05_bounds_and_truncation`・`rel_c5.rs::edge_budget_surfaces_truncated_coverage` | ✅ T0 |
| PC-C2-REL06 | `pc_c6_oracle_rel.rs::rel06_stale_unavailable_and_revision_delta`・`pc_c2_relation.rs::rel_c2_03_*`・`rel_c5.rs::stale/unknown_*` | ✅ T0 |
| PC-C2-REL07 | `pc_c6_oracle_rel.rs::rel07_large_graph_measurement` | ◐ 絶対性能閾値は baseline 未確定で NotRun（sanity 上限のみ） |

## PC-C3-TR01〜TR08（Control Trace）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| PC-C3-TR01 | `pc_c6_oracle_tr.rs::tr01_stage_independence`・`pc_c6_oracle_tr_dispatch.rs::tr01_stage_independence_real_ledger`・`trace_c3.rs::trace_c3_01_*`・`trace_c3_05.rs::trace_c3_05_stages_fill_from_ledger_events` | ✅ T0+T2 headless |
| PC-C3-TR02 | `pc_c6_oracle_tr.rs::tr02_context_mismatch_dimensions`・`pc_c6_oracle_tr_dispatch.rs::tr02_context_mismatch_dimensions_real_ledger`・`trace_c3.rs::trace_c3_02_context_mismatch_is_stale` | ✅ T0+T2 |
| PC-C3-TR03 | `pc_c6_oracle_tr.rs::tr03_transport_seek_loop_restart`・`pc_c6_oracle_tr_dispatch.rs::tr03_context_change_stale_real_ledger` | ✅ T0+T2 |
| PC-C3-TR04 | `pc_c6_oracle_tr.rs::tr04_plugin_unknown_vs_probe_states`・`pc_c6_oracle_tr_dispatch.rs::tr04_plugin_unavailable_vs_probe_states`・`trace_c3.rs::trace_c3_03_probe_channel_states` | ✅ T0+T2 |
| PC-C3-TR05 | `pc_c6_oracle_tr.rs::tr05_stamp_provenance_and_age_matrix`・`pc_c6_oracle_tr_dispatch.rs::tr05_plugin_reported_stamp_provenance`・`trace_c3.rs::trace_c3_02_stamp_quality_classification` | ✅ T0+T2 |
| PC-C3-TR06 | `pc_c6_oracle_tr.rs::tr06_silence_vs_unobserved`・`pc_c6_oracle_tr_dispatch.rs::tr06_silence_vs_unobserved_real_ledger` | ✅ T0+T2 |
| PC-C3-TR07 | `pc_c6_oracle_tr.rs::tr07_shared_probe_two_contexts`・`pc_c6_oracle_tr_dispatch.rs::tr07_shared_probe_two_contexts_real_ledger` | ◐ 実 2 Inspector/2 window は #102/#114 責務 — headless は 2 trace 独立性まで |
| PC-C3-TR08 | `pc_c6_oracle_tr.rs::tr08_determinism_and_bounded_size`・`pc_c6_oracle_tr_dispatch.rs::tr08_bounded_retention_metrics_real_ledger`・`trace_c3_05.rs::trace_c3_05_bounded_history_and_expired`・`trace_c3_05_rt.rs::rt_trace_adapter_hot_path_alloc_free` | ✅ T0+T2 / ⛔ 実 RT device NotRun |

## PC-C4-PL01〜PL08（Plugin Binding/Dispatch）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| PC-C4-PL01 | `synvelis-player/tests/pc_c6_oracle_pl.rs::pc_c6_pl01_*`・`pc_c4_binding.rs::pc_c4_05_verified_frame_base_contract`・`pc_c4_02_stage1/instance_authority_*` | ✅ T2 headless+fake plugin / ⛔ 実 plugin capability T3 |
| PC-C4-PL02 | `pc_c6_oracle_pl.rs::pc_c6_pl02_param_identity_and_capability_contract`・`pc_c4_binding.rs::pc_c4_02_typed_rejects_no_misdelivery`・`pc_c4_03_observation.rs`・`pc_a.rs::pc_a_07_capability_honesty` | ✅ T2/T0 |
| PC-C4-PL03 | `pc_c6_oracle_pl.rs::pc_c6_pl06_plan_staleness_and_generation_axes`（semantic=Rule(PL03)） | ✅ T2 |
| PC-C4-PL04 | `pc_c6_oracle_pl.rs::pc_c6_pl03_queue_full_and_atomic_batch_rejection`（semantic=Rule(PL04)） | ✅ T2 |
| PC-C4-PL05 | `pc_c6_oracle_pl.rs::pc_c6_pl05_min_process_frame_offset_delivery`・`pc_c6_oracle_pl_rt.rs::rt_pl05_param_delivery_alloc_free`・`pc_c4_binding_rt.rs`・`tim03_process_boundary.rs`/`tim03_request_plan.rs` min_frames 系 | ✅ T2+RT noalloc headless / ⛔ 実 plugin/実 RT device NotRun |
| PC-C4-PL06 | `pc_c6_oracle_pl.rs::pc_c6_pl04_reload_keeps_instance_old_records_dead`（semantic=Rule(PL06)）・`pc_c4_07_capability_rebind.rs`・`pc_c4_binding.rs::pc_c4_04_generation_consistency` | ✅ T2 |
| PC-C4-PL07 | `pc_c6_oracle_pl.rs::pc_c6_pl07_shared_param_and_stale_record_zero_misdelivery`・`pc_c4_binding.rs::pc_c4_02_typed_rejects_no_misdelivery`・`synvelis-player/tests/pc_b.rs::pc_b_04/05_*` | ✅ T2 / ⛔ 実 plugin 誤配送 NotRun |
| PC-C4-PL08 | `pc_c6_oracle_pl.rs::pc_c6_pl08_headless_fixture_boundary_marker` | ✅ marker / ⛔ 実 VST3/CLAP/GUI/RT device は NotRun（boundary marker が Pass 昇格を禁止） |

## PC-C5-UI01〜UI07（Control Surface headless oracle）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| PC-C5-UI01 | `synvelis-workspace/tests/pc_c6_oracle_ui.rs::pc_c6_ui01_*`・`rel_c5.rs::projection_is_read_only_and_deterministic`・`sf_05.rs` view 系・`aut_04.rs::automation_view_is_read_only_and_never_mutates_source` | ◐ 3 view の単一 fixture 同時照合は無し（view 別の declared-truth 照合の複合証拠） |
| PC-C5-UI02 | `pc_c6_oracle_ui.rs::pc_c6_ui02_nav_roundtrip_both_directions`・`rel_c5.rs` nav/hop 系・`uic5.rs::uic5_03_explore_history_back_forward` | ✅ T0 |
| PC-C5-UI03 | `pc_c6_oracle_ui.rs::pc_c6_ui03_stale_revision_never_current`・`rel_c5.rs::stale_expected_revision_*`・`uic5.rs::uic5_02_commit_drops_on_stale_revision`・`sf_05.rs::expected_revision_mismatch_is_stale_with_empty_graph` | ✅ T0 |
| PC-C5-UI04 | `pc_c6_oracle_ui.rs::pc_c6_ui04_two_window_epoch_and_cancel`・`uic5.rs::uic5_02_commit_drops_on_window_epoch_change`・`uic5_03_two_window_dock_focus_no_retarget`・`sv47_controls.rs` window/detach 系・`sf_05_s2.rs::routing_gesture_window_transfer_drops_stale` | ✅ T0 / ⛔ 実 GPUI 2 window/focus は T3（#114） |
| PC-C5-UI05 | `pc_c6_oracle_ui.rs::pc_c6_ui05_gesture_lifecycle_one_undo`・`uic5.rs::uic5_02_edge_begin_preview_commit_undo`・`uic5_02_preview_and_cancel_leave_project_untouched`・`sv47_controls.rs` gesture 系 | ✅ T0 |
| PC-C5-UI06 | `pc_c6_oracle_ui.rs::pc_c6_ui06_probe_ownership_boundary_marker` | ✅ marker |
| PC-C5-UI07 | `pc_c6_oracle_ui.rs::pc_c6_ui07_headless_window_host_boundary_marker` | ✅ marker / ⛔ 実 GUI は T3（#114） |

## PC-C6-COMP01〜COMP05（Cross-gate 判定）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| PC-C6-COMP01 | `synvelis-gateway/tests/gw10_system.rs::e2e1_ai_off_human_pipeline_commits_undoes_and_recovers`・headless suite 全体非回帰・`docs/designs/ai-native-baseline-design.md` | ◐ headless 構成証拠のみ / ⛔ 製品 Gate A 判定は #23 |
| PC-C6-COMP02 | TR01/TR05/TR04 oracle 群（configured/observed/estimated 分離）・`pc_a.rs::pc_a_04_breakdown_and_observation_states`・`pc_c4_03_observation.rs` | ◐ headless 分離証拠 / ⛔ 製品 Gate B 判定は #23 |
| PC-C6-COMP03 | REL01 Both・UI02 nav・`uic5_02_edge_and_routing_gestures_are_distinct` | ◐ headless 双方向証拠 / ⛔ 実 GUI は #114 |
| PC-C6-COMP04 | `Cargo.lock` 単一 Ruviform pin + `tools/check-ruviform-rev.mjs`・Synvelis `*_rt.rs` alloc-free 群 | ◐ Synvelis 側のみ / ⛔ 両 Consumer 独立判定は ruviform#219/#294 + synvelis#28 |
| PC-C6-COMP05 | 録音=`pc_c1`/`pc_c6_oracle_rec`/`aut_03/04`、RT=`*_rt.rs`、Export/Graph/Transport=`l1_e2e`/`sf03_*`/`tim03_*`/`sf03_render`/`l2_e2e`/`x_e2e`（render は参照のみ）、Plugin=`synvelis-player/tests/pc_b`/`pc_c4_*`/`synvelis-player/tests/tim04_plugin_state` | ◐ headless 非回帰 / ⛔ 実 device/plugin export・長時間回帰 NotRun |

## PC-X1〜X6（固定 Matrix 由来）

| caseId | 被覆 fixture | 判定 |
| --- | --- | --- |
| PC-X1 | `pc_a.rs::pc_a_01/02/03`・`controller_store.rs`・`synvelis-project/tests/ctl_01.rs`（legacy scope tag・v6 migration・scope 行列・singleton）・`alib02_v5_real.rs`・`sv47_controls.rs::duplicate_command_id_is_exactly_once` | ✅ T0 / ⛔ GUI 側は #114 |
| PC-X2 | `aut_01.rs` 15 tests（touch 150ms smoothstep・return 非破壊・mode 系・boundary/transport）・`aut_03.rs` loop 系・`sv47_controls.rs` transport/seek/focus・`pc_a.rs::pc_a_04` | ◐ mode×transport/seek/loop/focus の全直積 fixture は無し（次元別被覆）/ ⛔ 実機 touch NotRun |
| PC-X3 | `aut_02.rs` 15 tests（step/linear replace・boundary・undo・RT eval parity・`cut_range_points` 回帰）・`aut_04.rs::automation_view_*` | ✅ T0 / ⛔ GUI Range 編集は #114 |
| PC-X4 | `aut_03.rs`（1Pass・rearm・seal 系）・`pc_c6_oracle_rec.rs::pc_c6_r02/r03`・`pc_c1.rs::rec_c1_03_*`・`uic5_04.rs`/`sv47_controls.rs` 2 window・`pc_c6_oracle_tr_dispatch.rs` plugin trace | ✅ T0+T2 |
| PC-X5 | `aut_04.rs`（exact rational・view LOD reduced/truncated・drain gap/budget）・`aut_04_rt.rs::record_event_rt_alloc_free_bounded_and_typed`・`aut_03.rs::aut_03_raw_values_saved_separately_survive_reload` | ◐ 絶対性能閾値 NotRun（baseline 未確定）/ ⛔ 実 device NotRun |
| PC-X6 | `tim03_process_boundary.rs`/`tim03_request_plan.rs`（min_frames）・`pc_c4_binding.rs::pc_c4_05_verified_frame_base_contract`・`pc_c4_07_transport_observe.rs`・`synvelis-player/tests/pc_b.rs::pc_b_04/05_*`・`synvelis-player/tests/tim04_plugin_state.rs`（best_effort/coordinated/quiescent/unsupported）・`synvelis-project/tests/plugin_state.rs`・`pc_c6_oracle_pl.rs::pc_c6_pl07/pl04` | ✅ T2 headless+fake plugin / ⛔ 実 plugin capability/state・旧 RawID 実機 NotRun |

## GUI-PC-01〜04（Scenario Owner: synvelis#114）

| caseId | 独立 Oracle 側の証拠対応 | 判定 |
| --- | --- | --- |
| GUI-PC-01 | `pc_c6_oracle_rec.rs::pc_c6_r02/r04/r05`・`sv47_controls.rs` gesture/undo 系・`uic5.rs::uic5_02_*` | ✅ headless Oracle / ⛔ 実 GUI Scenario は #114 NotRun |
| GUI-PC-02 | `pc_c6_oracle_pl.rs::pc_c6_pl04/pl07`・`pc_c4_07_capability_rebind.rs`・`pc_c4_binding.rs::pc_c4_04_*` | ✅ headless Oracle / ⛔ 実 GUI/plugin は #114/#176 NotRun |
| GUI-PC-03 | `pc_c6_oracle_ui.rs::pc_c6_ui01/ui02`・`pc_c6_oracle_rel.rs::rel01_*`・`uic5.rs::uic5_02_edge_and_routing_gestures_are_distinct` | ✅ headless Oracle / ⛔ 実 GUI は #114 NotRun |
| GUI-PC-04 | `pc_c6_oracle_ui.rs::pc_c6_ui03/ui04`・`sv47_controls.rs` window/detach 系・`pc_c6_oracle_tr_dispatch.rs::tr04_*` | ✅ headless Oracle / ⛔ 実 GUI/plugin GUI loss は #114/#176 NotRun |

## fixture 補完判定

全 53 行の headless 実施可能部分は既存 fixture で被覆済み。
新規 test 追加は不要（詳細は `acceptance-ledger.md`「fixture
補完判定」節）。

## 判定ルール

- ✅ は「当該 SHA で `mise run test <package>` が green だった」
  ことのみを意味し、case 全体の受入 Pass ではない。
- ◐ は部分被覆 — 残件を行内に明記する。
- headless の成功を GUI/device/plugin/consumer の Pass に
  昇格しない。
