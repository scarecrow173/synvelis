---
id: "DESIGN-034X7wOrHbsoz1senpZA9w"
type: "design"
status: "approved"
title: "Transport Integration Design — Engine Frame canonical, process-start-relative offsets, capability/degraded policy, TT-001〜030 mapping"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/transport-integration.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/transport-integration.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/specs/automation-modulation.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/specs/recording-take-comping.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/transport-integration.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  derived-by:
    - "docs/plans/transport-integration.md"
    - "docs/test-specs/transport-integration.md"
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/transport-integration.md"
  defers: []
  deferred-by: []
---

# Transport Integration Design — Engine Frame canonical, process-start-relative offsets, capability/degraded policy, TT-001〜030 mapping

## Context

Issue #20 は Time / Transport / MIDI / Automation /
Plugin Integration v0.5（Span・Event・State Contracts）
の canonical 設計契約と TT-001〜030 fixture 写像を要求
する。本 design は spec `transport-integration.md`
（TT-R-01〜15）の詳細化であり、上流 Approved baseline
`time-transport-midi-automation-plugin-integration-
architecture.md` v0.5（14 合意項目 + §14 TT-025〜030 補
遺）と `configurable-ppqn-musical-time-architecture.md`
（#19）を実装 Owner が参照する形へ統合する。

既存資産: TIM-01（#125 PPQN）・TIM-02（#126 MIDI/SMF/
RT-offline parity）・TIM-03（#127 process boundary/
session wiring）・TIM-04（#128 plugin state snapshot）
の headless 実装済み slice が main に存在し、
`crates/synvelis-player/src/process_boundary.rs` と
`tests/tim03_*`・`tim02_*`・`tim_02_*`・`tim04_*` が TT
受入の大半を headless でカバーする。Ruviform pinned
`fd1d4d5` の `ruviform-media-core`/`plugin-core`/
`plugin-host` が generic primitive（`BeatPosition`/
`EngineFrame`/`TransportScheduler`/`AppliedReceipt`/
`BlockContract`/`InBlockTransportSupport`/`NoteLedger`/
`build_call_event_list`）を提供し、TT-ID を冠した
upstream test が独立 oracle として使える。

## Scope

### In Scope

- 時間/世代 domain 分離・PPQN 非依存 Rational Beat・
  Source Native Tick 非破壊性の contract 構造。
- Transport Command → Applied Receipt pipeline と
  Half-open Span Scheduling の data/control flow。
- Timed Event Envelope・Plugin Event offset 再計算規約。
- Physical Process legality・Capability/Degraded/Unknown
  判定・Offline Exact Fail-closed。
- Native MIDI・NoteLedger・Sustain/Chase・Automation
  Write 分離・Plugin State Capture Level の contract。
- Single RT Owner/Generation・stale generation 配送
  禁止・Live/Offline/Recording 共有意味論。
- Synvelis/Ruviform 責務分離と upstream Issue 運用。
- TT-001〜030 → Owner/fixture/oracle/状態の写像表。

### Out of Scope

- 実装コードの変更（`crates/`・Cargo・CI）。
- 実 Plugin/実 OS MIDI/実 device/実 GUI の受入実行
  （headless 設計契約のみ。実機は EXT-L3/GUI-L2 Owner）。
- SMF SMPTE/Format2（#129）・MIDI2/MPE（#130）の初期
  契約実装。
- 上流 architecture v0.5 の再定義（既存合意の統合・
  明文化が目的）。
- 新規製品方針・数値 SLO・OS サポート宣言。

## Components and Responsibilities

