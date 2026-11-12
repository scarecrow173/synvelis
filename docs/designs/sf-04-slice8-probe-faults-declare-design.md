---
id: "DESIGN-7qW2mX5cV9bN4tY8zK1jH6f"
type: "design"
status: "approved"
title: "SF-04 slice8 設計 — SF-PROBE-05 fault fixtures + seek 以外の不連続宣言点"
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
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/sf-04-slice3-clock-epoch-discontinuity-design.md"
    - "docs/designs/sf-04-slice6-probe-compare-design.md"
  defers: []
  deferred-by: []
---

# SF-04 slice8 設計 — SF-PROBE-05 fault fixtures + 不連続宣言点

## Context

slice3 は `RoutingEngine::seek` を唯一の cursor-series epoch
宣言点として `probes.bump_clock_epoch()` を配線し、slice6 は
`probe_compare`/`ProbeCoverage`（`PROBE_COVERAGE_MAX_EVENTS =
2048`・`lost_records` floor）を merge 済み（PR #348）。
本 slice は (a) SF-PROBE-05 の fault 証拠層を headless fixture
で固定し、(b) slice3/6 spec が deferred とした「seek 以外の
不連続宣言点」を transport 非依存の小 API として追加する。
fixture 設計の過程で `ProbeSession::drain` の segment close
順序に実ギャップを確認し、許可範囲内で `probe.rs` を修正する。

## Scope

### In Scope

- `RoutingEngine::declare_probe_discontinuity` +
  `SessionRoutingEngine`（synvelis-app engine）passthrough。
- `ProbeSession::drain` の `PendingClose` — drain batch 内で
  close status より先に生成された chunk を segment に計上。
- `tests/sf04_s8_probe_faults.rs`（5 件）・
  `tests/sf04_s9_probe_discontinuity_declare.rs`（5 件）。
- `docs/` 文書チェーン。

### Out of Scope

- `syn.probe.compare` app op、SF-PROBE-08/09、SF04-RCP-01、
  `ui/src/ipc.ts`、loop transport、engine restart API、
  device restart の context 引き継ぎ。
- 他 Worker 占有 file（`dispatch`/`trace`/`routing_control`/
  `driver`/`lib.rs`/`service`/`ui_ops`/`app`/`screens`/
  `binding`/`ops`/`control`/`snap`/`state`/`ppqn_ui`/
  `pc_c6_oracle_tr_dispatch`/`scenario`/`synvelis-recording`）。
- 実機・#109・#108・consumer 照合（外部 gate — NotRun）。

## Components and Boundaries

### `declare_probe_discontinuity`（player → app）

```rust
// crates/synvelis-player/src/routing_engine.rs
pub fn declare_probe_discontinuity(&mut self) -> Result<(), ProbeRequestError> {
    self.probes.bump_clock_epoch()
}
```

- `ProbeSession::bump_clock_epoch` → `ProbeBroker::bump_epoch`
  へ delegate。`seek` が best-effort（`let _ =`）で呼ぶのに対し
  明示宣言は `Result` を返す — 失敗を隠さず caller が retry
  判断を持つ。
- broker は epoch を **push 前に** bump する: `CommandChannel
  Full` 時は `probe_time_context().epoch`（broker 側）と deck
  の `current_mark` が乖離する。乖離は観測可能（`probe_drain`
  の record mark vs `probe_time_context`）で、次の成功した
  `wire_clock_mark`/`bump_epoch` が deck を resync する —
  恒久的な不整合にはならない。
- 意味論: cursor を動かさない。`stop`/`run` は
  `DriverState` のみを変え cursor に触れないため宣言点では
  ない。device restart は現状 engine 再生成であり、新しい
  `ProbeSession`/clock domain/epoch 0 の fresh context —
  旧 series の epoch を引き継がない（doc 明記）。
- app passthrough: `SessionRoutingEngine::declare_probe_
  discontinuity` が bridge 経由で engine へ届くだけの薄い層。

