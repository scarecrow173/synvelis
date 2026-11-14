---
id: "TSPEC-a6ZCHY6LRL2KeFER0sLwl7"
type: "test-spec"
status: "approved"
title: "Recording / Take / Comping — Fixture 契約・受入検証仕様（synvelis#12）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/recording-take-comping.md"
    - "docs/designs/recording-take-comping-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/recording-take-comping.md"
    - "docs/designs/recording-take-comping-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/recording-take-comping.md"
    - "docs/tasks/recording-take-comping-task.md"
    - "evidence/sv12/owner-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/recording-take-comping-design.md"
  derived-by: []
  verifies:
    - "docs/specs/recording-take-comping.md"
    - "docs/designs/recording-take-comping-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/synvelis/issues/116"
    - "https://github.com/scarecrow173/synvelis/issues/117"
    - "https://github.com/scarecrow173/synvelis/issues/118"
---

# Recording / Take / Comping — Fixture 契約・受入検証仕様（synvelis#12）

## 目的

Issue #12 の基本契約に対し、各 Feature が満たすべき Rule・検証 Example・Guarantee を固定する。実装は #115〜#118、実機/GUI/Plugin Gate は #114/EXT-L3。本書は「何を保証すべきか」を定義し、実装方法・数値を確定しない。

## 状態語彙

`POLICIES.md` 準拠。Requirement=`Required`/`Deferred`/`NotApplicable`、実行結果=`Pass`/`Fail`/`NotRun`/`Unsupported`。`Pass (headless)` は headless/unit 契約の実証であり、製品 GUI・実機・実 Device・実 Plugin Pass ではない。`evidence/sv12/owner-matrix.md` が各 AC の現在状態の正本。

## Feature 群

### F-CAPTURE — Capture/Stream/Segment/Source Frame（AC-01〜05, 15〜18, 49, 57, 60〜62, 64）

- **Rule**: Capture Stream は 1 入力/Lane/Clock Epoch の物理系列。Segment は半開 `[start,end)` で Session⇔Source⇔Placement⇔Clock/Format⇔整合性を解決する。Source Frame は Writer 検証済み実保存 PCM のみから確立し、RT queue 受理だけでは確立しない。同 rate・無 gap の正常 Segment 内でのみ Session duration = Source duration。Punch/Writer gap/stream 変更を跨ぐ連続性は偽装しない。
- **Example**: `punch_in_at_frame_113_of_257_block`（block 途中境界の半開分割）、`three_loop_passes_and_wrap`（Loop 3 Pass・Session Frame 単調増加・Project 区間同一）、`epoch_discontinuity_latches_and_no_fabrication`（clock 不連続で latch・捏造なし）、`normal_recording_exact_pcm_and_tiling`（PCM bit-exact）。
- **Guarantee**: 同一 Sample/Event の二重帰属なし・gate 外録音なし・未 Arm は録音なし・複数入力の同 Clock 相対同期。

### F-WRITER — Writer/Staging/Journal/Recovery/Commit（AC-09, 12〜14, 22〜23, 34〜35, 58, 63, 65〜67）

- **Rule**: `Staging→verified ranges→Journal Checkpoint→Finalize→Project Controller atomic reference mutation`。OS write 成功と durable sync は別。crash recovery は verified/synced frontier を識別し未検証 tail を finalize しない。Commit identity で retry 冪等、Undo は元 media を即削除しない。Stream/Segment 別 Valid/Invalid/Discontinuous と Journal を比較検証可能。
- **Example**: `vsc_recover_last_checkpoint`、`vsc_unverified_tail_never_finalized`、`vsc_finalize_idempotent`、`vs_d_crash_before_commit_retry`/`vs_d_crash_after_commit_retry_no_dup`、`rcd02_unsynced_loss_truncates_to_sync_frontier`、`rcd02_fault_matrix_journal_short_write_torn_append`、`recording-poc` `vsc_process.rs::vsc_process_kill_then_recover`・`vsd_process.rs::vsd_process_e2e_deterministic`。
- **Guarantee**: 復旧可能区間を失わない・二重 Asset/Take/Clip なし・欠落/部分録音を識別・P0 Project 管理に収束・VCS 未接続でも独立録音/再生。

### F-MONITOR — Monitor/Punch/Loop/Latency（AC-06〜07, 33, 49〜56）

- **Rule**: Monitor Off/Auto/On・arm・transport・punch gate・mute/solo・hardware direct facts は raw capture と独立。Monitoring 状態は raw bytes/span を変えない。Hardware direct は `DeviceControlled`/`UserReported`/`Unknown` を区別し Unknown/double は notice。Latency は measured/reported/estimated/unknown・clock epoch を区別し二重加算しない。配置補正は native placement で raw を破壊しない。Gain Ramp は録音 sample を増減しない。
- **Example**: `raw_capture_bytes_are_identical_across_monitor_states`、`raw_capture_spans_identical_across_monitor_states_with_punch`、`monitor_does_not_double_correct_placement_across_tempo_boundary`、`decide_auto_without_arm_never_routes_input`/`decide_on_requires_verified_io_and_ignores_arm_gate`（monitor_gate route model）、`monitor_arm.rs` E1 route matrix、`monitor_punch_session.rs`（sv#117 fixture）。
- **Guarantee**: 表示と可聴経路の一致（可聴は GUI-L2/EXT-L3 Deferred）・Direct Monitoring を制御済みと偽装しない・未知/stale を sample accurate と表示しない。

### F-RT — RT/Handoff 契約（AC-11, 37, 48, 63, 68）

