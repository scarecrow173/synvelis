# PD Headless Contract Evidence — `rf631.plugin-dock` view-mount (synvelis#176 / SV-GPUI-02)

Date: 2026-11-15. Status: **headless semantic contract evidence only —
not Native Dock, not real plugin, not pixel/input/focus evidence.**

This file records the only lane this slice executed: the shared
`rf631.plugin-dock` vocabulary (`ruviform-ui-test`, Cargo.lock pin
`ac10a2f7761c08db73aebd861c0a5c1f53f269b7`) exercised through the
driver-neutral `SemanticGuiTest` port on synvelis-app's two semantic
adapters. It exists so the freeze documents
(`pd-01-07-case-definitions.md` Headless/Simulated column) can point at
real executed evidence instead of asserting coverage that never ran.

Case definitions: `evidence/sv176/pd-01-07-case-definitions.md`.
Verification intent: `docs/test-specs/pd-01-07-plugin-dock.md`
(PD-FRZ-01〜07). Implementation record:
`docs/impl/ir/sv-gpui-02-plugin-dock-gate.md`.

## What ran

| Lane | Command | OS | Toolchain | Exit | Result |
| --- | --- | --- | --- | --- | --- |
| Headless Fake (`FakeGuiTest`, driver `Fake`) | `mise run test synvelis-app --test plugin_dock_contract` | Windows (worktree `G:\orca\workspaces\synvelis\sv176-pd-gate-freeze`) | cargo test, locked deps, ruviform pin `ac10a2f7` | 0 | 5 passed / 0 failed |
| GPUI-simulated (`GpuiGuiTest`, driver `GpuiSimulated`, feature `gpui-test`) | `mise run test-app-gpui --test plugin_dock_contract_gpui` | Windows (same worktree) | cargo test `--features gpui-test`, locked deps | 0 | 2 passed / 0 failed |

Test files: `crates/synvelis-app/tests/plugin_dock_contract.rs`,
`crates/synvelis-app/tests/plugin_dock_contract_gpui.rs`.

## What each lane proves — and what it does not

### `view-mount` case (`rf631.plugin-dock` / case `view-mount`)

Both adapters mounted `plugin_dock_view_spec()` — a `Panel`
(`plugin.dock`) holding one `NativeSlot` keyed `plugin.editor` —
through the production composition shape (`ViewHost` mount; GPUI lane
adds `create_view_window`), resolved the slot component through
`TargetQuery`/`resolve_target`, and observed it. Verdict `Pass` with
`lane_status() == "pass"`.

The `ScenarioReport` records only what the adapter actually is:

- Fake lane: `driver: Fake`, `capabilities: {SemanticAction,
  MultiWindow}`.
- GPUI lane: `driver: GpuiSimulated`, `capabilities: {SemanticAction,
  MultiWindow}`.

Neither report claims `NativePluginDock`, `VisualCapture`,
`RealOsPointerOrKeyboard`, `Ime`, `SimulatedPointerOrKeyboard`, or any
pixel/dock evidence. The outcome `detail` states verbatim: "semantic
view mount only — contract conformance, not dock/window/pixel
evidence".

**This is contract conformance of the declarative spec mount.** It is
not evidence that a plugin editor surface docks, attaches, receives
input, or renders pixels. PD-01〜07 Real OS / Real Plugin ABI /
Native Dock rows remain `NotRun`.

### `plugin_dock_spec` capability gate (spec-helper contract)

`plugin_dock_spec(run, case, None)` — the `ruviform-ui-test` helper
for consumer-declared specs — declares
`required = {NativePluginDock, VisualCapture}`. On both adapters
`run_scenario` returned `verdict: Unsupported` (`lane_status()
== "unsupported"`) at pre-flight — the scenario body was never
executed (asserted via a `Cell` flag).

This verifies a spec-level honesty property: any spec that
*requires* native-dock or pixel evidence cannot be satisfied by a
semantic adapter, so such a spec can never emit `Pass` on
`Fake`/`GpuiSimulated`. It does **not** model the upstream
`rf631.plugin-dock` probe cases: those are built with upstream
`probe_spec` (`required = {}`), gate on plugin-artifact presence
inside the body, and drive `InProcessHost`/`PluginEditorSessions`
directly under the `PluginHostProbe` driver. PD case rows therefore
cannot be upgraded by Fake/GpuiSimulated passes either way — but the
mechanism that keeps them `NotRun` is missing dependencies and
missing real environments, not this capability gate.

### Supporting contract checks (Fake lane)

- `plugin_dock_view_spec()` shape: root `Panel("plugin.dock")` with
  exactly one `NativeSlot("plugin.editor")` child — the slot key is
  the `bind_native_slot` lookup key drivers use after window
  creation; binding itself is out of scope for the view contract.
- `PluginArtifactEvidence` serializes all six identity dims
  (`path`, `sha256`, `plugin_id`, `name`, `vendor`, `version`) into
  `environment.plugin_artifact`, and `evidence_ref()` carries the
  path + digest prefix — the schema PD evidence rows must fill before
  any plugin `Pass` is attributable to exact bytes.
- `plugin_dock_spec` pins `scenario = "rf631.plugin-dock"` and the
  required capability set verbatim.

## Explicit non-claims

- No real OS window, real OS input, IME, focus, popup, DPI, pixel
  capture, or multi-window OS behavior was exercised.
- No real plugin binary (VST3/CLAP/AU) was loaded; no
  `PluginHostProbe` case ran (`artifact`, `host-lifecycle`,
  `session-lifecycle`, `host-terminate`, `open-or-degrade` remain
  `NotRun`/Deferred pending Ask-first dependency additions;
  `editor-attach` remains upstream `Unsupported` via `createView`
  null; `dock-pixels` remains `NotRun` — no `VisualCapture`).
- No product PluginDock view, `bind_native_slot`, or
  `SurfaceSlotId` binding exists yet — this evidence covers the
  shared upstream spec's mount contract inside synvelis's test stack
  only.
- Known upstream DComp pixel defect (`scarecrow173/ruviform#532`)
  is untouched and remains an open Fail/pending item for the real
  lanes.

## Lane status after this slice

| `rf631.plugin-dock` case | synvelis status |
| --- | --- |
| `view-mount` | headless `Pass` (contract conformance — Fake + GpuiSimulated) |
| `artifact` | `NotRun`/Deferred — probe deps not added (Ask-first) |
| `host-lifecycle` | `NotRun`/Deferred — same |
| `editor-attach` | `Unsupported` (upstream `createView` null) |
| `session-lifecycle` | `NotRun`/Deferred — same |
| `host-terminate` | `NotRun`/Deferred — same |
| `open-or-degrade` | `NotRun`/Deferred — same |
| `dock-pixels` | `NotRun` — no `VisualCapture` capability |

PD-01〜07 Real OS / Real Plugin ABI / Native Dock: all `NotRun`.
