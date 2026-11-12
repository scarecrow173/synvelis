---
id: "IMPL-034Wfpu0yifoZyGTZUA0pe"
type: "impl"
status: "completed"
title: "RCD-01 実機 lane 拡張 — real-device capture coverage"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "docs/specs/rcd-01-real-input-coverage.md"
      - type: "file"
        path: "docs/designs/rcd-01-real-input-coverage-design.md"
      - type: "file"
        path: "docs/plans/rcd-01-real-input-coverage-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-01-real-input-coverage-task.md"
      - type: "file"
        path: "docs/test-specs/rcd-01-real-input-coverage.md"
      - type: "file"
        path: "docs/impl/ir/rcd-01-real-input-coverage.md"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/real_input.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/rcd-01-real-input-coverage-task.md"
    - "docs/plans/rcd-01-real-input-coverage-plan.md"
    - "docs/designs/rcd-01-real-input-coverage-design.md"
    - "docs/specs/rcd-01-real-input-coverage.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-01-real-backend-readiness-slice.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-01-real-input-coverage.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/251"
  defers:
    - "physical USB 抜去・2ch 超 MultiChannel・cross-clock multi-device（#119）・macOS/Linux lane（ruviform#251 Stage M/L）・製品 E2E（#114）— 本 slice 対象外"
    - "PCM fidelity deterministic source（OS loopback 等）— deterministic source 不在のため NotRun 維持"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "pump を専用 scoped thread で駆動（本番非 RT pump thread の模型・test orchestration との timeshare 回避）"
      - "`thread::sleep` cadence + `REAL_CALLBACK_QUEUE=256`（~2.5 s stall 耐性）で負荷環境の scheduling stall を jitter 余裕として吸収"
      - "長時間 lane の continuity 系 fault（EPOCH/INPUT_DISCONTINUITY）を verified prefix 付き NotRun/blocked として分類（raw stream でも再現する device/driver 実挙動）"
    rejected:
      - "spin_loop / yield_now busy-wait pump（WASAPI worker thread を飢餓させ実 discontinuity を誘発）"
      - "queue-full の ambient 分類（誘発は専用 test の極小 queue=8 のみ — 他 test の QUEUE_FULL は hard fail のまま）"
---

# RCD-01 実機 lane 拡張 — real-device capture coverage

## Summary

`tests/real_input.rs` の opt-in 実機 lane を拡張し、実
`Device` → `InputSession`/`InputAdapter` → 実 `StreamWriter` /
staging → checkpoint/finalize → commit payload の end-to-end を
Audient iD4（WASAPI）で検証する lane へ拡張した（既存 2 test
に新規 8 test を追加し計 10 test）。`DeviceRig`
の構築経路を `&dyn Device` + 明示 `CaptureStreamId` 対応の
`RealDeviceRig` へ一般化し、fake rig は `open_rig_core` 経由で
温存した。RT queue-full の決定的誘発・format/rate fail-closed
reject・punch の sample 単位検証・~30 s 長時間 continuity 観測・
multi-device same-clock を実機でカバーし、deterministic PCM
source 不在の fidelity 行は正直に NotRun とした。

## Context

Issue #115 / RCD-01。先行 slice（`rcd-01-real-backend-readiness`）
で composition root と 2 test の実機到達を確認済みだが、lane は
InspectionWriter 止まりで coverage が不足していた。本 slice は
spec `docs/specs/rcd-01-real-input-coverage.md` の AC1–AC4 に従い、
実機 evidence を honest な Pass/NotRun/Unsupported/blocked 分類で
拡張する。編集範囲は `tests/real_input.rs`・`tests/common/mod.rs`
・必要時の `src/input.rs`/`src/real_input.rs` 最小変更と doc
chain に限定し、`mise.toml`/`Cargo.toml`/`Cargo.lock`/CI/
`synvelis-app/**` は不変。

## Implementation

### Changes

