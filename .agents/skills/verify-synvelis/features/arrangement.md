# Arrangement tab

The Arrangement tab is the main screen's default view — a selectable
table of the project's clips (track/lane/clip/source/start/end/muted)
whose row select feeds the Inspector focus, plus the clip-waveform
viz surface. Selection is a view-local wire action; the same
`syn.view.select` op drives it headlessly.

## Sub-features

- `arrangement-table` — `arrangement.table` lists one row per clip
  placement (track, lane, clip, source, start/end seconds, muted).
- `arrangement-select` — a row press mints the view-local `select`
  Action on `VIEW_STATE_TARGET` with `params {"row": n}`; the service
  side lowers `syn.view.select` into `ObjectSelections` + inspector
  focus.
- `seek-range` — the shared seek range derives from
  `project_length_seconds` (120 s in the scenario fixture) and feeds
  `seek.position`.
- `clip-waveform` — the Clip waveform viz surface reports
  status/meta/span through `syn.viz.*` binds.

## How to get to it (user POV)

- Open `synvelis-native` — the Arrangement tab is index 0 and active
  by default; click a row to select it.
- `screen.tabs` cycles to it from any other tab (`tab-select`
  `{"index": 0}`).
- Rows come from the open project — an empty project shows an empty
  table, not an error.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` + `syn-ctl` build; live legs need an
  interactive desktop session.
- `mise run test synvelis-app` lanes run headless on any host.

- **Row select (semantic).** `mise run test synvelis-app --test
  screen_scenario arrangement_select_row_and_tab_cycle` proves
  `CASE_ARRANGEMENT_SELECT` — row 0 press mints `select {"row": 0}`
  on `VIEW_STATE_TARGET` and the snapshot reports it selected.
- **Same case on GPUI.** `mise run test-app-gpui` runs the identical
  scenario body on `GpuiGuiTest` (`scenario_gpui.rs`) — the
  simulated-rendering parity half.
- **Mounted dispatch.** `mise run test synvelis-app --test
  view_mount` proves `syn.view.select` end to end: dispatch →
  `ControlsUi` → `ObjectSelections` + receipt back to the host.
- **Op surface.** `mise run test synvelis-app --test ui_ops` covers
  the `syn.view.*`/`syn.viz.*` headless op contract.
- **Live observation.** `syn-ctl drive --op syn.view.list` (live)
  reads the view registry — the mounted arrangement view with its
  window/generation. Read-only.

## Gotchas

- `arrangement.table` is selectable — unlike `inspector.table`, which
  is mounted read-only and rejects row select; do not carry an
  assertion across both tables.
- The select Action is view-local wire (`VIEW_STATE_TARGET`), not a
  `syn.*` op — semantic proof lives in the scenario lanes, while
  `syn.view.select` is the service-level equivalent.
- Waveform rows need a `syn.viz.bind` first — bind is mutating, so
  the waveform surface is scratch-only live or stays in `ui_ops`.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves live dispatch reached the app, not the
  row content — selection semantics stay in the test lanes.
