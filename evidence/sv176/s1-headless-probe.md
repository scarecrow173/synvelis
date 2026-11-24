# S1 Headless Real-ABI Plugin-Dock Probe — `rf631.plugin-dock` (synvelis#176 / SV-GPUI-02)

Date: 2026-11-25. Status: **headless real-ABI probe evidence — real
plugin binaries in-process, no real OS window, no child HWND, no
pixel/input/focus oracle.**

This file records the slice that lifted the dependency gate the freeze
documents cite: `synvelis-app` now dev-deps `ruviform-plugin-dock-test`,
`ruviform-plugin-vst3`/`ruviform-plugin-clap` (`host` feature), and the
ruviform#630 test-plugin crates `ruviform-plugin-test-vst3` /
`ruviform-plugin-test-clap`, all at the workspace ruviform pin
`20dc41f71c47674b3e51d1eb0921124b94f8bfaf` (single-SHA —
`check-ruviform-rev` pass). The dev-dep edge builds the fixture cdylibs
into `target/<profile>/deps/`, so the upstream `HEADLESS_MATRIX` and the
new product-level legs execute against real plugin binaries.

Artifacts: `evidence/sv176/s1-headless-probe.jsonl` (the canonical
`run_all` lane document — identical shape to the upstream lane binary's
output) and `evidence/sv176/s1-legs/*.jsonl` (one `case` line per
product-level leg). Test file:
`crates/synvelis-app/tests/plugin_dock_probe.rs` (7 tests).

## What ran

| Lane | Command | OS | Toolchain | Exit | Result |
| --- | --- | --- | --- | --- | --- |
| `rf631.plugin-dock` HEADLESS_MATRIX via `run_all` + product/CLAP legs | `mise run test synvelis-app --test plugin_dock_probe` (evidence run: `SYNVELIS_PLUGIN_DOCK_PROBE_JSONL=evidence/sv176/s1-headless-probe.jsonl SYNVELIS_PLUGIN_DOCK_PROBE_DIR=evidence/sv176/s1-legs cargo test -p synvelis-app --test plugin_dock_probe`) | Windows x86_64 (worktree `G:\orca\workspaces\synvelis\sv176-s1-headless`) | cargo test, locked deps, ruviform pin `20dc41f7`, repo `90b2ab34` at run time | 0 | 7 passed / 0 failed |

## `HEADLESS_MATRIX` verdicts (real run, `environment_kind: test-plugin`)

From `s1-headless-probe.jsonl` — `run_id: rf631-plugin-dock`, lane
`sv176-s1-headless`, driver `plugin-host-probe` unless noted:

| Case | Verdict | Basis |
| --- | --- | --- |
| `view-mount` | pass | Fake driver; semantic mount only — contract conformance, not dock/window/pixel evidence |
| `command-receipt` | pass | Fake driver; applied + rejected receipts recorded |
| `artifact` | pass | `ruviform_plugin_test_vst3-1ac2c390d2654dc3.dll`, sha256 `c12df015ce60b4092b3335cc6bc192cb431992c3579c5a8856f5c49aeaf55f4d`, `plugin_id 218a4f6d7c3b059ef1a24c68b9d37e51`, name `test_gain`, vendor `Ruviform Test`, version `0.0.0` |
| `host-lifecycle` | pass | Scan → Instantiate → Initialize → Activate(48 kHz, 1..=512) → Deactivate+Destroy, all `ok` on the real binary |
| `editor-attach` | unsupported | `createView` returned null → `HostError::Unsupported` → `EditorOpError::Unsupported` (test_gain ships no editor surface) — a defined refusal, never a fake pass |
| `session-lifecycle` | pass | begin/session-busy/lease/refused-open/`Failed{Unsupported}`/close-converge/`Closed`/end/rebegin — generation bumped, slot reused, instance deactivated+destroyed |
| `host-terminate` | pass | `Terminated{HostLost}` emitted exactly once; second notify is a no-op; session ends `closed-concluded` |
| `open-or-degrade` | pass | double open refusal, `Failed{Unsupported}`, **no `Degraded` event** — a refused dock never degrades silently |
| `dock-pixels` | not_run | no docked editor surface exists and no `VisualCapture` driver is present |

