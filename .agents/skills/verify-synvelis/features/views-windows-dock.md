# Views, windows, and dock moves

The window/dock surface: `syn.view.*` owns the view lifecycle
(attach/detach/close/list/select), `syn.window.*` owns adoption and
the two-phase dock moves (prepare/commit/abort/detach/reattach into
the detached pool or a new driver window), and `syn.window.close_guard`
exposes the close-veto reasons. The product mounts the main view plus
the secondary transport-deck window on this surface.

## Sub-features

- `view-lifecycle` — `syn.view.attach`/`detach`/`close` manage the
  view registry; `detach` bumps the generation (stale pins die) and
  `close` drops UI state only.
- `view-read` — `syn.view.list` (registry: id/window/attached/
  generation) and `syn.view.select` (arrangement row →
  `ObjectSelections` + inspector focus).
- `window-registry` — `syn.window.adopt` registers driver-created
  windows; `syn.window.list` reports windows + docked/detached views
  + pending moves; `syn.workspace.mount` summarizes the mount point.
- `dock-move` — `syn.window.dock_prepare`(+`_new_window`) /
  `commit`/`abort`/`detach`/`reattach` run the two-phase move;
  `syn.window.lifecycle` forwards host lifecycle internally.
- `close-guard` — `syn.window.close_guard` returns veto reasons
  (`unsaved_changes`/`recording_active`/`recorder_active`) — the
  last-window close veto evidence.

## How to get to it (user POV)

- Views dock into windows; the secondary transport deck is the
  second mounted view — `mise run run-app-native` shows the default
  layout.
- Closing the last window asks the guard — unsaved changes or an
  active recording/recorder vetoes it.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` + `syn-ctl` build; live legs need an
  interactive desktop session.
- `mise run test synvelis-app` lanes run headless on any host.

- **Read the registry live.** `syn-ctl drive --op syn.view.list`,
  `--op syn.window.list`, `--op syn.workspace.mount`, or
  `--op syn.window.close_guard` inject the read-only registry ops —
  each journals the dispatch receipt pair.
- **Mounted end-to-end.** `mise run test synvelis-app --test
  view_mount` proves a `ViewHost` mounted through the real `syn.*`
  runtime on `FakeDriver` — the Fake↔GPUI parity claim's headless
  half.
- **Two windows.** `mise run test synvelis-app --test two_window`
  proves the two-`ViewHost` mount (`view-1` main, `view-2` deck).
- **Dock ops.** `mise run test synvelis-app --test dock_ops` proves
  the `syn.window.dock_*` family over `NativeShell`; `--test
  sv113_dock` covers the contract layer.
- **Close veto.** `mise run test synvelis-app --test close_veto`
  proves `close_guard` + the last-window `CloseRequested` veto.
- **View ops.** `mise run test synvelis-app --test ui_ops` covers the
  `syn.view.*`/`syn.control.*`/`syn.viz.*` headless contract.

## Gotchas

- `syn.view.detach` bumps the view generation — pins resolved before
  it go `StaleGeneration`; re-resolve after any lifecycle op.
- `dock_commit`/`dock_abort` need a pending `dock_prepare*` — a bare
  commit is a named domain error, not a seam failure.
- `syn.window.lifecycle` is internal forwarding — it exists for the
  host path, not a drive recipe.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves live dispatch reached the app, not the
  registry contents — dock/view semantics stay in the lanes.
