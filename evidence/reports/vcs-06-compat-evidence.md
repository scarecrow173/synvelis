# VCS-06 COMPAT-03/04 — CI verify-leg evidence (synvelis#97)

CI verify legs that actually ran covering the VCS-06 fixture PRs
(#268 / #306 / #323), recorded with the required/observed leg
distinction defined by `.github/workflows/ci.yml` +
`tools/ci-summary.mjs`. These are **CI lane records only** — a CI
leg result is never converted into real-OS GUI, plugin-host, RT, or
consumer acceptance (`*-REAL-*` / COMPAT-02 lanes stay `NotRun`).

Collected 2026-11-11 via `gh pr checks` / `gh run list` /
`gh run view` against `scarecrow173/synvelis`; re-verified
2026-11-11T19:21Z and again 2026-11-12 (run list unchanged —
latest ci.yml run is still 42938180676, latest daily-ci
decision still 42938147481, next cron had not yet fired);
re-verified 2026-11-13 — a manual `workflow_dispatch` run
[43428177865](https://github.com/scarecrow173/synvelis/actions/runs/43428177865)
(triggering actor `scarecrow173`) covered `main` head
`7c206289b0d72af54aaf753ca5d94b9786a58c45`, recorded below.

## Anchor

- **VCS-COMPAT-03** was framed against synvelis#83 (independent
  PR/Push CI gate). #83 is **Closed** (2026-11-04). The living
  anchor is the current `.github/workflows/ci.yml` Full CI:
  `workflow_dispatch` only — `pull_request`/`push` triggers were
  removed by #145 — with two aggregation jobs:
  - `evidence` — the required status context consumed by the
    `main-checkstatus` ruleset; pr-mode fail-closed gate over the
    non-macOS required legs.
  - `ci-summary` — the Full CI verdict; **all** legs including
    the macOS legs plus `evidence` itself are required.
- `daily-ci.yml` (cron `29 21 * * *` UTC + `workflow_dispatch`)
  decides whether to dispatch `ci.yml` on `main` and does so as
  `github-actions[bot]`.

## Lane classification (tools/ci-summary.mjs, current main)

Required legs under the pr-mode `evidence` gate:

| leg | job name | runs |
| --- | --- | --- |
| verify | `verify (ubuntu-latest)` | prek, lint-actions, `mise run fmt-check`/`check`/`lint`/`test` (`cargo test --workspace --locked`), Cargo.lock diff guard |
| verify | `verify (windows-latest)` | same |
| rt-verify | `rt-verify` | `mise run test-rt`-class RT contract selection |
| gw10-verify | `gw10-verify (ubuntu-latest)` / `(windows-latest)` | GW-10 harness suites |
| msrv-verify | `msrv-verify` | rust 1.89 toolchain check/test |
| ui-verify | `ui-verify (ubuntu-latest)` / `(windows-latest)` | UI typecheck/build lanes |
| dep-guard | `dep-guard` | ruviform dep access + single-SHA lockfile guard |

Observed-only under pr mode (required under `ci-summary` full
mode — a macOS failure is a Full CI failure):

| leg | job name |
| --- | --- |
| verify-macos | `verify-macos` (same step set as `verify`, `macos-latest`) |
| gw10-verify-macos | `gw10-verify-macos` |
| ui-verify-macos | `ui-verify-macos` |

## PR-head CI

`gh pr checks` reports **no checks** on the head branches of all
three PRs (`scarecrow173/sv97-vcs06-matrix`,
`test/97-vcs-ui-headless-matrix`,
`test/97-vcs06-monitoring-matrix`). This is by contract, not an
omission: `ci.yml` has had no `pull_request` trigger since #145,
and no `workflow_dispatch` run targeted those branches. PR-head
CI evidence: **none exists — NotRun by design**.

## Post-merge `main` coverage

Merge commits (squash) and merge times (UTC):

| PR | merge commit | merged (UTC) | content |
| --- | --- | --- | --- |
| #268 | `3e699c7d6bf5c22922e6ef8f86fb9f074564b854` | 2026-11-10T16:09:41Z | `vcs06_{diff,apply,restore,git}_matrix.rs` + ledger |
| #306 | `76a3b1d46cc5f3ab2d43b1c355e2c3af0ef45a3c` | 2026-11-11T00:41:59Z | `vcs06_ui_matrix.rs` + ledger split |
| #323 | `9d6b963f4b93d5679435819e3aa600bd45241533` | 2026-11-11T10:07:20Z | `vcs06_monitoring_matrix.rs` + ledger rows |

`git merge-base --is-ancestor` against each ci.yml run's
`head_sha`:

| ci.yml run | head_sha | covers #268 | covers #306 | covers #323 | conclusion |
| --- | --- | --- | --- | --- | --- |
| [43428177865](https://github.com/scarecrow173/synvelis/actions/runs/43428177865) (2026-11-13T12:47:38Z, `workflow_dispatch`) | `7c206289b0d72af54aaf753ca5d94b9786a58c45` | yes | yes | **yes** | **success** |
| [43422470644](https://github.com/scarecrow173/synvelis/actions/runs/43422470644) (2026-11-13T12:07:05Z, `workflow_dispatch`) | `69153c09bf42f56ee54e8aa0056994086f2d5714` | yes | yes | **yes** | **success** |
| [42938180676](https://github.com/scarecrow173/synvelis/actions/runs/42938180676) (2026-11-11T01:20:02Z, `workflow_dispatch` by daily-ci run [42938147481](https://github.com/scarecrow173/synvelis/actions/runs/42938147481)) | `13e651891a8bdc5506f8dd542bfa66af62cab94b` | yes | yes | **no** | **failure** |

Earlier `main` runs (42543371916 success on `307002c6` @
2026-11-09, 42750871026 failure on `17fc3dec` @ 2026-11-10 —
`verify-macos` rt_cache alloc flake, tracked as synvelis#245)
all **predate** the three merges and cover none of them.

### Run 42938180676 leg outcomes (head_sha `13e6518`)

| leg | required? | outcome | notes |
| --- | --- | --- | --- |
| verify (ubuntu-latest) | required | **success** | `mise run test` green; vcs06 suites observed in log: `vcs06_apply_matrix` 8, `vcs06_diff_matrix` 5, `vcs06_restore_matrix` 8, `vcs06_git_matrix` 3, `vcs06_ui_matrix` 4 — all `ok` |
| verify (windows-latest) | required | **failure** | `mise run test` failed in `synvelis-library --test webdav`: 3 tests (`auth_revoked_between_pin_and_conditional_get`, `browse_partial_on_foreign_and_unaddressable_hrefs`, `browse_lists_children_complete`) hit `ProviderError::TransientIo` (os error 10053/10054) against the webdav MockServer. Cargo test fail-fast aborted the run before the `synvelis-project` package's test binaries executed — the VCS-06 project-side suites have **no** windows-leg result in this run. Unrelated to VCS-06 fixture content (library webdav lane). |
| rt-verify | required | success | |
| gw10-verify (ubuntu-latest) | required | success | |
| gw10-verify (windows-latest) | required | success | |
| msrv-verify | required | success | |
| ui-verify (ubuntu-latest) | required | success | |
| ui-verify (windows-latest) | required | success | |
| dep-guard | required | success | |
| verify-macos | observed (required under full) | success | `mise run test` green incl. all vcs06 suites present at `13e6518` |
| gw10-verify-macos | observed (required under full) | success | |
| ui-verify-macos | observed (required under full) | success | |
| evidence | gate (pr-mode) | **failure** | fail-closed: `verify (windows-latest)` required leg failed |
| ci-summary | gate (full-mode) | **failure** | fail-closed: required-leg failure propagates |

### Run 43428177865 leg outcomes (head_sha `7c206289`)

Manually dispatched 2026-11-13T12:47:38Z (`workflow_dispatch`,
actor `scarecrow173`) after the #323 merge and subsequent main
merges (#338/#340 fixture slices, #376–#379 CI/policy work).
All legs **success**; `evidence` and
`ci-summary` both green — the first uniformly-green covering run
for all three fixture PRs.

| leg | required? | outcome | notes |
| --- | --- | --- | --- |
| verify (ubuntu-latest) | required | **success** | `mise run test-full` green; all six vcs06 binaries observed in log |
| verify (windows-latest) | required | **success** | `mise run test-full` green; the webdav `TransientIo` failure seen in 42938180676 did not recur. vcs06 suites observed: `vcs06_ui_matrix` 4, `vcs06_git_matrix` 3, `vcs06_apply_matrix` 8, `vcs06_diff_matrix` 5, `vcs06_monitoring_matrix` 6, `vcs06_plugin_instance_matrix` 1, `vcs06_restore_matrix` 8 — all `ok` |
| rt-verify | required | success | `mise run test-rt` selection green; vcs06 binaries observed in both rt_ selection and RT contract steps |
| gw10-verify (ubuntu-latest) | required | success | |
| gw10-verify (windows-latest) | required | success | |
| msrv-verify | required | success | rust 1.89 `test-msrv` green; vcs06 binaries observed |
| ui-verify (ubuntu-latest) | required | success | |
| ui-verify (windows-latest) | required | success | |
| dep-guard | required | success | |
| verify-macos | observed (required under full) | success | `test-full` green incl. all vcs06 suites |
| gw10-verify-macos | observed (required under full) | success | |
| ui-verify-macos | observed (required under full) | success | |
| evidence | gate | **success** | all required legs green |
| ci-summary | gate (full-mode) | **success** | all legs incl. macOS green |

Run [43422470644](https://github.com/scarecrow173/synvelis/actions/runs/43422470644)
(2026-11-13T12:07:05Z, head `69153c09`) is an earlier same-day
covering run that also concluded **success** on every leg;
run [43420276313](https://github.com/scarecrow173/synvelis/actions/runs/43420276313)
(head `91a5e467`) was **cancelled** mid-flight and provides no
evidence.

### Coverage verdict per PR

- **#268 / #306 / #323**: covering ci.yml runs now exist for all
  three. Run 43428177865 (main `7c206289`, 2026-11-13) is
  uniformly green — every required and observed leg succeeded,
  both gates passed, and all six vcs06 test binaries ran and
  passed on ubuntu, windows, macOS, MSRV, and the rt-verify
  selection. The earlier covering run 42938180676 remains
  recorded as-is: ubuntu green, windows failed on an unrelated
  webdav mock transient, both gates failed. CI lane verdict:
  **Pass** on the latest covering run; the historical failure is
  preserved, not overwritten.
- **This branch** (`test/97-vcs06-real-git-fs-fault`): the new
  real-git/FS-fault rows added by this slice are **not** in any
  ci.yml run yet — `ci.yml` has no `pull_request` trigger, so PR
  head CI is NotRun by design, and post-merge coverage arrives
  with the next daily-ci dispatch after merge. Local
  `mise run test` results are recorded in the fixture matrix
  ledger, not here.

## What this does and does not establish

- Establishes: the repo's independent CI machinery ran on `main`
  content containing the #268/#306 fixture slices; per-leg
  required/observed outcomes and run URLs are recorded above.
- Does **not** establish: real-OS acceptance (a GitHub-hosted
  runner leg is not the product OS matrix), plugin-host/consumer
  acceptance, or PR-head gating (none exists by design). The
  windows-leg failure is a `synvelis-library` webdav mock issue —
  whether it is flake or residual defect is outside this slice;
  it is recorded, not adjudicated.
