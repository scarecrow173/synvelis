---
id: "ADR-034UqJ2qZNzuxcpsia82P3"
type: "adr"
status: "accepted"
title: "SV-GUI-04 slice2 dock transfer: workspace adoption seam と dock/UI 権威分界、Destroyed policy"
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
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
  implemented-by: []
  depends-on:
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
    - "docs/adr/sv-gui-04-01-two-window-second-view.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md"
    - "docs/plans/implement-sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
    - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
    - "docs/designs/workspace-docking-state-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# SV-GUI-04 slice2 dock transfer: workspace adoption seam と dock/UI 権威分界、Destroyed policy

## Context and Problem Statement

Issue #113（SV-GUI-04 Multiwindow Workspace/Dock）の dock transfer
（Detach→Reattach・move abort/rollback）を GUI op として配線する
には、headless で検証済みの `WorkspaceManager` dock 契約を製品
app の live window/view へ接続する設計が必要である。現行 main
（`ab235a5` 以降）では次の 4 つの断絶がある:

1. **host 固定**: `SynvelisApp.workspace` は
   `WorkspaceManager<ShellWindowHost>` に固定され、
   `ShellWindowHost::create_window` は `BackendUnavailable` を返す。
   `WorkspaceManager` の `create_window_at` は host 成功後にのみ
   `dock.register_window` + `WindowRecord` を行うため、製品 app で
   workspace の window/dock 経路は一度も登録されない（dormant）。
2. **adoption seam 欠如**: window/view の生成権威は `WindowDriver`
   （RF386、両 lane 共通）と composition root（`NativeShell::
   attach_view` → `syn.view.attach`）にあるが、`synvelis-workspace`
   には driver が発行した既存 `WindowId` や mount 済み view を
   `WorkspaceManager` へ取り込む API がない。
3. **二重の真実**: dormant な `WorkspaceManager.dock`
   （`DockWorkspace` + `ViewRegistry` + `ViewStateRegistry` +
   `WindowRecord`）と live な `ui.views`/shell `views`/
   `ControlsUi`/`VizUi`（実 UI binding・generation・subscription）
   の責務分界と同期規則が未定義。
4. **Destroyed policy 未決**: `NativeShell::on_destroyed` は孤児
   view に `syn.view.close` を replay して UI state を破棄する。
   `WorkspaceManager::on_window_event(Destroyed)` は
   `WindowCloseDisposition`（既定 `DetachViews`）を実装済みだが、
   製品経路からは呼ばれない。

これらを解消しない限り、`begin_transfer`/`commit_transfer`/
`abort_transfer` の headless 契約を GUI op へ写像できない。

## Decision Drivers

* **単一の生成権威**: window 生成は既に `WindowDriver`（両 lane:
  webview `run_multi` / GPUI `GpuiDriver`）が所有し、view の UI
  mount は `NativeShell::attach_view` が所有する。workspace が
  第二の生成経路（`host.create_window`）を製品で有効化すると、
  ID 発行・Ready タイミング・破棄の権威が二重化する — 今回解消
  しようとしている「二重の真実」と同型の問題を新たに作る。
* **headless 契約の保存**: `tests/sv113_dock.rs` が固定した
  `HeadlessWindowHost` + `WorkspaceManager` の commit/abort/detach
  semantics を GUI 経路へそのまま写像し、再実装しない。
* **同期 op の atomicity**: `UiRuntime`/`AppService`/`SynvelisApp`
  は単一 writer の同期 dispatch であり、1 op 内の複数 step は外部
  から不可分に見える。window 生成だけが driver event loop 経由の
  非同期であり、そこだけ two-phase 化が必要。
* **非破壊保証**: 「Dock/Window close で Project/Graph/Recording/
  Job が暗黙停止しない」（Issue 受入・ユーザー指定）を view 層へ
  拡張し、window 破棄で view・view state を暗黙破棄しない既定を
  選ぶ。
* **最小の整合的変更**: adoption seam は `WorkspaceManager` の
  既存 bookkeeping（`WindowRecord`/registry/states/`next_view`）を
  再利用し、driver 共有参照や host generic 化といった所有権の
  再構成を避ける。

## Considered Options

