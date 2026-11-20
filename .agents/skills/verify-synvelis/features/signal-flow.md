# Signal Flow tab and probes

The Signal Flow tab renders the routing graph — status, nodes, edges,
and probe rows — over the `syn.signal_flow.*` op family and the
`syn.probe.*` semantic-probe surface. Per-view visual state
(fold/zoom/scope) persists per mounted view; probes capture
segmented traces for comparison.

## Sub-features

- `flow-view` — `syn.signal_flow.view` projects coverage/nodes/edges/
  status (`view_id?`, `scope?`, `roles?` optional).
- `flow-visual` — `syn.signal_flow.visual` runs view-local visual ops
  (`move_node`/`set_group`/`fold`/`unfold`/`zoom`/`pan`/`link_focus`/
  `set_scope`) persisted per view.
- `flow-inspection` — `syn.signal_flow.inspection` drives the
  Follow/Explore/Pinned inspection focus
  (`inspect`/`set_mode`/`set_follow_source`/`back`/`forward`).
- `flow-routing` — `syn.signal_flow.routing` is the port-drag gesture
  (`begin`/`hover`/`drop`/`confirm`/`cancel`/`state`).
- `probe-lifecycle` — `syn.probe.control` runs
  request/set_mode/capture/stop_capture/remove;
  `syn.probe.state` projects bindings/segments/drain/traces;
  `syn.probe.compare` aligns captures into an `AcousticObservation`
  (SF-04).

## How to get to it (user POV)

- Open the Signal Flow tab (`screen.tabs` index 4) — status, node
  and edge tables, and the probe list live there.
- Nodes drag, groups fold, the scope selector reslices the graph;
  probes arm from the surface's own controls.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` + `syn-ctl` build; live legs need an
  interactive desktop session.
- `mise run test synvelis-app` lanes run headless on any host.

- **Flow surface.** `mise run test synvelis-app --test
  sf05_signal_flow` proves `view`/`visual` ops, per-view persisted
  visual state, and honest engine-evidence status (SF-05).
- **Probe wiring.** `mise run test synvelis-app --test
  sf04_probe_wiring` proves `syn.probe.control`/`syn.probe.state`
  and the `ProbeTraceBridge` drain hook; `--test sf05_probe_share`
  covers the subscriber-lease/capture ownership half.
- **Probe compare.** `mise run test synvelis-app --test
  sf04_probe_compare_op` proves `syn.probe.compare`'s
  `ProbeComparisonRequest → AcousticObservation` wire.
- **Session engine.** `mise run test synvelis-app --test
  sf03_session_engine` proves the `ProjectSession` →
  `SessionRoutingEngine` revision feed behind the tab.
- **Live projection.** `syn-ctl drive --op syn.probe.state` and
  `syn-ctl drive --op syn.signal_flow.view --params '{}'` inject the
  read-only projections through the seam — dispatch journaled,
  payload semantics verified in the lanes.

## Gotchas

- `syn.signal_flow.view` accepts an optional `view_id` — without one
  it projects the default scope, not a mounted view's visual state;
  the per-view visual half only exists after a `visual` op ran.
- `visual`/`inspection`/`routing` and `probe.control`/`compare`
  mutate view or capture state — live `--op` legs stay read-only;
  gestures belong to scratch sessions or the test lanes.
- `syn.probe.compare` needs a `request` object — a bare `--params
  '{}'` is a named domain error, not a seam failure.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves the intent reached live dispatch, not
  the projection payload — flow/probe semantics stay in the lanes.
