# Issue #602 verification — frozen 2-arm benchmark harness (slice 1)

- synvelis SHA: `6a26788e34f905dba7969ab990080bbef3505ea2` (worktree
  `sv602-frozen-bench`, branch `feature/602-frozen-bench-slice1`)
- ruviform resolved SHA: `84a68f25d5afd1b1f201018d76894d25b122f150`
  (lockfile pin; `check-ruviform-rev` REV match after `sync-ui-deps`)
- toolchain: `rustc 1.99.1 (58cf8f9c3 2026-10-13)` / cargo 1.99.1
- environment: windows / x86_64 / native-cli (16 cores)
- spec: `docs/test-specs/issue-602-frozen-render-bench.md`

## Lane matrix

| lane | command | expected | observed | verdict |
| --- | --- | --- | --- | --- |
| fmt | `mise run fmt-check` | clean | clean | pass |
| check | `mise run check synvelis-render` / `synvelis-player` | exit 0 | exit 0 (`--all-targets` incl. bench) | pass |
| lint | `mise run lint synvelis-render` / `synvelis-player` | clippy `-D warnings` clean | clean | pass |
| test | `mise run test synvelis-render` | pass | 99 tests pass | pass |
| test | `mise run test synvelis-player` | pass | all suites pass | pass |
| bench | `mise run bench-frozen` | gate pass + exit 0 | gate `pass` (5/5 cells), `errors=0`, `sha_mismatch=0`; `run-gate-pass.jsonl` | pass |
| selftest | `… --selftest` | gate fail + exit 1 | gate `fail`: K cells `ratio_below_threshold`, P cells `fingerprint_mismatch`; `selftest-gate-fail.jsonl` | pass (fail_expected) |
| fail-closed | `… --baseline <missing>` | fail + exit 1 | `baseline_unreadable` → gate fail | pass (fail_expected) |
| fail-closed | `… --baseline <candidate approval:null>` | fail + exit 1 | `unreadable-or-unpromoted` → gate fail | pass (fail_expected) |
| fail-closed | `… --baseline <malformed>` | fail + exit 1 | gate fail | pass (fail_expected) |
| filter | `… zzz-nomatch` | `gate not_run` + exit 0 | as expected | pass |
| smoke | `… --test` | `gate not_run` + exit 0 | as expected, `errors=0` all cells | pass |
| headless-deps | `mise run check-headless-deps` | pass | 12 headless crates clean | pass |
| ruviform-rev | `mise run check-ruviform-rev` | REV match | `84a68f25` match (after `sync-ui-deps`; vendor was stale `20a24a66`, pre-existing worktree copy — synvelis#230) | pass |
| hooks | `mise exec -- prek run --all-files` | pass | all hooks pass | pass |

NotRun/Unsupported: none for this slice (lane is headless/native-cli by
design; no interactive-session requirement).

## Measured cells (gate-pass run)

| cell | metric | value | bound |
| --- | --- | --- | --- |
| `mix/4x256` | ratio naive/prod | 0.0906 | `min_ratio` 0.0465 |
| `mix/8x512` | ratio | 0.1263 | 0.0596 |
| `mix/16x256` | ratio | 0.1009 | 0.0505 |
| `export/4x48000` | normalized | 2.63 | `max_normalized` 6.15 |
| `export/8x96000` | normalized | 5.73 | 11.69 |

Arm-K correctness: A/B PCM bit-exact (`errors=0`) on all cells.
Arm-P fingerprints: `1177683d…` / `bdda6380…` stable across reps
(`sha_mismatch=0`); `export_frames` 48064/96064 = range + `Known(64)`
tail.

## ruviform#823 (found during this slice)

`Prefetch::manual` initializes its free-block pool inside
`debug_assert!`; in release builds (`cargo bench`) the `try_push` side
effect is elided → producers `Backpressured` forever → consumers
underrun → all-silence (`written=0, mismatch=24576`). Debug builds
(`cargo test`, l1_e2e) never hit it. Workaround in root `Cargo.toml`:
`[profile.bench.package.ruviform-media-reader] debug-assertions = true`
with a removal condition comment. Upstream issue filed:
`scarecrow173/ruviform#823`.

## Files

- `decision.tsv` — appended run ledger (hillclimb schema)
- `run-gate-pass.jsonl` — full JSONL of the gate-pass run
- `selftest-gate-fail.jsonl` — full JSONL of the injected-regression run
- `baseline-candidate.sv602.json` — emitted candidate
  (`"approval": null`, kept unpromoted as evidence)
- promoted pin: `crates/synvelis-render/benches/frozen/baseline.sv602.json`