* **Option 1（採用）: adoption seam + 権威分界 + DetachViews 既定**
  - `synvelis-workspace` に `WorkspaceManager::adopt_window` /
    `adopt_view` を追加し、driver/composition root が生成した
    window/view を登録する。`SynvelisApp.workspace` の host 型は
    `ShellWindowHost` のまま維持する。
  - `WorkspaceManager.dock` = 論理配置・per-view workspace state・
    transfer token・window record の権威。`ui.views`/shell
    `views`/`ControlsUi`/`VizUi` = live UI binding の権威。
    同期は新設 `syn.window.dock_*` op と `syn.view.*` の拡張で
    一箇所に集約する。
  - `Destroyed` は `WindowCloseDisposition::DetachViews` 既定で
    view を detached pool へ退避させ、`syn.view.close` replay を
    やめる。
* **Option 2: `WindowDriver` → `WindowHandle` adapter で host を
  driver-backed にする**
  - `Rc<RefCell<dyn WindowDriver>>` を `WindowHandle` として実装し
    `WorkspaceManager<Adapter>` にすれば `create_window`/
    `detach_to_new_window`/`request_close` がそのまま動く。
  - しかし driver は bin が所有し `NativeShell` へ `&mut` で貸す
    構造であり、sealed `SynvelisApp` 内へ共有 mutable 参照を通す
    再構成が必要。`WindowHandle::eval`（JS script）に driver 側の
    対応物がなく、`WindowConfig`（url/devtools）と `WindowSpec`
    の語彙差も adapter で吸収する必要がある。window 生成権威の
    二重化を解消するどころか、adapter が第三の窓口になる。
* **Option 3: `WorkspaceManager` を迂回し app が `DockWorkspace`
  を直接駆動する**
  - `WindowRecord`（veto/disposition/geometry）・`ViewStateRegistry`
  ・`next_view`・`capture`/`restore` の bookkeeping を app 側で
  再実装することになり、責務が分散する。`WorkspaceManager` が
  存在する意味が失われる。
* **Option 4（Destroyed policy の代替）: 現行維持 — `Destroyed` で
  `syn.view.close` replay**
  - view の UI state を window 寿命に紐付けたままにする。
  `workspace-docking-state-architecture.md` §7.1/§7.2 の「View を
  Window 寿命から分離する」製品方針、および Detach→Reattach の
  Issue 受入と矛盾する。

## Decision Outcome

**Option 1 を採用する。** 理由: window/view の生成権威は既に
`WindowDriver` と composition root にあり、adoption seam はその権威を
変えずに workspace の bookkeeping だけを接続する最小の境界である。
transfer 系 API（`begin_transfer`/`commit_transfer`/`abort_transfer`/
`move_view`/`detach_to_pool`/`on_window_event`）は `host` に一切
触れないため、adoption さえ済めば `ShellWindowHost`・
`HeadlessWindowHost` の両方で同一 semantics が動く — host 型の
generic 化は本 slice の要件を満たさないまま所有権再構成だけを
持ち込む。

### 決定事項

**D1. `synvelis-workspace` に adoption seam を追加する。**
`WorkspaceManager` に `adopt_window` / `adopt_view` を新設する
（署名・error・`next_view` 前進規則は design doc で確定）。
window は `dock.register_window` + `WindowRecord`（既定
`DetachViews`・`veto_close: false`）、view は `registry.register` +
`states.ensure` + dock 配置（`DockTarget` または detached pool）を
行い、`mint_view` 由来の `next_view` を採用 id 以上へ前進させて
将来の `spawn_view` との衝突を防ぐ。重複 adoption は拒否する
（silent overwrite 禁止）。`host.create_window` は呼ばない —
window は既に driver が生成済みであることが前提。

**D2. dock/UI の権威を分界し、同期を op 層に集約する。**
`WorkspaceManager`（`dock`/`registry`/`states`/`windows`）を
**論理配置と per-view workspace state の権威**、`ui.views`/
shell `views`/`ControlsUi`/`VizUi` を **live UI binding の権威**
とする。不変条件: docked ⇔ UI attached、detached pool ⇔ UI
detached、transferring ⇔ 編集 lock（`ViewBusy`）。この対応を保つ
writer は service op 層に限定し、`syn.view.attach`/`detach`/`close`
を dock-aware に拡張、移動は新設 `syn.window.dock_*` op が両側を
1 dispatch 内で更新する。`syn.view.*` の外部契約（params/result
形状）は additive にのみ変更する。

**D3. `Destroyed` の既定 policy を `DetachViews` とする。**
window `Destroyed` 時、記録された `WindowCloseDisposition` に従い
`WorkspaceManager::on_window_event` を呼ぶ。既定 `DetachViews`
では view は detached pool へ退避し、UI 側は `syn.view.detach`
相当（gesture cancel・generation bump・subscription 解放）のみを
行い `syn.view.close` は replay しない。`ReleaseViews` は app
quit・明示的 close-all 等「view 破棄が意図された」経路に限定し、
`RelocateTo` は #177 系の layout restore slice に defer する。
`CloseRequested` の veto 判定は close-veto decision フロー
（別 dispatch `ctx_2aed081be01f`）が `WindowRecord.veto_close`
を参照する形で接続する前提を明記する（現行 main では
`respond_close(Approve)` 固定 — 差分前提）。