- `tests/common/mod.rs`: `DeviceRig::open` の構築部を
  `open_rig_core` へ抽出し `&dyn Device` + 任意の明示
  `CaptureStreamId` を受け付けるよう一般化。`RealDeviceRig`
  （`open`/`open_one`）を追加 — session pump・writer pump・
  staging area・writer endpoint を `DeviceRig` と同型で配線し、
  negotiated `sample_rate` で `finish_all` を実行する。
  `run_for` は `thread::sleep` cadence（Windows timer 粒度を
  コメントで明記）、`pump_all` に `RIG_DEBUG` env の `OnceLock`
  診断を追加。journal/media 検証 helper
  （`capture_of`/`all_journal_spans`/`span_obs`/
  `assert_session_coverage`/`assert_source_contiguous`/
  `span_frame_total`/`assert_prefix_contiguous`）と実機 PCM 用の
  構造検証 `verify_capture_integrity`（CRC・media offset 連続・
  span 範囲・有限値。deterministic 期待値比較は行わない）、
  `tmp_dir` を追加。
- `tests/device_recording.rs`: 独自 temp dir helper を共通
  `tmp_dir` へ置き換え。
- `tests/real_input.rs`: `real_lane` module を拡張。
  helper: `usable_input`/`negotiated`（loud SKIP）・
  `drive_and_finish`/`drive_observe`（pump を専用 scoped
  thread で駆動し fault bit を返す観測版）・`real_budget`
  （`REAL_MAX_BLOCK=8192`・chunk/filled=64）・
  `REAL_CALLBACK_QUEUE=256`（~2.5 s stall 耐性）・
  `assert_adapter_rejects`/`probe_device_rejects`・
  `real_spec`/`run_duration`/`assert_complete_real`。
  test 追加: deterministic PCM source NotRun・実 device →
  StreamWriter/staging e2e（`CaptureStream` を project へ登録し
  `build_payload` まで検証）・format/rate reject fail-closed・
  2-track（`ChannelMap::mono(0)`/`mono(1)` 別 stream）・RT
  queue-full fail-stop + verified prefix + capture-fault marker +
  `build_payload` recoverable 分類・punch 境界の sample 単位検証・
  ~30 s 長時間 capture（frame/journal 連続性・drift 観測記録、
  continuity 系 fault は verified prefix を検証して
  NotRun/blocked 記録）・multi-device same-clock。

### Decisions During Implementation

- pump を専用 scoped thread（`std::thread::scope`）で駆動 —
  本番の非 RT pump thread を模型化し、test thread の
  orchestration/断言との timeshare が work queue / chunk pool
  を誤枯渇させないようにした。
- `spin_loop`/`yield_now` の busy-wait は却下 — WASAPI worker
  thread を飢餓させ device/driver の position discontinuity を
  誘発することを実測で確認（raw stream 35 s probe で adapter/
  pump 無しでも `ClockDiscontinuity` が発生）。`thread::sleep`
  cadence + 大容量 queue（`REAL_CALLBACK_QUEUE=256`）で jitter
  を吸収する方針へ変更。容量は plan/spec パラメータであり
  fault 経路は armed のまま。
- 長時間 lane は `drive_observe` で fault bit を観測し、
  `FAULT_EPOCH_DISCONTINUITY`/`FAULT_INPUT_DISCONTINUITY` のみ
  の場合は journal prefix 連続性・source 連続性・media CRC・
  有限 PCM を検証した上で「この device/driver では bounded
  gap-free capture が達成不能」として NotRun/blocked を記録。
  continuity 以外の bit は harness/系の欠陥として hard fail。
- continuity 系 fault の verified-prefix NotRun/blocked 分類
  は長時間 lane だけでなく punch 境界 test にも適用 — 短時間
  capture でも同じ device/driver discontinuity が発火し得る
  ことを実機 run で確認したため、`continuity_fault_prefix`
  helper に共通化し、prefix 検証後に NotRun/blocked を記録
  する。sample-exact 検証には gap-free capture が必要であり、
  faulted prefix での部分 Pass は主張しない。
- e2e の clean commit path は `build_payload` が要求する
  `project.recording.streams` 登録を本番 `RegisterStream` op
  相当の `CaptureStream` 挿入 + rig への明示
  `CaptureStreamId` 指定で満たした（faulted path は
  capture-fault marker が payload 構成を short-circuit する
  ため登録不要）。
