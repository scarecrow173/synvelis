# SV-OS consumer catalog verification evidence

Date: 2026-11-26. Branch: `feat/114-svos-consumer-catalog` (partial slice; `Refs` synvelis#114).
このファイルは本 slice の lane 分離と検証結果を記録する。Issue #114 全体の acceptance ではない。

## Lane verdict summary

| lane | driver/evidence | verdict | 意味づけ |
| --- | --- | --- | --- |
| catalog declaration | `ScenarioEntry`/`ScenarioMatrix`/`DriverBinding` | Pass（projection test） | SV-OS-01〜05 の case id/scenario id/required/spec extras/run function が declaration から追跡できる。SV-OS-06〜09 executable case は追加しない。 |
| headless Fake | `Fake` | Pass（3 tests） | semantic OS-L1 evidence。real OS/pixel/IME/native dock evidence ではない。 |
| GPUI simulated | `GpuiSimulated` | Pass（4 tests; gpui-test feature） | production-like ViewHost composition の simulated semantic evidence。pixel capture は Unsupported のまま。 |
| real OS composition | `RealOs` | Pass 5/5（interactive Windows, semantic lane） | 実 GPUI window/focus/geometry 上の semantic Command→Receipt→view-state evidence。physical OS input/pixel/IME/native dock pass ではない。 |
| product service oracle | mounted `NativeShell`/`SynvelisApp` | Pass（各 case の `observed.service_oracle`） | headless OS-L1 の独立製品 oracle。real window 経由の service-backed L2 acceptance へ昇格させない。 |
| Native Plugin Dock | なし | NotApplicable/NotRun | SV-OS-09/PD-01〜07 は本 slice 外。 |

## Real lane invocation（補助証拠; canonical = mise）

```sh
SYNVELIS_SV114_REAL_EVIDENCE=evidence/sv114/sv-os-real-run.jsonl \
  mise exec -- cargo test -p synvelis-app --features real-os-scenario --test sv_os_real --locked -- --nocapture
```

- OS/arch: Windows/x86_64（interactive desktop session）
- 実行形態: cargo 直接実行の opt-in 補助証拠。canonical mise task ではない。
- result: exit 0（1 test passed）
- verdict: SV-OS-01〜05 = pass/pass/pass/pass/pass
- artifact: `evidence/sv114/sv-os-real-run.jsonl`
- evidence に含まれるもの: case/driver/verdict/required/capabilities/expected/observed、semantic receipt object、target count、composition/backend (`gpui/real-os`)、GPU=`not_observed`、window geometry/focus、service oracle。
- evidence に含めない/主張しないもの: physical OS pointer/keyboard input、pixel readback、IME composition、DPI/monitor 横断、native plugin dock、PD-01〜07。

## Canonical local verification

後続の canonical 結果表は本ファイル更新時のローカル検証である。ローカル検証は変更範囲の検証であり、merge 後 main SHA の必須 CI 結果は Coordinator が確認する統合検証である。

| command | result | note |
| --- | --- | --- |
| setup: `mise exec -- cargo build --locked -p synvelis-plugin-test-l1-runner` | exit 0 | `tim03_l1_process_oracle` が要求する runner の事前 build。canonical test 自体は次行。 |
| `mise run fmt-check` | exit 1 | `cargo fmt --all -- --check` が rustfmt 起動前に `os error 206`。差分 diff は出ない。 pristine `origin/main` archiveでも同じ失敗を再現。`mise exec -- cargo fmt -p synvelis-app -- --check` は exit 0。 |
| `mise run check synvelis-app` | exit 0 | pass |
| `mise run lint synvelis-app` | exit 0 | pass |
| `mise run test synvelis-app` | exit 0 | pass（runner 事前 build 後）。 |
| `mise run build synvelis-app` | exit 0 | pass |
| `mise run check-app-native` | exit 0 | pass |
| `mise run lint-app-native` | exit 0 | pass |
| `mise run check-app-gpui` | exit 0 | pass |
| `mise run lint-app-gpui` | exit 0 | pass |
| `mise run test-app-gpui` | exit 0 | pass（235秒）。 |
| `mise run check-headless-deps` | exit 0 | pass。 |
| `mise run check-ruviform-rev` | exit 1 | stale copied vendor REV `20a24a66ee90f4d462224391e78c10c93f4f31ba` != lock `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`。lease 上 `ui/vendor/ruviform/` は触れない。 |
| `mise exec -- prek run --all-files` | exit 0 | pass。 |

## Scope-out verification findings

- `fmt-check`: Windows 上の workspace-wide `cargo fmt --all -- --check` invocation が rustfmt 起動前に `os error 206` で失敗する。変更ファイルを含む package scoped fmt は pass。pristine main でも同じ失敗なので本 slice 起因ではない。
- `check-ruviform-rev`: worktree に copy 済みの `ui/vendor/ruviform/REV` が古い場合、main 取込後の lock rev と不一致になる。`ui/vendor/ruviform/` は本 slice lease 外なので再同期しない。fresh checkout に vendor tree がなければ checker は `no vendor tree` として扱う。

## Acceptance boundary

AC6 には本 slice の partial evidence が追加されたが、physical OS input/pixel/IME/DPI/monitor、native dock、PD-01〜07、merge後対象 SHA の CI 証拠は未完了である。したがって Issue #114 の AC6 受入や Issue close はしない。`evidence/sv114/acceptance-ledger.md` の 2026-11-26 addendum もこの分類を保持する。