### `PendingClose`（`probe.rs` — fixture 露呈ギャップの修正）

`ProbeCollector::drain` は「全 queued telemetry → 全 queued
chunks」の順で配信し、cross-channel interleave は span 保証
でない（Ruviform doc 明記）。従来 `observe_status` は
`CaptureStopped`/`Removed`/`RebindDropped` を即時
`open.remove` → `close_segment` していたため、同 batch で
close status より前に生成された chunk（telemetry 側に後で
並ぶ）が `open` 不在で破棄され、segment が `Unknown`/
`frame_range=None` を過少申告し得た — `BudgetExhausted` は
最終 chunk と status が必ず同 callback で生成されるため系統的
に発生する。

修正: `ProbeSession::drain` が batch ローカルの
`BTreeMap<ProbeId, PendingClose>` を持ち、closing status は
`pending.entry(probe).or_insert(...)` で記録するだけにする。
`collector.drain` 完了後に flush して `close_segment` を適用
する。

- `PendingClose { end: CaptureSegmentEnd, clock: Option<
  ClockContextRef> }` — first close wins（`or_insert`）。
- pending 中の probe は「close 済み」として扱う: 後続
  status の epoch cut（`cut_segment_on_epoch_change`）と
  gap 記録（`record_gap`）を skip — close 後の証拠は segment
  に入らない。
- `observe_meta` は pending 中も chunk を計上するが、
  epoch が segment の記録済み clock と異なる record は
  cut も計上もせず discard（segment が epoch を跨がない
  不変条件を維持）。post-close chunk は mode が Monitor/Off
  に戻るため通常存在しない — 防御的扱い。
- adapter 側の同期 close（`remove()`・`reresolve_on_publish`
  の generation boundary cut）は drain batch を持たないため
  対象外 — 既存意味論を維持。

### fault fixture の構成

`sf04_s8_probe_faults.rs` は `sf04_s7_probe_compare.rs` の
bridge fixture を再利用（clip 無し・monitored silence で十分
— silence も観測データ）。fault は `ProbeConfig` 既定値で
再現可能なことを利用する:

- `pool_chunks=8` == `chunks_out` capacity: capture block 毎に
  `free.try_pop` → `chunks_out.try_push`。drain しない 12 block
  で 8 chunk + 4 `lost_blocks`。`emit_capture` は block 毎に
  1 chunk（`n = frames.min(chunk_frames)`）で、`chunk_frames=
  512` > `block_frames=256` でも chunk は block 毎に emit される
  ため枯渇は block 数で決まる。
- `telemetry_capacity=256`: 2 monitor probe × 140 block = 280
  record で ~24 `lost_records`。coverage floor は drain 前の
  `lost_records` を各 retained event に刻むため、baseline
  drain（floor=0）の event が残る限り陽性 delta で全 interval
  が `Unknown` へ降格 → joint valid 空 → `CoverageEmpty`。
- `Unobserved`/`LostRecords` status の到達不能性:
  `Unobserved{Stale|BorrowConflict}` は `run_site` の freshness/
  borrow audit でのみ発行されるが、public engine API では
  unconnected input が install 時 `Unavailable`、static
  schedule が bound slot を毎 callback 書込、`try_borrow` 競合は
  RT 内部 — e2e 到達不能。`LostRecords` status は deck が
  `lost_records` counter のみ増やし status を発行しない —
  写像は存在するが証拠は counter + coverage floor に留まる。
  両者とも test で「発行されないこと」を assert し、捏造しない。

### `sf04_s9_probe_discontinuity_declare.rs`

`sf04_s3` の clip fixture（transport/PCM 証拠）と `sf04_s7`
の bridge fixture（compare 証拠）を再利用:

- declare は `bump_epoch` のみ — clip handle/cursor に触れ
  ないため、declare 有り/無しの 2 engine が bit-identical PCM
  を出すことを `PlayReport.pcm`/`crc` で証明（seek なら
  re-anchor で PCM が変わる）。
