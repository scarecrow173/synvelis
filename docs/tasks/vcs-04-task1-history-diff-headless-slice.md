---
id: "TASK-VCS04Hist0ryV1ewTsk01"
type: "task"
status: "done"
title: "VCS-04 slice1 タスク分解 — History/Visual Diff headless"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/95"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/vcs/history.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/vcs_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/vcs04_history.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/vcs04_history.rs"
      - type: "file"
        path: "docs/specs/vcs-04-history-visual-diff-ui.md"
      - type: "file"
        path: "docs/designs/vcs-04-history-visual-diff-ui-design.md"
      - type: "file"
        path: "docs/plans/vcs-04-history-visual-diff-ui.md"
      - type: "file"
        path: "docs/tasks/vcs-04-task1-history-diff-headless-slice.md"
      - type: "file"
        path: "docs/test-specs/vcs-04-history-diff-fixtures.md"
      - type: "file"
        path: "docs/impl/ir/vcs-04-history-visual-diff-ui.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/vcs/mod.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/view.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/workspace.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vcs-04-history-visual-diff-ui.md"
  implemented-by:
    - "docs/impl/ir/vcs-04-history-visual-diff-ui.md"
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

- [x] `VcsHistoryAdapter` が committed/staged snapshot・history・
  relations を 1 binding で解決し、foreign repository を拒否する
  （PR #252 merge 済み・`vcs/history.rs` src unit test 6 件で固定）
- [x] `VcsHistoryViews` が per-view selection/pin/mode/cursor を保持し、
  `ChangeSetCache` を view 間で共有する（workspace
  `vcs04_history.rs` 13 test で固定）
- [x] `VcsHistory` view の state が Dock move/detach/reattach/2-window
  で `ViewState` と同規則で生存し、ReleaseViews で除去される
- [x] stale（Live revision 前進・index generation 変動・Saved HEAD
  移動）が Fresh と区別され、Committed pin は immutable のまま
- [x] `syn.vcs.view`/`syn.vcs.visual` が登録され、全 op が project/
  git/undo/transport を不変に保つ（app `vcs04_history.rs` 9 test で
  固定）
- [x] History screen spec が mount 可能で、Unknown/Partial/Stale を
  verbatim に表示する（VCS-06 UI matrix `vcs06_ui_matrix.rs` が
  Unknown/Partial 行を補強）
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt` が pass
  （merge 後の doc closeout slice で canonical 再実測し全 exit 0 —
  下記 Verification・impl record 参照）
- [x] impl record・index 再生成・commit/push・PR（`Refs #95`）

## Verification

- [x] `mise run fmt-check` — Pass（exit 0）
- [x] `mise run check` — Pass（exit 0）
- [x] `mise run lint` — Pass（exit 0・warning 0）
- [x] `mise run test` — Pass（exit 0・workspace `vcs04_history.rs`
  13/13・app `vcs04_history.rs` 9/9・`vcs06_ui_matrix.rs` 4/4 を含む）
- [x] `mise run test-rt` — Pass（exit 0）
- [x] `mise run test-msrv` — Pass（exit 0・rust 1.89.0）
- [x] `mise run check-app-native`/`lint-app-native` — Pass（存在
  するため実行・全 exit 0）
- [ ] 実 OS/GPUI・実 Git は NotRun（#114 gate・実機検証）—
  Pass 扱いしない。VCS-UI-05 は #114 で計測、native mount 配線・
  production `set_vcs_backend` 配線は design 明示の後続 slice

## Sources and Relations

- [spec](../specs/vcs-04-history-visual-diff-ui.md) /
  [design](../designs/vcs-04-history-visual-diff-ui-design.md) /
  [plan](../plans/vcs-04-history-visual-diff-ui.md) /
  [test-spec](../test-specs/vcs-04-history-diff-fixtures.md)
