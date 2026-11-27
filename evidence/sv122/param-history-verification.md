# CTL-02 Fader and SendLevel history verification

## Verified revision

- Repository: `scarecrow173/synvelis`
- Branch: `feature/122-param-history`
- Main base: `bf0442cfad73c5102d137f3f1e20ac74d84860b3`
- Tested commit: `29691cc83c5524f9d19f97d215cab015b1d70da8`
- Ruviform revision from `Cargo.lock`: `511e1a7944c35797942ba065c092b9263067c48a`
- OS: Windows 26H2 x64, build `10.0.26300.9974`
- Environment: default Cargo features, headless project controller, `InMemoryStore`

## Runtime verification

`fader_and_send_level_preserve_independent_values_through_history_and_reload` in
[`ctl_02.rs`](../../crates/synvelis-project/tests/ctl_02.rs) uses production
`ProjectOp::SetParameterBase`, `ProjectController::undo` and `redo`, and
`ProjectController::save` and `load`. It checks the original, distinct target IDs
and both values after each action.

| State | Fader | SendLevel |
| --- | ---: | ---: |
| Initial | 1.0 | 0.75 |
| Edit Fader | 0.625 | 0.75 |
| Edit SendLevel | 0.625 | 0.375 |
| Undo SendLevel | 0.625 | 0.75 |
| Undo Fader | 1.0 | 0.75 |
| Redo Fader | 0.625 | 0.75 |
| Redo SendLevel | 0.625 | 0.375 |
| Load in a new controller | 0.625 | 0.375 |

The test reads the JSON bytes written by both saves. The final envelope equals
the baseline with only the revision and the two target `base` fields changed.
A second persisted field that changes with either edit would fail this
comparison. The test uses the production serialization path but does not test
filesystem durability.

## Command results

| Command | Exit | Verdict | Observation |
| --- | ---: | --- | --- |
| `mise run fmt-check` | 1 | Fail (environment) | Windows `os error 206` prevented the workspace-wide rustfmt process from launching. |
| `mise exec -- cargo fmt --package synvelis-project -- --check` | 0 | Pass (supplemental) | Formatting check for the changed package. The canonical command remains `mise run fmt-check`. |
| `mise run check synvelis-project` | 0 | Pass | Package type check completed. |
| `mise run lint synvelis-project` | 0 | Pass | Package Clippy completed with warnings denied. |
| `mise run test synvelis-project --test ctl_02` | 0 | Pass | 22 passed, including the new history test and the existing send creation and failure cases. |
| `mise run test synvelis-project` | 0 | Pass | 657 passed, 0 failed, 3 ignored across 58 suites, including doc tests. |
| `mise exec -- prek run --all-files` | 0 | Pass | All applicable hooks passed. No files changed. |

## Acceptance limits

- AC1: Windows OS-L1 is Required and Pass for this example of Fader and
  SendLevel history and persistence. The existing atomic send creation tests
  also pass. Pan, Mute, Solo, GUI, and automation acceptance are NotRun in this
  slice.
- AC2: Graph and clock generation, FrameBase, actual plugin binding, and
  Pending/Blocked/Observed presentation are outside this slice and NotRun.
- AC3: Channel conversion approval, Host/Plugin/Realtime/Offline/2Window, and
  AudioLens non-regression acceptance are outside this slice and NotRun.
- Windows GUI-L2 and EXT-L3 are NotRun. macOS and Linux OS-L1 are NotRun
  locally; GUI-L2 and EXT-L3 are Deferred under `POLICIES.md`. No formal proof
  was run. The integration test is example evidence, not a proof.
- The local checks cover the changed package. Integrated verification remains
  NotRun until the required CI results are available for the merged main SHA.