- epoch cut は最初の new-epoch record で lazy に発火 —
  `observe_meta`/`observe_status` の既存経路をそのまま通る
  ため seek と同じ `ClockEpoch` end・旧 epoch clock_context・
  post-cut discard・no auto-recapture を assert。
- `DiscontinuityInWindow`: declare を跨ぐ compare window が
  seek と同じ verdict を返す（`sf04_s7` の seek 版と対称）。
- channel full: command channel（capacity 64）を install
  上限（`max_probes=32`）+ SetMode で埋め、declare が
  `Broker(CommandChannelFull)` を返しつつ epoch が進むこと、
  drain 後の record mark が旧 epoch のまま（deck 未達）で
  `time_context` と乖離すること、次の成功 wire で resync
  することを証明。

## Data and Control Flow

```
SF-PROBE-05 fault 経路（RT → collector → adapter → contract）:

chunk pool 枯渇:  emit_capture → free.try_pop 失敗
                 → lost_blocks++ + StatusKind::LostBlocks
                 → observe_status → record_gap(LostBlocks)
                 → status_update → ProbeObservation::Lost{count}
                 → segment Incomplete（has_data 残存）

telemetry overflow: emit_metric → telemetry.try_push 失敗
                 → lost_records++（status は発行されない）
                 → coverage record(loss_floor=drain前 counter)
                 → reconstruct: latest - oldest floor > 0
                 → 全 interval Unknown → CoverageEmpty fail-closed

segment close:    CaptureStopped/Removed/RebindDropped status
                 → pending.entry(probe).or_insert(PendingClose)
                 → 同 batch chunks → observe_meta 計上
                 → collector.drain 完了 → flush → close_segment
                 → CaptureSegmentEnd::{Requested,BudgetExhausted,
                   Removed,RebindDropped} + completeness

declare:          declare_probe_discontinuity
                 → probes.bump_clock_epoch
                 → broker.time.epoch++ → wire_clock_mark
                   (SetClockMark → deck.current_mark)
                 → 以後の record が新 epoch を刻印
                 → open segment は最初の new-epoch record で
                   ClockEpoch cut（lazy・既存経路）
```

## Non-Functional Constraints

- 非 RT・control plane のみ。`PendingClose` は drain batch
  ローカルの `BTreeMap` — batch を跨いで残らない bounded
  状態。allocation は既存 drain 経路と同程度。
- `unsafe` なし、workspace lint 準拠。
- fail-closed が既定: loss/欠落/未到達は validity を偽装
  しない。`declare` の channel full は error を返し、
  epoch 進行と deck 未達の乖離を隠さない。
- master PCM/DSP 出力に影響しない（probe 側のみ）。
  `declare` が transport を動かさないことを PCM 一致で証明。

## Risks and Trade-offs

- **`PendingClose` の end reason 優先順位**: first close wins —
  同 batch に複数 closing status が来ても最初のものが正直な
  終了理由（後続は close 後の binding 証拠）。`entry().
  or_insert` で実現。
- **pending 中の epoch 不一致 chunk**: 通常発生しない
  （close 後は Monitor/Off で chunk を出さない）が、来た場合は
  cut も計上もせず discard — segment の epoch 不変条件を守る。
- **`AlreadyCapturing` との相互作用**: `open` は flush まで
  残るため、同 batch 内に次の `capture()` は依然
  `AlreadyCapturing` — batch を跨ぐ API 呼出しは flush 後に
  しか来ないため実害なし。
- **adapter 側同期 close は未修正**: `remove()`/
  `reresolve_on_publish` の即時 close は drain batch を持た
  ない — queued chunk の取りこぼしは理論上残るが、本 fixture
  の対象外（別 slice の検討事項として記録）。
- **`unobserved`/`unbound` counter**: `unbound` は generation
  guard の一時的 skew で発生し得るため assert しない。
  `unobserved == 0` のみ fixture で主張。

## Deferred Design Concerns

- `syn.probe.compare` app op・session 露出 | `service.rs`/
  `ui_ops.rs`/`app.rs` 他 Worker 占有 | #108 系後続 slice |
  player `compare_probes` が受け口。
