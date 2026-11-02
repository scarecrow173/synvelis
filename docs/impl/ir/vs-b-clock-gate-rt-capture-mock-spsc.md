---
id: "IMPL-pQbrnczYyJPOw79LzmmIy"
type: "impl"
status: "completed"
title: "VS-B Clock/Gate/RT Capture — Mock Input・Buffer Pool・SPSC (E2E-R1)"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/src/bounded.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/plan.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/clock.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/gate.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/capture.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/handoff.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/engine.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/mock.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/vs_b.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/vs_b_rt_alloc.rs"
      - type: "file"
        path: "docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md"
      - type: "file"
        path: "docs/adr/vs-b-ruviform-rt-spsc-gate-schedule-fault-ownership.md"
      - type: "file"
        path: "docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md"
      - type: "file"
        path: "docs/plans/vs-b-clock-gate-rt-capture.md"
      - type: "file"
        path: "docs/test-specs/vs-b-e2e-r1-mock-gate-rt-capture.md"
      - type: "file"
        path: "docs/tasks/vs-b-task1-deps-bounded-plan-clock.md"
      - type: "file"
        path: "docs/tasks/vs-b-task2-gate-schedule.md"
      - type: "file"
        path: "docs/tasks/vs-b-task3-capture-handoff-engine.md"
      - type: "file"
        path: "docs/tasks/vs-b-task4-mock-driver-writer.md"
      - type: "file"
        path: "docs/tasks/vs-b-task5-poc-acceptance-tests.md"
      - type: "file"
        path: "docs/tasks/vs-b-task6-verification-merge-impl-record.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-recording/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-recording/src/lib.rs"
      - type: "file"
        path: "crates/recording-poc/src/main.rs"
      - type: "file"
        path: "crates/recording-poc/tests/poc_determinism.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/recording.rs"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements:
    - "docs/plans/vs-b-clock-gate-rt-capture.md"
    - "docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md"
    - "docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md"
    - "docs/adr/vs-b-ruviform-rt-spsc-gate-schedule-fault-ownership.md"
    - "docs/tasks/vs-b-task1-deps-bounded-plan-clock.md"
    - "docs/tasks/vs-b-task2-gate-schedule.md"
    - "docs/tasks/vs-b-task3-capture-handoff-engine.md"
    - "docs/tasks/vs-b-task4-mock-driver-writer.md"
    - "docs/tasks/vs-b-task5-poc-acceptance-tests.md"
    - "docs/tasks/vs-b-task6-verification-merge-impl-record.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VS-B Clock/Gate/RT Capture — Mock Input・Buffer Pool・SPSC

## Summary

Mock Clock/Input の決定的 frame 列を Preflight → Prepared Gate
Schedule → RT Capture → 事前確保 SPSC Handoff で扱う E2E-R1 縦切片。
Normal/Punch/Loop の正確な保存候補と、Overflow 時の非ブロック
fail-stop を `synvelis-recording` 内に実装した。

## Context

Synvelis Issue #14 (VS-B)。前提 VS-A (#13, PR #29) の時間型・Stable
ID・Project 基盤を再利用し、ruviform `ruviform-rt` の既存 SPSC を
採用。ruviform #187 (実 Device Input) / #190 (Span Scheduler) は
前提にしない Mock-first 証明。

## Implementation

### Changes

- `bounded.rs`: `unsafe_code` forbid の下で `Vec::with_capacity` を
  構築時 1 回だけ確保する `BoundedVec`。`head` カーソル消費で RT の
  memmove を回避。`push` は満杯時 `BoundedFull(T)` で payload を返す。
- `plan.rs`: `RecordingRequest`/`GateSpec`/`BufferBudget`/
  `PreparedRecordingPlan`/`preflight`。参照・容量・TempoMap
  revision/PPQN・gate 半開検証・punch_out クランプ・
  `max_block_frames <= chunk_frames`・`filled_capacity <= chunks`。
