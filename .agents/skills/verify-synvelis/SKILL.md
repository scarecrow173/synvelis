---
name: verify-synvelis
description: Verify Synvelis — the `synvelis-native` GPUI app driven over the `rvf.uitest.v1` external-control seam by the `syn-ctl` control CLI, plus the headless `synvelis-cli` surface — with evidence in `evidence/sv600/`. Use when asked to launch the app-under-test, run a health doctor, drive the live window, or record Issue #600 verification for synvelis.
---

# verify-synvelis

Project-local verification skill for `scarecrow173/synvelis`. It covers
three surfaces:

- **Seam-driven app surface** — `synvelis-native` built with
  `--features uitest` serves `rvf.uitest.v1` (`PROTOCOL_VERSION = 1`)
  over length-prefixed JSON frames, driven by `syn-ctl`
  (`crates/synvelis-app/src/bin/syn_ctl.rs`). The seam is doubly gated:
  the `uitest` Cargo feature compiles it in, and `RVF_TEST_*`
  environment opts it in at runtime — a `uitest` build with no env is
  the unchanged product path.
- **Headless CLI surface** — `synvelis-cli`
  (`action`/`workflow`/`script analyze`/`project`/`render`/`session`),
  the non-GUI user path (`crates/synvelis-cli`).
- **Scenario port** — the driver-neutral scenarios in
  `crates/synvelis-app/tests/common/{scenario,mounted}.rs`, run by the
  `gpui-test` lane — read-only reference here; `syn-ctl` does not drive
  them.

Verdict vocabulary everywhere: `pass` / `fail` / `not_run` /
`unsupported` / `blocked`. Never report a check that did not execute
as `pass`; a missing environment is `not_run` +
`environment_blocked`, never fake-`pass`ed (POLICIES.md §3:
Required/Deferred/NotApplicable requirement state ×
Pass/Fail/NotRun/Unsupported result).

## Launch

AUT + control CLI (same target dir — `syn-ctl` finds
`synvelis-native` as its sibling binary by default):

```sh
mise run build-app-uitest                                                 # AUT
cargo build -p synvelis-app --features uitest --bin syn-ctl --locked      # ctl
```

Normal launch stays `mise run run-app-native` — the seam never
activates without `RVF_TEST_CONTROL` (`stdio` | `tcp`), which
`TestDriver::spawn_stdio` / `spawn_tcp` sets for the child (TCP adds
`RVF_TEST_ADDR` + a run-bound `RVF_TEST_SECRET`). Running
`synvelis-native` directly with no env takes the normal product path;
`RVF_TEST_DISABLE=1` refuses every serve path — the fail-closed kill
switch, detected by `syn-ctl` before spawn as `environment_blocked`.

**Ready check:** `syn-ctl doctor` green — not "a window appeared".
The driver sends `test.hello`; the AUT answers a `Welcome` carrying
`v`, `session_id`, `run_id`, `app`, `ruviform_revision`, `backend`
(`gpui-windowed` / `gpui/windows` on the real path),
`capabilities`, `windows`, `os`, `arch`.

**Interactive session required.** The seam runs `WindowedIo` +
`GpuiDriver` — there is no headless serve path, so doctor/drive need
a real desktop session on Windows. On a non-interactive host the run
records `not_run` + `environment_blocked`, never a fake pass.

**Teardown:** `test.shutdown` (graceful) → `CleanupReport`
(`windows_open`/`windows_closed`/`exited`/`app`), then confirm the
child exits. `TestDriver`'s `Drop` kills and reaps a still-running
child — a dropped driver never leaves an orphan.

## Doctor

Read-only health probe — spawn, handshake, capabilities, identity,
graceful shutdown, child exit. It never sends a fixture/clock/fault/
window op.

```sh
# canonical lane (builds both binaries; the live leg is an
# interactive-session gate — not CI-required)
mise run test-app-uitest-ctl

# direct (auxiliary evidence — canonical = mise)
target/debug/syn-ctl doctor                                   # stdout JSON
target/debug/syn-ctl doctor --out evidence/sv600/syn-ctl-doctor.json
target/debug/syn-ctl doctor --transport tcp                   # loopback + run-bound secret — see evidence/sv600/syn-ctl-doctor-tcp.json
target/debug/syn-ctl doctor --app target/debug/synvelis-native.exe
target/debug/syn-ctl doctor --dry-run                         # plan only — spawns nothing
```