Fail-closed check: `missing_artifact_fails_closed_as_not_run` drives
`run_all` with a bogus explicit path — `artifact` + all dependents
record `NotRun`, `view-mount`/`command-receipt` stay `Pass`.

## Product-level and CLAP legs (`s1-legs/*.jsonl`)

All legs drive the product `PluginDock` through the `PluginEditorHost`
seam over a live `InProcessHost` — the same adapter-lend shape the GUI
driver's composition root will use, with the panel's issuer-side
`redeem_lease` validating the `ParentSlotLease`.

| Leg | Driver label | Verdict | Recorded outcome |
| --- | --- | --- | --- |
| `product-dock-test-gain-open` | `plugin-dock+in-process-host/vst3` | unsupported | `PluginDockError::Session(Ops(Unsupported))`; session `Failed{Unsupported}` stays observable in the binding, lease revoked, `SessionBusy` pins the instance, close converges, real instance Deactivate/Destroy still succeed (DSP non-interference oracle) |
| `product-dock-test-editor-attach` | `plugin-dock+in-process-host/vst3` | unsupported | `Ops(Plugin)` → `Failed{PluginFailed}` — `test_editor` ships a real `IPlugView` but the panel's lease payload is a slot index, not a live OS parent, so the plugin's `attached` hook delivers its defined `kResultFalse` rejection. Distinct from test_gain's capability-absence: an editor-bearing plugin refuses a non-real parent rather than fabricating a child window |
| `clap-artifact` | `clap-library/in-process` | pass | `ruviform_plugin_test_clap-b8085fbbc02de038.dll`, sha256 `b2a70e2bbdaa063de1d865a2a9719054fef92c17bb6afadd5525543eb776271c`, `plugin_id org.ruviform-test.clap-test-gain` |
| `clap-host-lifecycle` | `in-process-host/clap` | pass | Scan → Instantiate → Initialize → Activate on the real CLAP binary |
| `product-dock-clap-test-gain-open` | `plugin-dock+in-process-host/clap` | unsupported | `Ops(Unsupported)` → `Failed{Unsupported}` — CLAP `test_gain` has no `clap.ext.gui`, symmetric to the VST3 `test_gain` landing |

## Artifact resolution (dev-dep edge vs upstream lane layout)

Upstream `resolve_artifact` searches explicit env path →
`current_exe()/deps/<lib>` → `current_exe()/<lib>` for the un-hashed
fixture name — the layout the upstream lane binary gets from
`cargo build -p`. A dev-dep edge instead emits
`deps/<stem>-<hash><suffix>`. The probe preserves the upstream order and
adds `deps/../<lib>` (profile-root `cargo build -p` output) plus the
hashed dep-artifact glob as fallbacks; `RUVIFORM_PLUGIN_DOCK_TEST_VST3` /
`RUVIFORM_PLUGIN_DOCK_TEST_CLAP` still win verbatim, and a bogus explicit
path still fails closed to `NotRun`. The resolved path + sha256 land in
each report's `plugin_artifact` block, so every `Pass` is attributable
to exact bytes.

## Explicit non-claims

- No real OS window, child HWND, OS input, IME, focus, popup, DPI,
  pixel capture, or multi-window OS behavior was exercised —
  acceptance rows 1/7 stay `NotRun` (ruviform#630 real editor window,
  #631 runner, #532 DComp fix-and-rerun).
- `Pass`/`Unsupported` here are **in-process real-ABI** results, not
  Native Dock results. The headless lane proves host/process/editor
  binding on real binaries; it does not prove a docked surface exists.
- `test_editor`'s refusal proves only that a synthetic slot payload is
  rejected at the plugin attach hook — it is not evidence about real
  window parenting either way.
- Local run evidence only — row 4/8 (CI on the post-merge target SHA)
  remain the Coordinator's evidence step.
- macOS/Linux were not run in this slice; the same lanes execute there
  by construction but record no verdict until a CI run exists.
