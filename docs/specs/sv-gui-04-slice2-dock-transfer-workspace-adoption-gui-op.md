---
id: "SPEC-034UqIcJay7uhO1XhAdcCn"
type: "spec"
status: "approved"
title: "SV-GUI-04 slice2 設計: dock transfer workspace adoption + GUI op 配線"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
    - "docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md"
    - "docs/plans/implement-sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice2 設計仕様: dock transfer workspace adoption + GUI op 配線

## What Should Be Built

Issue #113（SV-GUI-04 Multiwindow Workspace/Dock）の **dock transfer
設計スライス（docs only）**。Detach→Reattach・move abort/rollback を
GUI op として配線するために先行して必要な設計・ADR・plan の文書
チェーンのみを作成し、コードは一切変更しない。

main（`ab235a5` 以降）には以下が実装済みであることを前提とする:

- `HeadlessWindowHost`（`synvelis-app/src/host.rs`）— multi-window
  `WindowHandle` の headless 参照実装。`WorkspaceManager` の
  `create_window`/`spawn_view`/`begin_transfer`/`commit_transfer`/
  `abort_transfer`/`detach_to_new_window`/`reattach`/
  `on_window_event` が headless に完結することを `tests/sv113_dock.rs`
  が固定済み。
- `SYNVELIS_SECOND_WINDOW` env-gated 実 2-window mount
  （`screens.rs`/`bin/synvelis-native.rs`/`tests/two_window.rs`）。
- `NativeShell` の `windows` map + `views` map（`view_id → WindowId`）
  + `attach_view` + `origin_view` owner 検証 + `Destroyed` teardown。

本スライスの設計文書が決定すべき既知の設計ブロッカー（探索済み・
`docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md` の
"Deferred Design Concerns" に明示済み）:

1. `SynvelisApp.workspace` が `WorkspaceManager<ShellWindowHost>` に
   固定されており、`ShellWindowHost::create_window` は
   `BackendUnavailable` を返すため workspace の window/dock 経路が
   製品 app では一度も登録されない（dormant）。
2. `synvelis-workspace` に window/view の **adoption API** がない —
   `WorkspaceManager` は `host.create_window` 経由でのみ window を
   登録し、driver が発行した既存 `WindowId` や mount 済み view を
   取り込む経路がない。
3. dormant な `WorkspaceManager.dock`（論理配置・view state・
   transfer token）と live な `ui.views`/shell `views`/
   `ControlsUi`/`VizUi`（実 UI binding・generation・subscription）
   の二重の真実の責務分界と同期規則が未定義。
4. `Destroyed` 時の policy（view を閉じるか detach-to-pool か）が
   未決定 — 現行 `NativeShell::on_destroyed` は `syn.view.close`
   を replay して view を破棄する。

## Why Now

Issue #113 の受入条件は「Detach→Reattach、move abort/rollback、
Window 生成失敗・安全復帰」「Dock/Window close で Project/Graph/
Recording/Job が暗黙停止しない」を要求する。headless 契約層と
2-window mount が main に揃った今、残る断絶は「driver が作る実
window/view を workspace の dock モデルへどう接続するか」という
設計判断だけであり、実装 dispatch に先行して文書で確定させる
必要がある。app 層（`shell.rs`/`service.rs`）は close-veto
decision フローを別 dispatch（`ctx_2aed081be01f`）が編集中のため、
本設計は現行 main の構造に対して書き、差分前提を明記する。

## Users and Value

- 後続の実装 dispatch（workspace API 追加 → app op 配線 →
  fixture → env-gated 実測）: 設計が実装可能な粒度で文書化されて
  いるため、判断を再発明せずに着手できる。
- Issue #113 の受入証拠チェーン: headless 契約（commit/abort/
  detach）と GUI op 配線の対応関係が文書で固定され、headless
  lane の Pass を実 OS Gate へ誤昇格しない記録方針を維持できる。

## Scope

### In Scope

- `docs/` チェーンのみ: spec → ADR → design → plan の 4 文書と
  managed index の再生成。**コード・`Cargo.toml`・`mise.toml`・
  CI の変更は一切含まない**。
- 設計文書での決定事項:
  - host 型の扱い（`WorkspaceManager<ShellWindowHost>` 固定を
    generic 化 / adoption seam / 別経路のいずれにするか）。
    `HeadlessWindowHost` と `ShellWindowHost` の両方で transfer
    が動く方向性。
  - `synvelis-workspace` への `adopt_window`（または同等）seam
    の API 案と、既存 `dock transfer prepare/commit/abort`
    （HeadlessWindowHost 契約）との対応関係。
  - `WorkspaceManager.dock`（dormant）と live `ui.views`/shell
    `views` の真実の一本化方針（権威分界と同期規則）。
  - `Destroyed` 時の policy: close views vs detach-to-pool。
  - 実装 slice の分割案（plan）: synvelis-workspace API 追加 →
    app 配線（op 名案: `syn.window.dock_prepare`/`commit`/`abort`
    等）→ fake/headless fixture → env-gated 実測。
  - Issue 受入条件への対応表（Detach→Reattach・move
    abort/rollback・Window 生成失敗/安全復帰・非破壊保証）。

