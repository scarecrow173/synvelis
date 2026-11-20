# Transport and seek

The Transport tab is the playback deck: the Play button, the rolling
status label, and the seek-position slider that commits a seconds
position on the `transport/position` target. The secondary window
mounts the same deck (`seek2.position`, `transport2.play`) — the
multi-window half of the surface.

## Sub-features

- `transport-play` — `transport.play` mints one discrete `Action`
  intent on the `transport` target, resolved to `transport_play` app
  side.
- `transport-status` — the status `Label` reflects the driven
  transport state (updates arrive through projections).
- `seek-commit` — `seek.position` rides the `Begin → Preview →
  Commit` gesture on `transport/position`; the controlled display
  settles on the seconds value (synvelis#175 seconds contract).
- `transport-set` — `syn.transport.set` `{playing: bool}` is the
  service-level roll control.
- `secondary-deck` — the secondary window mounts
  `secondary_screen_spec`: `seek2.position`/`transport2.play` run the
  identical gesture/invoke contract.

## How to get to it (user POV)

- Open the Transport tab on the main screen, or the secondary
  transport-deck window.
- Press Play to roll; drag the seek slider to scrub position.
- `syn.transport.set` is the wire-level entry — `{"playing": true}`.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` + `syn-ctl` build; live legs need an
  interactive desktop session.
- `mise run test synvelis-app` lanes run headless on any host.

- **Play invoke.** `mise run test synvelis-app --test
  screen_scenario transport_play_invokes` proves
  `CASE_TRANSPORT_PLAY` — one `Action` intent on `transport` with the
  `Applied` receipt on the button binding.
- **Seek parity.** `mise run test synvelis-app --test
  screen_scenario seek_gesture_and_projection` proves
  `CASE_SEEK_PARITY` — commit 8.0 s lands `Applied` + the
  authoritative position projection.
- **Secondary deck.** `mise run test synvelis-app --test
  screen_scenario secondary_window_runs_the_same_scenarios` proves
  `CASE_SECONDARY_SEEK` on `view-2` — `MultiWindow` evidence on the
  fake adapter.
- **GPUI parity.** `mise run test-app-gpui` reruns the same bodies on
  `GpuiGuiTest` (`scenario_gpui.rs`).
- **Plan a roll.** `syn-ctl drive --op syn.transport.set --params
  '{"playing":true}' --dry-run` prints the plan JSON naming the op —
  `syn.transport.set` is mutating, so the live leg is scratch-only;
  semantic proof stays in the lanes above.

## Gotchas

- `syn.transport.set` mutates the rolling state — a live `--op` leg
  belongs to a scratch AUT, never a session the user owns; the
  `--dry-run` plan is the honest env-independent recipe.
- Seek commits carry the seconds contract — the wire value is plain
  domain seconds, not frames; the scenario asserts parity, not the
  encoding.
- The secondary deck is a distinct `ViewInstanceId` — key names
  differ (`seek2.*`/`transport2.*`); pin the right window/view pair.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves live dispatch reached the app, not the
  playback state — transport semantics stay in the scenario lanes.
