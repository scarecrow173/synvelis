# Project lifecycle and PPQN

The Project tab and the `syn.project.*` ops are the project-bundle
surface: creating/opening/saving a bundle, inspecting the live
project, changing its PPQN (musical time resolution) with a read-only
preview, and undo/redo of project commands. The headless equivalent is
`synvelis-cli project info`.

## Sub-features

- `project-lifecycle` — `syn.project.new`/`open`/`save`/`reload`
  manage the bundle behind the session; `new` takes `params.path`
  (optional `name`/`ppqn`).
- `project-inspect` — `syn.project.info` returns name/id/revision/
  saved state; `syn.workspace.mount` summarizes the mount point
  (shell window, views, dock).
- `project-undo-redo` — `syn.project.undo`/`redo` step the project
  command history (`{"undone"|"redone": bool}`).
- `ppqn-change` — `syn.project.set_ppqn` rewrites project PPQN under
  the app gate (transport stopped / no automation write / merge
  commit — TIM-01).
- `ppqn-preview` — `syn.project.preview_ppqn` is the read-only
  dry-run: site counts, inexact/overflow, per-policy feasibility.
- `ppqn-ui` — the Project tab's select → preview → confirm/cancel
  gesture (`PpqnUi`) drives the same ops through view state.

## How to get to it (user POV)

- Open the Project tab on the main screen — PPQN status, preview, and
  change-effect labels live there.
- `synvelis-cli project info --project <path>` reads a bundle's
  metadata headlessly.
- `mise run run-app-native` launches the product window; project ops
  are the wire surface every GUI control lowers into.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` and the `syn-ctl` build produced both
  binaries; a live `syn-ctl` leg needs an interactive desktop session.
- `mise run test synvelis-app` lanes run headless on any host.

- **Inspect the live project.** `syn-ctl drive --op syn.project.info`
  (live, interactive session) injects the intent and journals the
  dispatch — read-only. Or plan it without spawning: `syn-ctl drive
  --op syn.project.info --params '{}' --dry-run` prints the plan JSON
  naming the op.
- **Service contract.** `mise run test synvelis-app --test
  service_ops` exercises the `syn.project.*` surface through the same
  dispatch the GUI uses — headless `syn.*` op evidence.
- **PPQN gate.** `mise run test synvelis-app --test tim01_ppqn_app`
  proves `set_ppqn`'s precondition gate and revision CAS headlessly.
- **PPQN preview.** `mise run test synvelis-app --test
  tim01_ppqn_preview` proves `preview_ppqn` read-only plus the
  optional `ppqn` param on `project.new`.
- **PPQN UI gesture.** `mise run test synvelis-app --test
  tim01_ppqn_ui` proves the view-owned select → preview →
  confirm/cancel flow over the real `syn.project.*` ops.
- **Headless read.** `synvelis-cli --format json project info
  --project <path>` prints the bundle metadata envelope (exit 0;
  missing `--project` is exit 2 `cli.usage`).

## Gotchas

- Mutating ops (`new`/`open`/`save`/`reload`/`set_ppqn`/`undo`/
  `redo`) write project state — drive them only against a scratch
  project path, never a live user session.
- `set_ppqn` is gated: a rolling transport or an in-flight automation
  write/merge commit rejects it — `preview_ppqn` is the honest probe
  that never mutates.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves the intent reached live dispatch, but
  the op's result payload is not readable through the seam — semantic
  result verification stays in the test lanes above.
