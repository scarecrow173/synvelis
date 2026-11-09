---
id: "TSPEC-034Uoh9algsOuSv2AuYVZJ"
type: "test-spec"
status: "approved"
title: "SV-GUI-04 slice1 precursor 検証仕様: headless multi-window dock fixture"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
    - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
    - "docs/tasks/sv-gui-04-01-headless-window-handle-dock.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice1 precursor test-spec: headless multi-window dock fixture

## Purpose

Issue #113（SV-GUI-04）の precursor スライスが固定する headless
契約を、`tests/sv113_dock.rs` の fixture test として記録する。
実 OS/GPUI window の検証ではない — headless lane の Pass を実
Native PASS に昇格しないことを明示するための canonical 記録。

## Feature: Headless multi-window WindowHandle + dock contract

`HeadlessWindowHost`（`synvelis-app/src/host.rs`）が発行する複数
`WindowId` を `WorkspaceManager` の dock/session 契約と
`NativeShell` の view-origin 契約が正しく扱えること。

## Rules

- **Rule H1: window identity は単調・非再利用**
  `create_window` は毎回新しい `WindowId` を発行し、destroy 後も
  同じ id を再発行しない。`windows()` は live な id のみを返す。

- **Rule H2: 2 window は同一 Project/Job を共有し、window 寿命と
  独立**
  `WorkspaceManager` 配下の全 window は 1 つの `ProjectSession` を
  共有する。window の destroy（`DetachViews` disposition）は
  project/session/revision・view registry・view state を破棄せず、
  view は detached pool に保持され `reattach` で復帰できる。

- **Rule H3: view state と focus→active view は window ごとに独立**
  異なる window に attach された view は独立した `ViewState` を持ち、
  `Focused(true)` はその window の dock tree から active view を
  解決する。他 window の focus 状態に影響されない。

- **Rule H4: cross-window `origin_view` は拒否される**
  `rvf.ui.command.v1` intent の `origin_view` が「コマンド到達
  window とは別の window に attach された view」を指す場合、
  `NativeShell` は `invalid_params` の `Result` で拒否する。
  同一 window の view を指す場合は受理される。

- **Rule H5: dock transfer は app host 契約から到達できる**
  `WorkspaceManager<HeadlessWindowHost>` 経由で
  `begin_transfer`→`commit_transfer` は view ownership を
  window 間で移動させ、`begin_transfer`→`abort_transfer` は
  source ownership を維持する。`detach_to_new_window` は host の
  `create_window` を経由して新 window を発行する。

- **Rule H6: `ShellWindowHost` の単一窓契約は不変**
  `ShellWindowHost::create_window` は引き続き
  `UiError::BackendUnavailable` を返す（本スライスは置き換えない）。

## Examples

- **Example E1（H2）**: `ws.create_window` で w1/w2 を発行し、
  各 window に view を `spawn_view` → project rename を
  `session_mut().apply` で適用 → `on_window_event(w2, Destroyed)`
  後も `session().project()`・registry・state が保持され、
  `reattach(view_b, w1)` で復帰する。

- **Example E2（H3）**: w1 の view に `set_tool(Trim)` と viewport
  scroll を設定しても w2 の view の `ViewState` は default のまま。
  `Focused(true)` を w1→w2 の順に送ると `focus().active_view` が
  それぞれの window の view を指す。

- **Example E3（H4）**: `NativeShell`+`FakeDriver` で w1/w2 を
  Ready にし、view-a→w1・view-b→w2 を `syn.view.attach`。
  w1 上で `origin_view:"view-b"` の action intent を送ると
  `invalid_params` Result、`origin_view:"view-a"` では受理
  （`tab-select` の ok Result）。

- **Example E4（H5）**: `begin_transfer(view, Window(w2))` →
  `commit_transfer` で `dock().owner_of(view) == w2`。
  別 view で `begin_transfer` → `abort_transfer` すると
  `owner_of` は source window のまま。`detach_to_new_window` は
  `host.created` に新 window を記録し `windows()` が増える。

- **Example E5（H6）**: `ShellWindowHost::new().create_window(..)`
  が `BackendUnavailable` を返す。

## Guarantees

- `HeadlessWindowHost` は `WindowHandle` の delivery safety
  contract（未知 window への fallible 操作は `UiError::Host`、
  fire-and-forget は無視）を満たす。
- 全 host 呼出しは `created`/`evals`/`resizes`/`focuses`/
  `close_requests`/`close_decisions`/`destroys`/`exited` に記録
  され、test が観測できる。
- headless fixture の Pass は「契約層の検証」であり、実 OS
  window・GPUI driver・IME・DPI の Pass を意味しない。

## Non-goals

- 実 GPUI window 生成・`create_view_window` backend 接続・
  `synvelis-native` 2 窓 mount・`screens.rs` 2nd screen・
  close-cancel・monitor 失踪・DPI/IME・layout restore の検証
  （後続スライス / Issue #114 の実 OS Gate）。
- `SynvelisApp.workspace` の host 型切替や `ShellWindowHost` の
  multi-window 化。
- `WindowCloseDisposition::RelocateTo`・close veto の headless
  検証（将来拡充）。

## Risk

- この fixture が実 backend と乖離した semantics を固定すると、
  後続スライスで「headless Pass ≠ 実装正しさ」の手戻りが起きる。
  緩和: test は `WindowHandle`/`DockWorkspace`/`NativeShell` の
  既存公開契約のみを assert し、host 内部実装詳細（id 採番順の
  具体値等）には依存しない。
