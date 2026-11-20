# Synvelis verification map

This directory is the maintained source for verifying Synvelis's
user-facing behavior. Read the index before driving the app, then use
the matching feature file as the recipe.

Status: seeded in the first `verify-synvelis` slice with the two
driveable surfaces — the seam-driven `synvelis-native` window and the
headless `synvelis-cli` ops. Normalizing every user-visible feature —
the project/session/arrangement/mixer/plugin/script/CLI areas the
Issue enumerates — is deferred to a later slice
(`scarecrow173/synvelis#600`).

## Baseline preconditions

- Build once per checkout: `mise run build-app-uitest` produces the
  AUT `target/debug/synvelis-native`, and `cargo build -p synvelis-app
  --features uitest --bin syn-ctl --locked` produces the control CLI
  beside it (`mise run test-app-uitest-ctl` builds both).
- The seam surface requires an interactive desktop session —
  `WindowedIo` + `GpuiDriver` has no headless path. The
  `synvelis-cli` surface is headless and needs no session.
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
- Drive the seam through `syn-ctl` (`doctor`, `drive`); drive the CLI
  through `synvelis-cli` directly. Treat every command as literal.
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
  windows, and driving a `syn.*` op through `ipc_emit` — the
  seam-driven GPUI surface.
- [Headless CLI ops](./cli-headless-ops.md) covers `synvelis-cli`'s
  offline surface — `action list`/`describe`, `project info`,
  `session list`, `script analyze` — no display required.

Pending / blocked (later slices — recorded honestly, not faked):

- `project`, `session` (project-model), `arrangement`, `mixer`,
  `plugin`, `script` areas of the native app — `blocked: seam
  drive-through not yet mapped`; the seam carries `syn.*` ops but the
  per-area recipes are unwritten.
- `synvelis` (webview) GUI surface — `blocked: seam 未対応` (the
  webview bin is outside the uitest seam and is being removed per
  `scarecrow173/synvelis#232`).
- TCP transport lane (`RVF_TEST_CONTROL=tcp` + secret handshake) —
  offered by `syn-ctl --transport tcp`, live evidence deferred.
- GUI scenario catalog driving through `syn-ctl` — the scenario port
  (`tests/common/scenario.rs`) runs under `test`/`test-app-gpui`
  lanes; a `syn-ctl` route is a later slice.
