---
id: "TSPEC-dtmF30GppYRVULWnH8CG9o"
type: "test-spec"
status: "approved"
title: "VS-B E2E-R Gate 検証仕様 — Mock Gate・Buffer Pool・SPSC・Fail-stop"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source: []
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
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md"
  derived-by: []
  verifies:
    - "docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VS-B E2E-R Gate 検証仕様 — Mock Gate・Buffer Pool・SPSC・Fail-stop

## Purpose

spec（VS-B）の全 Acceptance Criteria を実テストへ 1:1 に落とす正本。
全テストは headless・決定的・実デバイス非依存で、E2E-R（R1/R2）Gate の
完了判定の検証根拠となる。Mock の合格を実 Backend の時刻精度・RT 安全性
の証拠と混同しない。

## Feature

`RecordingRequest→preflight→PreparedRecordingPlan`、Prepared Gate
Schedule（半開・有界 Lookahead）、RT `CaptureEngine`、事前確保
Buffer Pool と filled/free 2 本の SPSC、Atomic Fault Latch + Fail-stop、
非 RT `InspectionWriter` と退役照合。

## Rules

- `SessionFrame` は pre-roll 中を含め単調増加、Fault/Fail-stop 後も
  巻き戻さない。`SourceFrame` は有効 capture 区間のみ進む（stream 内で
  [0, stored_total) の連続・重複なし）。
- Gate は半開 `[in,out)`、同一 sample は一意の pass/span/piece にのみ
  帰属し、gate 外は保存しない（無音合成しない）。
- SPSC Producer/Consumer は 1:1 所有。`try_push` 失敗返却の heap-owned
  chunk は RT で Drop せず `retained` 固定 slot で退役まで保持する。
- Fault は QUEUE_FULL / FREE_EXHAUSTED / SPAN_OVERFLOW /
  SCHEDULE_UNDERRUN / EPOCH_DISCONTINUITY / INPUT_DISCONTINUITY /
  WRITER_ERROR の bit latch。Fail-stop 後の callback は有界 no-op。
- 退役時に retained + free drain + filled drain + writer 保持の合計が
  pool 総数と一致（id 重複なし）。
- 全ての frame 演算は checked API 経由で panic しない。

## Examples（AC → test 対応表）

| Spec AC | テスト（crate/tests file::test name） |
| --- | --- |
| AC1 Normal 全 sample 一致・順序・重複・欠落なし | `tests/rt_capture.rs` `normal_recording_all_samples_match`（複数 block・stereo・source frame 連続 bitmap・sample=gen(frame,ch,seed)） |
| AC2 Block 途中 Punch In/Out（257F・in=113） | `tests/rt_capture.rs` `punch_mid_block_boundaries`（保存 144F・packed 長・session/source 対応・gate 外非保存） |
| AC3 3 周 Loop（120BPM/4/4/48k/4 小節） | `tests/rt_capture.rs` `loop_three_passes_session_monotonic`（session [0,384000)×3・pass_index 0/1/2・project [0,15360) 一致） |
| AC4 Punch+Loop・変動 Block・Tempo 境界・負 pre-roll の半開一意 | `tests/rt_capture.rs` `punch_in_loop_mixed_boundaries`・`tempo_boundary_split_unique`・`preroll_negative_project` |
| AC5 Queue Full・Free 枯渇・Span 容量超過が非 Blocking Fail-stop・RT 上で Heap Drop/alloc/OS I/O なし・retained 保持・退役照合 | `tests/rt_fault.rs` `queue_full_fail_stop_retained`・`free_exhaustion_fail_stop`・`span_overflow_fail_stop`・`retire_accounting_total`・`post_fault_bounded_noop` |
| AC6 Clock Epoch 断絶・入力 frame 不連続を報告・捏造なし | `tests/rt_fault.rs` `epoch_discontinuity_fail_stop`・`input_frame_gap_fail_stop` |
| AC7 SPSC 1:1 所有・退役時全 Buffer 回収 | `tests/rt_fault.rs` `spsc_single_ownership_accounting`（pool 総数照合・id 一意）＋型レベル所有（endpoint の move） |
| AC8 RT 無割当・禁止 Drop を計測 | `tests/rt_safety.rs` `rt_path_no_alloc_no_dealloc`（stats_alloc Region で正常+fault 経路の alloc/dealloc=0）※ 単独 test binary で並行 noise 排除 |
| AC9 `recording-poc` 決定的実行・2 回同一出力 | `tests/poc_determinism.rs`（bin 2 回実行→同一 stdout、vsb summary 含む） |
| AC10 #15 が消費できる Chunk/Sequence/Validity/Fault 契約 | `tests/rt_contract.rs` `chunk_contract_fields`（header sequence/epoch/input range/spans/validity/PCM packed 長の一貫性・sequence 単調） |
| preflight 検証（requirements） | `tests/plan.rs` `preflight_rejects_invalid`（不存在 lane/epoch・逆転区間・容量 0・tempo compile 不可を拒否し部分適用なし） |

## Guarantees

- 全テストは実デバイス・乱数・時刻・ネットワーク非依存（git 依存の
  初回 fetch を除く）。
- fault 注入（queue full・free 枯渇・span 超過・underrun・epoch 断絶・
  writer return 異常）は deterministic script で再現する。
- RT 計測は `stats_alloc` の global 計測を専用 test binary に限定して
  並行テストの noise を排除する。計測不能な環境では「計測済み」と
  記載しない。
- Fault 経路でも chunk 所有権総数が保存される（退役照合）。

## Non-goals

- 実 Writer/Journal/Staging/ファイル永続化・Project Commit/Recovery
  の検証（VS-C/VS-D）。
- 実 Device/Backend の時刻精度・xrun 検出・製品版全環境の RT 安全性
  保証（Mock の成功を実 Backend の証拠にしない）。
- 複数 Clock Domain の Drift/切替継続録音。
- 性能閾値の検証（計測結果記録のみ）。

## Risk

- これらの保証を落とすと、VS-C が信頼できない Chunk/Sequence/Fault
  契約の上に構築され、E2E-R/D の前提が壊れる。特に「RT での危険
  Drop」「失敗 push の所有権喪失」はリカバリ不能な媒体欠落に直結する。