| 責務 | Owner | 根拠 |
| --- | --- | --- |
| Project PPQN・Rational Beat Position・TempoMapRevision・non-destructive Source Tick | Synvelis #125（TIM-01） | `tim_01_*`・`tim_02_smf`・`tim_02_ppqn_smf` |
| Native MIDI Division/Tick・SMF Import/Export・NoteLedger/Chase | Synvelis #126（TIM-02） | `tim02_*`・`tim_02_*` |
| Transport Command/Receipt・Half-open Span・process boundary legality・Capability/Degraded | Synvelis #127（TIM-03） + Ruviform #190 | `tim03_*`・`process_boundary.rs`・`TransportScheduler` |
| Plugin State Capture quality・save/reload/HEAD/offline recipe | Synvelis #128（TIM-04） | `tim04_*`・`plugin_state.rs` |
| Generic Beat/Clock/Transport/Event/PluginHost/Probe primitives | Ruviform #188/#189/#190/#186/#196/#307 | `ruviform-media-core`・`plugin-core`・`plugin-host` |
| Automation Write/Read/Touch/Latch・Gesture Target・Recording Commit | Synvelis #6 + AUT-01〜04 + CTL-01 | `aut_01_*`・`aut_02_*`・`aut_03_*` |
| Device MIDI Input・Recording session・Take/Comp | Synvelis #12 + #19 + RCD-01 | `vs_d_*`・`loop_gate_*` |
| Probe/Observe・capability mirror | Synvelis #107 + PC-C4-07 + Ruviform #186 | `sf04_*`・`pc_c4_07_*`・`trace_c3_05` |
| 実 VST3/CLAP・実 OS MIDI・実 device・GUI 受入 | EXT-L3 / GUI-L2（#114・ruviform#253/#196） | 本 slice の外 |

## Data and Control Flow

### 1. Transport Command → Applied Receipt

```
UI/Controller/Recording ──Command──▶ Validation (nonRT)
        │                            │ reject → typed rejection
        ▼                            ▼
   Prepare (nonRT) ──▶ Bounded Scheduling ──▶ RT Apply
        │                                     │
        └──────── AppliedReceipt ◀────────────┘
            (Requested/Pending/Applied/Rejected)
```

- Command は intent のみ。`AppliedReceipt` が唯一の
  「実際に適用された」証拠で、Target/Transaction・
  Applied Engine Frame・TransportGeneration・
  PlanGeneration・成否/拒否理由を持つ。
- Loop Wrap は Transport Discontinuity。Device Clock
  Epoch は更新せず、Recording Session Frame を巻き戻さ
  ない。
- Seek は Pending → Applied の 2 相。古い
  TransportGeneration の Event は新 Plan へ配送しない
  （`tim03_node_stale_transport_generation_zero_
  delivery`・`tim03_prov_stale_generations_zero_
  delivery`）。

### 2. Half-open Span Scheduling

- `TransportScheduler`（ruviform）が callback 区間を
  Transport Context 変化位置（Seek/Loop Wrap/Tempo/
  Clip Boundary/Transport/Plugin Context）で `[start,
  end)` に分割し、各 Span の Project Position・
  LoopIteration・TempoRevision・TransportGeneration・
  Plugin Transport Context を確定する。
- 例（TT-006/TT-025〜028）: callback `[0,256)`、Loop at
  100 → Span A `[0,100)`・Span B `[100,256)`。Event at
  120 は Span B の offset 20、Event at 100 は Span B の
  offset 0 で 1 回だけ、Event at 256 は次 callback の
  offset 0。
- 境界 `== span_end` は次区間へ 1 回だけ配送。重複/
  欠落を抑制。

### 3. Event → Plugin EventList offset

```
TimedEvent { engine_frame, plan_gen, transport_gen,
             stable_id, loop_occurrence, source_seq }
        │
        ▼  (nonRT prepare で Physical Process 分割決定)
ProcessChunk { process_start, process_frames }
        │
        ▼  (RT 配送境界で再計算)
frame_offset = event_engine_frame − process_start   ∈ [0, frames)
```

- `frame_offset`・CLAP `clap_event_header.time` の起点は
  **各物理 process 呼出し開始**。Callback-relative/
  Span-relative を転送しない。
- 全区間は半開 `[start, end)`。0-frame flush は専用制御
  境界として通常 Audio 区間と混同しない。
- `build_call_event_list`（ruviform plugin-core）が
  process-relative offset を構築する正本。

### 4. Physical Process legality / Capability

- 各物理 Chunk は Activated Plugin の `min_frames ≤
  frames ≤ max_frames` を満たす。違法な短い Block を
  RT で送らない。
- Preflight（nonRT）が min/max Block・最大分割数・
  L0/L1 Block 契約・Transport Context 更新方式・
  Stateful DSP/PDC/Tail・時刻精度を確認。
- Capability class: `Exact` / `FrameAccurate` /
  `BlockOnly` / `DegradedWithConsent` / `Unsupported` /
  `Unknown`。`InBlockTransportSupport` は `Verified`
  のみ in-block transport 対応とみなす（規格に存在する
  だけで Sample-accurate を保証しない）。