**D4. window 生成失敗・move 失敗の安全復帰を two-phase +
app-level pending state で実現する。**
`begin_move` は destination window が dock 登録済みであることを
要求するため、新規 window 宛の detach は「driver で window 生成
→ `adopt_window` → `begin_transfer` → Ready + view mount →
`commit_transfer`」の非同期 saga となる。生成失敗・Ready 前の
破棄・attach 失敗では `abort_transfer`（view は source ownership
維持）+ window 破棄で rollback し、`commit_move` の
`TargetUnavailable` 自動 rollback と整合させる。saga の途中状態
は `UiOpsState` の pending move map が保持し、view は
`ViewBusy` 相当で編集 lock する。

**D5. op 名は `syn.window.dock_*` family とする。**
`dock_prepare` / `dock_commit` / `dock_abort` / `dock_detach` /
`dock_reattach` / `dock_prepare_new_window`（名称は design doc で
確定）を service op として追加し、task spec の候補名
`syn.window.dock_prepare`/`commit`/`abort` を核に据える。
`MoveToken` は wire に出さず、app が view_id → token の対応を
内部 map で保持する（`MoveToken` の内側 field は private であり、
直列化経路を新設しない）。

### Consequences

* Good, because window/view 生成権威が `WindowDriver` +
  composition root に一元化されたまま、workspace の論理 dock が
  製品で初めて live になる。headless 契約と GUI op の対応が
  1:1 で追跡できる。
* Good, because `Destroyed` → `DetachViews` により、window を
  閉じても view・view state・project/job が暗黙破棄されず、
  Issue #113 の非破壊受入と `workspace-docking-state-architecture`
  §7.1/§7.2 に整合する。
* Good, because adoption seam は `synvelis-workspace` 内で完結し、
  `crates/synvelis-app` 側は op 追加と lifecycle forwarding に
  限定される。`HeadlessWindowHost` を使う既存 test は無変更で
  通る。
* Bad, because `syn.view.attach`/`detach`/`close` が dock-aware に
  なり、op 層に「UI binding と論理配置を同時に更新する」責務が
  加わる。同期規則の bug は両権威の drift として顕在化し得る
  （design doc の不変条件と fixture test で抑止する）。
* Bad, because `WindowDriver` → `WindowHandle` adapter を作らない
  ため、`WorkspaceManager::detach_to_new_window` 等の
  host 依存 convenience は製品経路では使えず、GUI detach は
  shell orchestration の手順として残る（D4）。
* Bad, because `ui.views`/`shell.views` の detach 時の window
  association 表現（pool marker）と `syn.view.list` の attached
  区別に小さな additive 変更が必要になる。

### Confirmation

* `WorkspaceManager::adopt_window`/`adopt_view` が
  `host.create_window` を呼ばず、`dock`/`windows`/`registry`/
  `states`/`next_view` を正しく更新する（synvelis-workspace
  単体 test）。
* `syn.window.dock_*` op が prepare→commit/abort の全経路で
  `ui.views`/shell `views`/`ControlsUi`/`VizUi` と `WorkspaceManager.
  dock` を同一 dispatch 内で整合させる（synvelis-app fixture
  test）。
* window `Destroyed` が `DetachViews` 既定で view を pool へ退避
  し `syn.view.close` を replay しないこと、`ReleaseViews` が
  quit/明示経路に限定されること（fixture + env-gated native
  evidence は後続 slice）。

## Implementation Plan

* Affected paths: `crates/synvelis-workspace/src/workspace.rs`
  （adopt API）、`crates/synvelis-workspace/src/error.rs`
  （必要なら error variant）、`crates/synvelis-app/src/ui_ops.rs`
  （`syn.window.dock_*` op・pending move map・`UiOpsState`）、
  `crates/synvelis-app/src/service.rs`（op 登録）、
  `crates/synvelis-app/src/app.rs`（workspace 公開 accessor・
  adoption 呼出し）、`crates/synvelis-app/src/shell.rs`
  （lifecycle forwarding・Destroyed disposition・new-window
  orchestration）、`crates/synvelis-app/src/screens.rs`・
  `src/bin/*`（必要に応じ mount 配線）、対応 test。
  本 ADR は docs slice でありコード変更は後続 dispatch が行う。
