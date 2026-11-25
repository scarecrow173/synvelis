# TIM-01 PPQN Widget Intent — Headless L1 Evidence (synvelis#125)

Date: 2026-11-25. Lane: OS-L1 headless only (FakeDriver +
`NativeShell` + `UiRuntime`/`AppService`, `common::mounted`
fixture). All results below are **semantic/headless evidence
only** — they do not prove real OS window, pointer, focus, or
pixel behavior (GUI-L2/EXT-L3 = external gate `synvelis#114` +
`ruviform#631`).

Scope of this slice: Project tab `ppqn.*` widget specs carry
`rvf.ui.command.v1` Action params through
`ViewHost::invoke` → `decode_view_command` →
`answer_view_state_action` (`ppqn.` prefix on
`VIEW_STATE_TARGET`) → `dispatch_ppqn_action` mints
`syn.project.ppqn` intent → `op_project_ppqn` runs the
`ppqn_select`+`ppqn_preview_pending` / `ppqn_confirm` /
`ppqn_cancel` gestures → receipt → `publish_project_length`
spec reprojection syncs `ppqn.status`/`ppqn.preview`/`ppqn.sites`.

## Test entry points

- `crates/synvelis-app/tests/tim01_ppqn_widget.rs` — 10 tests,
  new. Widget spec shape, end-to-end
  widget→intent→gesture→op→receipt→spec reprojection,
  confirm-without-preview rejection, cancel, forged-action /
  unknown-action / domain-target-with-params / non-object-params
  rejections, typed `syn.project.ppqn` op-path equivalence.
- `crates/synvelis-app/tests/tim01_ppqn_ui.rs` — non-regression,
  unchanged semantics (spec gains candidates/gesture panels).
- `crates/synvelis-app/tests/common/mounted.rs` — shared mounted
  fixture; `press()` drives the real invoke → intent → op →
  receipt path.

## Action semantics pinned by tests

| `params.action` | Op behavior | Receipt |
| --- | --- | --- |
| `select` | `ppqn_select` + `ppqn_preview_pending` fused (one intent = selection + preview) | `{view_id, selected, preview}` |
| `confirm` | `ppqn_confirm` — CAS on preview-held expected revision; rejects when no preview pending | `{view_id, confirmed, project}` |
| `cancel` | `ppqn_cancel` — drops pending selection, project untouched | `{view_id, canceled}` |

Widget constraints fixed by tests: recommended-value Buttons on
`VIEW_STATE_TARGET` with `ppqn.*` action names (params admitted
only for view/synvelis.library/autx targets, so a dedicated
domain target cannot carry widget params); shell injects
`view_id`/`action` into op params — a widget cannot forge a
foreign view or action; Confirm/Cancel are always pressable and
reject honestly (no `disabled` visual state exists).

## Verification commands (canonical = mise)

| Command | Exit | Result |
| --- | --- | --- |
| `mise run fmt-check` | 0 | Worker's files clean. Pre-existing `library_ui.rs` fmt drift on `edc692ee` is out of lease — untouched, recorded here as auxiliary note. |
| `mise run check synvelis-workspace` | 0 | clean |
| `mise run lint synvelis-workspace` | 0 | clean |
| `mise run test synvelis-workspace` | 0 | all pass |
| `mise run check synvelis-app` | 0 | clean |
| `mise run lint synvelis-app` | 0 | clean |
| `mise run test synvelis-app` | 0 | 58 test binaries pass incl. `tim01_ppqn_widget` (10) |
| `mise run test-rt` | 0 | pass |
| `mise run check-app-native` | 0 | clean |
| `mise run lint-app-native` | 0 | clean |
| `mise run test-app-gpui` | 0 | 59 binaries, 0 failures |
| `mise run test-msrv` | 0 | workspace suite on Rust 1.89 — 335 "test result: ok", 0 failures |

Auxiliary (non-canonical): `cargo fmt -p synvelis-app -p
synvelis-workspace` used to apply formatting to worker files only
(補助証拠 — canonical fmt verdict is `mise run fmt-check` above).

## Explicitly NotRun / Deferred

- **GUI-L2** real OS window / pointer / pixel / focus:
  external gate (`synvelis#114` + `ruviform#631`). NotRun.
- **Issue acceptance rows 1–3** (Save/Reload/Undo/Redo + Beat
  invariants, v5 snapshot raw-MIDI preservation, long-song /
  overflow / division matrices): verified on main by PR
  `#235`/`#310`/`#345`/`#351`/`#385`; this slice maintains
  non-regression only — not re-verified.
- **Arbitrary 1..=32767 PPQN entry**: no text/number input
  widget exists in ruviform-ui `ComponentSpec`; out of slice
  (recommended values 480/960/1000/1920 via Buttons).
- **SMF export division ↔ project PPQN link**: separate slice,
  `app.rs` out of lease.
- **Presentation Driver time-source move**: separate slice,
  declared out of scope.