- Exact Offline は未保証精度で Fail-closed。Realtime
  Degraded は `DegradedConsent` + Capability 判定必須、
  `AppliedReceipt`/診断に実適用 Engine Frame と精度低下
  を記録。

### 5. Native MIDI / NoteLedger / Sustain

- SMF Import は source native Division/Tick を Raw 保全。
  Project PPQN 変更・録音・Quantize で破壊的変更しない
  （Quantize は別 Derived Version）。
- NoteLedger は Target/PluginInstance/Channel/Note
  Identity で追跡。Seek/Loop/Stop/Comp Boundary/Plugin
  Replace で Release/Reset/Controller Chase を明示。
  NoteOff 喪失は Event Integrity Fault。MIDI1 同
  Channel/Pitch 重複・Plugin 内部 Voice 制約は
  Capability/Best-effort。

### 6. Automation Write / Gesture / FrameBase

- 評価段階: Base Value → Automation/Manual Reference →
  Modulation → Clamp/Smoothing → Host-resolved →
  Dispatched → Plugin-reported/Observed。
- Write Mode（Off/Read/Touch/Latch/Write）・Write Arm・
  Gesture State・Recording Transaction を #6/AUT/CTL-01
  と整合。RT Event 配送は Project を直接変更せず nonRT
  ProjectController へ確定。
- Gesture 開始時に Stable TargetId・Plugin
  Instance/Binding Generation・Arm・TransportGeneration
  を固定。Inspector Follow 変更で対象を替えない。
- 既定は Modulation 前 Reference を記録し、LFO 適用済
  み値の再 Modulation を避ける。Block/Control/Sample-
  accurate/Per-note 精度は Adapter ごとに評価。

### 7. Plugin State Capture

- Capture 結果を Coordinated/Quiescent/Best-effort/
  Unsupported で区別。「特定 Frame に同期した完全
  Snapshot」と見なさない。File Save 成功・Plugin State
  Exact・RT Active は別 receipt。失敗/古い State は
  Degraded/明示 Error。Plugin Missing/Version 差でも
  Opaque Bytes を保全。RT に無期限停止や OS I/O を求め
  ない。

## Contract Sections

### Time / Generation domains（TT-R-01〜02）

| Domain | 正本 | 備考 |
| --- | --- | --- |
| Device Clock Domain/Epoch | Ruviform Clock | Device restart で rebase、Project 位置不変 |
| Engine Frame | Ruviform media-core | Event 位置の正本 |
| Recording Session Frame | Synvelis recording | Loop Wrap で巻き戻さない |
| Project Musical/Media Time | Synvelis TempoMap | Rational Beat Position が canonical |
| Source Native Time | Synvelis MIDI/SMF | 原 Division/Tick を Raw 保全 |
| TempoMapRevision / TransportGeneration / PlanGeneration | 各層 | 世代照合で stale 配送を拒否 |

- 異 Domain の値を無検証で比較・加算しない。
- Timestamp 信頼度は Verified/Estimated/Unknown。
- Beat Position は分解能非依存有理数。Project PPQN
  （既定 960・可変）は Tick 投影の文脈のみ。異 PPQN の
  Tick 整数を黙って比較しない。整数 Frame 丸めは変換
  連鎖の最後で一度だけ。表現不能は暗黙丸めでなく明示
  Fail/PoC 定義の結果。

### Transport / Span / Event（TT-R-03〜08）

- `TransportScheduler` + `AppliedReceipt` が generic
  正本。Synvelis は DAW 意味論（Track/Clip/Take/Comp・
  Automation Write・Project 設定）を上位に載せる。
- Half-open `[start,end)`・境界 1 回配送・Logical Span ≠
  Physical Process Call。
- Plugin Event offset = `event_engine_frame −
  process_start`。Callback/Span-relative を転送しない。
- Physical legality `min_frames ≤ frames ≤ max_frames`。
- Capability `Exact/FrameAccurate/BlockOnly/
  DegradedWithConsent/Unsupported/Unknown`・
  `InBlockTransportSupport::Verified` のみ in-block。
- Offline Exact Fail-closed・Realtime Degraded は
  `DegradedConsent` + Capability + receipt 記録。

### MIDI / NoteLedger / Sustain（TT-R-09）

