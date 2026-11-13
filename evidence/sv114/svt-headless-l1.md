# SVT-01〜09 Headless L1 Evidence Map (synvelis#114 / SV-GUI-05)

Date: 2026-11-14. Lane: OS-L1 headless (FakeDriver + `NativeShell` +
`UiRuntime`/`AppService`) and GPUI Test seam (`GpuiGuiTest`,
`gpui-test` feature). All results below are **semantic/headless
evidence only** — they do not prove real OS window/input/pixel,
native dock, plugin editor, or audio device behavior.

## Test entry points

- `crates/synvelis-app/tests/svt_product_e2e.rs` — 10 tests,
  product State/Receipt end-to-end (view → `syn.*`/`rvf.ui.command.v1`
  → `AppService` → receipt/projection → host display).
- `crates/synvelis-app/tests/svt_product_e2e_gpui.rs` — SVT-mapped
  view-layer scenarios on `GpuiGuiTest` (`GpuiSimulated` evidence).
- `crates/synvelis-app/tests/common/mounted.rs` — shared mounted
  fixture (same composition as `view_mount.rs`).

## SVT case coverage

| SVT | Headless product evidence | GPUI seam | Status notes |
| --- | --- | --- | --- |
| SVT-01 Project Create/Open→View | `svt01_*`: `syn.project.new` → `project.info` name change, view stays attached/listed, post-switch seek receipt `Applied`, arrangement rows honestly empty | — | real window re-composition = GUI-L2 NotRun |
| SVT-02 Selection→Inspector Focus | `svt02_*`: row select → `syn.view.list` selection/inspection == clip tag, inspector target follows, out-of-range `ok=false` preserves truth | `select_row_scenario` pass | real OS focus/IME = GUI-L2 NotRun |
| SVT-03 Fader gesture→receipt→UI | `svt03_*`: `Applied` receipt + `Observed` projection + committed/observed display | `fader_parity_scenario` pass | real pointer gesture = GUI-L2 NotRun |
| SVT-04 Undo/Redo + rejected truth | `svt04_*`: `set_ppqn` edit → `project.undo`/`redo` `undone`/`redone` true→false, unknown op error Result leaves `project.info` identical | — | revision-conflict CAS covered by project-layer tests |
| SVT-05 Transport Req/Applied/Observed | `svt05_*`: pre-delivery phase ≠ Observed, post-delivery Applied/Observed, `syn.control.receipt` broadcast `target=transport status=applied`, seek `Applied` | `seek_parity` + `invoke_scenario` pass | real playback audio = EXT-L3 NotRun |
| SVT-06 Waveform/Meter bounded/stale | `svt06_*`/`svt06b_*`: bounded `syn.viz.query` (`max_bytes` enforced, over-budget `ok=false`), `source_changed` resync → `status=ok`, unbound meta errors, meter push_frame/snapshot | — | NaN-gap rendering & real canvas/DOM pixel = NotRun; transient `view_stale=true` covered by `synvelis-workspace/tests/sv48_viz.rs` |
| SVT-07 Dock/2Window/Close/Job | `svt07_*`: second window + secondary view adopt, shared project session, `Destroyed` drops secondary ownership while project/main view truth preserved, surviving gesture `Applied` | secondary seek parity pass | real OS two-window/focus/native dock = GUI-L2/EXT-L3 NotRun; `CallOutcome::Job` producer = **Unsupported** (no `syn.*` op returns Job today) |
| SVT-08 Save/Reload oracle | `svt08_*`: `HEAD`→`snapshots/r<N>.json` parsed directly (bypasses app load path); name/track/clip counts + revision match post-reload `project.info`/`arrangement_rows` | — | real FS fault/durability = separate lane |
| SVT-09 IPC dup/delay/reorder/reattach | `svt09_*`: `syn.view.detach` → minted commands `Rejected` (not applied), `syn.view.attach` → `Applied` restored | stale generation/epoch pin rejection (existing `scenario_gpui.rs`/`binding.rs` tests) | duplicate/reordered *delivery* dedup = `binding.rs` receipt demux tests; wire-level reorder injector = NotRun |

## Real Service vs Mock/View-local invariants

- **Real Service (authority, proven here)**: `AppService` op
  `Result`/`Notify`, `syn.view.list`/`syn.project.info`/
  `syn.viz.meta`/`syn.viz.query` read models, on-disk bundle
  `HEAD`/`snapshots/r<N>.json` contents.
- **Mock/View-local (projection only)**: `ViewHost` display values,
  `CommandPhase`, optimistic selection, gesture state. Asserted as
  *observations of* authoritative receipts/projections — never as
  proof of product truth.
- **Plugin Panel**: `syn.app.info` reports plugin surface
  unavailable ("not wired in SV-GUI-01"); no fake plugin success is
  claimed. Plugin Editor Session / NativeSurfaceSlot product wiring
  is EXT-L3 / synvelis#176 scope — **Unsupported** here.

## Explicitly NotRun / Unsupported (not upgraded)

- Real OS window, user input, focus, IME, DPI, pixel capture
  (GUI-L2; Ruviform #313/#314 gates).
- Native dock move on real OS, real plugin editor session, real
  Audio/MIDI device (EXT-L3; synvelis#176).
- `CallOutcome::Job` product ops — no `syn.*` op produces jobs;
  `UiRuntime::drain_jobs` path exists but is unused by AppService.
- NaN-gap waveform rendering, real canvas/DOM paint.
- Wire-level IPC duplicate/delay/reorder injector (binding-layer
  dedup/stale tests cover the contract; no product-level injector
  exists).

## Verification commands (this slice)

- `mise exec -- cargo test -p synvelis-app --test svt_product_e2e`
  → 10 passed / 0 failed (auxiliary; canonical lane = mise tasks).
- GPUI lane: `mise run test-app-gpui` (see PR body for result).
