---
id: "PLAN-034UL6zrSYbZcDzr9MsHXB"
type: "plan"
status: "approved"
title: "SF-04 slice3 実装計画 — probe clock epoch discontinuity・capture segment cut"
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
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/designs/sf-04-slice3-clock-epoch-discontinuity-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice3 実装計画 — probe clock epoch discontinuity・capture segment cut Implementation Plan

## Goal

`RoutingEngine::seek` を起点に probe cursor-series の clock epoch を
進め、drain 側で open capture segment を `ClockEpoch` で切断し、
epoch 境界を跨ぐデータ帰属を fail-closed に固定する。あわせて
probe lifecycle の DSP 不変性と bounded fault の原子性を headless
fixture で固定する。

## Scope

単一実装ストリーム。`synvelis-project` の契約 variant 追加、
`synvelis-player` の session/engine 配線、1 つの統合テストファイル
で不可分（契約 variant 無しでは segment end を表現できず、
consumer 無しでは epoch 経路を検証できない）。

## Upstream Documents

- `docs/specs/sf-04-slice3-clock-epoch-discontinuity.md`

## Design Inputs

- `docs/designs/sf-04-slice3-clock-epoch-discontinuity-design.md`
- `docs/designs/sf-04-slice1-semantic-probe-adapter-design.md`
- `docs/designs/sf-04-slice2-controlsession-probe-trace-design.md`

## File Map

- `crates/synvelis-project/src/probe.rs` - `CaptureSegmentEnd::
  ClockEpoch` 追加、`ProbeCaptureSegment` doc 更新
- `crates/synvelis-player/src/probe.rs` - `ProbeSession::
  bump_clock_epoch`/`time_context`、`observe_meta`/`observe_status`
  の epoch 照合、`cut_segment_on_epoch_change`
- `crates/synvelis-player/src/routing_engine.rs` - `seek` の epoch
  bump、`probe_time_context()` accessor
- `crates/synvelis-player/tests/sf04_s3_probe_discontinuity.rs` -
  epoch/discontinuity・fault injection・DSP invariance fixture（新規）

## Implementation Sequence

- [x] spec → design → test-spec → plan → task 文書チェーン作成
- [x] `synvelis-project` `CaptureSegmentEnd::ClockEpoch` + doc
- [x] `synvelis-player` `ProbeSession` epoch API + drain 照合
- [x] `synvelis-player` `RoutingEngine::seek` bump + accessor
- [x] headless fixture `sf04_s3_probe_discontinuity.rs`（5 件）
- [x] mise 検証全通過
- [ ] commit/push/PR（`Refs #107`）

## Dependencies and Risks

- Ruviform `ProbeBroker::bump_epoch` は epoch increment →
  `wire_clock_mark` push の順で、channel full 時も broker epoch は
  進む — この非対称を契約として維持し補償しない（design 参照）。
- lazy cut の帰属境界: `SetClockMark` 適用前の旧 epoch record は
  正しく旧 segment に入る。bump 失敗時は deck が旧 epoch のまま
  record を出し続け segment は切れない — epoch が進んでいない
  record を新 epoch と見なさない正直な挙動。
- `ClockContextRef` 全体の等価で判定するため epoch 以外の成分
  （domain/runtime instance/sample rate）の変化でも切断される —
  cursor-series 同一性の変化を同一 span と見なさない契約どおり。

## Task Breakdown

- T1（本 plan）: 契約 variant + session/engine 配線 + 統合
  fixture。単一 task で実施（依存が直列で分割不能）。

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check` — exit 0
- [x] `mise run lint` — exit 0
- [x] `mise run test` — exit 0
- [x] `mise run test-rt` — exit 0
- [x] `mise run check-headless-deps` — exit 0
- [x] `mise run test-msrv` — exit 0
- [ ] `mise run check-ruviform-rev` — 既存不一致
  （`ui/vendor/ruviform/REV` vs `Cargo.lock`、本 diff 未変更の
  pre-existing。本 slice 範囲外として記録）

## Open Questions / Gaps

- なし（design で決定済み）。seek 以外の不連続宣言点・UI・実測は
  deferred concern として spec に記録済み。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker dispatch）
- Independent work streams: none
- User approval required before dispatch: no（review-protocol で
  blocker 無しを確認済みとして扱う）
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute task-by-task
  with review checkpoints.
- If declined or unavailable: execute inline using the same dependency
  graph and verification matrix.

## Review Handoff

- reviewer focus: lazy cut の帰属境界（triggering record の破棄・
  closing kind の除外・unwired clock の no-op）、bump 失敗時の
  broker/deck 乖離の扱い、`observe_status` での cut 順序
  （gap bookkeeping より先）、DSP invariance fixture の比較方法
  （2 engine・非 silence PCM）。
