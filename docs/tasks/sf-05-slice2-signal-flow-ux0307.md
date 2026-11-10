---
id: "TASK-hB3uTuJhPxgOVjGdWdSOVj"
type: "task"
status: "done"
title: "SF-05 slice2 タスク分解 — Signal Flow gesture/inspection/persist"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-05-slice2-signal-flow-ux0307.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-05-slice2-signal-flow-ux0307.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md"
    - "docs/test-specs/sf-05-slice2-signal-flow-ux0307.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-slice2-signal-flow-ux0307.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice2 タスク分解 — Signal Flow gesture/inspection/persist

## Work

Issue #108 の slice-2（SF-UX-03/04/07）。Port Drag routing
gesture（fixation/dry-run/consent/atomic commit/cancel）・
Signal Flow 専用 inspection focus（Follow/Explore/Pinned・
link 追従・session-only 履歴）・`SignalFlowViewState` の
`ViewInstanceId` 単位 persist（schema v2→v3・safe recovery）
を workspace + app に実装し、headless fixture で検証する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/sf-05-slice2-signal-flow-ux0307.md`
- `docs/adr/sf-05-slice2-routing-gesture-inspection-persist-boundary.md`
- `docs/designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md`
- `docs/plans/sf-05-slice2-signal-flow-ux0307.md`
- `docs/test-specs/sf-05-slice2-signal-flow-ux0307.md`

## Dependencies

- SF-05 slice-1（read model/visual state/ops）・SF-01/SF-02
  （routing model/transaction）・SF-03 slice5（session engine）
  ・PC-C5-02（gesture fixation/inspector mode 先例）の main
  済み契約。
- ブランチ `scarecrow173/sv108-sf05-ux0307`（`origin/main`
  起点）。

## Affected Files

- `crates/synvelis-workspace/src/routing_gesture.rs`（新規）
- `crates/synvelis-workspace/src/signal_flow_ui.rs`
- `crates/synvelis-workspace/src/state.rs`
- `crates/synvelis-workspace/src/workspace.rs`
- `crates/synvelis-workspace/src/link.rs`
- `crates/synvelis-workspace/src/persist.rs`
- `crates/synvelis-workspace/src/lib.rs`
- `crates/synvelis-workspace/tests/sf_05_s2.rs`（新規）
- `crates/synvelis-app/src/ui_ops.rs`
- `crates/synvelis-app/src/app.rs`
- `crates/synvelis-app/src/service.rs`
- `crates/synvelis-app/tests/sf05_signal_flow.rs`
- `docs/` チェーン一式

## Done When

- [x] `routing_gesture_*` が fixation/hover/drop/confirm/cancel
  を提供し、stale revision/capability/window・prohibited・
  consent を正しく扱う
- [x] `signal_flow_*` inspection API が Follow/Explore/Pinned・
  session-only 履歴・FollowSource filter を提供する
- [x] link delivery が明示変換成功時のみ Signal Flow Follow
  focus を更新し、selection/arrangement focus を変えない
- [x] `ViewState::signal_flow` が schema v3 で per-view 永続化
  され、v2→v3 migrate・malformed degrade が動く
- [x] `syn.signal_flow.inspection`/`routing` op が登録され、
  wire shape が fixture で検証される
- [x] `tests/sf_05_s2.rs`（18件）・`tests/sf05_signal_flow.rs`
  （10件）が pass
- [ ] `mise run fmt-check`/`check`/`lint`/`test` が pass
  （app-native/test-rt/test-msrv は存在範囲で実行）
- [ ] docs index 再生成・commit/push・PR（`Refs #108`）

## Verification

- [ ] `mise run fmt-check` exit 0
- [ ] `mise run check` exit 0
- [ ] `mise run lint` exit 0
- [ ] `mise run test` exit 0
- [ ] `mise run test-rt`/`check-app-native`/`lint-app-native`/
  `build-app-native`/`test-msrv` — 存在すれば実行、不能なら
  NotRun 記録
- [ ] 実 OS/GPUI は NotRun（#114 gate）

## Sources and Relations

- [spec](../specs/sf-05-slice2-signal-flow-ux0307.md) /
  [ADR](../adr/sf-05-slice2-routing-gesture-inspection-persist-boundary.md) /
  [design](../designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md) /
  [plan](../plans/sf-05-slice2-signal-flow-ux0307.md) /
  [test-spec](../test-specs/sf-05-slice2-signal-flow-ux0307.md)
