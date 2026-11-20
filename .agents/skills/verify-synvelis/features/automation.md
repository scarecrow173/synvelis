# Automation (AUTX) surfaces

The AUTX surfaces are the authority-automation views — action
palette, macro browser, workflow editor, and run monitor — driven by
the `syn.autx.*` ops: a lifecycle op, a catalog push that feeds
projections and request lowering, and a per-view action dispatch
(AUTX-07, synvelis#266).

## Sub-features

- `autx-visual` — `syn.autx.visual` opens a surface
  (`action-palette`/`macro-browser`/`workflow-editor`/`run-monitor`)
  or closes it by `view_id`.
- `autx-catalog` — `syn.autx.catalog` pushes the automation catalog
  document `{seq, capabilities, macros}` — the projections and
  request lowering read it.
- `autx-action` — `syn.autx.action` dispatches one surface command
  `{view_id, target, action, params?}` — view-local mutation or
  lowered wire requests.
- `autx-mount` — an opened AUTX view mounts
  `MountSpec::Automation` — its spec rebuilds from the shared
  publish cell each push.

## How to get to it (user POV)

- AUTX surfaces open as dedicated views; each is reachable only after
  its `visual open` creates it.
- The app layer cannot name `synvelis-gateway` types — the surface is
  the wire contract, not the gateway authority path.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` + `syn-ctl` build; live legs need an
  interactive desktop session.
- `mise run test synvelis-app` lanes run headless on any host.

- **Surface contract.** `mise run test synvelis-app --test
  autx07_automation` proves the driver-neutral automation surfaces
  headlessly.
- **Mount wiring.** `mise run test synvelis-app --test sv266_wiring`
  proves the composition-root mount and `autx.*` action dispatch
  (SV266-NR-05).
- **Plan a lifecycle.** `syn-ctl drive --op syn.autx.visual --params
  '{"action":"open","surface":"action-palette"}' --dry-run` prints
  the plan naming the op — `open`/`close` and `catalog`/`action` are
  mutations, so live legs stay in scratch sessions; the plan is the
  env-independent recipe.

## Gotchas

- All three ops mutate state or require a `view_id` minted by `open`
  — none is a baseline live-driveable read; honest recipes are
  `--dry-run` plans plus the test lanes.
- `autx.action` on an unknown `view_id`/target is a named domain
  error, not a seam failure — check the transcript step, don't blame
  the transport.
- The publish cell can be absent between open and push — a skipped
  spec update is honest, not a missing frame.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves live dispatch reached the app, not the
  catalog content — AUTX semantics stay in the lanes.
