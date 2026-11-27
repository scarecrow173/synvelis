# TIM-03 dynamic musical transport verification

Issue: `https://github.com/scarecrow173/synvelis/issues/127`  
Machine rows: `dynamic-musical-transport.jsonl` (one row per real plugin format)  
Raw focused real-ABI output: `dynamic-musical-transport-l1.log`

## Scope and verdict

The representative TIM-03 dynamic musical transport slice is Pass on Windows x86_64 for real out-of-process VST3 and CLAP `test_instrument` fixtures. Production composition used `compose_routing_engine_with_out_of_process_and_boundaries`; no direct endpoint `TransportInfo` injection was used. Both formats observed the independent dynamic click law exactly after a valid tempo edit before measured transport and after a valid tempo edit while playing.

## Dynamic measurements

| fact | VST3 | CLAP |
| --- | --- | --- |
| fixture SHA-256 | `941e7dfba9a5930c3a3136c37d1a4008e67c7ff1288c964c8a18ac04a0d03fe1` | `4296ccf1d3f4732e3a02be023e829990dee7fb175f6af7ebe84083c7878139dd` |
| host/project PPQN | 480 | 480 |
| adapter projection PPQN | 960 | 960 |
| initial/final tempo revision | 2/4 | 2/4 |
| before-measured edit | tick 4800, 75.000 BPM, revision 3 | same |
| before-measured click | expected 672, observed `[672]` | same |
| while-playing edit | tick 9600, 100.000 BPM, revision 4 | same |
| while-playing click | expected 632, observed `[632]` | same |
| refresh errors | `{}` after both edits | same |
| plan carries current project source | true after both edits | same |
| ordinary seek positives | 704/640/576 exact | same |
| loop positives | `[1024,1792]` exact | same |
| stopped/paused negatives | pass/pass | pass/pass |

For each dynamic case the machine row records control-surface evidence (`tempo_revision`, PPQN, current-source identity, empty refresh errors, event counts) and RT install receipts. The final receipt for each measured plan install is `Applied`; requested/pending transitional rows are included to make scheduling visible. Loop measurement is separated from dynamic positions by `ClearLoop` and additional blocks so old loop events cannot contaminate dynamic evidence.

## Player/control proof

`crates/synvelis-player/tests/tim03_musical_transport_plan.rs` passed 4/4 after the final lint cleanup. In addition to compile-source and invalid-input tests, the representative mixed-plan test proves that staged MIDI, unbound fast-control events, and `Some(TransportGeneration(1))` events preserve position, source sequence, payload, generation binding, and staged MIDI across musical refresh. The production construction-site compatibility mode is `FastControlWrite::transport_generation: None`; only the explicit test event uses `Some`.

## Verification commands

| lane | result | raw log |
| --- | --- | --- |
| `cargo check --locked --all-targets -p synvelis-player -p synvelis-app` | Pass, exit 0 | `dynamic-musical-transport-check.log` |
| `cargo clippy --locked --all-targets -p synvelis-player -- -D warnings` | Pass, exit 0 | `dynamic-musical-transport-lint-player.log` |
| `cargo clippy --locked --all-targets -p synvelis-app -- -D warnings` | Pass, exit 0 | `dynamic-musical-transport-lint-app.log` |
| focused player TIM-02/TIM-03 tests | Pass, exit 0 | `dynamic-musical-transport-player-tests.log` |
| package tests `cargo test --locked -p synvelis-player && cargo test --locked -p synvelis-app` | Pass, exit 0 | `dynamic-musical-transport-package-tests.log` |
| `mise run test-tim03-production-transport` | Pass, exit 0; VST3/CLAP 2/2 | `dynamic-musical-transport-final-lanes.log` |
| `mise run check-full` | Pass, finished in 52.18s | `dynamic-musical-transport-final-lanes.log` |
| `mise run test-rt` | recorded after final run | `dynamic-musical-transport-test-rt.log` |

The earlier focused real-ABI run was Pass for both formats. Final lane rows in this table are updated only after the post-lint canonical runs complete; any lane absent from the PR body is not claimed as Pass.

## Honest non-scope

The full dynamic tempo edit matrix beyond the two representative cases, full frame/boundary matrix, fault/state/crash isolation, GUI/EXT-L3, macOS/Linux runtime, 3OS CI, and AudioLens cross-consumer coverage remain NotRun/follow-up. Invalid committed tempo edits are rejected by project operation validation; typed invalid-state behavior is therefore covered at the player/control boundary rather than as a committed production edit.