- Native Division/Tick/Event 順保持・同 Frame 安定因果
  順（NoteOff 必ず先の普遍優先は採らない）。
- NoteLedger Release/Reset/Chase・NoteOff 喪失は Fault・
  Capability/Best-effort の限界を公開。

### Automation / Write（TT-R-10）

- 評価段階分離・Write Mode/Arm/Gesture 固定・Project
  Commit ≠ RT Applied・LFO Reference 記録分離・Adapter
  別精度評価。

### Plugin State（TT-R-11）

- Capture quality 4 分類・File Save ≠ Plugin State Exact
  ≠ RT Active・Opaque Bytes 保全・RT 安全制約。

### Owner / Generation（TT-R-12〜14）

- Single RT Owner・Stable TargetId→ParamId nonRT 検証・
  旧世代 Event 誤配送禁止・RT 危険操作禁止。
- Live/Offline/Recording は時間/Event 順を共有するが
  Driver/Instance/Failure Policy を区別。Bit-perfect
  無条件保証なし。
- Synvelis=DAW 意味論、Ruviform=generic primitive。generic
  不足は upstream Issue（ruviform#190/#188/#186/#189/
  #307/#196）へ起票し恒久重複実装しない。

## Failure Taxonomy

| 失敗 | 分類 | 対応 |
| --- | --- | --- |
| 旧世代 Event 配送 | Stale generation | `tim03_*_zero_delivery` で拒否・probe 記録 |
| 違法 process 長（min/max 違反） | Capability/Illegal boundary | Preflight reject・Exact は Fail-closed・Degraded は consent 必須 |
| In-block transport 未証明 | Unknown capability | `InBlockTransportSupport::Unknown` を supported とみなさない |
| Offline Exact 未保証 | Precision insufficient | Fail-closed・consent で救済しない |
| NoteOff 喪失 | Event integrity fault | NoteLedger Fault・Defined Recovery・Degraded 状態 |
| State capture 失敗/古い | Degraded/Error | quality receipt・last-good 保全・Opaque Bytes |
| Plan/Event budget 超過 | Typed fault | nonRT Restrict/Reject・RT は事前確保容量内 |
| 物理 driver 失敗 | Driver policy | Live/Offline/Recording の Failure Policy を区別 |

## Non-functional Constraints

- RT hot path は allocation/deallocation ゼロ
  （`tim03_process_boundary_rt` の allocation 計装）。
- RT 内に無期限 Wait・OS I/O・Project Mutation・危険
  Destructor を持ち込まない。
- Bounded Scheduling は事前確保容量以内。超過 Plan は
  nonRT で Restrict/Reject。
- `ruviform-media-core` の unchecked `Add`/`Sub` を
  Synvelis 側で直接使わず checked 演算を lint 対象。

## Risks

- **headless 証拠の実機誤読**: TT Pass は headless/mock
  fixture の結果。実 VST3/CLAP・実 OS MIDI・実 device・
  GUI は EXT-L3/GUI-L2 で別途受入。evidence 台帳で
  NotRun/Deferred を明示。
- **Capability 未証明 plugin**: `InBlockTransportSupport`
  の Verified のみ対応。規格存在≠保証。実 adapter ごとの
  検証が必要。
- **世代混在**: Transport/Plan/Plugin generation の照合
  を怠ると旧世代 Event 誤配送。Single RT Owner と
  nonRT Prepare で抑止。
- **数値未確定**: BeatPosition 幅/分母上限・Max
  Span/Event Budget・Touch Return Time は PoC 未確定
  （architecture §13 継承）。実装時に契約値が変わる可能
  性。

## TT-001〜030 Mapping

凡例: Req=Required（全 TT は Required）、Result は
headless fixture 観測。実機/OS/GUI は NotRun。
Owner は canonical 実装 Issue。詳細は
`docs/test-specs/transport-integration.md` と
`evidence/sv20/`。