### Out of Scope

- 実装・テスト — **対象外**（後続 dispatch。本スライスは docs
  only）。task/test-spec/impl 文書も実装 slice に持ち越す。
- monitor 失踪 layout restore（#177）・DPI/IME（RF#388/#389/#114）・
  Plugin Dock（#176）・実 OS 証拠（#114）— **NotRun**。
- close-veto decision フロー本体（別 dispatch `ctx_2aed081be01f`
  の成果物。main 未着の可能性があるため設計は現行 main に対して
  書き、差分前提を明記する）。
- `crates/` 以下のコード、`Cargo.toml`/`Cargo.lock`、`mise.toml`、
  `.github/`、`.devin/` の変更。

## Requirements

- 設計は既存の `HeadlessWindowHost` 契約（`WindowHandle` port・
  monotonic `WindowId`・live-set・`begin_transfer`/`commit_transfer`/
  `abort_transfer`/`detach_to_new_window`/`reattach`/`on_window_event`）
  と整合し、headless lane で検証済みの semantics を GUI op 経路へ
  そのまま写像できることを文書で示す。
- `WorkspaceManager.dock` と live UI binding の二重管理について、
  「どちらが何の権威か」「同期はどの op/イベントで行うか
  「不整合時にどちらへ収束するか」を一意に決める。
- `Destroyed` policy は「Dock/Window close で Project/Graph/
  Recording/Job が暗黙停止しない」というユーザー指定（Issue 受入
  そのまま）を view 層へ拡張して満たす — window 破棄で view・
  view state を暗黙破棄しない既定を決める。
- Window 生成失敗時の安全復帰（作成失敗→rollback→source 維持）を
  `DockWorkspace` の `TargetUnavailable` 自動 rollback と整合する
  手順として記述する。
- 設計は「実装可能な粒度」— 後続 dispatch がファイル・API・op
  名・状態遷移を判断なしに実装へ移れる具体度を持つ。

## Acceptance Criteria

- [x] Detach→Reattach、move abort/rollback の設計が実装可能な
  粒度で文書化される（実装そのものは対象外）。
- [x] 設計が既存の `HeadlessWindowHost` 契約（commit/abort/detach）
  と整合することを文書で示す。

## Deferred Design Concerns

- dock transfer の実装・fixture test・env-gated 実測 | 本スライスは
  docs only、実装は後続 dispatch | 実装 slice 着手時 | 設計と実装の
  乖離が残る。
- `WindowDriver` → `WindowHandle` adapter（driver-backed host）による
  `WorkspaceManager::create_window`/`detach_to_new_window` の製品
  経路有効化 | window 生成権威は既に `WindowDriver` にあり、
  adoption seam で本 slice の要件は満たせる | `detach_to_new_window`
  convenience を製品で使いたい slice | GUI detach が手動 orchestration
  のまま残る。
- `WindowCloseDisposition::RelocateTo`・layout snapshot/restore
  （#177）・monitor 失踪復帰 | 別 Issue/別 dispatch の owner 範囲 |
  #177/#114 着手時 | close 時の view 救済が detach-to-pool のみになる。

## Sources and Relations

- Issue #113 受入条件のうち本スライスが担う範囲: 「Detach→Reattach、
  move abort/rollback、close-cancel、Window 生成失敗・IPC timeout、
  layout corruption/monitor 不足で安全な復帰」「Dock/Window close
  で Project/Graph/Recording/Job が暗黙停止せず、誤 Command・
  subscription/GPU 漏れなし」。
- 上流: `docs/specs/sv-gui-04-01-headless-window-handle-dock.md`
  （headless WindowHandle + dock fixture、approved・実装済み）、
  `docs/specs/sv-gui-04-01-two-window-slice1.md`（実 2-window
  mount、approved・実装済み）、
  `docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md`
  （`WorkspaceManager`/`DockWorkspace` 契約）。
- 設計正本: `docs/designs/workspace-docking-state-architecture.md`
  §7.1 Detach/Reattach・§7.2 Close（View を Window 寿命から分離する
  製品方針）。
- Ruviform 側契約: `ruviform-ui::dock`（`DockWorkspace`/
  `MoveToken`/`WindowCloseDisposition`/`DockEvent`）、
  `ruviform-ui::window`（`WindowHandle`/`WindowId`/`CloseDecision`）、
  `ruviform-ui::driver`（`WindowDriver`/`WindowLifecycle`、RF386）。
  本スライスでは変更しない。
