---
id: "DESIGN-034UogNAUQrEmBkgZnhrAk"
type: "design"
status: "approved"
title: "SV-GUI-04 slice1 precursor 設計 — HeadlessWindowHost + dock fixture"
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
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice1 precursor 設計: HeadlessWindowHost + dock fixture

## Context

`crates/synvelis-app/src/host.rs` は現在 `ShellWindowHost` のみを
提供する。これは製品 shell が所有する **単一 OS window** を
`ruviform_ui::window::WindowHandle` port に写像する adapter で、
`create_window` は `UiError::BackendUnavailable` を返す — webview
host も GPUI driver も「2 枚目の window はまだ存在しない」ことを
正直に報告する契約である。

一方 `WorkspaceManager<H: WindowHandle>`（`synvelis-workspace`）は
WS-A で multi-window/dock の headless 契約を完備しており、
`create_window`/`detach_to_new_window`/`reattach`/
`begin|commit|abort_transfer`/`set_close_veto`/`on_window_event` を
公開する。Issue #113 の核はこの契約を **app 層の host port** に
接続することだが、app 層（`SynvelisApp.workspace` は
`WorkspaceManager<ShellWindowHost>` に固定、`NativeShell` の
2 窓 mount、`screens.rs` の screen spec）は別 dispatch が編集中
であり、本スライスでは変更できない。

そこで本スライスは **契約層だけ** を確定する: app crate が将来の
実 backend に対して要求する multi-window `WindowHandle` の
headless 参照実装を `host.rs` に置き、`WorkspaceManager` と
`NativeShell`+`FakeDriver` の両経路で Issue #113 の核となる
不変条件を fixture test で固定する。

## Scope

### In Scope

- `HeadlessWindowHost`: `WindowHandle` を実装する in-memory の
  multi-window host。`WindowId` を単調発行し、live 集合と全呼出し
  記録を保持する。
- `tests/sv113_dock.rs`: `WorkspaceManager<HeadlessWindowHost>` と
  `NativeShell`+`FakeDriver` を駆動する fixture test 群。

### Out of Scope

- `ShellWindowHost` の変更、`SynvelisApp` の
  `WorkspaceManager<ShellWindowHost>` 型固定の解除、実 GPUI
  backend 接続、`WindowDriver` 側の新規実装（`FakeDriver` が既に
  存在する）、workspace 層の変更。

## Components and Boundaries

- **`HeadlessWindowHost`（`synvelis-app/src/host.rs`、新規）**
  - 責務: `WindowHandle` port の deterministic in-memory 実装。
    window identity の発行・live 管理・close flow・呼出し記録。
    OS window・webview・GPU に一切触れない（non-RT、UI thread
    前提の port contract をそのまま満たす）。
  - `ShellWindowHost` との関係: **置き換えではなく併存**。
    `ShellWindowHost` は「現在の製品 shell が実際に持つ 1 窓」の
    正直な写像であり続ける。`HeadlessWindowHost` は「multi-window
    契約が満たすべき semantics」の headless 参照実装で、後続
    スライスで GPUI `WindowDriver`/`WindowHandle` adapter が
    同じ semantics を実装する際の検証基準になる。
  - `synvelis_workspace::host::FakeWindowHost` との関係: workspace
    crate が持つ test fake と機能は近いが、責務層が異なる
    （workspace 内部 test 用 vs app 層の host port 契約）。
    app 側の契約を app crate に置くことで、`SynvelisApp` 周辺の
    後続実装が workspace の test 専用型に依存しない。

- **`tests/sv113_dock.rs`（新規）**
  - `WorkspaceManager<HeadlessWindowHost>` fixture: 2 window・
    view 配置・project 変更・window destroy・transfer・
    detach_to_new_window を駆動。
  - `NativeShell`+`FakeDriver` fixture: `syn.view.attach` を 2 窓に
    行い、`rvf.ui.command.v1` の `origin_view` cross-window
    拒否を `shell.rs` 既存 validation 経由で検証。

## Data and Control Flow

- window identity: `HeadlessWindowHost.next` が `u64` を単調採番し
  `WindowId::new`（nonzero）で発行。`live: BTreeSet<WindowId>` が
  生存集合、`windows()` はそれを返す。destroy/Approve 後も
  `next` は戻らない（`WindowId` の非再利用契約）。
- close flow: `request_close` は live window のみ記録
  （未知 window は contract 通り無視）。`respond_close` は未知
  window に `UiError::Host`、記録後 `Approve` で live から除去し
  `destroys` に記録。`Veto`/`Defer` は live を維持。
- project 独立性: `WorkspaceManager` は 1 つの
  `ProjectSession` を全 window で共有する構造的事実を test で
  固定する — window destroy は `on_window_event` の
  `DetachViews` disposition で view を detached pool に移すだけで、
  session/project/view registry/view state を触らない。
- origin 検証: `NativeShell::on_view_command`（`shell.rs` 既存、
  無変更）が `origin_view` を `views` map（attach 時に宣言された
  `window_id` で reconcile 済み）と照合し、不一致を
  `invalid_params` `Result` で拒否する。test はこの既存経路を
  `FakeDriver` の 2 window で駆動するだけで、新規コードを要さない。

## Non-Functional Constraints

- RT 契約: `WindowHandle` port は non-RT・UI thread 前提。
  `HeadlessWindowHost` も同じ（内部に `Rc`/`Cell` を持ち込まず
  plain data のみ）。
- `unsafe` 無し、追加依存無し（`ruviform-ui`・`serde_json`・
  dev-dep `ruviform-ui-test` のみ使用）。
- lint: workspace の `clippy::all`+`pedantic`・`missing_docs` warn
  を遵守（pub field/関数は全て doc コメント必須）。

## Risks and Trade-offs

- **FakeWindowHost との重複懸念**: semantics は類似するが、app
  crate が workspace の test fake に依存するより、app 層の契約を
  app crate に持つ方が依存方向として正しい（app → workspace の
  一方向は既存どおり）。将来 GPUI backend 実装時に app 側の
  契約が必要になるため、ここで確定させる。
- **`ShellWindowHost` との 2 host 併存**: `SynvelisApp` は当面
  `ShellWindowHost` を使い続ける。`HeadlessWindowHost` は
  `WorkspaceManager` を直接組み立てる test/検証経路専用で、
  `SynvelisApp` の型を変えない（app.rs は本スライス対象外）。

## Deferred Design Concerns

- `SynvelisApp.workspace` の host 型を multi-window 対応に
  切り替える設計（enum host / trait object 化等）| app 層が別
  dispatch で編集中・実 backend 未接続 | app 層スライス | 切替時に
  `ShellWindowHost` の単一窓契約と multi-window 契約の統合判断が
  必要になる。
- `WindowDriver` 側（`NativeShell` が消費する driver-neutral
  port）の headless multi-window host を app crate に置くか
  `ruviform-ui-test` の `FakeDriver` に委ねるか | `FakeDriver` が
  既に multi-window 対応済みで本スライスの検証には十分 |
  driver backend 接続スライス | app 側に独自 fake driver が必要に
  なった場合は再検討。

## Verification Notes

- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test` が Pass（`tests/sv113_dock.rs` を含む）。
- [ ] `docs/test-specs/sv-gui-04-01-headless-window-handle-dock.md`
  の Rule/Example が全て fixture test でカバーされる。