| TT | 検証内容（要約） | Synvelis fixture / oracle | Ruviform linked | Owner | headless 結果 | capability gate |
| --- | --- | --- | --- | --- | --- | --- |
| TT-001 | PPQN 960/1000/1920 で Beat Position 不変・Tick 文脈 | `tim_01_ppqn` sweep/reproject/boundary・`tim_02_ppqn_smf` | `Ppqn`/`BeatPosition`/`TickPosition::rescale` | #125 | Pass(部分) | 実 project UI=Deferred |
| TT-002 | Beat=1/7 正本に無断丸めなし | `tim_01_ppqn` inexact reject/rt rounding・`aut_02` exact rational | `BeatPosition` num/den | #125+#19 | Pass(部分) | — |
| TT-003 | MIDI Source Division 480/1000 native Tick 保持 | `tim_02_smf` native division・`tim_02_ppqn_smf`・`midi_source` | SMF parse は synvelis-project・ruviform#188 timed MIDI | #126 | Pass(部分) | SMPTE div=Unsupported(#129)・実 MIDI=NotRun |
| TT-004 | Playback中 Tempo 変更で新旧 TempoRevision 混在なし | `tim03_req_simultaneous_loop_seek_tempo_param`・`md_a` tempo・`monitor_engine` punch boundary | `TempoMapRevision` generation | #127+project | Pass(部分) | 実 plugin tempo=NotRun |
| TT-005 | Negative Pre-roll 正確・有界変換 | `negative_preroll_maps_to_positive_session`（recording）・l2-master | `MediaTime`/`MediaRange` checked | #12+#127 | Pass(部分) | — |
| TT-006 | 256 Frame Loop at 100 → 2 Span・Plugin Transport 正しい | `tim03_pb_exact_windows_and_boundary_positions`・`tim03_node_*`・`tim03_sched_*` | `tt025_loop_wrap_splits_and_transports` | #127+#190 | Pass(headless) | 実 plugin=NotRun |
| TT-007 | 複数周 Loop Occurrence 区別・二重配送なし | `tim02_loop_replay_releases_and_refires`・`tim03_node_param_event_per_loop_occurrence` | `tt029_loop_occurrence_distinguishes_deliveries`・`loop_occurrence_refires_event_once_per_lap` | #126/#127 | Pass(部分) | 実 plugin=NotRun |
| TT-008 | Seek Pending Requested/Applied Frame・Generation 区別 | `tim02_seek_releases_and_chases`・`pc_c4_07_transport_observe` | `tt008_command_receipt_lifecycle`・`AppliedReceipt` | #127+#190 | Pass(部分) | 実 device seek=NotRun |
| TT-009 | Device Restart ClockEpoch 更新・Project 位置不変 | `sf04_s3_*`/`sf04_s9_*` epoch・`pc_c4_07_capability_rebind` | `tt009_restart_vs_seek_epoch_generation` | ruviform#187+probe | Pass(部分) | 実 device restart=NotRun |
| TT-010 | Same-Frame Note On/Off Stable Causal Order | `tim02_same_tick_off_precedes_ons`・`tim_02_smf` | `tt010_same_frame_source_order` | #126 | Pass | — |
| TT-011 | Plugin Replace 後 Old Plan Event 誤配送なし | `tim03_node_stale_transport_generation_zero_delivery`・`tim03_prov_*` | `stale_plan_generation_event_not_delivered` | #127+#196 | Pass(headless) | 実 plugin replace=NotRun |
| TT-012 | Event Budget Overflow Error/Fault/分類済 Loss・NoteOff 安全 | `tim03_sw_07_driver_queue_full_typed`・NoteLedger release_all | `tt012_span_budget_overflow_faults`・`EventBudgetExceeded`/`SpanBudgetExceeded`/`FaultLedgerOverflow` | ruviform#190+#127 | Pass(部分) | 実 overflow=NotRun |
| TT-013 | MIDI overlapping same-pitch Note Ledger と限界 | `tim02` hanging voice・NoteLedger feed/is_active/drain/release_all | ruviform `NoteLedger` | #126+#188 | Pass(部分) | MIDI1 同 pitch=Capability/Best-effort |
| TT-014 | Sustain CC64 + Seek/Loop Release/Reset/Chase | `tim02_controller_chase_is_channel_aware`・`tim02_seek_releases_and_chases`・`tim02_panic_*`・`tim02_loop_replay_*` | `chase_span_events`・`set_clock_jump_triggers_release_and_chase`・`ledger_eviction_delivers_synthetic_off` | #126+#188 | Pass(部分) | 実 MIDI device=NotRun |
| TT-015 | Automation Off+Modulation ON Base 復帰・Modulation 維持 | `ctl_01`/`ctl_02` owner time scope・`pc_c6_oracle_rel` | ruviform#186 control event | #6+#127 | Pass(部分) | 実 modulation DSP=NotRun |
| TT-016 | Read/Touch/Latch/Write Arm/Record/Gesture #6 整合 | `aut_01_*`・`aut_03_*` pass lifecycle | — | #6+AUT/CTL-01 | Pass(部分) | 実 GUI record=Deferred(#114) |
| TT-017 | Gesture 中 Inspector Follow 変更 Target 不動 | `aut_01_fixation_captures_begin_context`・`aut_03_fixation_survives_*`・`aut_01_target_deleted_closes_state` | — | #6+context-inspector | Pass(部分) | 実 GUI=Deferred(#114) |
| TT-018 | Recording 中 LFO Reference のみ記録・二重 Modulation なし | `aut_03_raw_values_saved_separately_survive_reload`・`pc_c6_oracle_rel` | — | #6+#127 | Pass(部分) | 実 LFO source=NotRun |
| TT-019 | Plugin Latency/State Failure Last-good/Degraded・RT 非 Block | `tim04_plugin_state` destroyed/unknown・`sf03_l3`・`tim03_process_boundary_rt` noalloc | ruviform plugin host lifecycle | #128+#18 | Pass(部分) | 実 plugin latency=NotRun |
| TT-020 | Processing 中 SaveState Consistency Level と成否を正しく表示 | `tim04_plugin_state` best_effort/coordinated/quiescent/unsupported・`plugin_state.rs` | ruviform plugin host state I/O | #128 | Pass(headless) | 実 plugin state=NotRun |
| TT-021 | CLAP/VST3 / Variable PPQN Transport 位置/丸め | `tim03_pb_precision_gates`・`tim03_aw_*` | ruviform plugin-clap/vst3 `transport_bridge` | #127+ruviform#253 | Pass(部分) | 実 CLAP/VST3=NotRun(EXT-L3) |
| TT-022 | Realtime vs Offline Fixture 同条件イベント/DSP 一致 | `tim02_rt_offline_parity`・`tim03_aw_delivery_offline_*`・`tim04_plugin_state` strict・`aut_02_rt_eval_matches_offline_after_replace` | `tt030_offline_exact_vs_live_degraded` | #127/#128+#18 | Pass(部分) | 実 plugin offline=NotRun |
| TT-023 | Project Loop 下 Raw MIDI Recording Session/Source Timestamp 不変 | `loop_gate_capture_spans_repeat_project_mapping_per_pass`・`punch_in_loop_*`・`three_loop_passes_and_wrap`・`vs_d_loop_three_passes`・`tim_02_midi_phrase` | ruviform#187 clock epoch | #12+#19 | Pass(部分) | 実 device record=NotRun |
| TT-024 | Event/Transport Probe Source→Plan→Target 時刻/世代/信頼度追跡 | `sf04_*` probe・`pc_c4_07_transport_observe`・`pc_c4_07_capability_rebind`・`trace_c3_05` | ruviform probe broker/diagnostic | #107+ruviform#186 | Pass(部分) | 実 probe device=NotRun |
| TT-025 | Callback 256/Wrap at100/min_frames=1 → 100+156 適法分割・Transport Context 正確 | `tim03_pb_exact_windows_and_boundary_positions`・`tim03_req_exact_matrix_*`・`tim03_sched_*` | `tt025_loop_wrap_splits_and_transports` | #127+#190 | Pass(headless+upstream) | 実 plugin min=1=NotRun |
| TT-026 | Callback 256/Wrap at100/min_frames=128 → 不正 100-Frame 呼出しなし・Reject or 明示 Degraded | `tim03_pb_min_max_illegal_segment_policy`・`tim03_cap_*`・`tim03_sched_exact_rejects_*`/`degraded_defers_*`/`exact_wrap_undeliverable`・`tim03_node_*`・`tim03_req_min_max_*`・`tim03_aw_delivery_*` | `tt026_min_frames_exact_never_calls_under_min`・`tt026_degraded_defers_to_legal_boundary` | #127+#190/#196 | Pass(headless+upstream) | 実 plugin min=128=NotRun・CLAP in-block=TT-027 |
| TT-027 | CLAP in-block Transport Event / no proven support → Capability 検証・規格存在≠保証 | `tim03_pb_precision_gates`（Verified のみ in-block）・`tim03_aw_connect_*` | `tt027_in_block_unproven_not_treated_as_supported`・`InBlockTransportSupport` | #127+#196 | Pass(部分) | 実 CLAP event=NotRun |
| TT-028 | Callback event at100/120/256 → 境界重複なし・ProcessOffset=0/20・256は次Callback 0 | `tim03_pb_offsets_zero_one_last`・`tim03_sched_events_boundary_and_stale_generation` | `tt028_event_process_relative_offsets`・`event_list_offsets_are_process_relative` | #127+#190 | Pass(headless+upstream) | 実 plugin event list=NotRun |
| TT-029 | L0/L1・Zero-frame flush・多周 Loop・Stateful Plugin → Block Gate・時刻順・Note Ledger・PDC/Tail・PlanGeneration 維持 | `tim03_node_*`・`tim02_loop_replay_*`・`tim03_node_param_event_per_loop_occurrence`・`tim03_process_boundary_rt` | `tt029_loop_occurrence_distinguishes_deliveries`・`tt029_zero_frame_flush_contract`・`tt029_l0_l1_min_frames_difference` | #127+#196 | Pass(部分) | 実 stateful plugin PDC/tail=NotRun |
| TT-030 | Offline exact vs explicit live degraded → Offline 無断丸め Fail・Live はユーザー選択精度のみ Receipt | `tim03_cap_offline_exact_fail_closed`・`tim03_aw_delivery_offline_*`・`tim03_aw_delivery_realtime_consent_degraded_clamped`・render `tim04_plugin_state` strict | `tt030_offline_exact_vs_live_degraded` | #127/#128+#190 | Pass(headless+upstream) | 実 offline render plugin=NotRun |

