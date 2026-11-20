# VCS history and A/B diff

The VCS surface is the project's version-control history: status,
commit rows, A/B node selection, freshness, and the deterministic
diff — `syn.vcs.view` projects it, `syn.vcs.visual` owns the view
lifecycle (open/close/select/pin/compare). Opening a VCS view mounts
`MountSpec::VcsHistory` onto a dockable window; `project.new`/`open`
auto-binds a `GitVcsProvider` when the bundle sits in a git
workspace.

## Sub-features

- `vcs-view` — `syn.vcs.view` projects status/rows/selection/
  freshness/diff for a `view_id`.
- `vcs-visual` — `syn.vcs.visual` runs the read-only view ops:
  `open`/`close`/`refresh`/`load_more`/`select_node`/`pin`/`compare`/
  `view_mode`/`clear`.
- `vcs-mount` — an opened VCS view mounts
  `history_screen_spec` (`MountSpec::VcsHistory`) keyed by its adopted
  `ViewInstanceId`.
- `vcs-backend` — `project.new`/`open` detect the enclosing git
  workspace and bind a real `GitVcsProvider` — production wiring,
  not a mock.

## How to get to it (user POV)

- A VCS History view is opened from the app's view surfaces (the
  `syn.vcs.visual` `open` lifecycle) and docks like other views.
- Honest degradation: unknown commits, partial diffs, and absent
  backends surface as named rows, never fabricated data.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` + `syn-ctl` build; live legs need an
  interactive desktop session.
- `mise run test synvelis-app` lanes run headless on any host.

- **Op surface.** `mise run test synvelis-app --test vcs04_history`
  proves the `syn.vcs.*` happy-path projections (VCS-04).
- **Honesty matrix.** `mise run test synvelis-app --test
  vcs06_ui_matrix` proves the Unknown/Partial honesty rows the
  happy-path fixtures skip (VCS-06).
- **Backend wiring.** `mise run test synvelis-app --test
  vcs_backend_wiring` proves `project.new`/`open` bind a real
  `GitVcsProvider` for bundles inside a git workspace.
- **Mount wiring.** `mise run test synvelis-app --test
  vcs04_native_mount` proves the dock-domain mount half; `mise run
  test-app-gpui` runs `vcs04_mount_gpui` — the mounted GPUI lane.
- **Live plan.** `syn-ctl drive --op syn.vcs.view --params
  '{"view_id":<id>}' --dry-run` prints the plan naming the op — a
  live `view` leg needs a mounted VCS view's `view_id`, so the
  live-driveable read here is deferred to scratch sessions; plan
  output is the env-independent recipe.

## Gotchas

- `syn.vcs.view` requires a `view_id` of an open VCS view — no such
  view exists at baseline, so the projection op is not a baseline
  live leg; `visual open` is the lifecycle mutation that creates one.
- `Unknown`/`Partial` are first-class honesty rows — a missing commit
  or backend is a named state, not an error verdict.
- The diff projection is deterministic — the same A/B pins always
  produce the same row set; freshness labels flag stale reads.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves live dispatch reached the app, not the
  diff content — VCS semantics stay in the test lanes.
