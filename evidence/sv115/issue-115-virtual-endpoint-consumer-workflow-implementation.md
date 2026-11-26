# Issue #115 RCD-01 virtual endpoint consumer workflow implementation evidence

対象 slice は Slice B consumer workflow implementation foundation である。
この evidence は workflow implementation の static/local validation を記録する。
provisioned Windows runner での EXT-L3 実走、artifact 検収、canonical run evidence
転記は merge 後 follow-up gate であり、ここでは実施していない。したがって本書は
EXT-L3 Pass evidence ではない。

## 実行環境と対象

| 項目 | 値 |
| --- | --- |
| 記録日時 | 2026-11-26（UTC 相当の作業記録） |
| 実施者 | Orca dispatched worker `orca_session_id:5c5f61f8-2ada-4f76-99eb-bd5a040bf99c` |
| 対象 Issue | `scarecrow173/synvelis#115` |
| 対象 slice | Slice B consumer workflow implementation foundation |
| 基準 SHA | `a24ff229d5084a6a44890ff3a2a06dc940cab0c9`（`origin/main`、本 diff の base。validation は commit 前 working tree で実施） |
| OS / arch | Windows / x86_64（local development host） |
| Ruviform lock rev | `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`（Synvelis `Cargo.lock` の single resolved revision） |
| Ruviform origin/main 参照時点 | `71f8bd4da047699eeb3ead781ff19e8a21320816`。workflow は main 追従ではなく lock rev を使う |

## 実装内容の確認

- `.github/workflows/virtual-endpoint-consumer-acceptance.yml` は `workflow_dispatch` 専用であり、`push`、`pull_request`、`schedule`、`workflow_call` に接続していない。
- workflow permissions は `contents: read`、runner は `windows-2025` 単一 job、timeout は 120 分である。
- step order は runner-local path 設定 → Synvelis checkout → `github.sha` / checkout HEAD / `expected_sha` の fail-closed 照合 → evidence directory 準備 → mise/toolchain/dependency access → Cargo.lock からの single Ruviform revision 抽出 → pin rev での Ruviform source checkout と actual HEAD 照合 → provision → readiness gate → consumer harness → pre-cleanup consumer evidence manifest/artifact upload → `if: always()` cleanup → post-cleanup cleanup artifact upload → terminal RunMetadata verdict 評価である。
- Ruviform revision 抽出は `Cargo.lock` 内の `scarecrow173/ruviform.git` source が 1 件以上存在し、全件が末尾 40 hex rev 形式で、distinct revision がちょうど 1 件の場合だけ成功する。零件、複数 rev、malformed source は workflow を fail-closed で停止する。
- Ruviform source checkout は `$RUNNER_TEMP/ruviform-source` で行い、Synvelis worktree を汚さない。checkout 後の actual HEAD が lock rev と一致しない場合、または checkout が dirty な場合は失敗とする。補助確認として、anonymous HTTPS で `git fetch --depth 1 origin +87a7ba8ff3fbe6d4502bea27d152d7f74c105899:refs/remotes/origin/87a7ba8ff3fbe6d4502bea27d152d7f74c105899` 後に同 SHA で detached checkout できることを local host で確認した（補助証拠。canonical lane ではない）。
- provision readiness は Ruviform state JSON `rvf-virtual-audio-provisioning/v1` を読み、`operation=provision`、`status=ok`、endpoint candidate あり、`pair_count>=1`、source rev が lock rev / actual checkout rev と一致、source dirty でない場合だけ harness を実行する。state 欠損・schema 不正・非 ok・candidate 不在では harness を実行しない。
- consumer harness には `SYNVELIS_RUN_REPO`、`SYNVELIS_RUN_SHA`、`SYNVELIS_RUN_METADATA_DIR`、`SYNVELIS_RUVIFORM_LOCK_REV`、`SYNVELIS_RUVIFORM_CHECKOUT_REV` を注入する。
- consumer evidence artifact `sv115-virtual-endpoint-consumer-<run_id>-<attempt>` は cleanup より前に `if: always()` で upload する。cleanup artifact `sv115-virtual-endpoint-cleanup-<run_id>-<attempt>` は別名で、cleanup 後に `if: always()` で upload する。upload-artifact v4 の immutable artifact 制約に従い、両者を同一 artifact に混ぜない。
- verdict 評価は terminal RunMetadata のみを権威とし、repo、SHA、level、environment、oracle、GitHub run evidence、Ruviform lock/checkout revision evidence の一致を確認する。job conclusion、artifact 存在、provision script exit、cleanup 成功、upstream Ruviform result を Pass に転用しない。GitHub Actions に neutral conclusion がないことを summary に明示する。
- `crates/synvelis-recording/tests/virtual_endpoint.rs` の変更は metadata output directory と env provenance の追加に限定した。oracle、endpoint selection、stream/capture/commit/reload 判定、production Recorder 呼出し、Pass/Fail/NotRun 条件は変更していない。`SYNVELIS_RUN_METADATA_DIR` 未設定時は従来どおり stderr `run_metadata:` を出し、`CARGO_TARGET_TMPDIR` fallback を維持する。