- queue-full 誘発は `callback_queue=8` + pump 停止で決定的に
  `FAULT_QUEUE_FULL` を起こし、fault 後の journal 非増加・
  prefix 連続性・durable marker・recoverable 分類を検証。
  誘発失敗は fail（silent pass にしない）。

## Related Experiments

### Adopted

- 専用 pump thread + sleep cadence + 大容量 in-flight queue
  （負荷環境 flake の実測に基づく採用）。
- continuity 系 fault の verified-prefix NotRun/blocked 分類
  （raw stream での再現確認に基づく採用）。

### Rejected

- busy-wait pump（WASAPI worker 飢餓 → 実 discontinuity 誘発）。
- ambient QUEUE_FULL の NotRun 分類（queue-full は専用誘発
  test のみが権威 — 他 test では hard fail を維持）。

## Validation

- `mise run fmt` / `fmt-check`: pass。
- `mise run lint-recording-real-input`: pass（real-device feature・
  clippy `-D warnings`、全 target）。
- `mise run test-recording-real-input`: 実機 lane（WASAPI・
  Audient iD4 "Analogue 1/2 (2- Audient iD4)"、negotiated
  44100 Hz stereo F32）— **10/10 pass**（~70 s）。
  - Pass: input adapter・timestamp probe・2-track（mono(0)/
    mono(1) 別 stream）・実 device → StreamWriter/staging e2e
    （`CaptureStream` 登録 → `build_payload` commit）・format/
    rate reject fail-closed・RT queue-full fail-stop（verified
    prefix + durable marker + recoverable 分類）・punch 境界
    sample 単位検証・deterministic PCM source の NotRun 記録。
  - multi-device same-clock: `NotRun`（同一 clock domain の
    第 2 input device が環境に不在 — cross-clock は #119 範囲）。
  - 長時間 ~30 s lane: nocapture run で clean Pass を実測
    （1,323,000 frames = 30.00 s @ 44100 Hz・3000 chunk/span・
    journal/media 連続・fault 0）。別 run では device/driver
    の position discontinuity（`FAULT_EPOCH_DISCONTINUITY`）
    を観測 — raw stream probe でも再現する実挙動のため
    verified prefix を検証した上で NotRun/blocked 記録
    （fail-stop + prefix 保持は確認済み）。
  - punch lane も同じ discontinuity を観測した run があり、
    同一分類（`continuity_fault_prefix`）を適用。gap-free
    capture が取れた run では sample-exact Pass を確認済み。
  - 補足: 長時間 capture 駆動中に iD4 が Windows の default
    input device 列から脱落する事象を観測（raw probe 35 s で
    `ClockDiscontinuity` 2 件と符合）。脱落後の run では全
    hardware test が loud SKIP（"no default input device"）
    で non-failing 終了することを確認 — hardware 不在契約の
    実証。
- headless 回帰: `common/mod.rs` は fake `DeviceRig` 経路を
  保持（`open_rig_core` 抽出 + `&dyn Device` coerce のみ）、
  `real_input.rs` の `real_lane` module は `real-device`
  feature 配下。`cargo test -p synvelis-recording --locked`
  （全 test target・feature 無し）全 pass — fake rig・
  device_recording・input_adapter 各 suite に回帰なし。

## Risks

- 実機 lane は timing sensitive で、並行 build 等の高負荷下では
  `FAULT_QUEUE_FULL`/`FREE_EXHAUSTED` が ambient に発火し得る
  （`REAL_CALLBACK_QUEUE=256` で ~2.5 s stall 耐性を確保済みだが
  無制限ではない）。
- iD4/WASAPI は長時間 capture で position discontinuity を
  発生させ得る（raw stream で再現確認済み）。fail-stop と
  verified prefix 保持は正しい挙動であり、lane はこれを
  NotRun/blocked として正直に記録する。
- PCM fidelity は deterministic source がないため NotRun —
  OS loopback source が利用可能になれば再検証可能。

## Follow-ups

- 実機 lane の最終結果（Pass 数・NotRun/blocked 行・実測値）を
  Issue #115 コメントと PR に記録する。
- iD4/WASAPI の長時間 discontinuity は device/driver 側の
  実挙動として upstream（ruviform#251 系）への報告候補。
