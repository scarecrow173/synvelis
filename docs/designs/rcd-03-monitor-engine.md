---
id: "DESIGN-034WCrWfgXKQV2I7XmJGE0"
type: "design"
status: "approved"
title: "RCD-03 monitor engine — MonitorSession/MonitorEngine design"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added:
      - "crates/synvelis-player/src/monitor_engine.rs"
      - "crates/synvelis-player/tests/monitor_engine.rs"
    modified:
      - "crates/synvelis-player/src/lib.rs"
      - "crates/synvelis-player/src/driver.rs"
      - "crates/synvelis-player/src/routing_engine.rs"
      - "crates/synvelis-recording/src/plan.rs"
      - "crates/synvelis-recording/tests/monitor_gate.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-03-monitor-engine.md"
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-03-monitoring-punch-latency.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  defers:
    - "docs/specs/rcd-03-monitor-engine.md"
  deferred-by: []
---

# RCD-03 monitor engine — MonitorSession/MonitorEngine design

## Context

PR #214 で `synvelis-recording` の `decide`/`MonitorProbe` と
`synvelis-player` の `MonitorAdapter`/`LatencyReport` は実装済み
だが、`MonitorAdapter` は誰からも構築・駆動されない未接続
状態。gate 観測は `GateScheduler`/`evaluate_gate` が唯一の
権威であり、route 意味論は `decide` が唯一の権威である。本
design はその2つを合成し、headless engine driver の実 block
経路（`rt.process` → master tap PCM）に monitor bus を載せる
非 RT 観測・制御経路を定義する。

## Scope

### In Scope

- `MonitorSession`: `MonitorProbe` + `TrackMonitorContext` +
  `MonitorAdapter` + 入力 PCM FIFO + `TransportPhase` +
  segment evidence log の合成。
- `MonitorEngine`: headless driver を block 単位で駆動し、
  各 block の実 master 出力 PCM を session へ供給する
  wrapper。`EngineBlocks` trait で `HeadlessDriver`/
  `RoutingEngine` の実 run 経路を抽象化。
- `PreparedRecordingPlan` への構築時 `sample_rate` 保持
  （`plan.rs`、追加フィールドのみ）。

### Out of Scope

- RT graph への monitor node 搭載・transport control 配線。
- 実入力 device からの monitor PCM 供給。
- `synvelis-app` からの session 構築・状態投影接続。
- engine 通常出力・raw capture・export への monitor PCM の
  混入（monitor bus は独立観測経路）。

## Components and Boundaries

- `EngineBlocks`（`monitor_engine.rs` 内 trait）:
  `run_blocks(blocks, policy) -> Result<PlayReport,
  ProcessError>` と `cursor() -> u64` を要求。
  `HeadlessDriver` と `RoutingEngine` が既存 `run`/`cursor`
  へ委譲実装する。両者は同一 `ProcessError`/`PumpPolicy`/
  `PlayReport`/`EngineConfig` 型を共有するため 1 trait で
  両対応できる。`PlayReport.pcm` は master tap の planar
  PCM であり、`blocks == 1` の呼び出しではその block 分
  だけを保持する — これが session へ供給する実経路 PCM
  となる。
- `MonitorSession<'a>`（`monitor_engine.rs`）:
  `MonitorProbe<'a>`（`&'a PreparedRecordingPlan` を借用、
  `GateScheduler` を共有）+ `MonitorAdapter` +
  `TrackMonitorContext`（probe から取得）+ `TransportPhase`
  + `VecDeque<Vec<Vec<f32>>>` 入力 PCM FIFO + segment
  evidence `Vec<MonitorSegmentRecord>` + `faulted` flag。
  `on_block(frame_start, frames, playback) -> Vec<Vec<f32>>`
  が per-block tap。
- `MonitorEngine<'a, D: EngineBlocks>`: driver + session +
  `EngineConfig` + attach 時 `frame_offset`（driver cursor）。
  `run(blocks, pump_every) -> Result<MonitorRunReport,
  ProcessError>`。
- `PreparedRecordingPlan`（`plan.rs`）: `preflight` の引数
  `sample_rate` を `pub sample_rate: u32` として保持。engine
  sample rate との一致検証に使用。

## Data and Control Flow

1. `MonitorEngine::new(driver, plan, ctx, engine_config,
   ramp_frames)`:
   - `plan.sample_rate == engine_config.sample_rate`、
     `block_frames` が u32 範囲内、channels > 0 を検証
     （不一致は `MonitorEngineError`）。
   - `frame_offset = driver.cursor()`（attach 時 engine
     position。以後 session 位置 = engine frame − offset）。
   - `MonitorSession::new` が `MonitorProbe::new(plan, ctx)`
     と `MonitorAdapter::new(engine_config, ramp_frames)` を
     構築。
