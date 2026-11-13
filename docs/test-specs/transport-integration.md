---
id: "TSPEC-034X7wP0GQjilI9S0FX7m0"
type: "test-spec"
status: "approved"
title: "Transport Integration Test Spec — TT-001〜030 case map to existing fixtures"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/transport-integration.md"
    - "docs/designs/transport-integration-design.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/transport-integration.md"
    - "docs/designs/transport-integration-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "evidence/sv20/transport-integration-case-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/transport-integration-design.md"
  derived-by: []
  verifies:
    - "docs/specs/transport-integration.md"
    - "docs/designs/transport-integration-design.md"
  verified-by: []
  references:
    - "evidence/sv20/transport-integration-case-ledger.md"
    - "evidence/sv20/transport-integration-verification.md"
  defers: []
  deferred-by: []
---

# Transport Integration Test Spec — TT-001〜030 case map to existing fixtures

## Purpose

Issue #20 の TT-001〜030 各受入ケースについて、何を検証し・どの
実装 Owner が・どの既存 fixture/oracle で・どの状態
（Pass/Fail/NotRun/Unsupported・Exact/Degraded/Unsupported/Unknown）
かを固定する。headless 証拠を実機受入に繰り上げないための正本。

## Feature

Transport Integration TT Acceptance Mapping — TT-001〜030 を
canonical contract・実装 Owner・検証手段・独立 oracle・状態へ
写像する。

## Rules

- **Rule: TT ID uniqueness** — TT-001〜030 は一意。TT-A/B/C/D
  群（001〜024）と TT-025〜030 補遺で重複・欠番なし。
- **Rule: Owner separation** — Synvelis=DAW 意味論
  （#125〜128・#6・#12・#107）、Ruviform=generic primitive
  （#186/#188/#189/#190/#196/#307）。混同しない。
- **Rule: Precision class ≠ result state** — Exact/FrameAccurate/
  BlockOnly/DegradedWithConsent/Unsupported/Unknown（能力 Class）
  と Pass/Fail/NotRun/Unsupported（検証結果）を別語彙で記録。
- **Rule: headless ≠ real** — headless/mock fixture の Pass は
  実 VST3/CLAP・実 OS MIDI・実 device・GUI の受入を意味しない。
  実機は EXT-L3/GUI-L2 で別途 NotRun/Deferred。
- **Rule: Offline Exact fail-closed** — Exact Offline は未保証
  精度で Fail。Realtime Degraded は `DegradedConsent` +
  Capability 判定 + receipt 記録が必須。
- **Rule: process-start offset** — Plugin Event offset は実
  process 呼出し開始相対。Callback/Span-relative を転送しない。
- **Rule: half-open span** — `[start,end)`・境界 `==span_end`
  は次区間で 1 回だけ。

## Examples

### Example: TT-025/026 — Callback 256・Loop at 100・min_frames

- Given callback `[0,256)`・Loop at 100・plugin `min_frames=1`
- When TransportScheduler が span 分割
- Then `[0,100)`+`[100,256)` の 2 physical process（100+156）で
  適法、Transport Context が各 span で正しい。
- Given 同条件で `min_frames=128`
- Then 100-frame の違法 process を呼ばない。Verified in-block
  transport event・preflight reject・明示的 user-approved
  degraded のいずれかを選択。Offline Exact は Fail-closed。

### Example: TT-028 — Event at 100/120/256

- Given events at Engine Frame 100/120/256・process 分割
- Then offset は process-start 相対で 0/20、frame 256 は次
  callback の offset 0。境界 event の重複なし。

## Guarantees

- TT-001〜030 の各ケースに Owner・fixture/oracle・状態・
  capability gate が記録される。
- headless Pass は実機受入と区別される。
- Exact/Degraded/Unsupported/Unknown と Pass/Fail/NotRun が
  混同されない。
- TT-025〜030 の process legality・offset 起点・Offline
  Fail-closed が契約として固定される。

## Non-goals

- 実装コードの変更・新規 fixture の作成（既存 fixture の写像
  のみ）。
- 実 VST3/CLAP・実 OS MIDI・実 device・GUI の受入実行
  （EXT-L3/GUI-L2 Owner）。
- SMF SMPTE/Format2（#129）・MIDI2/MPE（#130）の検証。
- 新規製品方針・数値 SLO の確定。

## Risk

- **fixture 名の陳腐化**: 既存 test 名がリネームされると台帳が
  ずれる。test-spec は fixture 名を「代表例」として記し、
  evidence ledger で revision/commit を固定。
- **capability 誤判定**: `InBlockTransportSupport::Unknown` を
  supported とみなす誤り。Verified のみ対応を契約化。
- **世代混在**: stale generation event の誤配送。Single RT
  Owner・nonRT Prepare・generation 照合で抑止。
