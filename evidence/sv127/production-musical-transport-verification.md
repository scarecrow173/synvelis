# TIM-03 production musical transport verification

Date: 2026-11-25 / branch `feat/127-production-musical-transport` / OS: Windows x86_64.

## Scope and verdict rule

This slice verifies the Synvelis production consumer obligation only: project-owned tempo map, tempo revision, and project PPQN are compiled once at the production control boundary and supplied to every production `TransportPlan` through `TransportPlanBuilder::with_musical_transport`. The end-to-end positive is production composition → routing activation → `control_apply_transport` → `PluginNode` → out-of-process real ABI `test_instrument` → independent PCM. Direct endpoint injection and caller-supplied `TransportInfo` are not used.

Pass/Fail/NotRun are kept separate. A missing, inconsistent, or uncompilable project musical half is an honest absent musical half plus typed evidence; it is never repaired with a fixed 960 PPQN fallback. Fixture-visible PPQN 960 in click-law arithmetic is the VST3/CLAP adapter projection, while the host/project plan PPQN is 480 in the production evidence.

## Ruviform pin

| item | value |
| --- | --- |
| before | `0479fa62e77e669a7bf84320d3d5418ffb949fc6` |
| after | `511e1a7944c35797942ba065c092b9263067c48a` |
| required upstream handoff | ruviform PR #1043 merge commit `84abc76f4674ba587482197d314005054d7bda6f` |
| after commit subject | `test(formal): Wire frame length decision core proof (#1045)` |
| candidate rule | ruviform `origin/main` at dispatch time, descendant of `84abc76f...` |
| scoped lock diff | exactly 72 lines: 36 ruviform git source rewrites, no unrelated dependency drift |
| handoff presence | pinned `ruviform-audio-plugin-bridge/src/node.rs` passes `musical.tempo_map` and `musical.ppqn` through `call_transport_info` |

`mise run sync-ui-deps` exited 0. `mise run check-ruviform-rev` exited 0 and reported resolved `511e1a7944c35797942ba065c092b9263067c48a`, 36 lockfile sources, lockfile-primary, REV match.

## Canonical command matrix

Canonical means the `mise` entry point. Direct `cargo fmt` is auxiliary evidence only.

| command | result | evidence |
| --- | --- | --- |
| `mise run sync-ui-deps` | Pass, exit 0 | vendor already synced at `511e1a...` |
| `mise run check-ruviform-rev` | Pass, exit 0 | lockfile-primary REV match |
| `mise run fmt-check` | Infrastructure failure, exit 1 | Windows Orca worktree path hits `os error 206` before/in `cargo fmt --all -- --check`; success condition not weakened |
| auxiliary `mise exec -- cargo fmt -p synvelis-player -p synvelis-app -- --check` | Pass, exit 0 | package-scoped formatting evidence only |
| `mise exec -- prek run --all-files` | Pass, exit 0 | pre-commit hooks passed |
| `mise run check-headless-deps` | Pass, exit 0 | no GUI framework deps in 12 headless crates |
| `mise run test-tim03-production-transport` | Pass, exit 0 | VST3 and CLAP both passed after the final implementation |
| `mise run test-tim03-transport` | Pass, exit 0 | `tim03-lanes-final.log` |
| `mise run test-tim03-instrument-transport` | Pass, exit 0 | `tim03-lanes-final.log` |
| `mise run test-tim03-l1` | Pass, exit 0 | `tim03-lanes-final.log` |
| `mise run check synvelis-player` | Pass, exit 0 | `player-lanes.log` |
| `mise run lint synvelis-player` | Pass, exit 0 | `player-lanes.log` |
| `mise run test synvelis-player` | Pass, exit 0 | `player-lanes.log` |
| `mise run check synvelis-app` | Pass, exit 0 | `app-lanes.log` |
| `mise run lint synvelis-app` | Pass, exit 0 | `app-lanes.log` |
| `mise run test synvelis-app` | Pass, exit 0 | `app-lanes.log` |
| `mise run check-full` | Pass, exit 0 | `check-full.log` |
| `mise run test-rt` | Pass, exit 0 | `test-rt.log` |

The final PR must copy the exact exit codes from the lane logs. Any lane not completed is NotRun, not Pass.

