---
id: "IMPL-rcd03-monitor-engine"
type: "impl"
status: "completed"
title: "RCD-03 MonitorAdapter engine 搭載 + Loop/PunchInLoop gate slice 実装記録"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/monitor_engine.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/monitor_engine.rs"
      - type: "file"
        path: "docs/specs/rcd-03-monitor-engine.md"
      - type: "file"
        path: "docs/designs/rcd-03-monitor-engine.md"
      - type: "file"
        path: "docs/plans/rcd-03-monitor-engine.md"
      - type: "file"
        path: "docs/tasks/rcd-03-task-monitor-engine.md"
      - type: "file"
        path: "docs/test-specs/rcd-03-monitor-engine.md"
      - type: "file"
        path: "docs/impl/ir/rcd-03-monitor-engine.md"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/src/plan.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/monitor_gate.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-03-monitor-engine.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-03-monitor-engine.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-03-monitoring-punch-latency-crate-slice.md"
    - "docs/impl/ir/rcd-03-app-arm-monitoring-ux-slice.md"
    - "docs/impl/ir/rcd-03-monitor-persistence-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/rcd-03-monitor-engine.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-03-monitor-engine.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-03 MonitorAdapter engine 搭載 + Loop/PunchInLoop gate slice 実装記録

## Summary

Issue #117（RCD-03 Monitoring / Punch / Latency）の残 headless
slice として、monitor gate の観測・制御経路を player/engine の
実経路へ搭載した。`MonitorSession` が recording 側 `MonitorProbe`
（`GateScheduler`/`evaluate_gate` を共有 — 第二 gate 実装なし）と
player 側 `MonitorAdapter`（PCM gain-ramp 混合）を合成し、
`MonitorEngine` が `HeadlessDriver`/`RoutingEngine` を 1 block ずつ
実 run 経路で駆動して各 block の実 master tap PCM
（`PlayReport.pcm`）を session へ供給する。engine frame ↔
session frame は 1:1 で、attach 時 cursor を frame offset とし、
以後のずれ（seek 等）は fail-closed（無音 + `faulted` 観測可能）。
monitor 出力は engine 通常出力・raw capture・export を変更しない
独立 bus で、native placement を保ち latency/PDC 補正・
リサンプル・オフセットを placement に適用しない
（`ramp_frames=0` では route 遷移が即時で PCM は bit 一致）。
本 slice は Issue #117 を Close しない（実 device monitor・
実 GUI 受入・count-in UX・AudioLens・app Command/状態投影・
RT graph monitor node は外部 gate / 別 slice）。

## Context

- spec `docs/specs/rcd-03-monitor-engine.md`
- design `docs/designs/rcd-03-monitor-engine.md`
- plan `docs/plans/rcd-03-monitor-engine.md`
- task `docs/tasks/rcd-03-task-monitor-engine.md`
- test-spec `docs/test-specs/rcd-03-monitor-engine.md`

先行 slice（crate slice・app UX・persistence・VCS diff 分類）で
`MonitorProbe`/`decide`/`MonitorAdapter`/`LatencyReport`・
Project 永続投影は確定済みで、「MonitorAdapter の engine 搭載」
と「Loop/PunchInLoop gate の headless 検証」が残件だった。
`synvelis-recording`/`synvelis-player` 以外の crate・app/UI 層・
docs（本 doc-chain 以外）は無変更。

## Implementation

### Changes

- `crates/synvelis-recording/src/plan.rs`:
  `PreparedRecordingPlan` に `sample_rate: u32` を追加し
  `preflight` で populate。engine sample rate との一致検証・
  frame↔tick 写像の基準として凍結する。
- `crates/synvelis-player/src/monitor_engine.rs`（新規）:
  - `EngineBlocks` trait — `HeadlessDriver`/`RoutingEngine` の
    既存 `run`/`cursor` へ委譲する block 実行抽象。
  - `MonitorSession` — `MonitorProbe` + `TrackMonitorContext` +
    `MonitorAdapter` + 入力 PCM FIFO + block 単位 `TransportPhase`
    + segment 証拠（`MonitorSegmentRecord`）を合成。`on_block`
    は session 位置照合 → `probe.feed` → session 終端超過分を
    `Playback` で補完 → 隙間/重複/空区間なし tile を検証 →
    segment ごとに `decide` route で adapter 駆動 → 証拠記録。
    契約違反はすべて `faulted` を立てる fail-closed（無音）。
    入力 PCM 未供給・shape 不一致の block で route `Input` が
    選ばれた segment は `Muted` へ降格 + `input_missing`
    notice（PCM を捏造しない）。
  - `MonitorEngine` — attach 時 `driver.cursor()` を frame
    offset として保持し、`run(blocks, pump_every)` が 1 block
    ずつ `run_blocks(1, ...)` を呼ぶ。pump cadence は呼出し内
    index ではなく `blocks_run` counter で `index % n == 0` と
    同一になるよう `Every(1)`/`Every(0)` を切替える
    （`run(1, ...)` 反復で pump が毎 block 化するのを防ぐ）。
    `cursor - frame_offset` が session 位置と再照合できない
    場合は `session.fault()` で fail-closed。`plan.sample_rate
    != engine.sample_rate`・`block_frames` 範囲外・`channels==0`
    は構築時に [`MonitorEngineError`] で拒否。
  - `MonitorRunReport` — master tap PCM・monitor PCM・処理
    frame 数を block 連結で返す。
