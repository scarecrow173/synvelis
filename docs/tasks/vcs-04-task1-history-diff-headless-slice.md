---
id: "TASK-VCS04Hist0ryV1ewTsk01"
type: "task"
status: "in-progress"
title: "VCS-04 slice1 タスク分解 — History/Visual Diff headless"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/95"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vcs-04-history-visual-diff-ui.md"
  implemented-by: []
  depends-on:
    - "docs/plans/vcs-04-history-visual-diff-ui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
    - "docs/test-specs/vcs-04-history-diff-fixtures.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-04-history-diff-fixtures.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 slice1 タスク分解 — History/Visual Diff headless

## Work

Issue #95 の headless 完結 slice。VCS-01/02/03 契約を読む adapter
（project）と、History view の per-view state・A/B pin・stale 判定・
共有 ChangeSet・read-model 投影（workspace）、および ViewSpec +
`syn.vcs.*` op（app）を実装し、headless fixture で検証する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/vcs-04-history-visual-diff-ui.md`
- `docs/designs/vcs-04-history-visual-diff-ui-design.md`
- `docs/plans/vcs-04-history-visual-diff-ui.md`
- `docs/test-specs/vcs-04-history-diff-fixtures.md`

## Dependencies

- VCS-01（#92）Snapshot Resolver・VCS-02（#93）ChangeSet/Diff・
  VCS-03（#94）Git Read Provider の main 済み契約。
- SF-05（#108）`signal_flow_ui` read-only 投影先例、SV-GUI-04
  （#113）2-window/dock 基盤。
- ブランチ `feature/95-vcs04-history-ui`（`origin/main` 起点）。

## Affected Files

- `crates/synvelis-project/src/vcs/history.rs`（新規）
- `crates/synvelis-project/src/vcs/mod.rs`（export）
- `crates/synvelis-workspace/src/vcs_ui.rs`（新規）
- `crates/synvelis-workspace/src/view.rs`（`ViewType::VcsHistory`）
- `crates/synvelis-workspace/src/workspace.rs`（vcs registry 統合）
- `crates/synvelis-workspace/src/lib.rs`（export）
- `crates/synvelis-workspace/tests/vcs04_history.rs`（新規）
- `crates/synvelis-app/src/screens.rs`（VCS screen spec）
- `crates/synvelis-app/src/app.rs`/`ui_ops.rs`/`service.rs`/`lib.rs`
- `crates/synvelis-app/tests/vcs04_history.rs`（新規）
- `docs/` チェーン一式

## Done When

- [ ] `VcsHistoryAdapter` が committed/staged snapshot・history・
  relations を 1 binding で解決し、foreign repository を拒否する
- [ ] `VcsHistoryViews` が per-view selection/pin/mode/cursor を保持し、
  `ChangeSetCache` を view 間で共有する
- [ ] `VcsHistory` view の state が Dock move/detach/reattach/2-window
  で `ViewState` と同規則で生存し、ReleaseViews で除去される
- [ ] stale（Live revision 前進・index generation 変動・Saved HEAD
  移動）が Fresh と区別され、Committed pin は immutable のまま
- [ ] `syn.vcs.view`/`syn.vcs.visual` が登録され、全 op が project/
  git/undo/transport を不変に保つ
- [ ] History screen spec が mount 可能で、Unknown/Partial/Stale を
  verbatim に表示する
- [ ] `mise run fmt-check`/`check`/`lint`/`test` が pass
  （app-native は存在範囲で実行、不能は NotRun 記録）
- [ ] impl record・index 再生成・commit/push・PR（`Refs #95`）

## Verification

- [ ] `mise run fmt-check` exit 0
- [ ] `mise run check` exit 0
- [ ] `mise run lint` exit 0
- [ ] `mise run test` exit 0
- [ ] `mise run test-rt` exit 0
- [ ] `mise run test-msrv` exit 0
- [ ] `mise run check-app-native`/`lint-app-native` — 存在すれば実行、
  不能なら NotRun 記録
- [ ] 実 OS/GPUI・実 Git は NotRun（#114 gate・実機検証）

## Sources and Relations

- [spec](../specs/vcs-04-history-visual-diff-ui.md) /
  [design](../designs/vcs-04-history-visual-diff-ui-design.md) /
  [plan](../plans/vcs-04-history-visual-diff-ui.md) /
  [test-spec](../test-specs/vcs-04-history-diff-fixtures.md)