## Deferred Design Concerns

- 実 VST3/CLAP plugin・実 OS MIDI backend・実 device・
  実 GUI の TT gate 実測 | headless/mock 設計契約のみ |
  synvelis#114・EXT-L3・ruviform#253/#196 | 実装 Owner が
  headless Pass を実機受入と誤読する。
- SMF SMPTE Division/Format2（#129）・MIDI2/MPE/Per-note
  （#130） | 初期契約は明示 Unsupported/Deferred |
  synvelis#129/#130 | 将来拡張時に native timebase 契約の
  再確認が必要。
- Tempo Ramp・BeatPosition の整数幅/分母上限の最終値・
  Max Span/Event Budget の数値・Touch Return Time・
  Sample-accurate 対応の細部 | PoC で確定（architecture
  §13 未確定事項を継承） | #125〜128・ruviform#189/#190 |
  実装時に契約値が変わる可能性。
- Plugin Adapter ごとの正確な State Capture/Precision・
  MIDI1 同音高重なりと CC64 復帰の限界 | Capability/
  Best-effort として公開 | ruviform#196・synvelis#127 |
  「全 Plugin で Stuck Note ゼロ」等の過剰保証を防ぐ。
- AudioLens native f64 精度の非回帰 | 型レベル維持・
  実測は別 Gate | synvelis#294・ruviform#294 | f64
  consumer 経路の実測未実施。