- `crates/synvelis-player/src/lib.rs`: `monitor_engine` module +
  公開型の re-export。
- `crates/synvelis-recording/tests/monitor_gate.rs`:
  Loop/PunchInLoop 6 test 追加 — Loop gate の連続 open 区間、
  block 384（pass/punch 境界を block 途中に跨ぐ）での
  PunchInLoop 再 gate、pass ごとの project mapping 連鎖被覆
  （span 分割粒度に依らない不変条件）、tempo 境界を跨ぐ
  PunchInLoop の project mapping、Loop/PunchInLoop での
  Off/Auto/On raw capture 一致。
- `crates/synvelis-player/tests/monitor_engine.rs`（新規、
  10 test）: 実 `HeadlessDriver` 経路での route/PCM pin —
  Punch open 区間の Input/Playback 切替（bit 一致・native
  placement）、input missing fail-closed、Off/On/Auto ×
  arm × mute/solo × phase の matrix、PunchInLoop pass ごとの
  open 区間、Loop 連続 open、tempo 境界 punch、monitor bus が
  engine playback を変更しないこと（plain driver と PCM
  bit 一致）、session 位置ずれ fail-closed、sample rate
  mismatch 拒否、`RoutingEngine` の `EngineBlocks` 実装 pin。
  すべて fixture 駆動の headless 検証であり、実 device・実 GUI
  の受入を主張しない。
- `docs/` doc-chain 5 文書 + 本 impl record。

### Decisions During Implementation

- gate/route 判定の権威は recording 側（`GateScheduler`/
  `evaluate_gate`/`decide`）に残し、player 側は PCM 実行と
  latency 解決のみを担う。`MonitorSession` は第二 gate 実装を
  持たず probe segment を消費するだけ。
- monitor PCM の playback 側ソースは `PlayReport.pcm`（実
  master/device tap）を使う — 推定 PCM を捏造しない。
  tap 未接続・channel/frame 不足は無音として扱う。
- session 終端を越える engine frame は `Playback` route で
  補完する（post-session は通常再生へ復帰）。session 位置は
  飽和させず engine frame との 1:1 を保つ。
- `TransportPhase`・入力 PCM は block 単位供給（block 内相
  変化は対象外）。`Vec` 確保を伴うため非 RT — RT graph への
  monitor node 搭載は別 slice。
- `PumpPolicy` の cadence は `blocks_run` counter で再現し、
  呼出しをまたいだ連続性を保つ（単一 `run(N)` と同一）。

## Related Experiments

### Adopted

<!-- 既知解の機械的実装 + fixture 作成。実験 log なし。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク、Windows 11 x64・
rust 1.99.0 pinned）:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check` — Pass（`cargo check --workspace
  --all-targets`、exit 0）
- `mise run lint` — Pass（`cargo clippy --workspace
  --all-targets -- -D warnings`、exit 0）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  exit 0。`monitor_gate.rs` 20 test・`monitor_engine.rs`
  10 test を含む全 workspace 緑）
- `mise run test-rt` — （実行結果は PR 本文に記録）
- `mise run check-app-native` — （同上）
- `mise run lint-app-native` — （同上）
- `mise run test-msrv` — （同上）
- `mise run build` — （同上）

補助証拠（canonical = mise、直接 cargo は補助）:
`cargo test -p synvelis-player --test monitor_engine` —
10 passed、`cargo test -p synvelis-recording --test
monitor_gate` — 20 passed。

## Deferred / Follow-up

- 実 device monitor/latency 受入・実 GUI 受入行（Issue #117
  残件、`Refs #117` で Open 継続）。
- count-in UX・AudioLens・app Command/状態投影への monitor
  engine 組み込み（別 slice）。
- RT graph への monitor node 搭載（`MonitorAdapter` は
  非 RT — `Vec` 確保を伴う）。
- `RoutingEngine` の `EngineBlocks` 経路での PCM 内容 pin
  （本 slice は trait 実装の型レベル pin のみ — `RoutingEngine`
  自体の全機能検証は既存テストが担う）。