- device restart の context 引き継ぎ | engine 再生成 =
  fresh domain/epoch 0 | 要件確定後 | 現行は fresh が正直。
- `Unobserved` の e2e 到達 | public API では到達不能 |
  Ruviform fault 注入経路が必要なら別 slice | mapping-only
  証拠 + counter 主張に留める。
- loop transport | API 非存在・YAGNI | 要件発生時 | 宣言 API
  は transport 非依存で再利用可能。
- 実機 fault 多様性 | #109 oracle gate | gate 後 | headless
  は既定 `ProbeConfig` のみ。

## Verification Notes

- `crates/synvelis-player/tests/sf04_s8_probe_faults.rs`
  （実 engine + bridge、`sf04_s7` fixture 再利用）:
  - `sf04_s8_chunk_pool_exhaustion_records_lost_blocks`:
    `lost_blocks == 4`・`lost_records == 0`（fault 分離）・
    `ProbeObservation::Lost{count:4}`・`last_status ==
    LostBlocks`・segment `Requested`/`Incomplete`/gap 全て
    `LostBlocks`・`frame_range` 残存・no auto-recapture。
  - `sf04_s8_telemetry_overflow_downgrades_coverage_fail_
    closed`: baseline `Aligned` → 280 record で
    `lost_records > 0`・`lost_blocks == 0` → drain →
    `Incomparable{CoverageEmpty}`・両側 `Unknown` gap・
    `interval.is_some()`。
  - `sf04_s8_capture_variants_close_with_distinct_reasons`:
    `AlreadyCapturing` 拒否・`BudgetExhausted`+`Complete`+
    `frame_range == 1024`（`PendingClose` 修正の証拠 —
    単一 drain batch で最終 chunk も計上）・`Requested`+
    `Complete`・session id 相異・`Removed`+`Stopped`。
  - `sf04_s8_observation_states_stay_distinct`:
    `Measuring{Silence}`・`Disabled`（Off）・resume で
    `Measuring{Silence}`・remove で `Missing` 非捏造・
    同一 point の別 subscription は継続・全 remove で
    `ProbeNotBound{Before}`。
  - `sf04_s8_unreachable_fault_statuses_are_unfaked`:
    reachable status（`Installed`/`CaptureStarted`/
    `CaptureStopped{BudgetExhausted}`/`ModeChanged`）の存在と
    `LostRecords`/`Unobserved` status の非発行・
    `unobserved == 0`・`lost_blocks == 0`・segment gap に
    `LostRecords`/`Unobserved` kind が無いことを assert。
- `crates/synvelis-player/tests/
  sf04_s9_probe_discontinuity_declare.rs`:
  - `sf04_s9_declare_cuts_segment_at_clock_epoch_without_
    moving_transport`: declare で epoch+1・`ClockEpoch` cut・
    旧 epoch clock_context・`Complete`・対照 engine は segment
    開放のまま・PCM/`crc` bit-identical・post-declare record
    が新 epoch・明示 recapture が新 session/新 epoch/
    `BudgetExhausted`。
  - `sf04_s9_declare_window_spanning_boundary_is_
    discontinuity_in_window`: baseline `Aligned` → declare
    跨ぎ window → `Incomparable{DiscontinuityInWindow}`。
  - `sf04_s9_stop_and_run_are_not_discontinuity_declarations`:
    `stop`/`run` で epoch 不変・record mark が全て旧 epoch・
    fresh engine は epoch 0 + 異なる clock domain。
  - `sf04_s9_declare_channel_full_reports_error_and_keeps_
    epoch`: channel full で `Broker(CommandChannelFull)`・
    epoch は進行・drain 後 record は旧 epoch（deck 未達）・
    `time_context` と乖離・次の成功 wire で resync。
  - `sf04_s9_declare_observation_updates_carry_epoch_
    boundary`: declare 後も `Measuring` update が継続。
- `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build`。
