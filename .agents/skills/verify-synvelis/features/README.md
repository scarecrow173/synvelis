# Synvelis verification map

This directory is the maintained source for verifying Synvelis's
user-facing behavior. Read the index before driving the app, then use
the matching feature file as the recipe.

Status: normalized. The map covers the seam-driven `synvelis-native`
window (`native-app-via-seam`), the six main-screen tabs
(`arrangement`, `transport`, `mixer`, `inspector-control`,
`signal-flow`, `project`), the detached/mounted surfaces (`session`,
`vcs-history`, `library`, `automation`, `variant-diff`,
`views-windows-dock`, `plugin-dock`, `script`), the headless
`synvelis-cli` (`cli-headless-ops`), and the pinned scenario catalog
(`scenario-catalog`).

## Baseline preconditions

- Build once per checkout: `mise run build-app-uitest` produces the
  AUT `target/debug/synvelis-native`, and `cargo build -p synvelis-app
  --features uitest --bin syn-ctl --locked` produces the control CLI
  beside it (`mise run test-app-uitest-ctl` builds both).
- The seam surface requires an interactive desktop session —
  `WindowedIo` + `GpuiDriver` has no headless path. The
  `synvelis-cli` surface and the `mise run test synvelis-app` /
  `test-app-gpui` lanes are headless and need no session.
- Run `syn-ctl doctor` and require `verdict: "pass"` before driving
  the seam surface — it proves spawn, handshake, capabilities,
  identity, shutdown, and child exit in one shot.
- `syn-ctl` always spawns its own AUT child — never attach a
  controller to a window the user owns.
- Record results under `evidence/sv600/` with the verdict vocabulary
  of `POLICIES.md` §3.

## Driving conventions

- Start every recipe from the baseline state unless its preconditions
  say otherwise.
- Drive the seam through `syn-ctl` (`doctor`, `drive`,
  `drive --op <syn.*> --params <json>`); drive the CLI through
  `synvelis-cli` directly; drive the GUI scenario port through the
  `mise run test synvelis-app` / `test-app-gpui` lanes. Treat every
  command as literal.
- For seam ops, select the injection target from
  `test.observe.windows` — the first `ready` window, never a guessed
  id.
- `ipc_emit` `params.body` is one `rvf.ipc.v1` intent JSON string
  (`{"v":1,"kind":"intent","id":…,"op":"syn.*","params":{…}}`) — the
  op reaches `AppService` through the identical inbound boundary as
  normal IPC.
- Capture the action and the resulting state. Confirm side effects
  with a second read-only observation (`test.observe.receipts`, a
  second `drive`, `synvelis-cli` inspect ops), not by trusting the
  write command's output.
- Restore or delete scratch state after a mutation. Do not remove
  proof artifacts during cleanup.

## Proof and skip reporting

- Seam proof is the `syn-ctl` artifacts: the doctor JSON report
  (`checks[]`, `welcome`, `cleanup`, `child_exit`) and the drive
  JSONL transcript (context + step lines + verdict).
- Emit boundary: the receipt journal is metadata-only
  (seq/direction/kind/bytes) — an injected op's result payload is not
  readable through the seam. `drive --op` proves the intent reached
  live dispatch; semantic verification stays in the test lanes.
- CLI proof includes the command, stdout, stderr, and exit code —
  `--format json` emits the versioned envelope.
- Mutation proof includes a read-only second view (receipt journal,
  second observation run).
- A path that cannot run records `NotRun` with the reason —
  `environment_blocked` for a seam drive on a headless host, an
  unwired surface as `blocked: seam 未対応` — never report it as
  `Pass`, and never fake a drive around the seam.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing
the user-visible behavior, then exactly four H2 sections in this
order:

1. `Sub-features` lists short IDs with one line for each behavior.
2. `How to get to it (user POV)` lists every user entry point.
3. `Driving it with <harness>` starts with `Preconditions:` and uses
   labeled bullets pairing each user action with an exact command and
   an observable result.
4. `Gotchas` lists traps that can waste or invalidate a run.

Keep implementation details out of the map: name only user paths,
stable handles, required state, commands, and observable proof.

## Features

- [App window via the control seam](./native-app-via-seam.md) covers
  launching `synvelis-native` under `rvf.uitest.v1`, observing its
  windows, and driving any `syn.*` op through `ipc_emit` —
  `drive --op`/`--params` and the TCP transport leg included.
- [Project lifecycle and PPQN](./project.md) covers
  `syn.project.*` — bundle lifecycle, info, undo/redo, and the
  gated PPQN change with its read-only preview.
- [Recording and monitor session](./session.md) covers
  `syn.recording.*`/`syn.monitoring.*`/`syn.recorder.*` — session
  projections, arm/punch declarations, and the recorder lifecycle.
- [Arrangement tab](./arrangement.md) covers the clip table, row
  select into inspector focus, the seek range, and the clip-waveform
  viz surface.
- [Transport and seek](./transport.md) covers Play invoke, the
  seek-position gesture, `syn.transport.set`, and the secondary
  transport-deck window.
- [Mixer tab](./mixer.md) covers the gain fader, strip toggles, the
  master meter, and `syn.control.fast_control` writes.
- [Inspector tab and the control surface](./inspector-control.md)
  covers the read-only inspector tables and the `syn.control.*` wire
  family.
- [Signal Flow tab and probes](./signal-flow.md) covers
  `syn.signal_flow.*` (view/visual/inspection/routing) and the
  `syn.probe.*` capture/compare surface.
- [VCS history and A/B diff](./vcs-history.md) covers `syn.vcs.*` —
  history projection, A/B selection, honest Unknown/Partial rows,
  and git-backend binding.
- [Asset library browser](./library.md) covers `syn.library.*` —
  provider catalog, the detached browser view, and browse/import
  actions.
- [Automation (AUTX) surfaces](./automation.md) covers
  `syn.autx.*` — palette/macro/workflow/run-monitor lifecycle,
  catalog push, and per-view action dispatch.
- [Variant diff explorer](./variant-diff.md) covers
  `syn.variant-diff.*` — the CB-09 compare/adopt surface with
  explicit pins and the approval ticket.
- [Views, windows, and dock moves](./views-windows-dock.md) covers
  `syn.view.*`/`syn.window.*` — lifecycle, two-phase dock moves, the
  detached pool, and the close veto.
- [Plugin dock](./plugin-dock.md) covers the `plugin.editor`
  native-slot contract — honest `NotRun`/`Unsupported` status for
  real-ABI/native-dock evidence.
- [Script editor and script analysis](./script.md) covers the
  `svscript.*` view-local command surface and `synvelis-cli script
  analyze`.
- [Headless CLI ops](./cli-headless-ops.md) covers `synvelis-cli`'s
  offline surface — `action list`/`describe`, `project info`,
  `session list`, `script analyze` — no display required.
- [Scenario catalog](./scenario-catalog.md) covers the pinned
  `SCENARIO_*`/`CASE_*` catalog the product screens are verified
  through on the fake and GPUI lanes.

Pending / blocked (recorded honestly, not faked):

- `synvelis` (webview) GUI surface — `blocked: seam 未対応` (the
  webview bin is outside the uitest seam and is being removed per
  `scarecrow173/synvelis#232`); no feature file is kept for it.
- Real-OS/plugin-dock EXT-L3 lanes (real ABI test plugin, native
  editor HWND, pixel/focus) — `NotRun` until their environment
  exists; see `plugin-dock.md` and `evidence/sv176/`.
- GUI scenario catalog driving through `syn-ctl` — the scenario port
  runs under `test`/`test-app-gpui` lanes; a `syn-ctl` route is a
  later slice.
