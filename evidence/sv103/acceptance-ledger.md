# Acceptance Ledger — synvelis#103 全受入行 → 証拠対応表（PC-C6 cross-gate Domain Oracle）

Date: 2026-11-14. Slice: acceptance-ledger residual slice
（Issue #103 固定 Test Matrix 全行の統合済み fixture/docs への
照合 audit + headless 欠落 fixture の有無判定 + evidence 台帳化）。
**新規 fixture/test 追加は監査の結果として不要と判定**（後述
「fixture 補完判定」）。実 OS/実 GUI/実 Plugin/実 Device/両
Consumer レーンは一切実行していない。headless の Pass を実
VST3/GPUI/OS/実 RT/consumer の Pass に昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/103
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | `eac70aca419508c9837fa18a03f4f75223e52749`（origin/main HEAD = 本 slice base。ledger 自体は同 commit 内。CI covering run は merge 後 SHA） |
| Ruviform resolved SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock` git dep） |
| OS / CPU / FS | Windows 11 Pro 10.0.26300 / Intel Core i9-9900K / NTFS（C:） |
| Device + Plugin | なし（headless — audio device・native plugin host 未接続。plugin は in-process fake） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024 |
| Project schema | v9 envelope 系（v5/v6/v7 migration fixture あり） |

## 判定語彙

- **Pass-measured (headless)**: 本 slice で `mise run test
  <package>` が当該 SHA で green だった fixture/contract test の
  存在を確認したことのみを意味する。case 全体の受入 Pass では
  ない。
- **NotRun**: 実 OS/実 GUI/実 plugin/実 device/実 RT/consumer
  gate・絶対性能閾値など、本 slice で実行していない部分。
- **Unsupported**: 環境/能力の技術的非対応（本 ledger では
  該当行なし — `Unsupported` は捏造しない）。
- **external owner**: 別 Issue/Repo が受入 Owner の部分
  （synvelis#114 実 GUI Scenario・synvelis#28 Ruviform 追従・
  ruviform#307 Bridge Clock・ruviform#219/#294 両 Consumer
  gate・synvelis#23 実行/製品 Gate・synvelis#176 Native Dock）。

行の判定は「headless で実施可能な部分が fixture で被覆され、
かつ本 slice の `mise run test` で green」の場合に
`Pass-measured (headless)` とし、実機依存部分は備考と
`external-gates.md` に `NotRun`/`external owner` として分離
記録する。

## 監査方法

Issue #103「固定 Test Matrix」（PC-C1-R01〜R08・PC-C2-REL01〜
REL07・PC-C3-TR01〜TR08・PC-C4-PL01〜PL08・PC-C5-UI01〜UI07・
PC-C6-COMP01〜COMP05）と、2026-11-04 追記の固定 Matrix 由来行
（PC-X1〜X6）・Shared GUI Scenario IDs（GUI-PC-01〜04）の
**計 53 行**を、以下の統合済み資産へ写像した。

- 実装 slice の doc chain: `docs/specs/pc-c6-oracle-rows.md`・
  `docs/specs/pc-c6-oracle-rel-tr.md`・
  `docs/specs/pc-c6-tr-ledger-oracle.md`・
  `docs/specs/pc-c6-tr02-tr06-realpath-oracle.md` と対応する
  `docs/test-specs/`・`docs/designs/`・`docs/plans/`・
  `docs/tasks/`・`docs/impl/{ir,exp}/`（spec/design/plan/task/
  test-spec の新規作成は本 slice 範囲外 — 既存鎖を参照）。
- fixture: `crates/synvelis-project/tests/`・
  `crates/synvelis-player/tests/`（本 slice の編集可能範囲）
  および `crates/synvelis-workspace/tests/`・
  `crates/synvelis-app/tests/`・`crates/synvelis-gateway/tests/`
  ・`crates/synvelis-render/tests/`（参照のみ・未編集）。
- PC-X 行の一次仕様は Issue #103 本文「2026-11-04 4領域合意」
  節と各先行 spec（`docs/specs/pc-a-*`・`pc-b-*`・`pc-c1-*`・
  `aut_0*` 系の test-spec）。

---

## PC-C1-R01〜R08 — Automation Recording Safety

一次 oracle: `crates/synvelis-workspace/tests/pc_c6_oracle_rec.rs`
（`OracleModel` 宣言 vs `control_with` 実経路の field 照合、
8 tests — 本 slice `mise run test synvelis-workspace` で全 pass）。
実装契約側: `crates/synvelis-workspace/tests/pc_c1.rs`
（`rec_c1_01`〜`rec_c1_06`）・`aut_03.rs`・`uic5_04.rs`。
実 RT device での録音/点列不変は **NotRun**（headless 範囲のみ）。

| 行 | 受入内容（Issue 転記の要約） | 証拠 pointer | 判定 |
| --- | --- | --- | --- |
| PC-C1-R01 | Record Start が Revision/Target/Lane/Clock Epoch/Scope/Loop Pass/Seal を固定 | `pc_c6_oracle_rec.rs::pc_c6_r01_record_start_fixes_context`、`pc_c1.rs::rec_c1_05_lifecycle_gates`、`aut_03.rs::aut_03_prepared_commit_pins_fixed_context` | Pass-measured (headless) / 実機録音 NotRun |
| PC-C1-R02 | Composite CAS/stale・失敗時の点列不消費（既存 points/columns 保持） | `pc_c6_oracle_rec.rs::pc_c6_r02_foreign_change_conflict_retains_points`、`pc_c1.rs::rec_c1_02_stale_revision_conflicts_then_explicit_rebase_commits`・`rec_c1_02_undo_during_recording_conflicts`、`aut_03.rs::aut_03_conflict_retains_points_until_explicit_rebase` | Pass-measured (headless) |
| PC-C1-R03 | 構造的失敗（lane/target 除去）の typed rejection で既存データ不変 | `pc_c6_oracle_rec.rs::pc_c6_r03_structural_rejection_retains_columns`、`pc_c1.rs::rec_c1_03_deleted_lane_rejects_with_columns_retained`・`rec_c1_03_deleted_target_rejects` | Pass-measured (headless) |
| PC-C1-R04 | 成功 commit = 1 Undo 境界 exactly-once・abort 無適用・duplicate ACK/retry の冪等 | `pc_c6_oracle_rec.rs::pc_c6_r04_commit_one_undo_exactly_once`、`pc_c1.rs::rec_c1_01_commit_success_one_undo_raw_manual_save_separate`・`rec_c1_04_duplicate_ack_is_exactly_once`・`rec_c1_04_stale_prepared_cannot_hijack_open_transaction` | Pass-measured (headless) |
| PC-C1-R05 | 明示 Rebase/Retry — 自動再束縛なし・rebase→再 preview→commit | `pc_c6_oracle_rec.rs::pc_c6_r05_explicit_rebase_repreview_then_commit`、`pc_c1.rs::rec_c1_02_*`、`uic5_04.rs::uic5_04_foreign_change_conflicts_then_explicit_rebase_pending` | Pass-measured (headless) |
| PC-C1-R06 | LoopWrap seal・playback pass 前進・seal 後 write 拒否・sealed commit は点列適用 | `pc_c6_oracle_rec.rs::pc_c6_r06_loop_wrap_seals_and_pass_advances`、`aut_03.rs::aut_03_wrap_seals_at_exact_loop_end_transport_continues`・`aut_03_later_passes_write_nothing_until_explicit_rearm`・`aut_03_pass_lifecycle`・`aut_03_multi_lane_pass_is_one_undo`・`aut_03_rearm_same_frame_as_wrap_fixes_new_pass` | Pass-measured (headless) |
| PC-C1-R07 | Read/Touch/Latch/Write mode と WriteArm の gating 行列 | `pc_c6_oracle_rec.rs::pc_c6_r07_mode_gating_oracle`、`aut_01.rs` の mode 系（`aut_01_touch_*`・`aut_01_latch_holds_until_boundary`・`aut_01_write_seeds_held_reference_and_stops`・`aut_01_read_override_lives_only_during_gesture`・`aut_01_mode_change_keeps_return_but_closes_holds`）、`pc_a.rs::pc_a_05_recording_gates_undo_seal_fixation` | Pass-measured (headless) |
| PC-C1-R08 | 固定 scope が project drift（対象外 op・Tempo/Clip 変更）後も維持・Note/Clip/Tempo lane で元データ/RT 不変 | `pc_c6_oracle_rec.rs::pc_c6_r08_fixed_scope_survives_project_drift`（`AddTempoEvent`・clip extent drift を含む）、`pc_c1.rs::rec_c1_06_scope_epoch_and_tempo_contexts`、`synvelis-project/tests/ctl_01.rs::the_owner_time_scope_matrix_is_enforced`・`migrated_clip_and_note_targets_evaluate_in_their_local_domains`、`synvelis-workspace/tests/pc_b.rs::pc_b_02_note_scope_records_note_relative`（record transaction 層の note-relative 記録） | Pass-measured (headless)。**Note scope は eval 層（`ctl_01`）+ record transaction 層の単一経路（`pc_b_02_note_scope_records_note_relative`）までの被覆で、R08 の Note/Clip/Tempo×drift 全組合せ fixture は無し**（部分被覆として記録）。実 RT device 不変は NotRun |

## PC-C2-REL01〜REL07 — Control Relation Index

一次 oracle: `crates/synvelis-project/tests/pc_c6_oracle_rel.rs`
（宣言 edge 集合からの独立 BFS 導出 vs `RelationIndex` 観測、
7 tests — `mise run test synvelis-project` で全 pass）。
実装契約側: `pc_c2_relation.rs`（`rel_c2_01`〜`rel_c2_05`）・
workspace `rel_c5.rs`（view 層 11 tests）・`uic5.rs`。

| 行 | 受入内容 | 証拠 pointer | 判定 |
| --- | --- | --- | --- |
| PC-C2-REL01 | Incoming/Outgoing/Both の有界 BFS（coverage/revision/relations 全件・順序）が oracle と一致 | `pc_c6_oracle_rel.rs::rel01_directions_agree_with_oracle`、`pc_c2_relation.rs::rel_c2_01_bidirectional_multi_source_multi_target`、`rel_c5.rs::parameter_rows_surface_direct_relations_verbatim` | Pass-measured (headless) |
| PC-C2-REL02 | LFO/Macro/StepSeq `ModulationSource`・`depth_source` assignment の `DepthControlledBy` 辺が canonical direction で一致 | `pc_c6_oracle_rel.rs::rel02_source_kinds_and_depth_driver`、`pc_c2_relation.rs::rel_c2_02_duplicate_assignments_depth_driver_disabled`、`rel_c5.rs::assignment_and_depth_hops_reach_affected_parameter` | Pass-measured (headless) |
| PC-C2-REL03 | 同一 source→target 複数 assignment は別辺・disabled 由来辺は既定で結果/traversal 双方に出ず `include_disabled` で `Disabled` state | `pc_c6_oracle_rel.rs::rel03_duplicate_assignments_and_disabled`、`rel_c5.rs::include_disabled_gates_disabled_edges` | Pass-measured (headless) |
| PC-C2-REL04 | enabled 辺のみの cycle は `Unresolved`・disabled 辺を含む cycle は cycle でない・dangling は `Unresolved`・`Unsupported` は予約語彙で結果に現れない | `pc_c6_oracle_rel.rs::rel04_cycles_dangling_and_unsupported`、`pc_c2_relation.rs::rel_c2_04_bounds_truncation_and_cycles`、`rel_c5.rs::cycle_edges_surface_unresolved_without_nuking_enabled_ones` | Pass-measured (headless) |
| PC-C2-REL05 | `max_depth`/`max_edges` の有界化・超過は `Truncated` + 決定的 prefix（`max_edges=0` でも到達辺があれば `Truncated`） | `pc_c6_oracle_rel.rs::rel05_bounds_and_truncation`、`pc_c2_relation.rs::rel_c2_04_*`、`rel_c5.rs::edge_budget_surfaces_truncated_coverage` | Pass-measured (headless) |
| PC-C2-REL06 | `expected_revision` 不一致は `Stale`・未知 anchor は `Unavailable`・anchor 実在/関係無しは `Complete`+空・削除/消失は再構築 index に反映・旧世代 query は `Stale` | `pc_c6_oracle_rel.rs::rel06_stale_unavailable_and_revision_delta`、`pc_c2_relation.rs::rel_c2_03_revision_delta_and_unresolved`、`rel_c5.rs::stale_expected_revision_surfaces_stale_coverage`・`unknown_anchor_surfaces_unavailable_coverage` | Pass-measured (headless) |
| PC-C2-REL07 | 数万 node/edge 級 fixture で build+bounded query が oracle と一致・有界（`Instant` 実測、絶対閾値は Baseline 未確定のため保守的 sanity 上限のみ） | `pc_c6_oracle_rel.rs::rel07_large_graph_measurement` | Pass-measured (headless)。**絶対性能閾値は NotRun**（baseline 未確定 — `verification.md` 参照） |

## PC-C3-TR01〜TR08 — Control Trace

一次 oracle（宣言 vs `ControlTrace`/`TraceMetrics` field 照合）:
`crates/synvelis-project/tests/pc_c6_oracle_tr.rs`（8 tests）。
実経路再証拠（`ParamDispatch`+`DispatchTraceAdapter`+
`GraphRuntime` + in-process fake plugin）:
`crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`
（8 tests）。単体契約:
`trace_c3.rs`（project, 6 tests）・`trace_c3_05.rs`（player,
5 tests）・`trace_c3_05_rt.rs`（RT alloc-free, 1 test）。
実 GUI/実 plugin/実 OS の trace 観測は NotRun。

| 行 | 受入内容 | 証拠 pointer | 判定 |
| --- | --- | --- | --- |
| PC-C3-TR01 | Configured/Prepared/Scheduled/Applied/Dispatched/Reported/Observed の 7 stage が独立・`Present` 以外は `Missing`/`Unsupported`/`Dropped`/`Expired` を保持し他 stage から合成しない・receipt は status 一致のみ fail-closed | `pc_c6_oracle_tr.rs::tr01_stage_independence`、`pc_c6_oracle_tr_dispatch.rs::tr01_stage_independence_real_ledger`、`trace_c3.rs::trace_c3_01_stage_slots_missing_stays_unknown`・`trace_c3_01_receipt_generation_mismatch_is_incoherent`、`trace_c3_05.rs::trace_c3_05_stages_fill_from_ledger_events` | Pass-measured (headless) |
| PC-C3-TR02 | Clock/Graph/Epoch/Plan/Revision 各差が対応 `StaleReason` で `Stale`（`Current` にしない）・古い Receipt を現行真値にしない | `pc_c6_oracle_tr.rs::tr02_context_mismatch_dimensions`、`pc_c6_oracle_tr_dispatch.rs::tr02_context_mismatch_dimensions_real_ledger`（7 次元 + composite check order）、`trace_c3.rs::trace_c3_02_context_mismatch_is_stale` | Pass-measured (headless) |
| PC-C3-TR03 | seek/loop/restart/pause/position 移動の世代差で旧 key の trace は `Stale` 分類・`Current` にならない | `pc_c6_oracle_tr.rs::tr03_transport_seek_loop_restart`、`pc_c6_oracle_tr_dispatch.rs::tr03_context_change_stale_real_ledger`（実 re-key 含む） | Pass-measured (headless) |
| PC-C3-TR04 | Plugin Lost/Unknown・Measured Silence と Unobserved（Missing/Disabled/NoEvidence）を別状態・dispatch 終端は観測 channel を含意しない・2 channel 独立分類 | `pc_c6_oracle_tr.rs::tr04_plugin_unknown_vs_probe_states`・`tr06_silence_vs_unobserved`、`pc_c6_oracle_tr_dispatch.rs::tr04_plugin_unavailable_vs_probe_states`、`trace_c3.rs::trace_c3_03_probe_channel_states`、`trace_c3_05.rs::trace_c3_05_reported_and_unavailable_stages` | Pass-measured (headless) |
| PC-C3-TR05 | stamp provenance `Estimated` は `Current` にしない・`now > stamp+max_age` は `Stale{Age}`・`now < stamp` は `Unavailable{ClockUntrusted}` | `pc_c6_oracle_tr.rs::tr05_stamp_provenance_and_age_matrix`、`pc_c6_oracle_tr_dispatch.rs::tr05_plugin_reported_stamp_provenance`（drain-time Estimated stamp の field 照合）、`trace_c3.rs::trace_c3_02_stamp_quality_classification` | Pass-measured (headless) |
| PC-C3-TR06 | `MeasuredValue::Silence` は `Measuring` の実測値で `Current` を返し得る — probe 欠落/disabled（Unobserved）とは別状態・target 間漏洩なし | `pc_c6_oracle_tr.rs::tr06_silence_vs_unobserved`、`pc_c6_oracle_tr_dispatch.rs::tr06_silence_vs_unobserved_real_ledger` | Pass-measured (headless) |
| PC-C3-TR07 | 同一 probe 証拠を持つ 2 trace が互いの freshness/coherence を汚染しない（2 inspector 共有 probe の headless 相当） | `pc_c6_oracle_tr.rs::tr07_shared_probe_two_contexts`、`pc_c6_oracle_tr_dispatch.rs::tr07_shared_probe_two_contexts_real_ledger` | Pass-measured (headless)。**実 2 Inspector/2 window の二重起動防止は workspace/UI 責務（#102/#114）— external owner** |
| PC-C3-TR08 | `ControlTrace` は履歴を持たない固定サイズ投影・bounded retention（events/history_lost/targets_lost/expired の会計）・RT alloc/block | `pc_c6_oracle_tr.rs::tr08_determinism_and_bounded_size`、`pc_c6_oracle_tr_dispatch.rs::tr08_bounded_retention_metrics_real_ledger`、`trace_c3_05.rs::trace_c3_05_bounded_history_and_expired`、`trace_c3_05_rt.rs::rt_trace_adapter_hot_path_alloc_free` | Pass-measured (headless)。実 RT device 計測は NotRun |

## PC-C4-PL01〜PL08 — Plugin Binding/Dispatch

一次 oracle: `crates/synvelis-player/tests/pc_c6_oracle_pl.rs`
（8 tests — fake hosted plugin + `PluginBindingSession` +
`ParamDispatch` + delivery log の field 照合）と
`pc_c6_oracle_pl_rt.rs`（`rt_pl05_param_delivery_alloc_free`、
`stats_alloc` 独立 binary）。実装契約側:
`pc_c4_binding.rs`（8 tests）・`pc_c4_binding_rt.rs`・
`pc_c4_03_observation.rs`（8）・`pc_c4_05_persisted_instance.rs`・
`pc_c4_07_capability_rebind.rs`・`pc_c4_07_transport_observe.rs`（10）・
`tim03_process_boundary.rs`/`tim03_request_plan.rs`（min_frames
partitioning）・`synvelis-player/tests/tim04_plugin_state.rs`（9）・
`synvelis-project/tests/plugin_state.rs`・
`synvelis-player/tests/pc_b.rs`（8）・`tim02_midi_*.rs`。
**実 VST3/CLAP capability・実 plugin GUI・実 RT device 性能は
NotRun — Pass にしない**（PL08 boundary marker が明示）。

注意: `pc_c6_oracle_pl.rs` 内の行コメント PL03/PL04/PL06 の
番号は test-spec Rule(PL03/04/06) の semantic 対応と一部
入れ替わっている（コメント PL03=QueueFull/batch は Rule(PL04)、
コメント PL04=GraphSwap/Reload は Rule(PL06)、コメント
PL06=plan staleness は Rule(PL03)）。本 ledger は **test-spec
（=Issue 受入条件）の semantic で写像**し、差異をここに記録する
（fixture 内容自体は全行を被覆）。

| 行 | 受入内容 | 証拠 pointer | 判定 |
| --- | --- | --- | --- |
| PC-C4-PL01 | attach 時 `frame_base` verify/pin + plugin identity・`StablePluginInstanceId`・node・capability generation・graph generation・param id の pin | `pc_c6_oracle_pl.rs::pc_c6_pl01_generation_and_identity_pins_reject_stale_records`、`pc_c4_binding.rs::pc_c4_05_verified_frame_base_contract`・`pc_c4_02_stage1_authority_and_plugin_identity`・`pc_c4_02_instance_authority_bind_gate` | Pass-measured (headless/fake plugin) / 実 plugin capability NotRun |
| PC-C4-PL02 | shared parameter・stale record は typed `DispatchError` で reject・param_id と pin の一致（ReadOnly/Stepped/PerNote means は capability 層） | `pc_c6_oracle_pl.rs::pc_c6_pl02_param_identity_and_capability_contract`、`pc_c4_binding.rs::pc_c4_02_typed_rejects_no_misdelivery`、`pc_c4_03_observation.rs`（generation gate・unbound 報告の排除）、`pc_a.rs::pc_a_07_capability_honesty` | Pass-measured (headless) |
| PC-C4-PL03 | plan staleness は orchestration boundary で検査・stale plan 由来 dispatch は fail closed | `pc_c6_oracle_pl.rs::pc_c6_pl06_plan_staleness_and_generation_axes`（test 内コメントは PL06 だが semantic は Rule(PL03)） | Pass-measured (headless) |
| PC-C4-PL04 | dispatch queue 有界・満杯は `DispatchError::QueueFull` 型付き観測・batch dispatch は atomic（1 stale で全体 reject・staging なし） | `pc_c6_oracle_pl.rs::pc_c6_pl03_queue_full_and_atomic_batch_rejection`（test 内コメントは PL03 だが semantic は Rule(PL04)） | Pass-measured (headless) |
| PC-C4-PL05 | delivery は min process frame offset 遵守・warm hot path allocation-free（`stats_alloc` 実測）・partitioned block の物理 min_frames 合法性 | `pc_c6_oracle_pl.rs::pc_c6_pl05_min_process_frame_offset_delivery`、`pc_c6_oracle_pl_rt.rs::rt_pl05_param_delivery_alloc_free`、`pc_c4_binding_rt.rs::rt_pc_c4_session_bound_dispatch_alloc_free`、`tim03_process_boundary.rs::tim03_pb_min_max_illegal_segment_policy`・`tim03_sched_exact_rejects_illegal_apply`・`tim03_req_min_max_process_length_fail_closed`・`tim03_req_simultaneous_loop_seek_tempo_param` | Pass-measured (headless)。実 plugin/実 RT device 計測 NotRun |
| PC-C4-PL06 | capability generation・graph generation drift・plugin reload（新 instance mint）で旧 binding は fail closed・旧 record は死ぬ | `pc_c6_oracle_pl.rs::pc_c6_pl04_reload_keeps_instance_old_records_dead`（semantic は Rule(PL06)）、`pc_c4_07_capability_rebind.rs`（rebind boundary drift・params drift junction）、`pc_c4_binding.rs::pc_c4_04_generation_consistency`、`pc_c4_07_transport_observe.rs::pc_c4_07_09_generation_swap_mints_fresh_mirror` | Pass-measured (headless/fake plugin) |
| PC-C4-PL07 | wrong plugin instance/node/generation/param id/capability/frame base は typed rejection・旧 Target 誤配送 0（delivery log 照合）・offline/playback absolute frame 一致 | `pc_c6_oracle_pl.rs::pc_c6_pl07_shared_param_and_stale_record_zero_misdelivery`、`pc_c4_binding.rs::pc_c4_02_typed_rejects_no_misdelivery`・`pc_c4_01_eval_dispatch_receipt_rt_offline_parity`、`synvelis-player/tests/pc_b.rs::pc_b_04_offline_render_matches_playback`・`pc_b_05_*`（seek/midrun activation/dispatch drain） | Pass-measured (headless)。旧 RawID 再利用の実 plugin 誤配送は NotRun |
| PC-C4-PL08 | headless boundary marker — 実 VST3/CLAP capability・実 plugin GUI・実 RT device 性能は未測定を Pass にしない | `pc_c6_oracle_pl.rs::pc_c6_pl08_headless_fixture_boundary_marker` | Pass-measured (marker)。実 plugin 系は **NotRun**（external-gates.md） |

## PC-C5-UI01〜UI07 — Control Surface（Headless Oracle）

一次 oracle: `crates/synvelis-workspace/tests/pc_c6_oracle_ui.rs`
（`FakeWindowHost` + `WorkspaceManager`、宣言 edge 集合 vs
`RelationView`/gesture/undo の field 照合、7 tests）。
実装契約側: `rel_c5.rs`（11 tests）・`uic5.rs`（14）・
`uic5_04.rs`（10）・`sv47_controls.rs`（25）・`sf_05.rs`/
`sf_05_s2.rs`（Signal Flow view 層）。実 GPUI/OS focus/IME/
plugin GUI は **NotRun — #114 gate**。

| 行 | 受入内容 | 証拠 pointer | 判定 |
| --- | --- | --- | --- |
| PC-C5-UI01 | Project/Parameter 唯一正本 — Automation/Inspector/Signal Flow を同一 Case で照合・view 側に正本複製なし | `pc_c6_oracle_ui.rs::pc_c6_ui01_relation_view_matches_declared_project_truth`、`rel_c5.rs::projection_is_read_only_and_deterministic`、`sf_05.rs::overview_projects_every_strip_and_every_connection_exactly_once`・`graph_queries_and_visual_ops_leave_the_session_untouched`、`aut_04.rs::automation_view_is_read_only_and_never_mutates_source` | Pass-measured (headless)。3 view を単一 fixture で同時照合する専用 test は無く、各 view が declared project truth と照合される複合証拠（部分被覆として記録） |
| PC-C5-UI02 | Parameter→Source→other Target の双方向探索・path 保持・`pop` が直前 anchor を復元 | `pc_c6_oracle_ui.rs::pc_c6_ui02_nav_roundtrip_both_directions`、`rel_c5.rs::nav_stack_roundtrips_back_to_origin_anchor`・`parameter_reaches_lane_source_and_assignment_by_hops`・`assignment_and_depth_hops_reach_affected_parameter`、`uic5.rs::uic5_03_explore_history_back_forward` | Pass-measured (headless) |
| PC-C5-UI03 | `expected_revision` pin の旧 revision query は `Stale`+空 rows・構築済み view は `is_stale` で検出 — 旧 receipt を現行真値にしない | `pc_c6_oracle_ui.rs::pc_c6_ui03_stale_revision_never_current`、`rel_c5.rs::stale_expected_revision_surfaces_stale_coverage`、`uic5.rs::uic5_02_commit_drops_on_stale_revision`、`sf_05.rs::expected_revision_mismatch_is_stale_with_empty_graph` | Pass-measured (headless) |
| PC-C5-UI04 | 2 Window stale/Focus/Graph Change — window A begin の gesture は window B 移動後 `Stale` drop（retarget しない・project/revision 不変）・cancel は undo 境界を残さない | `pc_c6_oracle_ui.rs::pc_c6_ui04_two_window_epoch_and_cancel`、`uic5.rs::uic5_02_commit_drops_on_window_epoch_change`・`uic5_02_focus_selection_change_never_retargets`・`uic5_03_two_window_dock_focus_no_retarget`、`sv47_controls.rs::second_window_begin_is_rejected_and_previews_never_misroute`・`dock_move_cancels_gesture_and_stale_generation_cannot_land`・`detach_cancels_write_gesture_without_committing`、`sf_05_s2.rs::routing_gesture_window_transfer_drops_stale` | Pass-measured (headless)。実 GPUI 2 window/focus は NotRun（#114） |
| PC-C5-UI05 | Begin/Preview/Commit/Cancel lifecycle — preview は in-memory のみで project/revision 不変・commit は assignment 1 件 + `Applied{revision}`・undo 1 回で除去（成功 1 Undo） | `pc_c6_oracle_ui.rs::pc_c6_ui05_gesture_lifecycle_one_undo`、`uic5.rs::uic5_02_edge_begin_preview_commit_undo`・`uic5_02_preview_and_cancel_leave_project_untouched`・`uic5_02_edge_begin_validates_endpoints_and_exclusivity`、`sv47_controls.rs::drag_commits_base_edit_and_all_windows_observe`・`cancel_discards_preview_and_rewrites_nothing`・`touch_gesture_writes_to_transaction_and_commits_as_one_undo_step` | Pass-measured (headless) |
| PC-C5-UI06 | Probe ownership boundary — headless surface に probe/trace 状態が漏れない（実 probe 証拠は TR gate の責務） | `pc_c6_oracle_ui.rs::pc_c6_ui06_probe_ownership_boundary_marker` | Pass-measured (marker のみ — boundary marker test が headless 範囲を明示) |
| PC-C5-UI07 | real GUI boundary — `FakeWindowHost` の headless window/dock 意味論（owner 解決）のみを証明・GPUI/OS focus/IME/plugin GUI は NotRun | `pc_c6_oracle_ui.rs::pc_c6_ui07_headless_window_host_boundary_marker` | Pass-measured (marker のみ)。実 GUI は **NotRun**（#114・GUI-L2） |

## PC-C6-COMP01〜COMP05 — Cross-gate 判定記録

Gate 判定行。headless 実施可能部分のみ Pass-measured とし、
製品 Gate 判定・両 Consumer・実機は NotRun/external owner。
Gate B/C 未受入でも Gate A は Signal Flow/AI/VCS に依存せず
独立に成立させる構成であり、録音/実 Device の Incomplete は
隠さない（`external-gates.md` 参照）。

| 行 | 受入内容 | 証拠 pointer | 判定 |
| --- | --- | --- | --- |
| PC-C6-COMP01 | Gate A — AI Off の通常 DAW（作成→編集→Undo/Save→復旧）が Signal Flow/AI/VCS 非依存で成立 | `crates/synvelis-gateway/tests/gw10_system.rs::e2e1_ai_off_human_pipeline_commits_undoes_and_recovers`（headless human pipeline）、`docs/designs/ai-native-baseline-design.md`（AI-GATE 独立第四軸の記録）、本 slice の `mise run test synvelis-{project,player,workspace}` 全 green（headless DAW 経路の非回帰） | headless 構成証拠 Pass-measured。**製品 Gate A 判定は synvelis#23 — NotRun（external owner）** |
| PC-C6-COMP02 | Gate B — 構成（Configured）/実観測（Observed）/推定（Estimated）分離 | `pc_c6_oracle_tr.rs::tr01_stage_independence`・`tr05_stamp_provenance_and_age_matrix`、`pc_c6_oracle_tr_dispatch.rs::tr01_stage_independence_real_ledger`・`tr05_plugin_reported_stamp_provenance`、`pc_a.rs::pc_a_04_breakdown_and_observation_states`、`pc_c4_03_observation.rs` | Pass-measured (headless)（構成/観測/推定の分離は oracle で検証済み）。製品 Gate B 判定は #23 — NotRun |
| PC-C6-COMP03 | Gate C — Control Source→Target と Target→Source の双方向探索・Audio Routing Edge との型分離 | `pc_c6_oracle_rel.rs::rel01_directions_agree_with_oracle`（Both direction）、`pc_c6_oracle_ui.rs::pc_c6_ui02_nav_roundtrip_both_directions`、`uic5.rs::uic5_02_edge_and_routing_gestures_are_distinct`、`rel_c5.rs` hop 系 | Pass-measured (headless)。実 GUI 双方向探索は #114 — NotRun |
| PC-C6-COMP04 | 両 Consumer — Ruviform two-consumer lockfile/RT/noalloc/f64 precision を候補 SHA ごとに独立判定 | `Cargo.lock` resolved Ruviform SHA `fd1d4d5f…`（単一 pin — `tools/check-ruviform-rev.mjs` が lock/vendor/manifest 整合を検証）、Synvelis 側 `*_rt.rs` alloc-free tests（`rt_pl05`・`rt_trace_adapter`・`rt_pc_c4`・`rt_midi`・`rt_process_boundary`・`rt_control_eval`・`aut_04_rt`） | Synvelis 側 lockfile 単一性 + headless RT noalloc は Pass-measured。**両 Consumer 精度/性能/CLI 非回帰の独立判定は ruviform#219/#294 + synvelis#28 — external owner / NotRun** |
| PC-C6-COMP05 | 録音/RT/Export/Plugin の非回帰（候補 SHA ごと） | 録音: `pc_c1.rs`・`pc_c6_oracle_rec.rs`・`aut_03/04.rs`。RT: `*_rt.rs` 各 binary + `mise run test` 内実行。Export/Graph/Transport: `l1_e2e.rs`・`sf03_*.rs`・`tim03_*.rs`（player）、`sf03_render.rs`・`l2_e2e.rs`・`x_e2e.rs`（render — 参照のみ・本 slice 未編集）。Plugin: `synvelis-player/tests/pc_b.rs`・`pc_c4_*.rs`・`synvelis-player/tests/tim04_plugin_state.rs` | headless 非回帰は Pass-measured（本 slice `mise run test` で対象 3 crate green）。実 device/plugin export・長時間回帰は NotRun |

---

## PC-X1〜X6 — 固定 Matrix 由来行（2026-11-04 追記）

Issue #103 本文「2026-11-04 4領域合意の Control/Recording/Time/GUI
Test Oracle 追加」節の固定 Matrix 行。domain は主に
`synvelis-workspace`（control/recording）・`synvelis-project`
（scope/store/plugin state）・`synvelis-player`（dispatch/
process boundary）に跨る。headless 被覆は先行 slice
（PC-A #40・PC-B #43・SV-47 #47・PC-C1〜C5・aut_01〜04・
ctl_01/02・tim03/04 系）で merge 済み。

| 行 | 受入内容 | 証拠 pointer | 判定 |
| --- | --- | --- | --- |
| PC-X1 | ControlOwner/AutomationTimeScope から Send Gain/Fader/Host/Plugin Parameter の単一 Value 正本・Legacy Scope migration・1 CAS+Undo・重複 Reject を型/保存で試験 | `pc_a.rs::pc_a_01_single_source_three_projections`・`pc_a_02_scope_separation_clip_follows_move`・`pc_a_03_conflict_reject_and_replace`、`controller_store.rs`（stale revision atomic reject・undo/redo revision・save/reload identity・schema reject）、`synvelis-project/tests/ctl_01.rs::owner_serde_roundtrips_and_accepts_the_legacy_scope_tag`・`v6_snapshot_migrates_owner_role_and_time_scope_deterministically`・`the_owner_time_scope_matrix_is_enforced`、`alib02_v5_real.rs::real_v5_snapshot_decodes_migrates_and_reloads`、`sv47_controls.rs::duplicate_command_id_is_exactly_once`、`synvelis-project/tests/ctl_01.rs::singleton_roles_are_unique_per_owner` | Pass-measured (headless)。GUI 側の同一 Case 照合は #114 — NotRun |
| PC-X2 | Touch 150ms Smoothstep（暫定・PoC 前）・Mode Read/Touch/Latch/Write × Transport/Seek/Loop/Focus・Return 時 Project Curve 非破壊・Modulation 前 Reference 比較 | `aut_01.rs::aut_01_touch_writes_gesture_then_returns_smoothstep`（75ms 中点 `w=0.5`・150ms 完了を実測）・`aut_01_return_custom_duration_and_clock_contract`・`aut_01_return_zero_duration_completes_instantly`・`aut_01_touch_return_is_nondestructive`・`aut_01_retouch_during_return_seeds_blended_reference`・`aut_01_return_follows_curve_per_position`・`aut_01_latch_holds_until_boundary`・`aut_01_write_seeds_held_reference_and_stops`・`aut_01_read_override_lives_only_during_gesture`・`aut_01_boundaries_close_runtime_state`・`aut_01_pause_resume_vs_new_play_boundary`、`aut_03.rs`（loop seal 系）、`sv47_controls.rs::transport_actions_report_applied_vs_noop`・`seek_preview_never_moves_the_playhead`・`uic5.rs::uic5_02_focus_selection_change_never_retargets`、`pc_a.rs::pc_a_04_breakdown_and_observation_states`（reference/breakdown） | Pass-measured (headless)。150ms は暫定値（`DEFAULT_TOUCH_RETURN_MS`）— 実 plugin/実機 touch 応答は NotRun。mode×transport/seek/loop/focus の全組合せは次元ごとの被覆（完全な直積 fixture は無し — 部分被覆として記録） |
| PC-X3 | Step/Linear Region Replace の外区間値保持・同時刻 Boundary・Undo・GUI Range 編集と Curve Golden・`cut_range_points` 既存契約の回帰 | `aut_02.rs` 15 tests（`aut_02_step_mid_range_replace_preserves_outside`・`aut_02_linear_mid_range_replace_preserves_outside`・`aut_02_point_at_boundaries_owns_anchor_position`・`aut_02_replace_exact_region_extent_sentinel_region_created`・`aut_02_one_undo_restores_complete_pre_state`・`aut_02_reject_and_stale_revision_leave_everything_untouched`・`aut_02_rt_eval_matches_offline_after_replace`・`aut_02_exact_rational_beat_anchors` 等 — file header が `cut_range_points` op apply 経路を明記）、`aut_04.rs::automation_view_*`（curve envelope/step edge/extremum golden） | Pass-measured (headless)。GUI Range 編集は #114 — NotRun |
| PC-X4 | Loop Automation 1Pass 終了・次周自動 Write なし・PC-C1 Prepare/Apply/Finalize 失敗時点列保全・再 Arm/2 Window/Plugin Parameter Trace | `aut_03.rs::aut_03_later_passes_write_nothing_until_explicit_rearm`・`aut_03_pass_lifecycle`・`aut_03_rearm_same_frame_as_wrap_fixes_new_pass`・`aut_03_multi_lane_pass_is_one_undo`・`aut_03_first_seal_wins`・`aut_03_wrap_flushes_in_flight_gesture`、`pc_c6_oracle_rec.rs::pc_c6_r02/r03`（失敗時点列保全）・`pc_c1.rs::rec_c1_03_*`、`uic5_04.rs`・`sv47_controls.rs`（2 window）、`pc_c6_oracle_tr_dispatch.rs`（plugin parameter の reported/observed trace） | Pass-measured (headless) |
| PC-X5 | Recorded Points の source-native 保持・Viewport LOD のみ縮約・Queue Gap の品質・Persistent Data Fidelity/Performance | `aut_04.rs::recorded_points_keep_exact_rational_positions_and_values`・`automation_view_exact_envelope_and_uncovered_nan`・`automation_view_step_edge_and_extremum_features`・`automation_view_reduced_quality_keeps_features`・`automation_view_truncated_quality_when_features_dropped`・`drain_reports_point_budget_overflow_and_gap`・`drain_is_bounded_and_reports_partial`・`queue_full_is_typed_and_folds_into_gap`、`aut_03.rs::aut_03_raw_values_saved_separately_survive_reload`、`aut_04_rt.rs::record_event_rt_alloc_free_bounded_and_typed`、`pc_c1.rs::rec_c1_01_commit_success_one_undo_raw_manual_save_separate` | Pass-measured (headless)。**絶対性能閾値は NotRun**（baseline 未確定）。実 device 録音 fidelity は NotRun |
| PC-X6 | Plugin minframes/Verified Framebase/Clock/Loop/Offline exact・Plugin State Coordinated/Quiescent/BestEffort/Unsupported・旧 Plugin RawID 再利用で誤配送 0 | `tim03_process_boundary.rs::tim03_pb_min_max_illegal_segment_policy`・`tim03_sched_*`・`tim03_node_*`・`tim03_req_*`（min_frames/max_frames partition legality）、`pc_c4_binding.rs::pc_c4_05_verified_frame_base_contract`、`pc_c4_07_transport_observe.rs`（loop region・seek・pause・generation swap の transport mirror）、`synvelis-player/tests/pc_b.rs::pc_b_04_offline_render_matches_playback`・`pc_b_05_*`、`synvelis-player/tests/tim04_plugin_state.rs::capture_while_processing_is_best_effort`・`capture_after_stop_processing_is_coordinated`・`capture_with_quiescent_mode_records_quiescent`・`unsupported_plugin_reports_unsupported`、`synvelis-project/tests/plugin_state.rs::standard_save_records_all_quality_classes_and_publishes`・`strict_save_rejects_degraded_and_publishes_nothing`、`pc_c6_oracle_pl.rs::pc_c6_pl07_shared_param_and_stale_record_zero_misdelivery`・`pc_c6_pl04_reload_keeps_instance_old_records_dead` | Pass-measured (headless/fake plugin)。実 plugin の minframes/capability/state・旧 RawID 実機誤配送は **NotRun** |

---

## GUI-PC-01〜04 — Shared GUI Scenario IDs（Scenario Owner: synvelis#114）

Issue #103 はこれらを **Shared GUI Scenario IDs** として定義し、
Scenario Owner を #114（実 GPUI GUI 製品 Scenario）に置く。
本 ledger は **独立 Oracle 側の証拠対応のみ**を記録し、実 GUI
Pass は主張しない。#103 は共通 GUI Runner/Plugin Host/Project
正本を再実装しない。

| 行 | Scenario 内容（Issue 転記） | 独立 Oracle 側の証拠対応（headless） | 判定 |
| --- | --- | --- | --- |
| GUI-PC-01 | Fader/recording gesture → 1 Controller Commit/Undo・Stale 失敗時点列保持 | `pc_c6_oracle_rec.rs::pc_c6_r04_commit_one_undo_exactly_once`・`pc_c6_r02/r05`（stale conflict で点列保持・明示 rebase）、`sv47_controls.rs::drag_commits_base_edit_and_all_windows_observe`・`touch_gesture_writes_to_transaction_and_commits_as_one_undo_step`、`uic5.rs::uic5_02_edge_begin_preview_commit_undo` | headless Oracle Pass-measured。**実 GUI Scenario は external owner synvelis#114 — NotRun** |
| GUI-PC-02 | Plugin param change 中の Graph swap/Capability change → 誤 Target 配送なし | `pc_c6_oracle_pl.rs::pc_c6_pl04_reload_keeps_instance_old_records_dead`（generation drift/reload fail closed）・`pc_c6_pl07_shared_param_and_stale_record_zero_misdelivery`（誤配送 0）、`pc_c4_07_capability_rebind.rs`、`pc_c4_binding.rs::pc_c4_04_generation_consistency` | headless Oracle Pass-measured。実 GUI/plugin scenario は #114/#176 — NotRun |
| GUI-PC-03 | Control Source→Target と Target→Source の双方向探索・Audio Routing Edge との型分離 | `pc_c6_oracle_ui.rs::pc_c6_ui01_relation_view_matches_declared_project_truth`・`pc_c6_ui02_nav_roundtrip_both_directions`、`pc_c6_oracle_rel.rs::rel01_directions_agree_with_oracle`、`uic5.rs::uic5_02_edge_and_routing_gestures_are_distinct`・`uic5_control_relation_inspect_explore_back_and_modes` | headless Oracle Pass-measured。実 GUI scenario は #114 — NotRun |
| GUI-PC-04 | 2Window/Focus/late ACK/Plugin GUI loss → Project/Control 正本不変 | `pc_c6_oracle_ui.rs::pc_c6_ui03_stale_revision_never_current`・`pc_c6_ui04_two_window_epoch_and_cancel`、`sv47_controls.rs::second_window_begin_is_rejected_and_previews_never_misroute`・`detach_clears_inflight_state_so_reminted_ids_are_fresh`、`pc_c6_oracle_tr_dispatch.rs::tr04_plugin_unavailable_vs_probe_states`（plugin lost/unavailable の trace 分類） | headless Oracle Pass-measured。実 GUI/plugin GUI loss scenario は #114/#176 — NotRun |

---

## fixture 補完判定（Headless 欠落 Fixture 補完の結論）

監査の結果、**本 slice で新規 test fixture の追加は不要**と判定した。

- PC-C1-R/PC-C5-UI/PC-X 系の domain（workspace control・
  recording・view）は `synvelis-workspace/tests/` の先行
  slice fixture（`pc_c6_oracle_rec/ui`・`pc_c1`・`aut_01〜04`・
  `pc_a`・`uic5*`・`rel_c5`・`sv47_controls`・`sf_05*`）が
  既に被覆しており、かつ当該 crate は本 slice の編集可能範囲
  外。
- PC-C2-REL/PC-C3-TR/PC-C4-PL の domain（project/player）は
  `pc_c6_oracle_{rel,tr}`・`pc_c6_oracle_tr_dispatch`・
  `pc_c6_oracle_pl(_rt)`・`trace_c3*`・`pc_c2_relation`・
  `pc_c4_*`・`tim03_*`・`tim04_plugin_state`・`plugin_state`・
  `pc_b*` が既に被覆。
- PC-X1〜X6・GUI-PC-01〜04 の headless 実施可能部分も上記
  fixture で被覆済み（GUI-PC は Scenario Owner #114 のため
  Oracle 側対応のみ）。
- 残る未被覆は全て「実 OS/実 GUI/実 plugin/実 device/実 RT/
  両 consumer/絶対性能閾値」を要する部分であり、headless
  fixture では原理的に埋められない — `external-gates.md` の
  NotRun/external owner として記録するのが正しい処理。

したがって `crates/synvelis-project/tests/`・
`crates/synvelis-player/tests/` への新規 test 追加は行わない
（「fixture が既存行を既に被覆している場合は新規 test を
追加しない」の条件に合致）。

## 集計

| 分類 | 行数 |
| --- | --- |
| Pass-measured (headless) — headless 実施可能部分が fixture 被覆 + 本 slice `mise run test` green | 42 行（PC-C1-R01〜R07・PC-C2-REL01〜REL06・PC-C3-TR01〜TR06/TR08・PC-C4-PL01〜PL08・PC-C5-UI02〜UI07・PC-X1/X3/X4/X6・GUI-PC-01〜04 Oracle 側） |
| 部分被覆（◐ — headless 部分は被覆するが、行内に残件を明記: 実機依存部・組合せ fixture 欠落・性能閾値・Gate 判定は Owner 別） | 11 行（PC-C1-R08・PC-C2-REL07・PC-C3-TR07・PC-C5-UI01・PC-C6-COMP01〜COMP05・PC-X2・PC-X5） |
| NotRun（実機/実 GUI/実 plugin/実 device/実 RT/絶対性能閾値を要する部分） | 全 53 行の実機依存部分（行内備考列・`external-gates.md` に分離記録） |
| external owner | 実 GUI Scenario=#114、実 Plugin Dock=#176、製品 Gate=#23、Ruviform 追従=#28、Bridge Clock=ruviform#307、両 Consumer=ruviform#219/#294 |
| Unsupported | 0 行（捏造しない） |

注: 行分類は `coverage.md` の ◐ 判定と一致する。COMP 行は
headless の構成/観測/推定分離・双方向探索・非回帰証拠を持つが
製品 Gate 判定が #23 Owner のため全行を部分被覆側に計上した
（保守的分類）。GUI-PC 行は Scenario Owner が #114 のため
「headless Oracle 側証拠」として Pass-measured に計上し、実 GUI
Scenario は NotRun/external owner として分離した。

## 判定ルール

- `Pass-measured (headless)` は「当該 SHA で `mise run test
  <package>` が green だった」ことのみを意味し、case 全体の
  受入 Pass・実機 Pass ではない。
- headless の成功を GUI/device/plugin/consumer の Pass に
  昇格しない。`NotRun`/`Deferred`/`Unsupported` を推測で
  Pass にしない（`POLICIES.md` §3）。
- 行の証拠 pointer は `file::test` 形式。fixture 本体の
  Expected/Observed 詳細は各 test と先行 slice の
  `docs/impl/ir/pc-c6-*.md` を正本とする。
