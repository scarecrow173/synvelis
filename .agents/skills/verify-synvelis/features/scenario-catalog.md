# Scenario catalog

The scenario port is the pinned catalog of driver-neutral GUI cases
— `tests/common/scenario.rs` bodies that run identically on
`FakeGuiTest` (headless semantic lane) and `GpuiGuiTest`
(GPUI-simulated lane, `gpui-test` feature). The same wire path the
real window uses is exercised without OS input or pixels.

## Sub-features

- `main-screen` — `SCENARIO_MAIN_SCREEN` (`synvelis.main.screen`)
  covers the `main_screen_spec` surface: `CASE_SEEK_PARITY`,
  `CASE_FADER_PARITY`, `CASE_TRANSPORT_PLAY`,
  `CASE_ARRANGEMENT_SELECT`, `CASE_TAB_CYCLE`,
  `CASE_READ_ONLY_INSPECTOR`.
- `secondary-screen` — `SCENARIO_SECONDARY_SCREEN`
  (`synvelis.secondary.screen`) covers the transport deck:
  `CASE_SECONDARY_SEEK` on `view-2` (`MultiWindow` evidence).
- `fake-lane` — `tests/screen_scenario.rs` runs every case on
  `FakeGuiTest`, claiming only `SemanticAction`/`MultiWindow`.
- `gpui-lane` — `tests/scenario_gpui.rs` runs the identical bodies on
  `GpuiGuiTest` under the `gpui-test` feature.
- `honesty` — capability pre-flight rejects unclaimed drivers;
  `HONESTY_ABSENT` pins `SimulatedPointerOrKeyboard`,
  `RealOsPointerOrKeyboard`, `Ime`, `VisualCapture`,
  `NativePluginDock` out of every report.

## How to get to it (user POV)

- The catalog is the user path replayed: drag the seek slider, pull
  the mixer fader, press Play, select an arrangement row, cycle the
  tabs — each case is a real gesture lowered to wire intents.
- `syn-ctl` does not drive this port — the scenario lanes are mise
  test lanes; the seam covers the live-window half.

## Driving it with the test lanes

Preconditions:

- `mise run test synvelis-app` runs headless on any host;
  `test-app-gpui` needs no display.

- **Full fake catalog.** `mise run test synvelis-app --test
  screen_scenario` runs all main + secondary cases — each test
  asserts the case's intents, snapshot, and an honest-capabilities
  report.
- **One case.** `mise run test synvelis-app --test screen_scenario
  seek` filters by name substring — e.g. `seek_gesture_and_projection
  _follow_the_shared_parity_scenario` for `CASE_SEEK_PARITY`.
- **GPUI parity.** `mise run test-app-gpui` runs the identical bodies
  on the simulated GPUI adapter (`scenario_gpui.rs`) — Pass means the
  same intents resolve on the real rendering stack's adapter.
- **Reports.** Each case emits a `ScenarioReport` — driver, case id,
  verdict, targets, observed state — the report's `lane_status()`
  feeds the verdict vocabulary.

## Gotchas

- A `Fake` pass is semantic evidence only — never OS-pointer, IME,
  pixel, or native-dock proof; `VisualCapture`/`RealOsPointerOr
  Keyboard` stay `Unsupported` by capability gate.
- `CASE_READ_ONLY_INSPECTOR` records `not_run` on purpose — the
  inspector table's rejection is the evidence, not a pass.
- The catalog keys (`seek.position`, `fader.gain`, `transport.play`,
  `screen.tabs`, `arrangement.table`, `inspector.table`) must match
  `screens.rs` literals — a spec rename breaks resolution honestly.
- `syn-ctl drive --op` does not replay these cases — emit proves
  dispatch, the lanes prove semantics.
