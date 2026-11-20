# App window via the control seam

Launching `synvelis-native` under the `rvf.uitest.v1` seam lets a
user-facing check observe the real GPUI window — mount, readiness,
journaled traffic — and drive `syn.*` ops through the same inbound
boundary as normal IPC. `syn.app.info` is the canned probe: app
identity (`name`, `version`, ruviform `revision`) answered through
the product's own op path.

## Sub-features

- `seam-launch` spawns the AUT with the control transport opted in —
  the same binary takes the plain product path without `RVF_TEST_*`.
- `window-observe` lists live windows with `ready`/`close_pending`/
  `ipc_down` state — the real mounted window, not a model.
- `syn-op-inject` delivers one `syn.*` intent through `ipc_emit` at
  the inbound boundary; the app's typed result leaves via
  `post_event`.
- `receipt-journal` reads the boundary journal — the `in "message"` /
  `out "post_event"` pair is the op's resulting state.
- `graceful-teardown` closes the session through `test.shutdown` —
  `CleanupReport` plus child exit, never a killed orphan.

## How to get to it (user POV)

- Launch `synvelis-native` from a terminal — the everyday product
  entry (`mise run run-app-native`).
- The same binary serves the control seam only when spawned with
  `RVF_TEST_CONTROL=stdio|tcp` — `syn-ctl` sets that up through
  `TestDriver` instead of the user exporting env vars.
- There is no UI element for the seam; the user-visible equivalent is
  "the app window is up and answering ops".

## Driving it with syn-ctl

Preconditions:

- `mise run build-app-uitest` and the `syn-ctl` build have produced
  `target/debug/synvelis-native` and `target/debug/syn-ctl`.
- An interactive desktop session — `GpuiDriver` needs a real window;
  on a headless host every step is `not_run`/`environment_blocked`.
- `RVF_TEST_DISABLE` is unset on the host (the kill switch beats
  every transport opt-in — `syn-ctl` reports it as blocked).

- **Health-gate first.** Run `target/debug/syn-ctl doctor`. Exit `0`;
  `checks[]` all `pass` including `identity` (`welcome.app.name` is
  `synvelis`, `revision` is the Cargo.lock ruviform pin). A green
  doctor means the seam is live and the AUT is this checkout.
- **Plan the probe.** Run `target/debug/syn-ctl drive --dry-run`.
  Exit `0`; the plan JSON lists the ops (`test.hello`,
  `test.observe.windows`, `test.fixture.prepare`,
  `test.observe.receipts`, `test.shutdown`) and `app_exists`.
- **Drive the app.** Run `target/debug/syn-ctl drive --out
  evidence/sv600/syn-ctl-drive-run1.jsonl`. Exit `0`; the transcript's
  `test.observe.windows` step shows a `ready:true` window, the
  `test.fixture.prepare` step reports `applied.window` = that id,
  `test.observe.receipts` shows the injected `syn.app.info` as an
  `in "message"` / `out "post_event"` pair, `test.shutdown` reports
  `exited:true`, and `child_exit.code` is `0`.
- **Second observation.** Run `target/debug/syn-ctl drive --out
  evidence/sv600/syn-ctl-drive-run2.jsonl`. Same reply shape — the
  read-only rerun proves the drive left no wedged state.
- **Proof.** Keep both JSONL files plus a doctor JSON in
  `evidence/sv600/` — the transcript, not the terminal scrollback,
  is the proof.

## Gotchas

- A non-`uitest` `synvelis-native` never enters the seam — the stdio
  hello stalls rather than failing (the app runs its normal product
  path). Always build the AUT with `mise run build-app-uitest`;
  `--transport tcp` bounds the wait but is not the default leg.
- `RVF_TEST_DISABLE` on the host makes every serve path refuse —
  `syn-ctl` detects it pre-spawn as `not_run` +
  `environment_blocked`; unset it rather than working around it.
- `test.observe.screenshot` on a full-size window trips the
  protocol's 1 MiB reply cap — an honest bounded `service_error`,
  not a doctor failure; don't treat it as a regression.
- `input_injection`/`dom_access` are `false` forever — the port
  structurally refuses them; drive through ops and fixtures, never
  by simulating input.
- The window id is assigned by the host at spawn — always read it
  from `test.observe.windows`, never hardcode `1`.