- **Rule**: RT Callback は bounded 境界 event のみ emit。Project Registry/Undo 変更・動的確保・lock・UI IPC・Project 変更・OS I/O 禁止。handoff は固定容量 queue・明示 Filled/Free ownership・queue-full は block せず fail-stop。Heap-owned buffer を RT で Drop しない。Non-RT Prepare→RT-safe Publish。
- **Example**: `rt_callback_is_alloc_and_drop_free`、`rt_path_is_alloc_and_drop_free`、`aut_04_rt.rs::record_event_rt_alloc_free_bounded_and_typed`、`queue_full_failstop_and_chunk_retained`、`free_pool_exhaustion_failstop`、`fake_rt_queue_full_fails_stop`。
- **Guarantee**: alloc/block/OS I/O/危険 Drop を計測検出・Mock 合格を全実 Backend 保証としない。

### F-SESSION — Session/Pass/Take/Take Group 運用（AC-03, 05, 10, 15〜16, 19, 21, 25〜26, 36, 62, 64）

- **Rule**: Pass execution state（Scheduled/Active/Closed）と result（Complete/Partial/Failed/Empty）は別軸。1 Session 複数 Track・二重 Record・複数 Window・録音中危険編集を安全に扱う。Track Arm/Input/Monitor/Transport Record/Automation Write Arm は独立。状態 Preflight→Capturing→Stop→Finalizing→Ready を区別。View/Window/Dock/focus 変更だけで target/undo を誤変更しない。
- **Example**: `vs_d_loop_three_passes`、`vs_d_two_track_two_ch_clock`、`same_clock_domain_two_streams_capture_together`、`mismatched_clock_domain_is_rejected`、`arm_off_monitor_on_creates_no_writer_and_captures_nothing`、`recording.rs` `session_state_machine_legal_path_only`/`registration_rejected_before_open`、`uic5_02`/`uic5_04` stale/window epoch、`rcd_04_product_e2e.rs::rcd04_stale_gesture_and_readonly_history_product_e2e`。
- **Guarantee**: Pass 3 件・Session Frame `[0,384000),[384000,768000),[768000,1152000)`・各 Take が対応 Pass 参照・revision 競合を検出。

### F-COMP — Comp/Take Group/Editor/MIDI（AC-08, 20, 24, 27〜32, 38〜47）

- **Rule**: Comp は指定候補区間のみ再生・元 Take/Source 非破壊。Region 非重複・Transition は 2 有効 Source Handle 参照・境界編集で Source 不変。Grouped Comp は全 Track 同期検証・単一 Undo。MIDI Comp は event/voice-state 評価（PCM mixing でない）・raw event/timestamp 不変・boundary action は virtual Note Off/controller 合成・raw を削除/短縮しない。`Effective Comp`/`Take Source`/`Compare` 別 mode・Raw Capture version 0 直接編集不可・gesture fixation で stale 拒否。Take Click/Audition/表示切替は Comp 採用/Revision/Undo を変えない。
- **Example**: `md_a.rs` comp 選択/遷移/handle 検証、`rcd_04_comp_cas.rs`（single CAS undo・raw immutable・group CAS reject no partial）、`rcd_04_take_comp_pcm.rs::rcd04_comp_pcm_rt_vs_offline_fixture`、`rcd_04_midi_phrase.rs`（native tick・no requantize・stable id undo/redo/save/load）、`rcd_04_midi_phrase_boundary.rs`（boundary action）、`tim_02_midi_phrase.rs::assert_no_hanging_notes`、`midi_source.rs`（`linked_clips_share_source_edits_and_detach_is_explicit`/`undo_redo_cover_note_edits_and_detach`）、`md_a_06_editor_targets_are_type_level_distinct`。
- **Guarantee**: CC64/sustain chase・no stuck note・seek/loop/stop/comp 切替の voice/CC state を Mock Plugin で検証・未対応 plugin 能力は制限明示・crossfade は SourceRange/Integrity Validation 必須。

### F-RECOVERY-UI — Recovery/Session 運用表示（AC-26, 28〜29, 33〜36, 39, 50, 52〜54, 56）

- **Rule**: Recovery UI は Verified/Partial/Invalid と実際の無音を区別・検証/取込/保留/破棄を安全に。Take Lane/Comp Editor の view-local state は project provenance と分離・Active Comp/Undo を変えない。Audition 終了で元状態復帰。二重監視・unknown hardware direct は notice。低レイテンシ/配置補正は raw を破壊しない。
- **Example**: `recovery_review.rs`（adopt/hold/discard idempotent）、`rcd_04_recovery_partial.rs`（group commit separates partial/held）、`monitor_gate.rs` hardware_direct_notices、`monitor.rs` latency 品質、`recording_projection.rs`（`syn.recording.info`）、`ed_a_05` link 非干渉。
- **Guarantee**: 実 window/OS/Device/Plugin の表示・可聴・audition は GUI-L2/EXT-L3 Deferred。headless は route model/projection/non-mutation 契約のみ。

## 非対象（Non-goals）

- 製品実装・実機/GUI/Plugin 受入（#115〜#118・#114・EXT-L3）。
- Creative Branch Take/Comp 候補（#278/#279/#288）。
- 詳細 schema・queue 容量・checkpoint 間隔・comp 既定値・UI 配置の数値確定（後続 PoC）。
- Independent Device Clock 正確さ（#119 RCD-F01）。
- Rust build/test — docs-only slice のため `NotRun`/`NotApplicable`。

## Fixture ↔ AC 対応

各 AC の具体 fixture・Owner Issue・OS Gate・現在状態は `evidence/sv12/owner-matrix.md` を正本とする。本書は Feature/Rule/Example/Guarantee の契約構造を固定し、個別 fixture 名の網羅は owner-matrix に委譲する（二重管理を避ける）。
