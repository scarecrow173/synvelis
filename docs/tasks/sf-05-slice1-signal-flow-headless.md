---
id: "TASK-034UudL7KhVSvQn5M5gy5n"
type: "task"
status: "done"
title: "SF-05 slice1 タスク分解 — Signal Flow headless"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/plans/sf-05-slice1-signal-flow-headless.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-05-slice1-signal-flow-headless.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md"
    - "docs/test-specs/sf-05-slice1-signal-flow-headless.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-slice1-signal-flow-headless.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice1 タスク分解 — Signal Flow headless

## Work

Issue #108 の headless 完結 slice（SF-UX-01/02/05）。
`RoutingModel` の read-only 投影・visual-only 操作の非破壊性・
Revision/Generation 状態表示を workspace read model + app 表示/op
として実装し、headless fixture で検証する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/sf-05-signal-flow-headless-hierarchy.md`
- `docs/adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md`
- `docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md`
- `docs/plans/sf-05-slice1-signal-flow-headless.md`
- `docs/test-specs/sf-05-slice1-signal-flow-headless.md`

## Dependencies

- SF-01/SF-03/SF-04/PC-C5 の main 済み契約（routing model・
  activation receipt・source map・probe binding・relation read
  model 先例）。
- ブランチ `feature/108-sf05-headless`（`origin/main` 起点）。

## Affected Files

- `crates/synvelis-workspace/src/signal_flow_ui.rs`（新規）
- `crates/synvelis-workspace/src/lib.rs`（additive export）
- `crates/synvelis-workspace/tests/sf_05.rs`（新規）
- `crates/synvelis-app/src/engine.rs`（probe passthrough）
- `crates/synvelis-app/src/app.rs`（view/evidence/publish）
- `crates/synvelis-app/src/screens.rs`（tab/tables）
- `crates/synvelis-app/src/ui_ops.rs`（visual state + ops）
- `crates/synvelis-app/src/service.rs`（op 登録）
- `crates/synvelis-app/tests/sf05_signal_flow.rs`（新規）
- `docs/` チェーン一式

## Done When

- [ ] `signal_flow_ui` が compile し、scope/collapse/role filter/
  dedup/stale/unsupported を verbatim に運ぶ
- [ ] `syn.signal_flow.view`/`syn.signal_flow.visual` が登録され、
  visual op が session/project/engine を不変に保つ
- [ ] Signal Flow tab が main view spec に additive に現れ、
  status/coverage/generation を表示する
- [ ] `tests/sf_05.rs`・`tests/sf05_signal_flow.rs` が pass
- [ ] `mise run fmt-check`/`check`/`lint`/`test` が pass
  （app-native は存在範囲で実行、不能は NotRun 記録）
- [ ] impl record・index 再生成・commit/push・PR（`Refs #108`）

## Verification

- [ ] `mise run fmt-check` exit 0
- [ ] `mise run check` exit 0
- [ ] `mise run lint` exit 0
- [ ] `mise run test` exit 0
- [ ] `mise run check-app-native`/`lint-app-native`/
  `build-app-native` — 存在すれば実行、不能なら NotRun 記録
- [ ] 実 OS/GPUI は NotRun（#114 gate）

## Sources and Relations

- [spec](../specs/sf-05-signal-flow-headless-hierarchy.md) /
  [ADR](../adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md) /
  [design](../designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md) /
  [plan](../plans/sf-05-slice1-signal-flow-headless.md) /
  [test-spec](../test-specs/sf-05-slice1-signal-flow-headless.md)
