# L1 Contract Comparison — Synvelis product vs Ruviform Fake/Fixture

Date: 2026-11-14. Scope: OS-L1 contract comparison only. Ruviform
revision under test: `fd1d4d5` (Cargo.lock resolved git dep).
Referenced Ruviform issues: #243 (two-window/focus fixtures), #313
(real window/OS input/focus gate), #388 (NativeSurfaceSlot),
#389 (PluginEditorSession/host lease).

This file records **observed contract points** from the Synvelis
product oracle tests and the Ruviform fake/fixture seam. It is not a
claim that real OS/plugin acceptance passed — those remain
`NotRun`/`Unsupported` per POLICIES.md.

## 1. Two-Window Ownership / Focus / Close / Undo / Save

| Contract point | Synvelis product oracle (this repo) | Ruviform fake/fixture side |
| --- | --- | --- |
| Window identity & adoption | `svt07_*`, `tests/two_window.rs`, `tests/sv113_dock.rs`, `tests/dock_ops.rs` — `syn.window.adopt`/lifecycle ops, `view→window` reconciliation in `NativeShell` | `FakeDriver`/`FakeGuiTest` window registry; `WindowId` minting; `MultiWindow` capability declaration only (semantic) |
| Ownership isolation | `foreign_window_commands_are_rejected`, `cross_window_origin_view_is_rejected` (sv113_dock/two_window) — `origin_view` must name the view attached to *this* window | `ScenarioError::StaleGeneration`/`StaleEpoch` target pins in `ruviform-ui-test` |
| Close / project lifetime | `tests/close_veto.rs` (dirty last-window veto, quit partial veto), `svt07_*` (Destroyed → view detach, project truth preserved) | `WindowLifecycle::CloseRequested`/`Destroyed` driver events; close policy is consumer-owned (Synvelis `close_guard`) |
| Undo/Save across windows | `svt04_*`, `tests/view_mount.rs` undo/redo + save/reload preserve mounted view | n/a — project ops are Synvelis domain, not Ruviform contract |

Honesty note: Fake/GPUI adapters claim `{SemanticAction,
MultiWindow}` only — they never claim real OS focus, pointer, or
window-manager behavior (Ruviform #313 gate remains open).

## 2. Stale generation / attach epoch

| Contract point | Synvelis product oracle | Ruviform fake/fixture side |
| --- | --- | --- |
| Attach epoch namespace | `AttachEpochAllocator` session-owned; `reattach_bumps_generation_and_fresh_host_absorbs_stale_namespace` (view_mount.rs) | `AttachEpoch`/`AttachEpochAllocator` in `ruviform-ui::view`; `ScenarioTarget` pins epoch+generation |
| Stale command rejection | `rvf_command_on_stale_window_after_reattach_is_rejected`, `svt09_*` (detach → `Rejected`, reattach → `Applied`) | `check_target` in `FakeGuiTest`/`GpuiGuiTest` → `StaleEpoch`/`StaleGeneration` errors |
| Delayed projection floor | `projection_floors` high-water mark in `ViewHost` (ruviform-ui view.rs) — pre-remount projections cannot land | same host code drives both lanes (shared `ViewHost`) |

## 3. Plugin Editor Session / NativeSurfaceSlot (L1 contract only)

| Contract point | Synvelis product state | Ruviform owner |
| --- | --- | --- |
| NativeSurfaceSlot | Not wired into Synvelis views; VCS History native mount exists (PR #384) but plugin editor surface is absent | Ruviform #388 owns the slot contract/OS constraints |
| PluginEditorSession / host lease / recovery | `syn.app.info` reports plugin surface **unavailable**; no fake session is fabricated | Ruviform #389 owns session/lease/recovery |
| Product claim | **Unsupported** at this slice — recorded, not faked | EXT-L3 real plugin acceptance = synvelis#176 |

## 4. Job / IPC channel

- `UiRuntime` exposes `drain_jobs`/`drain_jobs_routed`/`poll` and
  `CallOutcome::Job`/`Event::Job` types, but **no `syn.*` op
  produces a job** — `AppService` documents this explicitly.
  Job-lifetime SVT coverage is therefore **Unsupported** at the
  product layer; the runtime plumbing is upstream-owned.

## 5. Gaps deferred to external gates

- Real OS window/input/focus/IME/DPI/pixel → GUI-L2 (Ruviform #313,
  #314).
- Real native dock / plugin editor / audio device → EXT-L3
  (synvelis#176, Ruviform #388/#389 acceptance).
- CI Run URL collection for these tests → Coordinator evidence step
  (merge 後の対象 SHA の CI 証拠)。
