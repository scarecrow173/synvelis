---
id: "TSPEC-2hN6pR8sT4wQ1yU5iO3aZ7c"
type: "test-spec"
status: "approved"
title: "SF-04 slice8 テスト仕様 — SF-PROBE-05 fault fixtures + 不連続宣言点"
created: "2026-11-12"
updated: "2026-11-12"
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
  implements:
    - "docs/specs/sf-04-slice8-probe-faults-declare.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice8-probe-faults-declare.md"
    - "docs/designs/sf-04-slice8-probe-faults-declare-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/test-specs/sf-04-slice6-probe-compare.md"
  defers: []
  deferred-by: []
---

# SF-04 slice8 テスト仕様 — SF-PROBE-05 fault fixtures + 不連続宣言点

## 対象

- `crates/synvelis-player/tests/sf04_s8_probe_faults.rs`
  （headless、実 `RoutingEngine` + `ProbeTraceBridge`）。
- `crates/synvelis-player/tests/
  sf04_s9_probe_discontinuity_declare.rs`
  （headless、実 `RoutingEngine` clip fixture + bridge
  fixture を再利用）。

## 検証方針

- fault は既定 `ProbeConfig`（`pool_chunks=8`・
  `chunk_frames=512`・`telemetry_capacity=256`・
  `command_capacity=64`）と RT callback 数の制御だけで
  決定的に再現する — 実機・RT thread は不要。
- 到達不能な runtime status（`LostRecords` status・
  `Unobserved{Stale|BorrowConflict}`）は捏造せず、「発行
  されないこと」+ 到達可能証拠（counter・coverage floor・
  `unobserved == 0`）で主張する — slice6 先例に倣う。
- 非 seek 宣言は `seek` と同じ epoch/segment/compare 意味を
  持つことを対称 fixture で証明し、PCM bit-identical で
  transport 非干渉を証明する。
- PCM 相関・signal payload・実測 latency は対象外。

## Test Cases

| # | Test | 期待 |
| --- | --- | --- |
| F1 | `sf04_s8_chunk_pool_exhaustion_records_lost_blocks` | `pool_chunks=8`・undrained 12 block → `lost_blocks == 4`・`lost_records == 0`・`ProbeObservation::Lost{count:4}`・`last_status == LostBlocks`・segment `Requested`/`Incomplete`・gap 全て `LostBlocks`・`frame_range` 残存・no auto-recapture |
| F2 | `sf04_s8_telemetry_overflow_downgrades_coverage_fail_closed` | baseline `Aligned` → 280 record で `lost_records > 0`・`lost_blocks == 0` → drain → `Incomparable{CoverageEmpty}`・両側 `Unknown` gap |
| F3 | `sf04_s8_capture_variants_close_with_distinct_reasons` | `AlreadyCapturing` 拒否・`BudgetExhausted`+`Complete`+`frame_range == 1024`（`PendingClose` 証拠）・`Requested`+`Complete`・session id 相異・remove 中 capture が `Removed`/`Stopped` |
| F4 | `sf04_s8_observation_states_stay_distinct` | `Measuring{Silence}`・`Disabled`（Off）・resume で `Measuring{Silence}`・remove で `Missing` 非捏造・全 remove で `ProbeNotBound{Before}` |
| F5 | `sf04_s8_unreachable_fault_statuses_are_unfaked` | reachable status の存在・`LostRecords`/`Unobserved` status の非発行・`unobserved == 0`・`lost_blocks == 0`・segment gap に `LostRecords`/`Unobserved` kind 無し |
| F6 | `sf04_s9_declare_cuts_segment_at_clock_epoch_without_moving_transport` | declare で epoch+1・`ClockEpoch` cut・旧 epoch clock_context・`Complete`・対照 engine は segment 開放のまま・PCM/`crc` bit-identical・post-declare record が新 epoch・明示 recapture が新 session/新 epoch/`BudgetExhausted` |
| F7 | `sf04_s9_declare_window_spanning_boundary_is_discontinuity_in_window` | baseline `Aligned` → declare 跨ぎ window → `Incomparable{DiscontinuityInWindow}` |
| F8 | `sf04_s9_stop_and_run_are_not_discontinuity_declarations` | `stop`/`run` で epoch 不変・record mark が全て旧 epoch・fresh engine は epoch 0 + 異なる clock domain |
| F9 | `sf04_s9_declare_channel_full_reports_error_and_keeps_epoch` | channel full で `Broker(CommandChannelFull)`・epoch は進行・drain 後 record は旧 epoch（deck 未達）・`time_context` と乖離・次の成功 wire で resync |
| F10 | `sf04_s9_declare_observation_updates_carry_epoch_boundary` | declare 後も `Measuring` update が継続（観測系列の断絶なし） |

## 結果記録

- 実行: `cargo test -p synvelis-player --test
  sf04_s8_probe_faults` → 5 passed / 0 failed。
- `cargo test -p synvelis-player --test
  sf04_s9_probe_discontinuity_declare` → 5 passed / 0 failed。
- 既存 `sf04_*` test 群（adapter・s2・s3・s7 含む
  25 test）は `probe.rs` `PendingClose` 変更後も無変更で
  green。

## 外部 Gate（NotRun — Pass 扱いしない）

- 実機 fault（デバイス断・late bind・format 変化）→ #109
  実測 oracle gate。
- #108 UI/session 統合・`syn.probe.compare` app op。
- consumer 照合・診断解釈（SF-PROBE-08/09）・
  SF04-RCP-01・`ui/src/ipc.ts`。
