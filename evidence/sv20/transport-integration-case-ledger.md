# SV20 Transport Integration — TT-001〜030 Case Ledger

Issue #20 の TT-001〜030 各受入ケースについて、実装 Owner・実行
手段/Fixture・独立 oracle・Revision・結果状態
（Pass/Fail/NotRun/Unsupported）・capability gate（Exact/
Degraded/Unsupported/Unknown と実機/OS/GUI の未実施範囲）を
記録する正本。headless/mock 証拠を実機受入に繰り上げない。

- 対象 Issue: scarecrow173/synvelis#20
- 文書鎖: `docs/specs/transport-integration.md`・
  `docs/designs/transport-integration-design.md`・
  `docs/plans/transport-integration.md`・
  `docs/tasks/transport-integration-task.md`・
  `docs/test-specs/transport-integration.md`・
  `docs/impl/ir/transport-integration.md`・
  `docs/impl/exp/transport-integration.md`
- Synvelis revision: `58538fb`（worktree head、docs slice）
- Ruviform pinned revision: `fd1d4d5`（`ruviform-media-core`/
  `plugin-core`/`plugin-host`）
- 結果状態語彙: `Pass` / `Pass(部分)`（headless で関連契約は
  確認できるが TT 全体の独立 oracle を満たさない）/ `Fail` /
  `NotRun` / `Unsupported`。能力 Class（Exact/FrameAccurate/
  BlockOnly/DegradedWithConsent/Unsupported/Unknown）とは別語彙。

凡例: fixture 名は代表例（`*` は glob）。「headless」は
mock/fake/計装による検証であり、実 plugin・実 OS・実 device・
実 GUI を意味しない。