`checks[]`: `spawn` → `handshake` → `capabilities` → `identity` →
`shutdown` → `child-exit`. `identity` is the doctor's own value: it
proves the AUT is *this* checkout's synvelis — `welcome.app.name ==
"synvelis"`, `app.version ==` this binary's `CARGO_PKG_VERSION`, and
`app.revision == synvelis_app::HOST_REVISION` (the ruviform pin baked
from `Cargo.lock`, currently `84a68f25…`). A wrong binary that speaks
the protocol fails here, not silently.

Report: `type:"doctor"`, `lane:"syn-ctl-doctor"`, `welcome`,
`capabilities`, `cleanup`, `child_exit`,
`environment{os,arch,environment_kind}` (`gpui-windowed` ⇒
`real-window`), `repo.sha`, `verdict`. Exit `0` = all checks pass;
`1` = `fail`, or `not_run` + `environment_blocked` (AUT missing, or
`RVF_TEST_DISABLE` set); `2` = CLI misuse. Failures carry
`error.stage` and `error.next_step`.

## Drive

The `rvf.uitest.v1` closed op set (measured on the live
`synvelis-native` seam — `capabilities` from `evidence/sv600`):

| Op | Mutating? |
| --- | --- |
| `test.hello`, `test.capabilities`, `test.shutdown`, `test.notices` | no (hello/shutdown are session bounds) |
| `test.observe.windows`, `test.observe.receipts`, `test.observe.screenshot` | no — observation only |
| `test.fixture.prepare`, `test.clock.advance`, `test.fault.set`, `test.fault.clear`, `test.window.request_close` | yes — drive ops |

Measured capability set: `fixture:true`,
`clock_domains:["host_tick"]`, `faults:["window_crash",
"ipc_disconnect"]`, `observation:["windows","receipts","notices"]`,
`screenshot:true`, `input_injection:false`, `dom_access:false`
(structurally denied by the port, not just absent). Screenshot of a
full-size window currently trips the protocol's 1 MiB reply cap —
that is an honest bounded `service_error`, not an adapter fault
(ruviform-side follow-up candidate).

`syn-ctl drive` injects one `syn.*` intent — `syn.app.info` by
default (the canned probe, the reproducible leg of the slice-1 live
proof), or any other op selected with `--op`/`--params`:

```sh
target/debug/syn-ctl drive --out evidence/sv600/syn-ctl-drive-run1.jsonl
target/debug/syn-ctl drive --dry-run    # plan JSON — spawns nothing

# arbitrary syn.* op drive (additive — drive only):
target/debug/syn-ctl drive --op syn.view.list                      # read-only live leg
target/debug/syn-ctl drive --op syn.control.status --out evidence/sv600/syn-ctl-drive-syn.control.status.jsonl
target/debug/syn-ctl drive --op syn.app.info --params '{}' --dry-run   # plan names the selected op
```

Steps: `test.hello` (implicit in the handshake) →
`test.observe.windows` → first `ready` window →
`test.fixture.prepare{ name:"ipc_emit", params:{window, body} }` →
`test.observe.receipts` → `test.shutdown` → child exit. `body` is one
`rvf.ipc.v1` intent JSON string — the default probe injects
`{"v":1,"kind":"intent","id":"syn-ctl-drive-1","op":"syn.app.info","params":{}}`;
an explicit `--op` derives `id` as `syn-ctl-drive-<op>`. The intent
crosses the app's real inbound boundary as a
`WindowLifecycle::Action` and takes the identical `syn.*` dispatch →
`post_event` path as normal IPC. The journaled `in "message"` →
`out "post_event"` receipt pair is the resulting state.

`--op` must name a `syn.*` op and `--params` must parse as one JSON
object — violations exit `2` before anything spawns (`doctor` rejects
both flags the same way). Live `--op` legs stay on read-only ops —
`syn.view.list`, `syn.window.list`, `syn.control.status`/`targets`,
`syn.probe.state`, `syn.monitoring.state`, `syn.recorder.status`,
`syn.recording.info`/`status`, `syn.library.providers`,
`syn.project.info`, `syn.app.capabilities`, `syn.signal_flow.view` —
mutating ops are scratch-session only or `--dry-run` plans. The
receipt journal is metadata-only (seq/direction/kind/bytes): `drive
--op` proves the intent reached live dispatch — the op's result
payload is not readable through the seam, so semantic verification
stays in the test lanes. An unknown `syn.*` op is dispatched
honestly and lands in the journal as a domain error.

Headless CLI surface (separate, non-seam): `synvelis-cli` —
`action list|describe|prepare|invoke`, `action run`, `workflow run`,
`script analyze`, `project info`, `render`,
`session issue|list|revoke`. Conventions: `--format human|json|ndjson`
(versioned envelope), `--dry-run` refuses side effects (read-only
commands reject it by name), exit codes are deterministic per outcome
(`crates/synvelis-cli/src/exitcode.rs`). `syn-ctl` does not wrap this
surface — drive it directly.

Scenario port (read-only reference): `tests/common/scenario.rs` +
`mounted.rs` hold the `SemanticGuiTest`/`GpuiGuiTest` shared
scenarios — run by `mise run test synvelis-app` (fake driver) and
`mise run test-app-gpui` (GPUI-simulated), not by `syn-ctl`.

Interactive note: the live drive legs need a desktop session (see
Launch). A `drive` that cannot spawn/serve records the step's
`not_run`/`fail` in the JSONL transcript — never a fabricated pass.

## Formal verification (Issue #601)

Two additional lanes verify the journal wire codecs
(`synvelis-gateway` commit journal + `synvelis-recording` staging
journal) — headless, no AUT:

```sh
mise run test-formal        # proptest property suites (always-on)
mise run test-formal-kani   # Kani bounded proofs (native, else WSL2)
KANI_JOBS=2 mise run test-formal-kani   # cap CBMC solver parallelism
                            # on shared/small-RAM hosts (default -j4)