## Canonical local validation

`2026-11-26` に local Windows host で実施した。`mise run test-recording-virtual-endpoint` の lane exit 0 は test runner の終了コードであり、EXT-L3 acceptance verdict ではない。

| コマンド | exit code | verdict | 要点 |
| --- | ---: | --- | --- |
| `mise run lint-actions` | 0 | Pass（static workflow lint） | actionlint が新規 workflow を含めて成功 |
| `mise run fmt-check` | 0 | Pass | 初回は本 diff の Rust 整形不足で exit 1。対象 file を rustfmt 形式へ直した後の再実行で exit 0 |
| `mise run check synvelis-recording` | 0 | Pass | 対象 crate compile check 成功 |
| `mise run lint synvelis-recording` | 0 | Pass | clippy 成功 |
| `mise run test synvelis-recording` | 0 | Pass | 非 real-device feature の canonical test lane 成功。virtual endpoint harness の unit test も成功 |
| `mise run lint-recording-real-input` | 0 | Pass | `--features real-device` compile/lint lane 成功。mbx cache mapping warning は観測されたが cache 動作の警告であり clippy failure ではない |
| `mise run test-recording-virtual-endpoint` | 0 | NotRun（EXT-L3 未実走） | terminal RunMetadata `result=not-run`、observed `ENUMERATION_EMPTY no input devices on this host`。local host に endpoint pair がなく、`NOT_PROVISIONED` 系の acceptance 未達。Pass ではない |
| `mise run check-ruviform-rev` | 1 | Fail（本 slice スコープ外の既存/stale vendor 状態） | `ui vendor REV 20a24a66ee90f4d462224391e78c10c93f4f31ba != lockfile resolved 87a7ba8ff3fbe6d4502bea27d152d7f74c105899`。worktree copy 済み `ui/vendor/ruviform` が stale で、本 slice の lease 外生成物のため同期していない。`Cargo.lock` の Ruviform rev 自体は workflow が期待する `87a7ba8f...` |
| `mise exec -- prek run --all-files` | 0 | Pass | trim whitespace、end-of-file、YAML/TOML、private key、actionlint hook が成功。lease 外 file の変更は発生しなかった |

補助証拠として、metadata 拡張の focused test `mise run test synvelis-recording --test virtual_endpoint metadata_` は exit 0 で、`metadata_output_dir_prefers_run_metadata_dir` と `run_metadata_file_writes_one_machine_readable_json` を含む 3 件が成功した。canonical test lane でも同じ対象を含む。

## Workflow 逻辑功能性验证（mock fixture 驱动）

workflow 内嵌的 readiness gate 与 verdict PowerShell 逻辑块、lock rev 提取 bash
逻辑块被逐块提取后在本机以 mock fixture 实际执行验证（非静态 review）：