| TT | Owner | Fixture / test（代表） | 独立 oracle | Req | headless 結果 | revision/証拠 | capability gate（未実施範囲） |
| --- | --- | --- | --- | --- | --- | --- | --- |
| TT-001 | #125 | `crates/synvelis-project/tests/tim_01_ppqn*.rs`（ppqn sweep/reproject/boundary）・`tim_02_ppqn_smf.rs`・`midi_source.rs` | `Ppqn`/`BeatPosition`/`TickPosition::rescale`（ruviform-media-core） | Required | Pass(部分) | synvelis `58538fb`・ruviform `fd1d4d5` | 実 project UI=Deferred（#114）・実 plugin=NotRun |
| TT-002 | #125+#19 | `tim_01_ppqn*.rs` inexact reject/rt rounding・`aut_02_exact_rational_beat_anchors` | `BeatPosition` num/den rational・`beat_from_wide` TimeOverflow | Required | Pass(部分) | 同上 | — |
| TT-003 | #126 | `crates/synvelis-project/tests/tim_02_smf.rs`・`tim_02_ppqn_smf.rs`・`crates/synvelis-player/tests/tim02_midi*.rs`・`midi_source.rs` native_divisions | synvelis-project `smf.rs` parse・ruviform#188 timed MIDI | Required | Pass(部分) | 同上 | SMPTE Division/Format2=Unsupported(#129)・実 MIDI device=NotRun(ruviform#253) |
| TT-004 | #127+project | `tim03_req_simultaneous_loop_seek_tempo_param`・`md_a` tempo edit・`monitor_engine` punch_gate_across_tempo_boundary | `TempoMapRevision` generation（ruviform-media-core） | Required | Pass(部分) | 同上 | 実 plugin tempo=NotRun |
| TT-005 | #12+#127 | `negative_preroll_maps_to_positive_session`（recording）・l2-master design | `MediaTime`/`MediaRange` checked ops | Required | Pass(部分) | 同上 | — |
| TT-006 | #127+#190 | `tim03_pb_exact_windows_and_boundary_positions`・`tim03_node_*`・`tim03_sched_*` | `tt025_loop_wrap_splits_and_transports`（ruviform） | Required | Pass(headless) | 同上 | 実 plugin=NotRun |
| TT-007 | #126/#127 | `tim02_loop_replay_releases_and_refires`・`tim03_node_param_event_per_loop_occurrence` | `tt029_loop_occurrence_distinguishes_deliveries`・`loop_occurrence_refires_event_once_per_lap`（ruviform） | Required | Pass(部分) | 同上 | 実 plugin=NotRun |
| TT-008 | #127+#190 | `tim02_seek_releases_and_chases`・`pc_c4_07_transport_observe` | `tt008_command_receipt_lifecycle`・`AppliedReceipt`（ruviform） | Required | Pass(部分) | 同上 | 実 device seek=NotRun |
| TT-009 | ruviform#187+probe | `sf04_s3_*`/`sf04_s9_*` epoch tests・`pc_c4_07_capability_rebind` | `tt009_restart_vs_seek_epoch_generation`・`device_restart_rebases_epoch_not_frame_base`（ruviform） | Required | Pass(部分) | 同上 | 実 device restart=NotRun |
| TT-010 | #126 | `tim02_same_tick_off_precedes_ons`・`tim_02_smf` ordering | `tt010_same_frame_source_order`（ruviform） | Required | Pass | 同上 | — |
| TT-011 | #127+#196 | `tim03_node_stale_transport_generation_zero_delivery`・`tim03_prov_stale_generations_zero_delivery`・`tim03_prov_foreign_dispatch_refused` | `stale_plan_generation_event_not_delivered`（ruviform） | Required | Pass(headless) | 同上 | 実 plugin replace=NotRun |
| TT-012 | ruviform#190+#127 | `tim03_sw_07_driver_queue_full_typed`・NoteLedger release_all | `tt012_span_budget_overflow_faults`・`EventBudgetExceeded`/`SpanBudgetExceeded`/`FaultLedgerOverflow`（ruviform） | Required | Pass(部分) | 同上 | 実 overflow=NotRun |
| TT-013 | #126+#188 | `tim02` hanging voice・NoteLedger feed/is_active/drain_evicted/release_all | ruviform `NoteLedger` | Required | Pass(部分) | 同上 | MIDI1 同 Channel/Pitch 重複=Capability/Best-effort・実 plugin voice=NotRun |
| TT-014 | #126+#188 | `tim02_controller_chase_is_channel_aware`・`tim02_seek_releases_and_chases`・`tim02_panic_releases_hanging_voice_and_resume`・`tim02_loop_replay_releases_and_refires` | `chase_span_events`・`set_clock_jump_triggers_release_and_chase`・`ledger_eviction_delivers_synthetic_off`（ruviform） | Required | Pass(部分) | 同上 | 実 MIDI device=NotRun |
| TT-015 | #6+#127 | `ctl_01`/`ctl_02` owner time scope・`pc_c6_oracle_rel` modulation relation | ruviform#186 control event | Required | Pass(部分) | 同上 | 実 modulation DSP=NotRun・実 plugin param=NotRun |
| TT-016 | #6+AUT/CTL-01 | `aut_01_*`（touch/latch/write/return）・`aut_03_*` pass lifecycle | —（project 側） | Required | Pass(部分) | 同上 | 実 GUI record=Deferred(#114) |
| TT-017 | #6+context-inspector | `aut_01_fixation_captures_begin_context`・`aut_03_fixation_survives_mode_and_arm_drift`・`aut_01_target_deleted_closes_state` | — | Required | Pass(部分) | 同上 | 実 GUI=Deferred(#114) |
| TT-018 | #6+#127 | `aut_03_raw_values_saved_separately_survive_reload`（pre-modulation values）・`pc_c6_oracle_rel` | — | Required | Pass(部分) | 同上 | 実 LFO source=NotRun |
| TT-019 | #128+#18 | `tim04_plugin_state` destroyed_instance_fails_closed・unknown_processor_is_unavailable・`sf03_l3` lifecycle・`tim03_process_boundary_rt` noalloc | ruviform plugin host lifecycle | Required | Pass(部分) | 同上 | 実 plugin latency=NotRun |
| TT-020 | #128 | `tim04_plugin_state` capture_while_processing_is_best_effort・capture_after_stop_processing_is_coordinated・capture_with_quiescent_mode_records_quiescent・unsupported_plugin_reports_unsupported・`plugin_state.rs` store | ruviform plugin host state I/O | Required | Pass(headless) | 同上 | 実 plugin state=NotRun |
| TT-021 | #127+ruviform#253 | `tim03_pb_precision_gates`・`tim03_aw_*` precision gates | ruviform plugin-clap/plugin-vst3 `transport_bridge` | Required | Pass(部分) | 同上 | 実 CLAP/VST3 transport=NotRun(EXT-L3) |
| TT-022 | #127/#128+#18 | `tim02_rt_offline_parity`・`tim03_aw_delivery_offline_*`・`tim04_plugin_state` render strict/standard・`aut_02_rt_eval_matches_offline_after_replace` | `tt030_offline_exact_vs_live_degraded`（ruviform） | Required | Pass(部分) | 同上 | 実 plugin offline=NotRun・全 plugin bit-perfect 非保証 |
| TT-023 | #12+#19 | `loop_gate_capture_spans_repeat_project_mapping_per_pass`・`punch_in_loop_capture_spans_repeat_project_mapping_per_pass`・`three_loop_passes_and_wrap`・`vs_d_loop_three_passes`・`tim_02_midi_phrase` recording | ruviform#187 clock epoch | Required | Pass(部分) | 同上 | 実 device record=NotRun |
| TT-024 | #107+ruviform#186 | `sf04_*` probe tests・`pc_c4_07_transport_observe`・`pc_c4_07_capability_rebind`・`trace_c3_05` | ruviform probe broker/diagnostic | Required | Pass(部分) | 同上 | 実 probe device=NotRun |
| TT-025 | #127+#190 | `tim03_pb_exact_windows_and_boundary_positions`・`tim03_req_exact_matrix_windows_and_offsets`・`tim03_sched_*` | `tt025_loop_wrap_splits_and_transports`（ruviform） | Required | Pass(headless+upstream) | 同上 | 実 plugin min=1=NotRun |
| TT-026 | #127+#190/#196 | `tim03_pb_min_max_illegal_segment_policy`・`tim03_cap_offline_exact_fail_closed`・`tim03_cap_realtime_degraded_requires_consent`・`tim03_cap_unsupported_typed_refusal`・`tim03_sched_exact_rejects_illegal_apply`・`tim03_sched_exact_wrap_undeliverable`・`tim03_sched_degraded_defers_apply`・`tim03_node_exact_rejects_illegal_boundary`・`tim03_node_degraded_consented_boundary`・`tim03_req_min_max_process_length_fail_closed`・`tim03_aw_delivery_*` | `tt026_min_frames_exact_never_calls_under_min`・`tt026_degraded_defers_to_legal_boundary`（ruviform） | Required | Pass(headless+upstream) | 同上 | 実 plugin min=128=NotRun・CLAP in-block=TT-027 |
| TT-027 | #127+#196 | `tim03_pb_precision_gates`（Verified in-block→ExactWithinVerifiedCapability・Unknown/Declared not supported）・`tim03_aw_connect_*` | `tt027_in_block_unproven_not_treated_as_supported`・`InBlockTransportSupport`（ruviform） | Required | Pass(部分) | 同上 | 実 CLAP event=NotRun・規格存在≠保証 |
| TT-028 | #127+#190 | `tim03_pb_offsets_zero_one_last`・`tim03_sched_events_boundary_and_stale_generation` | `tt028_event_process_relative_offsets`・`event_list_offsets_are_process_relative`・`build_call_event_list`（ruviform plugin-core） | Required | Pass(headless+upstream) | 同上 | 実 plugin event list=NotRun |
| TT-029 | #127+#196 | `tim03_node_*`・`tim02_loop_replay_releases_and_refires`・`tim03_node_param_event_per_loop_occurrence`・`tim03_process_boundary_rt` | `tt029_loop_occurrence_distinguishes_deliveries`・`tt029_zero_frame_flush_contract`・`tt029_l0_l1_min_frames_difference`（ruviform） | Required | Pass(部分) | 同上 | 実 stateful plugin PDC/tail=NotRun |
| TT-030 | #127/#128+#190 | `tim03_cap_offline_exact_fail_closed`・`tim03_aw_delivery_offline_ignores_consent`・`tim03_aw_delivery_offline_requires_certified_exact`・`tim03_aw_delivery_realtime_consent_degraded_clamped`・render `tim04_plugin_state` strict | `tt030_offline_exact_vs_live_degraded`（ruviform） | Required | Pass(headless+upstream) | 同上 | 実 offline render plugin=NotRun |