## Sources and Relations

- `docs/specs/transport-integration.md`（TT-R-01〜15・
  TT-AC-01〜09）。
- `docs/designs/time-transport-midi-automation-plugin-
  integration-architecture.md`（v0.5 Approved — 14 合意
  項目・TT-001〜024・§14 TT-025〜030 補遺）。
- `docs/designs/configurable-ppqn-musical-time-architecture.md`
  （#19 Approved 方向）。
- `docs/designs/playback-mixer-generic-execution-graph-
  architecture.md`（v0.4 — Plan 世代・PDC・失敗分類）。
- TIM-01〜04 spec/design/test-spec/impl（#125〜128
  headless 実装済み slice）。
- `docs/specs/sf-03-*`・`docs/specs/pc-c4-07-*`・
  `docs/specs/automation-modulation.md`・
  `docs/specs/ctl-01-*`・`docs/specs/recording-take-comping.md`。
- Ruviform pinned `fd1d4d5`: `ruviform-media-core`/
  `ruviform-plugin-core`/`ruviform-plugin-host` の generic
  契約と TT-ID upstream test。
- 関連 Issue: synvelis #6/#12/#17/#18/#19/#20/#21/#22/#23/
  #98〜103/#106/#107/#109/#125〜130、ruviform #113/#184/
  #186〜190/#191/#196/#253/#294/#307/#309。
