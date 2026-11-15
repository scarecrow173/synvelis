---
id: "IMPL-5XHxD8MXj3eXsJmWlbQFBE"
type: "impl"
status: "completed"
title: "Issue #115 rf#628 consumer oracle — synvelis#115 slice"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/tests/rf628_oracle.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/rf628_oracle_rt_alloc.rs"
      - type: "file"
        path: "docs/specs/issue-115-rf628-consumer-oracle.md"
      - type: "file"
        path: "docs/designs/issue-115-rf628-consumer-oracle.md"
      - type: "file"
        path: "docs/plans/issue-115-rf628-consumer-oracle.md"
      - type: "file"
        path: "docs/tasks/issue-115-rf628-consumer-oracle.md"
      - type: "file"
        path: "docs/test-specs/issue-115-rf628-consumer-oracle.md"
      - type: "file"
        path: "docs/impl/ir/issue-115-rf628-consumer-oracle.md"
      - type: "file"
        path: "docs/impl/exp/issue-115-rf628-consumer-oracle.md"
      - type: "file"
        path: "evidence/sv115/issue-115-rf628-consumer-oracle-verification.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-recording/Cargo.toml"
      - type: "file"
        path: "evidence/sv115/rcd-01-virtual-l1-oracle-case-matrix.md"
      - type: "file"
        path: "evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-115-rf628-consumer-oracle.md"
    - "docs/specs/issue-115-rf628-consumer-oracle.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-115-rf628-consumer-oracle.md"
    - "docs/plans/issue-115-rf628-consumer-oracle.md"
    - "docs/test-specs/issue-115-rf628-consumer-oracle.md"
    - "docs/impl/exp/issue-115-rf628-consumer-oracle.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-115-rf628-consumer-oracle.md"
  derived-by: []
  verifies:
    - "docs/test-specs/issue-115-rf628-consumer-oracle.md"
  verified-by:
    - "docs/test-specs/issue-115-rf628-consumer-oracle.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-115-rf628-consumer-oracle.md"
    rejected: []
---

# Issue #115 rf#628 consumer oracle — Implementation Record

## Summary

`synvelis#115` の `rf#628-fixture` lane consumer oracle slice。
ruviform pin を `ac10a2f7` → `abb9f372`（`crates/ruviform-audio-test`
の `pipeline/` 収録済み最新 main）へ更新し、`ruviform-audio-test` を
workspace dep + `synvelis-recording` dev-dep として追加。
`ScriptedBackend`/`PcmScenario`/`StreamFault`/`VirtualClock`/`oracle`
を本番 `InputSession`/`InputAdapter` → `StreamWriter` → staging
media/journal → `commit_session` → save → reload 経路へ通し、
独立 `pcm_reference_block` oracle で SV-1〜SV-6 を実測した。
production `src/`・`tests/common/mod.rs`・既存 test file は非変更。

## What was implemented

- **pin/dep**: `Cargo.toml` `[workspace.dependencies]` に
  `ruviform-audio-test` 追加、`crates/synvelis-recording/Cargo.toml`
  `[dev-dependencies]` に `ruviform-audio-test.workspace = true`。
  `cargo update -p ruviform-*` で 28 package を単一 SHA
  `abb9f372d3ee95ee824b3c4f7dc68e8083ce2774` に再解決（lockfile
  差分は ruviform source fragment の SHA 更新 + 新規 package のみ）。
- **`tests/rf628_oracle.rs`**（9 test）: `rf628_record`（scripted
  device → `RealDeviceRig::open` の `&dyn Device` 経路 → drive →
  finish）、`drive_session`（`BLOCK=500` 刻み・末尾は `Frames` fault
  で厳密一致・`VirtualClockDriver` 追従）、`verify_scenario_media`
  （journal span → segment 単位 `pcm_reference_block` →
  `pcm_assert_eq`/`Exact`/`AbsEpsilon(1e-5)`）、
  `assert_scenario_stream`/`assert_faulted_prefix`（digest 連鎖・
  `verified_sha256` `sha2` 再計算・recoverable 分類）。
  - SV-1 `rf628_stereo_scenario_commit_reload`: Noise→Impulse→Sine
    3 segment stereo E2E + `PumpReport` metadata 断言。
  - SV-2 `rf628_multichannel_two_mono_taps_commit_reload`: 4ch→
    mono(1)/mono(3) tap 分離。
  - SV-3 `rf628_two_devices_same_clock_commit_reload` +
    `rf628_clock_domain_mismatch_rejected`。
  - SV-4 `rf628_pcm_gap_verified_prefix`: scenario gap→Xrun→
    `FAULT_INPUT_DISCONTINUITY`→verified prefix。
  - SV-5 `rf628_frame_position_fault_verified_prefix` /
    `rf628_clock_discontinuity_error_fault` /
    `rf628_open_fault_rejected_then_recovers` /
    `rf628_device_vanish_fails_stop`。
- **`tests/rf628_oracle_rt_alloc.rs`**（1 test）:
  `rt_scripted_input_callback_is_alloc_and_drop_free` — warm-up 後の
  `pump_input_next` 1 回分を `Region` で計測し allocations/
  deallocations = 0 を断言。
- **evidence/doc chain**: slug `issue-115-rf628-consumer-oracle`
  の spec/design/plan/task/test-spec/impl-ir/impl-exp +
  `evidence/sv115/` 更新。

## Decisions made during implementation

- `tests/common/mod.rs` への glue 追加は不要と判断 —
  `RealDeviceRig::open` が既に `&dyn Device` を受け、
  `ScriptedDeviceHandle`（`Clone` で state 共有）がそのまま使えた。
- delivery は `pump_input_next` が callback 発火を兼ねるため
  fake `deliver` とは別経路 — 共通化せず `rf628_oracle.rs` 内に
  閉じた（design D3）。
- `session_frames` は plan から実行時計測し、scenario 被覆は
  固定 `SCENARIO_END=16384`（PCM 事前生成なしのため余剰に
  コストなし）、delivery 端数は `StreamFault::Frames` で厳密一致。
- `expect_err` が `TapWriterEnds`/`InputSession` の `Debug` 非実装で
  使えないため `let...else` に変更（clippy `manual_let_else` 準拠）。

## Verification performed

- `cargo test -p synvelis-recording --test rf628_oracle
  --test rf628_oracle_rt_alloc` — 10 pass / 0 fail（補助証拠）。
- `mise run fmt-check` / `check synvelis-recording` /
  `lint synvelis-recording` / `test synvelis-recording` — pass。
- `mise run test-rt` — workspace `rt_` 収集で pass（新規
  `rt_scripted_input_callback_is_alloc_and_drop_free` 含む）。
- `mise run check-headless-deps` / `check-ruviform-rev` /
  `node tools/check-ruviform-rev.mjs --require-vendor` /
  `mise exec -- prek run --all-files` — pass。
- 詳細なコマンド・OS・toolchain・exit code は
  `evidence/sv115/` の本 slice 検証記録を参照。

## Out of scope / deferred

- rf#629 仮想 endpoint EXT-L3・physical・GUI（Option B）・
  macOS/Linux L1 CI・`test-recording-real-input` opt-in HW lane —
  NotRun/NotRequired のまま。
- SV-9 device-injected MIDI・SV-10 transport MIDI mapping・
  project persistence/timebase — Deferred。
- Issue #115 は open 継続（`Refs #115` のみ、auto-close なし）。
