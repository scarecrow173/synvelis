---
id: "PLAN-034WCvbw5IfHDl5knLAOrW"
type: "plan"
status: "approved"
title: "RCD-03 monitor engine implementation plan"
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
      - "docs/specs/rcd-03-monitor-engine.md"
      - "docs/designs/rcd-03-monitor-engine.md"
      - "docs/plans/rcd-03-monitor-engine.md"
      - "docs/tasks/rcd-03-task-monitor-engine.md"
      - "docs/test-specs/rcd-03-monitor-engine.md"
      - "docs/impl/ir/rcd-03-monitor-engine-crate-slice.md"
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
    - "docs/designs/rcd-03-monitor-engine.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/rcd-03-monitoring-punch-latency.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-monitor-engine.md"
    - "docs/designs/rcd-03-monitor-engine.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  defers:
    - "docs/specs/rcd-03-monitor-engine.md"
  deferred-by: []
---

# RCD-03 monitor engine implementation plan

## Goal

`MonitorSession`/`MonitorEngine` を `synvelis-player` に実装し、
headless engine 実経路に monitor bus を載せて Loop/
PunchInLoop gate を含む route 結果を fixture で pin する。

## Scope

1 ストリーム。`synvelis-recording`（`plan.rs` の
`sample_rate` 追加 + `monitor_gate.rs` テスト追加）と
`synvelis-player`（`monitor_engine.rs` 新規 + driver trait
実装 + `monitor_engine.rs` テスト新規）の 2 クレートに
またがるが、同一の session 構成契約を成すため分割しない。

## Upstream Documents

- `docs/specs/rcd-03-monitor-engine.md`
- `docs/designs/rcd-03-monitor-engine.md`
- `docs/specs/rcd-03-monitoring-punch-latency.md`（前提契約）

## Design Inputs

- `docs/designs/rcd-03-monitor-engine.md`（approved）

## File Map

- `crates/synvelis-recording/src/plan.rs` - `PreparedRecordingPlan`
  に `pub sample_rate: u32` を追加し `preflight` で設定。
- `crates/synvelis-player/src/monitor_engine.rs` - 新規。
  `EngineBlocks` trait、`MonitorSession`、`MonitorEngine`、
  `MonitorRunReport`、`MonitorSegmentRecord`、
  `MonitorEngineError`。
- `crates/synvelis-player/src/driver.rs` - `EngineBlocks` の
  `HeadlessDriver` impl（`run`/`cursor` 委譲）。
- `crates/synvelis-player/src/routing_engine.rs` - 同上
  `RoutingEngine` impl。
- `crates/synvelis-player/src/lib.rs` - `monitor_engine`
  module 公開 + re-export。
- `crates/synvelis-player/tests/monitor_engine.rs` - 新規
  fixture テスト。
- `crates/synvelis-recording/tests/monitor_gate.rs` - Loop/
  PunchInLoop/tempo の probe segment・Raw Byte 独立・
  project mapping テスト追加。

## Implementation Sequence

- [ ] T1: `plan.rs` に `sample_rate` フィールド追加
      （構築時 rate 保持）。`check` で既存呼び出しへの影響
      確認。
- [ ] T2: `monitor_engine.rs` に `EngineBlocks` trait +
      `MonitorSession` + `MonitorEngine` + 関連型を実装。
      `HeadlessDriver`/`RoutingEngine` に trait impl。
      `lib.rs` 公開。
- [ ] T3: `monitor_gate.rs` に Loop/PunchInLoop gate の
      probe segment（block 途中境界 Gap/重複 0）・Raw Byte
      独立（Off/Auto/On で capture bytes 一致）・tempo
      境界 project mapping テストを追加。
- [ ] T4: `monitor_engine.rs`（player tests）に engine 実経路
      fixture を追加: Off/Auto/On × Arm × Transport × Punch
      × Mute/Solo の route 結果 pin、Loop/PunchInLoop pass
      境界、input 未供給 fail-closed、高 Latency FX/Tempo
      組合せ。
- [ ] T5: `mise run fmt-check` → `check` → `lint` → `test`
      → `test-rt` → `check-app-native` → `lint-app-native`
      → `test-msrv` → `build` の canonical 9 lane 実行。
- [ ] T6: review-protocol レビュー → 指摘対応 →
      `git fetch origin main && git merge origin/main` →
      docs index 再生成 → commit → push → PR。

## Dependencies and Risks

- `EngineBlocks` trait の `run_blocks(1, ...)` 反復で pump
  cadence が狂うリスク → `MonitorEngine` が block counter で
  pump 要否を判定し `Every(1)`/`Every(0)` を切替えて
  `index % n == 0` と同一 cadence を再現（design 記載）。
- `MonitorAdapter::process_block` の `Vec` 確保は非 RT —
  本 slice は RT callback 外のみ。RT-safe 化は別 slice。
- `RoutingEngine` の device_in/master tap が `HeadlessDriver`
  の master_in と同じ「実経路 PCM」か → 両者とも
  `output_reader(port)` の master/device tap を `PlayReport.
  pcm` に蓄積する同一構造であることを確認済み。
- session 終端後の Playback 遷移は `MonitorProbe::feed` の
  既存契約に依存 — fixture で pin。

## Task Breakdown

- T1–T4 は実装タスク。T5–T6 は検証・提出タスク。
- `docs/tasks/rcd-03-task-monitor-engine.md` に詳細化。

## Verification

- [ ] `mise run test` で `monitor_gate`・`monitor_engine`
      テストを含む全テスト pass。
- [ ] `mise run test-rt` で RT 関連テスト pass。
- [ ] canonical 9 lane すべて exit 0。
- [ ] `review-protocol` で blocker なし。

## Open Questions / Gaps

- `RoutingEngine` fixture の構築に必要な lower/prepare 経路は
  `l1_e2e.rs` の `HeadlessDriver` 構築手順を踏襲。`RoutingEngine`
  は app 経路のため、fixture では `HeadlessDriver` を主対象とし
  `RoutingEngine` は trait impl の存在確認に留めるか、両方を
  pin するかは実装時に既存テストの再利用度で判断。
- `TransportPhase` の block 単位供給は caller 責務 — fixture
  は run 間で phase を固定し、block 内相変化は対象外。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
      consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一 worker、依存鎖が
  直列のため分割不要）。
- Independent work streams: none
- User approval required before dispatch: no（coordinator の
  review-protocol で代替）
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute
  task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- reviewer は `monitor_engine.rs` の fail-closed 契約
  （segment tiling・input 未供給・session 位置ずれ）と
  pump cadence emulation の正しさ、`plan.sample_rate` 追加が
  既存 `preflight` 呼び出しを壊さないことを重点確認。