```

- `test-formal` runs `cargo test --locked … --test formal_journal`
  for both crates — same entry as every test lane, no environment
  requirements.
- `test-formal-kani` runs `tools/run-formal-kani.mjs`: native
  `cargo kani` first, then a WSL2 distro holding `cargo` +
  `cargo kani` (Kani cannot run on native Windows). WSL executes
  with `CARGO_TARGET_DIR` on the WSL-native filesystem — the
  Windows managed target dir is untouched.
- Verdict discipline is the same vocabulary: the runner prints one
  JSON verdict line — `pass` (proofs held), `fail` (a real
  counterexample/unwinding violation — exit 1), or `not_run` +
  `environment_blocked` (no Kani environment — exit 0, never a
  fake pass).
- Coverage, bounds, assumptions, and the provision runbook live in
  `docs/specs/formal-verification.md`; evidence lands in
  `evidence/sv601/` (`test-formal-kani.log`, `verification.md`,
  mutation-fail records).

## Evidence

Ledger: `evidence/sv600/` (per-issue convention). Artifact kinds:

- **Doctor JSON** — `syn-ctl doctor --out` report; `checks[]` rows
  carry the verdict vocabulary plus `error.stage`/`next_step` on
  failure.
- **Drive JSONL** — `syn-ctl drive --out` transcript: a `context`
  line (`lane`, `run_id`, `app`, `transport`, `repo.sha`,
  `environment`), one `step` line per op with params + result,
  `child_exit`, and a closing `verdict` line.
- **`verification.md`** — the human-readable ledger: SHA (synvelis +
  resolved ruviform `84a68f25`), OS/arch, environment kind, commands,
  expected vs observed, NotRun/Unsupported with reasons.

Existing slice-1 assets (`uitest-inert-probes.jsonl`,
`uitest-live-transcript-run{1,2}.jsonl`, `uitest-live-receipts.jsonl`)
stay — the `syn-ctl` artifacts sit beside them.

Proof standard: evidence must show a real user path driven end to
end — the actions taken *and* the resulting state (Welcome JSON,
journaled receipts, cleanup report, child exit). A final output
alone is not proof. Side effects are confirmed by a second read-only
observation after the run (a second `drive` reproducing the same
reply shape, no leftover processes). Distinguish change-scope local
verification from integration verification on the merged SHA.

## Cleanup

Remove only what you started:

- `test.shutdown` returns a `CleanupReport` — `exited: true` means
  the AUT tore itself down; the driver then observes the child's
  exit status.
- `TestDriver::Drop` kills and reaps a still-running child — a
  stalled AUT never becomes an orphan.
- `syn-ctl` kills the child itself when `child-exit` fails and
  reports it in `checks[]` / the transcript.
- Never `kill` by process name; kill only PIDs you spawned.
- Never delete evidence — `evidence/sv600/` artifacts must survive
  teardown.
- `mbx`/`target` build artifacts are managed — do not redirect or
  hand-clean them.

## Helpers

`syn-ctl` — the control CLI (`crates/synvelis-app/src/bin/syn_ctl.rs`,
test-only; `required-features = ["uitest"]` keeps it out of
default/product builds):

```sh
target/debug/syn-ctl doctor [options]
target/debug/syn-ctl drive  [options]
  --app <path>        AUT binary (default: sibling synvelis-native)
  --transport <t>     stdio (default) | tcp
  --dry-run           print the plan JSON, spawn nothing
  --out <path>        write report (doctor) / JSONL (drive) to a file
  --op <syn.*>        drive only — the syn.* op to inject (default
                      syn.app.info); anything else exits 2
  --params <json>     drive only — the intent's params object
                      (default {}); non-object/parse failure exits 2
  --help, -h          usage + exit codes + next steps

syn-ctl --help        # same usage; exit 0
```

mise lanes: `check-app-uitest` / `lint-app-uitest` /
`test-app-uitest` / `build-app-uitest` (feature gates),
`test-app-uitest-ctl` (the ctl lane — manual/opt-in, interactive
gate), `run-app-native` (normal launch), `check-headless-deps`
(headless tree non-regression), `check-ruviform-rev`
(lock/vendor/manifest pin agreement), `test-app-gpui`
(scenario-port lane), `test-formal` / `test-formal-kani`
(Issue #601 journal codec property + bounded-proof lanes).

The `features/` map is normalized: every user-visible area — the
seam-driven window, the six main-screen tabs, the detached/mounted
surfaces (session/VCS/library/AUTX/variant-diff/script/plugin-dock),
the headless CLI, and the scenario catalog — has an entry with
verified recipes. The honest pending/blocked set (webview bin,
real-OS/plugin-dock EXT-L3, `syn-ctl` scenario drive) is recorded in
`features/README.md`.
