# Recording and monitor session

The runtime session surface: the workspace `ProjectSession` (single
project/session source of truth) plus the recording and monitor
families — `syn.recording.*` projects persisted sessions/passes/
takes, `syn.monitoring.*` declares arm/monitor/punch/session/latency
state, and `syn.recorder.*` runs the recorder lifecycle
(preflight → input → staging → commit/discard).

## Sub-features

- `recording-info` — `syn.recording.info` projects persisted
  recording entities (sessions/passes/streams/segments/takes) plus
  recorder status (RCD-01).
- `recording-status` — `syn.recording.status` is the UI-C5 read
  model (conflict/pending/applied/unknown + provenance + legal
  actions); `syn.recording.action` runs its explicit
  inspect/retry/rebase/abort actions.
- `monitor-declare` — `syn.monitoring.set` patches or clears
  session-scope track arm/monitor/binding/hw-direct (RCD-03).
- `monitor-session` — `syn.monitoring.session` start/stop,
  `syn.monitoring.transport` play/pause/stop/seek in project ticks,
  `syn.monitoring.punch` marks, `syn.monitoring.latency` reports,
  `syn.monitoring.state` reads the whole projection.
- `recorder-lifecycle` — `syn.recorder.start`/`pump`/`stop`/
  `commit`/`discard`/`mark_take_verified` run the recorder;
  `syn.recorder.status` is its read-only owner projection.

## How to get to it (user POV)

- Recording/monitoring controls are wire ops the GUI surfaces lower
  into — the session state appears on monitor/recorder projections,
  not a separate tab.
- `syn.recorder.status`/`syn.monitoring.state` are what a monitor or
  record panel reads each frame.
- Real-device and real-FS durability are separate opt-in lanes, not
  the default session surface.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` + `syn-ctl` build; interactive desktop
  session for live legs.
- `mise run test synvelis-app` lanes run headless on any host.

- **Read the session live.** `syn-ctl drive --op syn.recording.info`,
  `--op syn.recording.status`, `--op syn.monitoring.state`, or
  `--op syn.recorder.status` inject read-only projections through the
  seam — each journals the dispatch receipt pair.
- **Recording projection.** `mise run test synvelis-app --test
  recording_projection` proves `syn.recording.info` over persisted
  entities (RCD-01 headless).
- **Recording status/actions.** `mise run test synvelis-app --test
  uic5_app` proves `syn.recording.status` + `syn.recording.action`
  through the product wire surface.
- **Monitor arm/punch.** `mise run test synvelis-app --test
  monitor_arm` proves the RCD-03 arm/monitor spec (E1–E7) and the
  `syn.monitoring.state` projection.
- **Recorder lifecycle.** `mise run test synvelis-app --test
  recorder_wire` drives `syn.recorder.*` start→pump→stop→commit wire
  end to end.
- **Product E2E.** `mise run test synvelis-app --test
  rcd_04_product_e2e` covers the Comp Transition / MIDI Phrase /
  Recovery Review flows over `syn.*` dispatch.

## Gotchas

- Recorder ops are sequence-dependent — `stop`/`commit`/`discard`
  presuppose `start`; a bare `commit` on an empty session is a domain
  error, not a seam failure.
- `syn.monitoring.*` action ops and `syn.recording.action` mutate
  session state — scratch sessions only; the `*.state`/`*.status`
  reads are the live-safe legs.
- Real audio input (`mise run test-recording-real-input`) and real-FS
  durability (`test-recording-real-fs`, `test-project-real-fs`) are
  opt-in `#[ignore]` lanes requiring hardware/real FS — `not_run` by
  default, not implied by headless passes.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves live dispatch reached the app; the op's
  result payload is not readable through the seam — session-state
  verification stays in the test lanes.