| 测试 | fixture | 结果 |
| --- | --- | --- |
| T1 readiness·合法 ok state | schema/operation/status/endpoint candidate/rev 全合规 | `ready=true` 输出正确 |
| T2 readiness·status=error | `status=error detail=WDK_INSTALL_FAILED` | `ready=false`、`reason=PROVISION_STATUS_error_WDK_INSTALL_FAILED` |
| T3 readiness·state 缺失 | state JSON 不存在 | `ready=false`、`reason=PROVISION_STATE_MISSING` |
| T4 verdict·合规 pass metadata | result=pass + 全 provenance/evidence 一致 | verdict=`ext-l3-pass-candidate`、exit 0 |
| T5 verdict·not-run metadata | result=not-run | verdict=`acceptance-not-run`、exit 1 |
| T6 verdict·SHA mismatch | pass metadata 但 sha 与 target 不符 | verdict=`evidence-insufficient`、reason 含 PROVENANCE_MISMATCH、exit 1 |
| T7 verdict·readiness 未过 | ready=false | verdict=`acceptance-not-run`、exit 1 |
| T8 verdict·pass 但 artifact upload 失败 | upload outcome=failure | 拒绝 pass-candidate、exit 1 |
| 提取·真实 Cargo.lock | 本 repo 实际 lockfile（36 个 ruviform source） | 正确抽出 single rev `87a7ba8f...` |
| 提取·双 distinct rev | 两个不同 rev | fail-closed exit 1 |
| 提取·零 source / 短 rev / 无 rev | 各 malformed 形态 | 全部 fail-closed exit 1 |

全部 7 个 pwsh 逻辑块通过 PowerShell  parser 语法检查，全部 5 个 bash 逻辑块通过
`bash -n` 语法检查。workflow YAML 行尾已确认为 LF（`.gitattributes` 要求）。

## Harness metadata output / provenance

`SYNVELIS_RUN_METADATA_DIR` が非空白で指定された場合、terminal RunMetadata JSON を
`run-metadata-sv115-rcd01-virtual-endpoint-render-capture-recorder.json` という 1 file として書き出す。
env provenance は次を evidence へ追加する。

- `github-run:<repo>:<run_id>:<attempt>` — GitHub Actions 実行との照合用
- `ruviform-lock-rev:<40hex>` — Synvelis `Cargo.lock` 由来 rev との照合用
- `ruviform-checkout-rev:<40hex>` — actual Ruviform source checkout rev との照合用

local host ではこれらの GitHub/runner env を設定していないため、local RunMetadata の
`evidence` は空だった。これは workflow 上でのみ provenance が付くことを意味し、
local NotRun を Pass へ転用する根拠にはならない。

## 未実施 / 後続 gate

| 項目 | status | 理由 / 後続 |
| --- | --- | --- |
| provisioned manual run | NotRun | GitHub 公式仕様上、新規 `workflow_dispatch` workflow は default branch に入るまで manual dispatch できない。merge 後に実行する |
| consumer evidence artifact 検収 | NotRun | provisioned manual run 未実施のため artifact が存在しない |
| cleanup artifact 検収 | NotRun | provisioned manual run 未実施のため artifact が存在しない |
| canonical run evidence 転記 | NotRun | artifact 検収後に通常 PR で `evidence/sv115/` へ転記する |
| local/self-hosted provisioning | NotRun / 対象外 | provision/unprovision script は GitHub-hosted ephemeral `windows-2025` runner 専用。local host では実行していない |
| GUI Option B、fault injection 拡張、macOS/Linux endpoint 化、physical device 検証 | NotRun / 対象外 | 本 slice の scope 外 |

## 判定

本 slice の implementation validation は、workflow static validation と local harness
regression で `Pass` 相当の実装証拠を得た。ただし virtual endpoint EXT-L3 acceptance
verdict は terminal RunMetadata `result=not-run` の local 観測にとどまり、provisioned
manual run は未実施である。したがって sv#115 の Windows EXT-L3 受入は依然 `NotRun`
であり、本 evidence を Pass として扱ってはならない。