## Production end-to-end measurements

Machine rows are in `../production-musical-transport.jsonl`; there is one row per format. Both rows were produced by `mise run test-tim03-production-transport` on Windows x86_64 with `--test-threads=1`.

| fact | VST3 | CLAP |
| --- | --- | --- |
| fixture SHA-256 | `36001409b432c0046fc7f0bec45db463cd55d8754e8d52397915539fc60ee7ff` | `97cbe8ab69426ae0c4af5bd53bb4d7c990a1792e6a1bb9f698b39ed6d39a35b1` |
| production seam | `compose_routing_engine_with_out_of_process_and_boundaries` | same |
| direct endpoint `TransportInfo` | not used | not used |
| BoundaryDeclaration | `FrameAccurateEvents` / `InBlockTransportSupport::Unknown` | same |
| host/project PPQN | 480 | 480 |
| adapter projection PPQN | 960 | 960 |
| tempo revision | 2 | 2 |
| tempo events | `(0,120.000 BPM)`, `(1,93.750 BPM)` | same |
| positive seek cases | delta 3/2/1 host ticks; observed 704/640/576 | same |
| loop positives | observed 1024 and 1792 | same |
| stopped/paused negative | pass/pass | pass/pass |
| amplitude / channel law | 0.5, stereo identical, non-click frames silent | same |

The independent positive law is recorded per case in JSONL:

```text
ceil((boundary_tick - position_ticks) * sample_rate * 60000
     / (bpm_milli * adapter_ppqn)) + lookahead_frames
```

At 93.750 BPM and adapter-projected PPQN 960, one projected tick is 32 frames at 48 kHz. L1 lookahead is two 256-frame blocks (512 frames). Thus the three fresh positive windows are `512 + 192 = 704`, `512 + 128 = 640`, and `512 + 64 = 576`. Seek/position-jump freshness uses a different quarter and delta for each case so bit-identical duplicate blocks are not counted as independent evidence.

Loop evidence uses a 12-host-tick loop (768 frames = three blocks). The quarter boundary lands at block-local frame 0 for each iteration and becomes visible after the two-block lookahead at absolute fresh positions 1024 and 1792. Earlier pass-through/substituted/lookahead blocks are explicitly excluded from fresh evidence.

## Player-level consumer proof

`crates/synvelis-player/tests/tim03_musical_transport_plan.rs` passed 3/3 in the focused canonical package test binary:

- production mint paths carry the complete project source (non-default PPQN 480, non-default tempo, revision 2);
- musical refresh preserves event positions, ordering, payload, and transport-generation binding;
- invalid/mismatched project state is honestly absent and does not synthesize fixed PPQN 960 or a partial pair.

## Honest negatives and non-scope

- Missing/unknown/compile-failure musical state: covered at player plan level; production surface reports typed `Unavailable` evidence and plans carry neither half.
- Stopped/paused: real ABI negative controls pass for both plugin formats.
- Dynamic tempo edit matrix, full 100/128/256 × boundary matrix, fault/state/crash isolation, GUI/EXT-L3, macOS/Linux runtime, 3OS CI, AudioLens cross-consumer: NotRun/follow-up, not Pass.
- Docs indexes are intentionally untouched; Coordinator synchronizes generated indexes at integration time.

## Failure triage during implementation

- `clippy` first reported a manual `Default` implementation and redundant `map(|plan| Some(plan))`; both were fixed mechanically and the package lint lane was rerun.
- `tim03_l1_production_musical_transport` first exceeded `clippy::too_many_lines`; the test was split into small execution/evidence helpers without weakening assertions.
- An intermediate full `synvelis-player` test run exposed generation drift in pre-existing observation tests because the initial empty musical plan was installed during raw surface attachment. The initial empty install was narrowed to the production MIDI-plan reconciliation boundary (`sync_midi_plans`), preserving content-free production delivery without changing manual attach generation semantics. The focused `pc_c4_03_observation` suite then passed 8/8.
- Workspace-wide `fmt-check` is an infrastructure failure caused by the long Orca worktree path (`os error 206`). Package-scoped rustfmt checking passed; the workspace failure remains documented rather than hidden.
