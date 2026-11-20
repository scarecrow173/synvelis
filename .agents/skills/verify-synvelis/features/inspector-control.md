# Inspector tab and the control surface

The Inspector tab shows the selected target's breakdown — a read-only
table plus the control-relations table — and behind it the
`syn.control.*` family is the wire surface every GUI control lowers
into: dispatch, subscriptions, bindable targets, fast param writes,
and relation/edge inspection (UI-C5).

## Sub-features

- `inspector-target` — the target `Label` names the selected control;
  updates arrive through projections.
- `inspector-table` — the breakdown `Table` mounts read-only: row
  select is rejected by spec, not ignored.
- `inspector-relations` — the relations `Table` lists control
  relation rows (source/target labels + resolved endpoints).
- `control-wire` — `syn.control.dispatch` issues `CommandIntent`s
  (ack only, receipts notify); `subscribe`/`unsubscribe` manage
  (view_id, target) projections; `targets` lists bindable targets.
- `control-evidence` — `syn.control.status` reports
  `SurfaceReport`/`TickReport`/`ReportDrainReport`/generations;
  `syn.control.observe` inspects one call window's dispatch stages.
- `control-gestures` — `syn.control.fast_control` point-in-time
  writes; `syn.control.relation`/`syn.control.edge` drive the UI-C5
  relation inspector and mod-source→param edge gestures.

## How to get to it (user POV)

- Open the Inspector tab (`screen.tabs` index 3) after selecting an
  arrangement row or control — the breakdown and relations tables
  follow the selection.
- The tab is inspect-only: mutating a control happens on its own
  surface (fader, toggle), never through the inspector table.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` + `syn-ctl` build; live legs need an
  interactive desktop session.
- `mise run test synvelis-app` lanes run headless on any host.

- **Read-only honesty.** `mise run test synvelis-app --test
  screen_scenario transport_play_invokes` also proves
  `CASE_READ_ONLY_INSPECTOR` — the inspector table rejects row
  selection and the report records `not_run`, not a fake pass.
- **Control surface.** `mise run test synvelis-app --test
  pc_c4_06_control_op_surface` proves the deferred `syn.control.*`
  APIs return typed evidence (PC-C4-06).
- **Relations/edges.** `mise run test synvelis-app --test uic5_app`
  proves `syn.control.relation`/`syn.control.edge` through the
  product wire surface.
- **Mounted dispatch.** `mise run test synvelis-app --test
  view_mount` proves intent → `syn.control.dispatch` → receipt the
  long way round.
- **Live evidence.** `syn-ctl drive --op syn.control.status` and
  `--op syn.control.targets` inject read-only surface reads through
  the seam — dispatch journaled.

## Gotchas

- `inspector.table` is mounted read-only — a select Action on it is
  an explicit rejection; `arrangement.table` is the selectable one.
- `syn.control.dispatch` is mutating and ack-only — the semantic
  result arrives as receipt `Notify`s, not the call's return.
- `subscribe`/`unsubscribe`/`fast_control`/`edge` mutate control
  state — live `--op` legs stay on `status`/`targets`.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves live dispatch reached the app, not the
  report payload — control semantics stay in the lanes.