* Patterns to follow: `syn.view.*` op の params/result 形状と
  `ErrorBody`/`codes` 規約、`ControlsUi::attach_view` の
  implicit detach(dock-transfer)+generation bump、`shell.attach_view`
  の composition-root mount seam、`on_destroyed` の replay→
  disposition 置換、`workspace-docking-state-architecture.md` §7。
* Patterns to avoid: `host.create_window` の製品経路有効化、
  `MoveToken` の wire 直列化、`syn.view.*` 外部契約の破壊的変更、
  `ui.views` と `WorkspaceManager.dock` の片側だけを更新する op、
  `syn.view.close` replay の温存（DetachViews 既定と矛盾）。
* Dependencies or configuration: なし（既存 `ruviform-ui` 契約型
  のみ使用。`Cargo.toml`/`Cargo.lock`/`mise.toml` 変更なし）。

## Verification

* [ ] `WorkspaceManager::adopt_window`/`adopt_view` の単体 test:
  重複拒否・`next_view` 前進・pool/window 両 target・
  `host.create_window` 非呼出し。
* [ ] `syn.window.dock_prepare`→`dock_commit` で view の論理 owner
  と `shell.views`/`ui.views` の window が一致し、`MoveCommitted`
  event が drain される。
* [ ] `dock_abort` / commit 時 `TargetUnavailable` で view が
  source ownership・UI binding を維持する（rollback）。
* [ ] `dock_prepare_new_window` + window 生成失敗で view が
  source に残り孤児 window が残らない。
* [ ] window `Destroyed`（DetachViews）で view が pool へ退避し
  UI binding が detach 相当になり、`syn.view.close` が replay
  されない。`ReleaseViews` 経路では view が除去される。
* [ ] `mise run check`/`lint`/`test`/`fmt-check` PASS（実装 slice
  で実施 — 本 docs slice では docs index/lint のみ）。
* [ ] headless lane の Pass を実 OS Gate（#114）へ昇格しない —
  env-gated native evidence は別 slice で `SYNVELIS_SECOND_WINDOW`
  系 gate として記録する。

## Pros and Cons of the Options

### Option 1（採用）

* Good, because 生成権威の一元性・headless 契約保存・op atomicity
  を同時に満たし、変更面が workspace API + app op/shell に限定
  される。
* Bad, because dock/UI 同期責務が op 層に集中し、実装・test の
  密度が上がる。

### Option 2

* Good, because `detach_to_new_window`/`request_close` 等の
  host 依存 convenience が製品でそのまま使える。
* Bad, because sealed app への driver 共有参照・`WindowConfig`/
  `WindowSpec`/`eval` の語彙差吸収・生成タイミング差の解消が
  必要で、変更が所有権の根幹に及ぶ。YAGNI — adoption で本 slice
  の要件は満たせる。

### Option 3

* Good, because `WorkspaceManager` を触らず app 内で完結する。
* Bad, because `WindowRecord`/registry/states/snapshot の
  bookkeeping を重複実装し、責務分散そのものが今回の blocker
  #3 を悪化させる。

### Option 4

* Good, because 変更が最小（現行維持）。
* Bad, because view を window 寿命に縛り続け、Detach→Reattach
  と非破壊受入に反する。

## More Information

- Spec: `docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md`
- Design: `docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md`
- Plan: `docs/plans/implement-sv-gui-04-slice2-dock-transfer-workspace-adoption.md`
- 設計正本: `docs/designs/workspace-docking-state-architecture.md`
  §7.1 Detach/Reattach・§7.2 Close
- 前提契約: `docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md`、
  `docs/adr/sv-gui-04-01-two-window-second-view.md`
- Ruviform 契約: `ruviform-ui::dock`（`DockWorkspace`/`MoveToken`/
  `WindowCloseDisposition`/`DockEvent`）、`ruviform-ui::window`
  （`WindowHandle`/`WindowId`/`CloseDecision`）、`ruviform-ui::driver`
  （`WindowDriver`/`WindowLifecycle`、RF386）。本スライスでは変更しない。
- 差分前提: close-veto decision フローは別 dispatch
  `ctx_2aed081be01f` が `shell.rs`/`service.rs` 側で実装中であり、
  main 未着の可能性がある。本 ADR は現行 main の構造
  （`CloseRequested` → `respond_close(Approve)` 固定）に対して
  書き、D3 の veto 接続点を `WindowRecord.veto_close` と明記する。
