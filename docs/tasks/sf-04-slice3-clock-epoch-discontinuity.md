---
id: "TASK-034UL3a4aL5H7PC2FRWUp7"
type: "task"
status: "in-progress"
title: "SF-04 slice3 T1 — probe clock epoch discontinuity・capture segment cut・fault/DSP invariance"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/plans/sf-04-slice3-clock-epoch-discontinuity.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-04-slice3-clock-epoch-discontinuity.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-04-slice3-clock-epoch-discontinuity.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice3 T1 — probe clock epoch discontinuity・capture segment cut・fault/DSP invariance

## Work

1. `synvelis-project`: `CaptureSegmentEnd::ClockEpoch` 追加 +
   `ProbeCaptureSegment` doc の境界明文化（generation 境界または
   cursor clock-epoch 変化を跨がない）。
2. `synvelis-player`: `ProbeSession::bump_clock_epoch`（Ruviform
   `ProbeBroker::bump_epoch` 委譲）/`time_context`、drain 側の
   epoch 照合（`observe_meta`/`observe_status` +
   `cut_segment_on_epoch_change` — lazy cut・triggering record
   破棄・closing kind 除外・unwired clock no-op）。
3. `synvelis-player`: `RoutingEngine::seek` の epoch bump +
   `probe_time_context()` accessor。
4. テスト: `tests/sf04_s3_probe_discontinuity.rs`（5 件 —
   seek epoch・capture segment cut・DSP invariance・capacity・
   rebind channel-full atomicity/recovery）。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sf-04-slice3-clock-epoch-discontinuity.md`
- `docs/designs/sf-04-slice3-clock-epoch-discontinuity-design.md`
- `docs/specs/sf-04-slice3-clock-epoch-discontinuity.md`
- `docs/test-specs/sf-04-slice3-clock-epoch-discontinuity.md`

## Dependencies

- SF-04 slice1 Semantic Probe Adapter（PR #200・main マージ済み）
- SF-04 slice2 ControlSession probe trace（PR #215・main
  マージ済み）
- Ruviform #309 `ProbeTimeContext`/epoch/clock-mark（pin
  `0f5b5737` に含まれる）

## Affected Files

- `crates/synvelis-project/src/probe.rs`（`ClockEpoch` variant）
- `crates/synvelis-player/src/probe.rs`（epoch API + drain 照合）
- `crates/synvelis-player/src/routing_engine.rs`（seek bump +
  accessor）
- `crates/synvelis-player/tests/sf04_s3_probe_discontinuity.rs`（新規）

## Done When

- [x] seek 後の probe record が新 epoch を運び、capture segment が
      `ClockEpoch` で切断され旧 epoch context を保持する
- [x] epoch 境界を跨ぐ segment が存在せず、暗黙の再 capture が
      起きず、明示 `probe_capture` が新 session/epoch で開始する
- [x] Monitor/Capture probe が master PCM/CRC を変えない
- [x] bounded fault（capacity・rebind channel-full）が fail-closed
      で原子性を保ち次 publish で回復する
- [x] headless fixture が green

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check` — exit 0
- [x] `mise run lint` — exit 0
- [x] `mise run test` — exit 0
- [x] `mise run test-rt` — exit 0
- [x] `mise run test-msrv` — exit 0
- [x] `mise run check-headless-deps` — exit 0
- [ ] `mise run check-ruviform-rev` — exit 1（**既存 failure**:
      `ui/vendor/ruviform/REV` `20a24a66` vs `Cargo.lock`
      `0f5b5737`。両ファイルとも本 diff 未変更の pre-existing
      状態。`ui/`・`synvelis-app` は編集禁止範囲のため未対処・
      PR に記録）
