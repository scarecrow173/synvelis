---
id: "SPEC-VCS04NativeMount001"
type: "spec"
status: "approved"
title: "VCS-04 History/Visual Diff — native mount/dock 配線 + gpui-test slice（synvelis#95）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "ISSUE-95"
    - "ISSUE-7"
    - "ISSUE-114"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/vcs-04-native-mount-design.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/95"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-04-native-mount.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 History/Visual Diff — native mount/dock 配線 + gpui-test slice

## 目的

Issue #95 の残件 slice。PR #252 で統合済みの headless 基盤
（`VcsHistoryBackend`/`VcsHistoryAdapter`、workspace
`VcsHistoryViews`、app `syn.vcs.view`/`syn.vcs.visual` ops、
read-only `history_screen_spec`）を、製品 GPUI アプリの
`ViewHost`/`ViewMount`/`create_view_window` 経路へ実配線し、
`gpui-test`（`GpuiGuiTest`）で mount を headless 検証する。

前提 spec `SPEC-VCS04Hist0ryV1ewUI001` の「実 GPUI Window は
NotRun」とした残件のうち、driver-neutral な mount/dock 配線と
simulated GPUI scenario で検証可能な範囲を本 slice が担う。
実 OS GUI・2Window・IME・pixel・実 Plugin の受入は Issue #114、
VCS Domain Oracle は #97 であり本 slice の対象外。

## 入力と前提

- `synvelis-app::screens::history_screen_spec` / `VcsScreenState` —
  read-only 投影（status/selection/diff label + history/changes/
  issues の 3 read-only table）。
- `synvelis-app::screens::{mount_main_view, mount_secondary_view}` —
  session-owned `AttachEpochAllocator` から mint する
  `ViewHost` mount の先例（RF387 fresh-host 契約）。
- `synvelis-app::ui_ops` — `syn.view.attach`/`detach`/`close`、
  `syn.window.dock_*` saga、`syn.vcs.visual{open,close}`。
- `synvelis-app::shell::NativeShell` — driver-neutral shell。
  `ViewOp` result bookkeeping が dock-domain 判定を担う。
- `synvelis-workspace` — `ViewType::VcsHistory` registry entry、
  `adopt_view`/`remove_view`/dock pool。**書き込み側変更なし**
  （投影接続の読み取り調整のみ許容）。
- `ruviform-ui-gpui::GpuiGuiTest`/`GpuiObserver` —
  `create_view_window(WindowSpec, ViewMount)` mount seam、
  `SemanticGuiTest` port（resolve/observe/select/invoke/settle）。

## 中核契約

### dock-domain 解決の一般化

- 静的 `dock_view_info`（`view-1`/`view-2`）は composition root
  の予約 view のみを知る。`syn.vcs.visual{open}` が mint する
  `view-{n}` は workspace registry descriptor が metadata
  （kind/title）を持つため、`syn.view.*`/`syn.window.dock_*` は
  registry を fallback として解決する（`dock_domain_view`）。
- `view-{n}` 形で registry にも静的表にも無い id は **fail closed**
  —「UI attached だが dock 未登録」の半端状態を許さない。
- `syn.view.attach` の result は `dock_domain: bool` を返し、
  shell はそれを学習して detach/close/window-destroy の
  bookkeeping（pool vs legacy close replay）を決める。

### screen-state publication

- `SynvelisApp::vcs_screen_cell()` が登録済み
  `ViewType::VcsHistory` view ごとに 1 つの `VcsScreenState` を
  publish する共有 cell（`main_screen_cell` と同じ hand-out 契約）。
- publish は post-op tail（`publish_project_length`）で走るため、
  VCS op と project 変更（Live endpoint を stale にする）の双方で
  mounted spec が最新投影へ追従する。
- close された view の entry は次回 publish で落ちる。

### mount

- `mount_vcs_history_view(attach_epochs, instance, state)` —
  `mount_screen_view` 経由で採用済み `ViewInstanceId` の host を
  fresh mint する。reserved root id は使わない。
- `synvelis-native` は `SYNVELIS_VCS_HISTORY` env gate で
  `syn.vcs.visual{open}` → publish → mount → `create_view_window`
  を行い、run loop が main screen と同じく VCS screen map の差分で
  `update_view` する。

### read-only 不変条件（前提 spec から継承）

- view navigation/attach/detach/close・pin/compare/refresh は
  project revision・undo/redo・transport・engine graph build・
  Git index/worktree・commit state を一切変更しない。
- `Unknown`/`Partial`/`Stale`/`no-backend`/`failed`/`cancelled`/
  `empty` は mounted spec 上でも区別された label/table として
  表示され、equal/empty に潰れない。

## 受入条件（本 slice）

- [ ] `syn.vcs.visual{open}` で開いた view が `syn.view.attach`
  経由で dock-domain view として attach/detach/reattach/close
  でき、registry descriptor が metadata を供給する
- [ ] `vcs_screen_cell` が view ごとの `VcsScreenState` を publish
  し、op 後・project 変更後に追従、close で除去される
- [ ] `mount_vcs_history_view` が session-owned epoch で
  `ViewHost` を構築し、`history_screen_spec`（read-only）を mount
  する
- [ ] `NativeShell`+`FakeDriver` で open→attach→window destroy
  （pool）→close が成立し、new-window dock saga
  （`dock_prepare_new_window`→`WindowRequest`→saga Ready
  attach+commit）が動的 view で完走する
- [ ] `GpuiGuiTest` scenario が mount→全 `vcs.*` component 解決→
  read-only（select/invoke 拒否・intent 0）→`update_view` 追従→
  detach stale→2 window remount→close を検証し、project authority
  不変を確認する
- [ ] 実 OS GUI/IME/pixel/plugin dock は `Unsupported`/`NotRun`
  のまま記録し、simulated pass を昇格させない

## 非目標

- 実 OS window・実 Git repo・実 remote・製品 E2E（#114）。
- Overlay / Evolution Map / Rendered Audio Diff。
- `synvelis-workspace` の書き込み側変更、Project apply/restore
  （VCS-05）、Git 書き込み系。
- production backend の自動 bind（`set_vcs_backend` の製品配線は
  後続 slice — 本 slice は注入済み backend を前提とする）。