- **headless 過信**: mock plugin の Pass を実 plugin に一般化
  する誤り。evidence で NotRun/Deferred を強制。

## TT-001〜030 Case Map

凡例: Req=Required。Result=現行 headless fixture 観測。
capability gate=実機/OS/GUI の未実施範囲。
詳細 oracle・fixture パス・revision は
`evidence/sv20/transport-integration-case-ledger.md` を正本とする。

| TT | Owner | 既存 fixture / oracle（代表） | Ruviform linked | Req | headless Result | capability gate |
| --- | --- | --- | --- | --- | --- | --- |
| TT-001 | #125 | `tim_01_ppqn`・`tim_02_ppqn_smf`・`Ppqn`/`BeatPosition`/`TickPosition::rescale` | #189 | Required | Pass(部分) | 実 project UI=Deferred |
| TT-002 | #125+#19 | `tim_01_ppqn` inexact/rt・`aut_02` exact rational・`BeatPosition` num/den | #189 | Required | Pass(部分) | — |
| TT-003 | #126 | `tim_02_smf` native division・`tim_02_ppqn_smf`・`midi_source`・SMF parse | #188 | Required | Pass(部分) | SMPTE=Unsupported(#129)・実 MIDI=NotRun |
| TT-004 | #127+project | `tim03_req_simultaneous_loop_seek_tempo_param`・`md_a` tempo・`monitor_engine` punch | `TempoMapRevision` gen | Required | Pass(部分) | 実 plugin tempo=NotRun |
| TT-005 | #12+#127 | `negative_preroll_maps_to_positive_session`・`MediaTime`/`MediaRange` | checked ops | Required | Pass(部分) | — |
| TT-006 | #127+#190 | `tim03_pb_exact_windows_and_boundary_positions`・`tim03_node_*`・`tim03_sched_*` | `tt025_loop_wrap_splits_and_transports` | Required | Pass(headless) | 実 plugin=NotRun |
| TT-007 | #126/#127 | `tim02_loop_replay_releases_and_refires`・`tim03_node_param_event_per_loop_occurrence` | `tt029_loop_occurrence_distinguishes_deliveries` | Required | Pass(部分) | 実 plugin=NotRun |
| TT-008 | #127+#190 | `tim02_seek_releases_and_chases`・`pc_c4_07_transport_observe`・`AppliedReceipt` | `tt008_command_receipt_lifecycle` | Required | Pass(部分) | 実 device seek=NotRun |
| TT-009 | ruviform#187+probe | `sf04_s3_*`/`sf04_s9_*` epoch・`pc_c4_07_capability_rebind` | `tt009_restart_vs_seek_epoch_generation` | Required | Pass(部分) | 実 device restart=NotRun |
| TT-010 | #126 | `tim02_same_tick_off_precedes_ons`・`tim_02_smf` | `tt010_same_frame_source_order` | Required | Pass | — |
| TT-011 | #127+#196 | `tim03_node_stale_transport_generation_zero_delivery`・`tim03_prov_*` | `stale_plan_generation_event_not_delivered` | Required | Pass(headless) | 実 plugin replace=NotRun |
| TT-012 | ruviform#190+#127 | `tim03_sw_07_driver_queue_full_typed`・NoteLedger release_all | `tt012_span_budget_overflow_faults`・`EventBudgetExceeded`/`SpanBudgetExceeded`/`FaultLedgerOverflow` | Required | Pass(部分) | 実 overflow=NotRun |
| TT-013 | #126+#188 | `tim02` hanging voice・NoteLedger feed/is_active/drain/release_all | `NoteLedger` | Required | Pass(部分) | MIDI1 同 pitch=Capability/Best-effort |
| TT-014 | #126+#188 | `tim02_controller_chase_is_channel_aware`・`tim02_seek_releases_and_chases`・`tim02_panic_*`・`tim02_loop_replay_*` | `chase_span_events`・`set_clock_jump_triggers_release_and_chase`・`ledger_eviction_delivers_synthetic_off` | Required | Pass(部分) | 実 MIDI device=NotRun |
| TT-015 | #6+#127 | `ctl_01`/`ctl_02` owner time scope・`pc_c6_oracle_rel` | ruviform#186 control event | Required | Pass(部分) | 実 modulation DSP=NotRun |
| TT-016 | #6+AUT/CTL-01 | `aut_01_*`・`aut_03_*` pass lifecycle | — | Required | Pass(部分) | 実 GUI record=Deferred(#114) |
| TT-017 | #6+context-inspector | `aut_01_fixation_captures_begin_context`・`aut_03_fixation_survives_*`・`aut_01_target_deleted_closes_state` | — | Required | Pass(部分) | 実 GUI=Deferred(#114) |
| TT-018 | #6+#127 | `aut_03_raw_values_saved_separately_survive_reload`・`pc_c6_oracle_rel` | — | Required | Pass(部分) | 実 LFO source=NotRun |
| TT-019 | #128+#18 | `tim04_plugin_state` destroyed/unknown・`sf03_l3`・`tim03_process_boundary_rt` noalloc | ruviform plugin host lifecycle | Required | Pass(部分) | 実 plugin latency=NotRun |
| TT-020 | #128 | `tim04_plugin_state` best_effort/coordinated/quiescent/unsupported・`plugin_state.rs` | ruviform plugin host state I/O | Required | Pass(headless) | 実 plugin state=NotRun |
| TT-021 | #127+ruviform#253 | `tim03_pb_precision_gates`・`tim03_aw_*` | ruviform plugin-clap/vst3 `transport_bridge` | Required | Pass(部分) | 実 CLAP/VST3=NotRun(EXT-L3) |
| TT-022 | #127/#128+#18 | `tim02_rt_offline_parity`・`tim03_aw_delivery_offline_*`・`tim04_plugin_state` strict・`aut_02_rt_eval_matches_offline_after_replace` | `tt030_offline_exact_vs_live_degraded` | Required | Pass(部分) | 実 plugin offline=NotRun |
| TT-023 | #12+#19 | `loop_gate_capture_spans_repeat_project_mapping_per_pass`・`punch_in_loop_*`・`three_loop_passes_and_wrap`・`vs_d_loop_three_passes`・`tim_02_midi_phrase` | ruviform#187 clock epoch | Required | Pass(部分) | 実 device record=NotRun |
| TT-024 | #107+ruviform#186 | `sf04_*` probe・`pc_c4_07_transport_observe`・`pc_c4_07_capability_rebind`・`trace_c3_05` | ruviform probe broker/diagnostic | Required | Pass(部分) | 実 probe device=NotRun |
| TT-025 | #127+#190 | `tim03_pb_exact_windows_and_boundary_positions`・`tim03_req_exact_matrix_*`・`tim03_sched_*` | `tt025_loop_wrap_splits_and_transports` | Required | Pass(headless+upstream) | 実 plugin min=1=NotRun |
| TT-026 | #127+#190/#196 | `tim03_pb_min_max_illegal_segment_policy`・`tim03_cap_*`・`tim03_sched_exact_rejects_*`/`degraded_defers_*`/`exact_wrap_undeliverable`・`tim03_node_*`・`tim03_req_min_max_*`・`tim03_aw_delivery_*` | `tt026_min_frames_exact_never_calls_under_min`・`tt026_degraded_defers_to_legal_boundary` | Required | Pass(headless+upstream) | 実 plugin min=128=NotRun・CLAP in-block=TT-027 |
| TT-027 | #127+#196 | `tim03_pb_precision_gates`（Verified のみ in-block）・`tim03_aw_connect_*` | `tt027_in_block_unproven_not_treated_as_supported`・`InBlockTransportSupport` | Required | Pass(部分) | 実 CLAP event=NotRun |
| TT-028 | #127+#190 | `tim03_pb_offsets_zero_one_last`・`tim03_sched_events_boundary_and_stale_generation` | `tt028_event_process_relative_offsets`・`event_list_offsets_are_process_relative` | Required | Pass(headless+upstream) | 実 plugin event list=NotRun |
| TT-029 | #127+#196 | `tim03_node_*`・`tim02_loop_replay_*`・`tim03_node_param_event_per_loop_occurrence`・`tim03_process_boundary_rt` | `tt029_loop_occurrence_distinguishes_deliveries`・`tt029_zero_frame_flush_contract`・`tt029_l0_l1_min_frames_difference` | Required | Pass(部分) | 実 stateful plugin PDC/tail=NotRun |
| TT-030 | #127/#128+#190 | `tim03_cap_offline_exact_fail_closed`・`tim03_aw_delivery_offline_*`・`tim03_aw_delivery_realtime_consent_degraded_clamped`・render `tim04_plugin_state` strict | `tt030_offline_exact_vs_live_degraded` | Required | Pass(headless+upstream) | 実 offline render plugin=NotRun |

## Sources and Relations

- `docs/specs/transport-integration.md`（TT-R-01〜15・TT-AC-01〜09）
- `docs/designs/transport-integration-design.md`（TT Mapping 表）
- `docs/designs/time-transport-midi-automation-plugin-integration-architecture.md`
  （v0.5 — TT-001〜024・§14 TT-025〜030）
- `evidence/sv20/transport-integration-case-ledger.md`（詳細 oracle・
  fixture パス・revision の正本）
- `evidence/sv20/transport-integration-verification.md`（検証実行記録）