2. `MonitorEngine::run(blocks, pump_every)`:
   `blocks` 回の iteration で各 1 block ずつ実経路を駆動する。
   a. `cursor = driver.cursor()` → session 位置
      `session_frame = cursor - frame_offset`（underflow は
      fail-closed fault）。
   b. caller が block 単位で `session.set_phase(phase)` /
      `session.push_input_block(pcm)` を供給（`run` の
      引数ではなく session の mutable API。block 間で呼ぶ）。
   c. pump cadence の emulation: 既存 `run` の pump 判定は
      呼び出し内 `index % n` であり、`run(1, ...)` を繰り返すと
      index が常に 0 に戻り cadence が狂う。`MonitorEngine`
      は自前の block counter で「この block で pump すべきか」
      を判定し、pump block は `run_blocks(1, Every(1))`、
      非 pump block は `run_blocks(1, Every(0))` を呼ぶ
      （`Every(1)` は index 0 で必ず pump、`Every(0)` は
      never — 単一 `run(N, Every(n))` の `index % n == 0`
      と同一 cadence）。
   d. `report = driver.run_blocks(1, policy)?` — 実経路で
      1 block 処理。`report.pcm` がその block の master tap
      PCM（実経路由来）。
   e. `session.on_block(session_frame, block_frames,
      report.pcm)` → probe.feed（session-frame 精度で gate
      segment 分割）→ segment ごとに decide →
      adapter.process_block → monitor PCM + effective route
      + notices。
   f. `MonitorRunReport { playback, monitor, frames }` に
      block ごとの PCM を蓄積。
3. fail-closed: probe segment が block を隙間・重複なく
   tile しない、adapter 出力長不一致、session 位置ずれ、
   input 未供給で route == Input（Muted 降格 + notice）は
   `faulted` を立て、以後の monitor 出力は無音。

### 検証済み前提（コード調査で確定）

- `HeadlessDriver::run` / `RoutingEngine::run` は共に
  `rt.process(block, ctx)` + pump + `output_reader(port)` で
  master PCM を `PlayReport` へ蓄積する per-block ループ。
  `cursor` は処理済み frame 数。両者は同一 `ProcessError`/
  `PumpPolicy`/`PlayReport`/`EngineConfig` 型を共有。
- `MonitorProbe::feed(range, phase)` は
  `scheduler.extend` + `evaluate_gate` を呼び、
  `SessionRange`/`SessionFrame` 精度の open/closed
  `MonitorSegment` を返す。gate 評価の第二実装は不要。
- `MonitorAdapter::process_block(MonitorBlock)` は
  channel-major PCM を受け取り `MonitorOutput`（PCM +
  effective route + notices）を返す。input 不在時は
  route Input を Muted へ fail-closed 降格する。
- `MonitorProbe` は `&PreparedRecordingPlan` を借用し
  `GateScheduler` を共有するため、録音側 gate と monitor
  観測は同一 schedule を参照する。

## Non-Functional Constraints

- monitor bus は engine 通常出力・raw capture・export を
  変更しない独立経路。fixture でも実機でも同じ不変条件。
- `MonitorAdapter::process_block` は現状 `Vec<Vec<f32>>` を
  確保する非 RT 実装。本 slice は RT callback 外の headless
  駆動のみを対象とし、RT-safe 化は後続 slice。
- engine frame ↔ session frame は 1:1。`plan.sample_rate ==
  engine.sample_rate` を構築時に強制し、tempo/loop の
  musical 単位差は `GateScheduler`/`evaluate_gate` が session
  frame へ写像済みの segment を返すため monitor 側で
  再計算しない。

## Risks and Trade-offs

- `EngineBlocks` trait が `PlayReport` に PCM を含めるか
  tap PCM を別途返すか: 既存 `run` は `PlayReport` に
  playback PCM を蓄積する。monitor session へ渡す playback
  PCM は同じ master tap 由来でなければ「実経路の PCM」と
  言えないため、`run_blocks` が block ごとの tap PCM を
  返す形にする（`PlayReport` 蓄積は維持）。
- `RoutingEngine` 対応: app の実経路は `RoutingEngine`。
  `HeadlessDriver` は legacy 並行経路。trait で両方を受け、
  fixture は `RoutingEngine` を主対象、`HeadlessDriver` も
  同一 contract で pin する。
- session 終端後: `probe.feed` が `session_end` を越えた
  range に対して closed segment のみ返す（Playback）—
  録音終了後は通常再生が可聴になる製品規則と一致。
- block 途中の phase/arm/monitor 変化: 本 slice は block
  単位供給のみ。block 内相変化は `MonitorProbe`/`decide`
  の契約外として扱い、fixture でも block 境界でのみ変化
  させる。

## Deferred Design Concerns

- RT graph への monitor node 搭載・in-graph RT processor 化 |
  本 design は engine 実経路の非 RT 観測・制御合成 |
  実 device/RT 統合 slice | adapter の RT-safe 化と graph
  node 化が未検証
- 実入力 device monitor PCM 供給・per-strip pre-fader tap |
  fixture は staging PCM・master tap のみ | 実 device monitor
  slice | monitor 入力経路が未接続
- `synvelis-app` からの session 構築・transport phase 導出・
  状態投影接続 | app 層は別残件 | app 側 dispatch | engine
  monitor bus が app に未接続

## Verification Notes

- [ ] `monitor_gate.rs` に Loop/PunchInLoop の probe
      segment・Raw Byte 独立・tempo 境界 project mapping
      テストを追加し pass。
- [ ] `monitor_engine.rs`（player tests）に engine 実経路
      経由の route 別 PCM・block 途中境界 Gap/重複 0・
      Loop/PunchInLoop pass・高 Latency FX/Tempo 組合せ
      fixture を追加し pass。
- [ ] `mise run test` で追加テストを含む全テスト pass。
- [ ] canonical 9 lane すべて exit 0。
