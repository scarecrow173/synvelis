---
id: "TASK-034X1GJrKQRwmioEuJ4m78"
type: "task"
status: "completed"
title: "RCD-03 monitor/punch fake session fixture"
created: "2026-11-13"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-03-monitor-punch-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-03-monitor-punch-plan.md"
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
    - "docs/test-specs/rcd-03-monitor-punch.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 monitor/punch fake session fixture

## Work

`crates/synvelis-recording/tests/monitor_punch_session.rs` を
新規作成し、`DeviceRig`（fake device → `InputSession` →
`StreamWriter` → `StagingArea`）と `MonitorProbe` を同一
block 駆動で合成する Fake Session fixture を実装する。
`PreparedRecordingPlan` は非 Clone のため capture 側・monitor
観測側に同一 spec から 2 個構築する。4 test で route/capture
coherence・raw/commit 独立性・device fault の monitor 非依存
性・punch 中 loss→fail-stop→新規 session recovery・2 session
fault 分離を pin する。`src/` 変更は行わない。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/rcd-03-monitor-punch.md`
- `docs/designs/rcd-03-monitor-punch-design.md`
- `docs/plans/rcd-03-monitor-punch-plan.md`
- `docs/test-specs/rcd-03-monitor-punch.md`

## Dependencies

- PR #214（`decide`/`MonitorProbe`/`GateScheduler`）— merge 済み。
- PR #373（`MonitorAdapter` engine 搭載）— merge 済み。
- RCD-01/RCD-02 の `DeviceRig`/staging/commit 基盤 — merge 済み。

## Affected Files

- `crates/synvelis-recording/tests/monitor_punch_session.rs`（新規）
- `evidence/sv117/rcd-03-monitor-punch-session.md`（新規）
- `docs/` doc-chain 7 文書 + docs index（再生成）

## Done When

- [x] `monitor_punch_session.rs` の 4 test が pass し、
      route 区間と capture gate 区間の一致・Raw File 独立性・
      fault 挙動の monitor 非依存性・loss→recovery・session
      分離・commit payload の monitor 非依存性を pin する。
- [x] `evidence/sv117/` に Expected/Observed・fake Pass・
      実機/GUI NotRun を記録する。
- [x] canonical 検証が exit 0。
- [x] review-protocol で blocker なし。

## Verification

- [x] `mise run test synvelis-recording` — 全 test pass。
- [x] `mise run fmt-check` — exit 0。
- [x] `mise run check synvelis-recording` — exit 0。
- [x] `mise run lint synvelis-recording` — exit 0。
- [x] `review-protocol` レビュー実施・blocker 解消。
