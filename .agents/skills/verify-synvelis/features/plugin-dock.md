# Plugin dock

The plugin-dock surface is the native-plugin editor story: a `Panel`
holding one `NativeSlot` keyed `plugin.editor`, exercised through the
shared `rf631.plugin-dock` scenario vocabulary and its six-dimension
artifact evidence. Its honest status today is contract-conformance —
no real plugin ABI, native editor HWND, or pixel/focus evidence is
claimed.

## Sub-features

- `dock-slot` — `plugin_dock_view_spec()` mounts a `Panel` with the
  `plugin.editor` `NativeSlot` — layout/semantic proof only.
- `capability-gate` — `plugin_dock_spec()` requires
  `{NativePluginDock, VisualCapture}`, capabilities no semantic
  adapter claims, so the gated spec pre-flights `Unsupported`.
- `artifact-evidence` — `PluginArtifactEvidence` pins the six
  identity dims (path/SHA-256/plugin_id/name/vendor/version) a
  PD-evidence row must record before a `Pass` counts.
- `gpui-lane` — `plugin_dock_contract_gpui` runs the identical
  `view-mount` body on `GpuiGuiTest` under the `gpui-test` feature.

## How to get to it (user POV)

- There is no user-facing plugin-browser/dock surface yet — the
  contract surface is the scenario vocabulary plus the `NativeSlot`
  spec; PD-01〜07 Real OS / Real Plugin lanes stay `NotRun`
  (`evidence/sv176/`).
- `syn-ctl` carries no `syn.plugin.*` op family — plugin dock is not
  a `drive --op` surface (`blocked: seam 未対応` for live plugin
  evidence).

## Driving it with the test lanes

Preconditions:

- `mise run test synvelis-app` lanes run headless on any host;
  `gpui-test` lanes need no display.
- Real ABI test plugins / native editor windows are the
  `scarecrow173/synvelis#176` follow-up lanes, not this map's
  runnable recipes.

- **Semantic mount.** `mise run test synvelis-app --test
  plugin_dock_contract` resolves and observes the `plugin.editor`
  native slot on a mounted fake window — `Pass` here means contract
  conformance, never dock/pixel evidence.
- **GPUI mount.** `mise run test-app-gpui` runs
  `plugin_dock_contract_gpui` — the same body on `GpuiGuiTest`.
- **Honest gate.** The spec's required capability set keeps the
  real-dock claim `Unsupported` in semantic lanes — recorded, never
  promoted.

## Gotchas

- A semantic `view-mount` `Pass` is not native-dock proof: no real
  plugin ABI, child HWND, pixel, focus, or resize evidence is
  implied — PD lanes record `NotRun` until their environment exists.
- The capability pre-flight is the honest gate: a spec requiring
  `NativePluginDock`/`VisualCapture` must come back `Unsupported`, not
  silently pass.
- `test.observe.screenshot` on a full-size window trips the
  protocol's 1 MiB reply cap — a bounded `service_error`, not dock
  evidence.
- The receipt journal is metadata-only — even where a plugin-related
  `syn.*` op existed, emit proves dispatch only; plugin-dock
  semantics stay in the contract lanes.
