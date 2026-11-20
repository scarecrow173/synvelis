# Mixer tab

The Mixer tab is the strip surface: the gain `Fader` bound to the
mixer control target, strip toggles, and the Master meter viz row.
Fader gestures ride the same `Begin → Preview → Commit` wire path as
seek; `syn.control.fast_control` is the service-level point-in-time
param write behind it.

## Sub-features

- `fader-gain` — `fader.gain` on the `mixer_gain_target` control
  commits a param change through the shared gesture path
  (synvelis#185 domain contract).
- `strip-toggles` — per-strip toggle components forward view-local
  actions onto their tagged control targets.
- `master-meter` — the Master meter viz row reports status/meta plus
  the latest complete frame through `syn.viz.snapshot`.
- `control-write` — `syn.control.fast_control` writes a plugin param
  point-in-time (`target`/`value`/`means`/`position_frames`/
  `project_revision`) → `Delivered|Queued|Blocked`.

## How to get to it (user POV)

- Open the Mixer tab (`screen.tabs` index 2) — fader, strip toggles,
  and the meter readout live there.
- Dragging the fader commits the gain; the meter row reflects the
  bound viz source.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` + `syn-ctl` build; live legs need an
  interactive desktop session.
- `mise run test synvelis-app` lanes run headless on any host.

- **Fader parity.** `mise run test synvelis-app --test
  screen_scenario fader_gesture_and_projection` proves
  `CASE_FADER_PARITY` — commit 0.4 lands `Applied` and the projection
  settles the controlled display.
- **GPUI parity.** `mise run test-app-gpui` runs the same fader body
  on `GpuiGuiTest` (`scenario_gpui.rs`).
- **Mounted dispatch.** `mise run test synvelis-app --test
  view_mount` proves the `ControlsUi` route from minted intent to
  `syn.control.dispatch` end to end.
- **Control surface.** `mise run test synvelis-app --test
  pc_c4_06_control_op_surface` proves `fast_control` returns typed
  `Delivered|Queued|Blocked` evidence, and `mise run test synvelis-app
  --test ui_ops` covers the `syn.viz.*` headless op contract.
- **Live control status.** `syn-ctl drive --op syn.control.status`
  (live) reads the control-surface report
  (`SurfaceReport`/`TickReport`/generation counters) — read-only.

## Gotchas

- `syn.control.fast_control` is a point-in-time write — mutating and
  gated on `project_revision`; live legs belong to scratch sessions.
- Meter frames need `syn.viz.register` + `syn.viz.push_frame` — both
  mutating setup, so the meter surface's semantic proof stays in
  `ui_ops`.
- `Delivered|Queued|Blocked` is evidence, not transport state — a
  `Queued`/`Blocked` write is a correct honest outcome, not a
  failure.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves live dispatch reached the app, not the
  param value — mixer semantics stay in the test lanes.
