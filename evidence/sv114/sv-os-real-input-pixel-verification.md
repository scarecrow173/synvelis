# SV-OS-01 real input/pixel vertical slice verification evidence

Date: 2026-11-26. Branch: `feat/114-svos-real-input-pixel` (partial slice; `Refs` synvelis#114).
このファイルは Worker が実行可能な compile/static/headless 検証と、interactive real lane を Coordinator gateへ分離する記録である。Issue #114 全体の acceptance ではない。

## Implementation evidence boundary

| lane | verdict | 意味づけ |
| --- | --- | --- |
| SV-OS-01 input/pixel code path | implemented / compile Pass | `project.save` targetに OS automation pointer move/down/up、Command intent、harness-posted Applied receipt、settled observation、capture metadataを記録する code pathが `real-os-scenario` で compile/lint pass。interactive実走は NotRun。 |
| headless Fake | Pass | `sv_os_headless` fake lane 3/3 pass。semantic OS-L1 evidenceであり GUI-L2 evidenceではない。 |
| GPUI simulated | Pass | `test-app-gpui` 内 `sv_os_headless` 4/4 pass。`GpuiSimulated` semantic evidenceであり pixel/real inputを主張しない。 |
| interactive real OS input/pixel | NotRun | Workerは実 windowを開く laneを実行しない。Coordinator gateで SV-OS-01 rowの ops/Command/Receipt/oracle/captureを確認する。 |
| Native Plugin Dock / IME / DPI / monitor / PD-01〜07 | NotApplicable or Deferred/NotRun | 本 sliceの scope外。Passへ昇格しない。 |

## Canonical local verification

ローカル検証は変更範囲の検証である。merge後 main SHAの必須 CI結果は Coordinatorが確認する統合検証である。

| command | result | note |
| --- | --- | --- |
| setup: `mise exec -- cargo build --locked -p synvelis-plugin-test-l1-runner` | exit 0 | 既存 `pc_c4_l1_real_abi_binding`/`tim03` testsが要求する test-only runnerの事前 build。canonical test自体は下行。 |
| `mise run fmt-check` | exit 1 | `cargo fmt --all -- --check` が rustfmt起動前に Windows `os error 206`（file name or extension too long）。workspace-wide invocationが long worktree pathで process command limitに達する環境問題。 |
| `mise exec -- cargo fmt -p synvelis-app -- --check` | exit 0 | 補助証拠（canonical = mise）。変更 crateの formatは pass。 |
| `mise run check synvelis-app` | exit 0 | pass。 |
| `mise run lint synvelis-app` | exit 0 | pass。 |
| `mise run test synvelis-app` | exit 0 | pass。runner事前 build後。`sv_os_headless` fake lane 3/3 passを含む。 |
| `mise run test-app-gpui` | exit 0 | pass。`sv_os_headless` 4/4 pass（Fake matrix/classification + GpuiSimulated lane）を含む。 |
| `mise run check synvelis-app --features real-os-scenario` | exit 0 | pass。compileのみで real windowは開かない。 |
| `mise run lint synvelis-app --features real-os-scenario` | exit 0 | pass。compile/lintのみで real windowは開かない。 |
| `mise exec -- cargo test --locked -p synvelis-app --test sv_os_headless` | exit 0 | 補助証拠（canonical = mise）。3/3 pass。 |

初回 `mise run test synvelis-app` / `mise run test-app-gpui` は、scope外の既存 test-only L1 runner binaryが fresh targetに無く `pc_c4_l1_real_abi_binding` で失敗した。runnerを buildした後の同一 canonical commandは両方 exit 0である。

## Coordinator interactive gate（NotRun by Worker）

Workerは interactive desktopで実 windowを開く次の laneを実行していない。

```sh
SYNVELIS_SV114_REAL_EVIDENCE=evidence/sv114/sv-os-real-run.jsonl \
mise exec -- cargo test -p synvelis-app --locked --features real-os-scenario --test sv_os_real -- --nocapture
```

確認点は `docs/test-specs/sv114-svos-real-input-pixel.md` の Coordinator interactive gate節どおり、SV-OS-01 rowで次を同一 runに確認する。

1. `observed.real_input_pixel.ops.pointer-move/mouse-down/mouse-up` がすべて `tag=injected`。
2. `command.actions` が `project/save` 1件と一致し、receiptが harness-posted `Applied` として command idと対応する。
3. `settled_observation` と `product_state_oracle.save` が記録される。
4. `capture.outcome=captured` で backend/width/height/scale factor/pixel countがあり、`pixel_count > 0`。
5. `Unsupported`/`NotRun`/`EnvironmentBlocked` が Passへ昇格していない。
6. IME/DPI/monitor/native dock/production wiringが claimされていない。

## Evidence schema notes

- 既存 `evidence/sv114/sv-os-real-run.jsonl` は interactive実行なしに再生成していない。
- real laneが成功した場合、新規 rowsは既存 schema `sv114-svos-real/v1` に additiveな `observed.real_input_pixel` と `expected.real_input_pixel` を持つ。
- `input_method` は OS automation injectionであり human physical manual inputではない。
- harness-posted Applied receiptは production service receiptではない。`product_state_oracle` は独立 headless service oracleである。

## Scope-out zero-diff confirmation

- `docs/**/README.md`、`mise.toml`、`Cargo.toml`、`Cargo.lock`、production `crates/synvelis-app/src/**`、`crates/synvelis-app/tests/common/scenario.rs`、`crates/synvelis-app/tests/common/mod.rs`、ruviform/audio-lens repositoryは変更していない。
- `fmt-check` の workspace-wide `os error 206` は本 sliceの code diffに依存しない Windows path/process invocation制限として記録する。package-scoped rustfmtは pass。