## 集計

- TT-001〜030 全 30 件。Req=Required（全件）。
- headless 結果: Pass=1（TT-010）・Pass(headless)=3（TT-006・
  TT-020・TT-025）・Pass(headless+upstream)=3（TT-025・TT-026・
  TT-028・TT-030 のうち upstream test を含む 4 件を headless+
  upstream に分類）・Pass(部分)=残り。Fail=0・NotRun=0・
  Unsupported=0（headless fixture 範囲内）。
- capability gate: 実 VST3/CLAP・実 OS MIDI・実 device・実 GUI・
  実 plugin latency/state/offline は全件 NotRun/Deferred
  （EXT-L3/GUI-L2 Owner）。SMF SMPTE/Format2=Unsupported(#129)。
- 設計レビュー Pass と実測実装 Pass を区別。docs slice は
  設計契約 + 写像のみで、実装受入は #125〜128・#129/#130・
  ruviform#190 系・EXT-L3/GUI-L2 が担う。

## 未実施範囲（capability 別 NotRun/Deferred/Unsupported）

- 実 VST3/CLAP plugin transport/state/offline: NotRun（EXT-L3・
  ruviform#253/#196・synvelis#114）。
- 実 OS MIDI backend・実 MIDI device: NotRun（ruviform#253・
  synvelis#12 recording Owner）。
- 実 GUI automation record・Inspector gesture: Deferred(#114)。
- SMF SMPTE Division/Format2: Unsupported(#129)。
- MIDI2/MPE/Per-note: Deferred(#130)。
- 全 plugin bit-perfect offline 再現性: 非保証（契約上の
  Non-goal）。
- post-merge CI: merge 後の main SHA に対する必須 CI 結果が
  統合検証の証拠（本 slice では未実施）。