- `clock.rs`: `InputBlock`（epoch + first_input_frame + frames）。
- `gate.rs`: `GateScheduler` が gate を pass 基準 frame offset の
  template 列へコンパイル（非 RT）。tempo 境界で分割し、Loop pass は
  同一 template の平行移動。`GateSchedule` = 有界 window +
  `scheduled_until` watermark + 生成カーソル。`evaluate_gate`（RT）は
  engine 所有 scratch へ `CapturePiece` を書き出す。
- `capture.rs`: `CaptureChunk`（id/stream_index/header/validity/
  spans/pcm/stored_*）・`CaptureSpan`・`CaptureBlockHeader`・
  `ChunkValidity`。再利用時は `reset()` で spans・カウンタを初期化。
- `handoff.rs`: `FaultLatch`（AtomicU32 side channel、満杯 Queue 非依存）
  + fault bit 定数 10 種 + `build_handoff`（pool 一括確保し全 chunk を
  free へ）。
- `engine.rs`: `CaptureEngine::process` の RT パス。検査（epoch/input
  baseline 採種・連続性・PCM 長）→ Gate 評価 → free pop → PCM/span
  書き込み → filled push → cursor 前進。gate 閉塞 block でも
  session/input cursor を必ず進める。失敗 push 返却 chunk と部分
  書き込み chunk は `retained` へ move（RT dealloc なし）。
- `mock.rs`: `MockPcm`（frame/channel/seed の純関数）、`MockInput`、
  `InspectionWriter`（hold 注入）、`Scenario`/`StreamRig`/
  `run_to_end`/`feed_engine_only`/`recover_all`/`verify_log`。
- `tests/vs_b.rs`: 15 受け入れテスト（Normal tiling+PCM 検算・
  257-frame block 内 punch in=113・3 loop pass+wrap・部分終 pass・
  PunchInLoop・tempo 境界・負 pre-roll・epoch/input 断絶・
  queue full/free 枯渇/span overflow fail-stop・underrun・
  SPSC 1:1・回収照合）。
- `tests/vs_b_rt_alloc.rs`: `stats_alloc` グローバルアロケータで
  process の allocations/deallocations = 0 を計測（通常・queue full・
  gate closed の 3 パス。warm-up 済み。並列テスト汚染を避けるため
  専用バイナリの単一テスト）。
- `recording-poc`: VS-B 3-pass loop シナリオを run() に追加し
  chunks/spans/frames/pass_sum/fault/recovered/checksum を決定的出力。
- VS-B doc chain: spec/ADR/design/plan/test-spec/6 task + index 再生成。

### Decisions During Implementation

- 1 block > chunk_frames は preflight が `max_block_frames` で
  排除する契約とし、超過時は `PCM_OVERFLOW` で fail-stop（複数 chunk
  分割は VS-B では持ち越し）。
- `GateClosed` validity は契約値として残すが、全区間閉塞 block は
  chunk を生成しない（長い閉塞区間で pool を枯渇させない）。
- scheduler は無状態化し、生成カーソルを `GateSchedule` 側へ持たせて
  stream ごと独立化。
- `scratch` は `mem::replace` で engine から取り出して fill ループの
  借用を解決（`with_capacity(0)` は非割当て）。

## Related Experiments

### Adopted

None — 設計レビューで 2 reviewer の P1/P2 指摘を全て設計反映済み。

### Rejected

None.

## Validation

- `mise run fmt-check` / `check` / `lint` / `test` / `build`: 全 pass。
- `mise run test`: vs_b 15/15・vs_b_rt_alloc 1/1・既存テスト全 pass。
- `recording-poc` 2 回実行で stdout byte-identical（vsb_frames=
  1152000, vsb_recovered=8, vsb_checksum 固定）。
- `git fetch origin && git merge origin/main`: already up to date。
- docs index 再生成: `doc_maintenance.js apply` で 6 README 再生成。

## Risks

- `PCM_OVERFLOW` は `max_block_frames <= chunk_frames` 契約で到達不能な
  防御 bit（契約違反検出用）。
- Mock 証拠は real backend (#187/#190) の検証を主張しない。

## Follow-ups

- Issue #15: Writer/Journal が `CaptureChunk`/`CaptureSpan`/
  `ChunkValidity`/`FaultLatch` を消費する。
- `retire_channel` 系の汎用 retire 需要が出たら O-3 を再評価。
