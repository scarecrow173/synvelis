# Verification — TIM-04 permission fault matrix slice

対象 branch は `feat/128-tim04-permission-matrix`、最終検証前に `origin/main` `7c6bb6c04fd594bc5b86439fe726718ea5dbb56c` を fast-forward で取り込んだ。OS/Arch は Windows / x86_64。raw log は `verification-logs/`、permission matrix の仕様証拠は `tim04-permission-matrix.md` と `tim04_l1_abi_state.jsonl` を参照する。

## Canonical lane results

| command | exit code | result | note |
| --- | ---: | --- | --- |
| `mise run fmt-check` | 1 | Fail（scope外 environmental） | `cargo fmt --all -- --check` が全 workspace target列挙中に Windows `os error 206`（filename or extension too long）。本 sliceは新規 Rust fileを追加しておらず、変更fileのみの `cargo fmt --package synvelis-project --package synvelis-player -- --check` は exit code 0（補助証拠）。 |
| `mise run check synvelis-project` | 0 | Pass | `cargo check --locked --all-targets` |
| `mise run lint synvelis-project` | 0 | Pass | `cargo clippy --locked --all-targets -- -D warnings` |
| `mise run test synvelis-project` | 0 | Pass | `plugin_state` 19/19 Passを含む |
| `mise run check synvelis-player` | 0 | Pass | `cargo check --locked --all-targets` |
| `mise run lint synvelis-player` | 0 | Pass | `cargo clippy --locked --all-targets -- -D warnings` |
| `mise run test synvelis-player` | 0 | Pass | `tim04_l1_abi_state` 4/4 Pass（permission matrix testを含む）、`tim04_real_abi_state` 6/6 Pass |
| `mise run test-rt` | 0 | Pass | RT lane非回帰。state操作はnon-RT control pathに限定 |
| `mise run check-headless-deps` | 0 | Pass | headless dependency gate非回帰 |
| `mise run check-ruviform-rev` | 1 | Fail（scope外 known stale vendor hazard） | ui vendor REV `20a24a66ee90f4d462224391e78c10c93f4f31ba` != lockfile resolved `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`。本 sliceは ruviform pinを変更しておらず、`ui/vendor`・`sync-ui-deps` は占有外のため触れていない。 |

## Evidence generation run

```sh
SYNVELIS_TIM04_L1_EVIDENCE_DIR=evidence/sv128 mise exec -- cargo test -p synvelis-player --test tim04_l1_abi_state l1_permission_fault_matrix_real_rfst -- --nocapture
```

exit code 0、1/1 Pass。cargo直接実行のため補助証拠であり、canonical package testの代替ではない。permission matrix JSONL rowsは30件で Pass 28 / Unsupported 2 / Fail 0 / NotRun 0。

## Scope-out failure assessment

`fmt-check` failureはソース差分ではなく workspace全体のtarget path列挙長に依存するWindows process/path制限で再現した。`check-ruviform-rev` failureは既知のstale ui vendor REV mismatchであり、対象SHA・現象・scope外理由をここに記録する。どちらも本 sliceのコードで修正したとは主張しない。
