---
id: "TASK-7fK2mQ9xR4vN8pLs3tYw6u"
type: "task"
status: "done"
title: "SV-GUI-04 slice2 タスク分解 — dock transfer workspace adoption + GUI op 配線"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/implement-sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
  implemented-by: []
  depends-on:
    - "docs/plans/implement-sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
    - "docs/adr/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
    - "docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/implement-sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice2 タスク分解 — dock transfer workspace adoption + GUI op 配線

## Work

SV#113 dock transfer slice。merge 済み設計チェーン
（spec/ADR/design/plan — PR #208）に従い、headless で検証済みの
`WorkspaceManager` dock semantics を live GUI 経路へ接続する。
`adopt_window`/`adopt_view` seam、`syn.window.dock_*` op family、
dock-aware `syn.view.*`、NativeShell lifecycle forwarding、
`Destroyed` → `WindowCloseDisposition` 適用、new-window saga を
実装する。window/view の権威は `WorkspaceManager` 一箇所に保ち、
shell/UI 層に第二の権威を作らない。

## Classification

- kind: `feature`（設計済み contract の実装 slice）
- size: L（workspace seam + op 層 + shell lifecycle + fixture の
  4 層にまたがる）
- risk: 中〜高（view/window ownership の三重 bookkeeping
  （dock・`ui.views`・`shell.views`）の drift が dock transfer
  全体の正しさを左右する。busy lock・saga rollback・disposition
  の全経路を fixture で固定する）

## Subtasks

- [x] synvelis-workspace: `WorkspaceError::DuplicateWindow` +
  `adopt_window`/`adopt_view`（dock 先行登録・`next_view` 前進・
  重複拒否）+ `tests/adopt.rs` 8 case。
- [x] synvelis-app op 層: `UiOpsState.dock_moves`/`DockMove`/
  `WindowRequest`、wire codec、`syn.window.adopt`/`list`/
  `dock_prepare`/`dock_prepare_new_window`/`dock_commit`/
  `dock_abort`/`dock_detach`/`dock_reattach`/`lifecycle` 実装、
  `syn.view.attach`/`detach`/`close` の dock-aware 拡張
  （dock-domain scoping・`AwaitingWindow` 例外は lifecycle が
  名付けた destination のみ）、`service.rs` 登録。
- [x] synvelis-app shell: `on_windowed` lifecycle forwarding
  （`Created`/`Ready` は `dock_view` 付きで adopt）、
  `ViewOp::{DockCommit,DockDetach,DockAbort}` reconcile、
  `on_destroyed` disposition flow（`DetachViews`→detach replay・
  `ReleaseViews`→close replay・`syn.view.close` 非 replay）、
  `WindowRequest` drain + new-window saga（Ready で
  attach+commit、失敗で abort+destroy）。
- [x] screens.rs: `DockViewInfo`/`dock_view_info`（view-1/view-2
  metadata 解決）。
- [x] tests: `tests/dock_ops.rs` 12 case 新規、`tests/adopt.rs`
  8 case、`tests/close_veto.rs`/`tests/view_mount.rs` の
  `DetachViews` pool 契約への期待値更新。
- [x] docs chain: task/test-spec/impl record + index 再生成。
- [x] mise 全検証 + review-protocol + impl record。
- [x] commit / push `feature/113-dock-transfer-impl` / PR /
  worker_done。
