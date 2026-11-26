# Issue #115 RCD-01 stereo preserve 後 provisioned virtual endpoint consumer rerun evidence

対象 slice は、rf#1029 stereo preserve 修正を含む Ruviform pin を Synvelis main が解決するようになった後、default branch の manual-only consumer workflow を再実行し、artifact を検収して canonical evidence へ転記する rerun gate である。本 evidence は upstream Ruviform の stereo preserve Pass ではなく、Synvelis consumer の EXT-L3 観測記録である。

結論は **consumer Fail** である。terminal RunMetadata は `result=fail` であり、workflow verdict も `consumer-fail` だった。render/capture endpoint の candidate scan と negotiated format はいずれも stereo だったが、consumer round-trip は `output timestamp regressions: 9` で失敗した。provisioning、readiness、artifact upload、cleanup は成功したが、それらを Pass へ転用しない。今回の run でも EXT-L3 Pass evidence は得られなかった。

## Run identity と dispatch

| 項目 | 値 |
| --- | --- |
| 記録日時 | 2026-11-26 |
| 実施者 | Orca dispatched worker `orca_session_id:8240dbd1-1256-4988-bb71-bca98dba6be1` |
| 対象 Issue | `scarecrow173/synvelis#115` |
| 対象 SHA | `bff8114ed5056be82b6b3ce370c2141bf2e355d1`（dispatch 時点の main HEAD） |
| run id / attempt | `45834341661` / `1` |
| run URL | <https://github.com/scarecrow173/synvelis/actions/runs/45834341661> |
| workflow | `virtual-endpoint-consumer-acceptance` |
| event / ref | `workflow_dispatch` / `refs/heads/main` |
| run created / completed | `2026-11-26T13:58:29Z` / job completed `2026-11-26T14:06:09Z` |
| runner | GitHub-hosted `windows-2025`、runner OS `Windows`、arch `X64`、image `win25-vs2026`、image version `20261123.321.1` |
| runner OS detail | Microsoft Windows Server 2025 Datacenter `10.0.26100` |
| Ruviform lock rev | `0479fa62e77e669a7bf84320d3d5418ffb949fc6` |
| Ruviform checkout rev | `0479fa62e77e669a7bf84320d3d5418ffb949fc6` |
| dependency access | anonymous |

dispatch は次のコマンドで行った。

```sh
gh workflow run virtual-endpoint-consumer-acceptance.yml \
  --repo scarecrow173/synvelis \
  --ref main \
  -f expected_sha=bff8114ed5056be82b6b3ce370c2141bf2e355d1
```

run の `headSha`、workflow 内 `github_sha`、checkout 後 `target_sha`、`expected_sha` input、terminal RunMetadata `sha` はいずれも `bff8114ed5056be82b6b3ce370c2141bf2e355d1` だった。raw run log の fail-closed step には `target_sha=bff8114ed5056be82b6b3ce370c2141bf2e355d1 github_sha=bff8114ed5056be82b6b3ce370c2141bf2e355d1 expected_sha=bff8114ed5056be82b6b3ce370c2141bf2e355d1` が残っている。

watch は `gh run watch 45834341661 --repo scarecrow173/synvelis --exit-status` を foreground で行った。実行環境の出力上限で watch が途中で切れたため、run を cancel せず同じ run id に対して foreground watch を再開し、最終 exit status `1` を確認した。job conclusion は `failure` だった。これは acceptance verdict ではない。acceptance verdict は verdict step が terminal RunMetadata から評価した `consumer-fail` であり、両者を分けて記録する。

## 実行前提の fail-closed 確認

dispatch 前に最新 main、lock revision、rf#1032 ancestry、pin revision の stereo preserve 内容を確認した。確認は read-only の GitHub PR/API/raw contents と Synvelis worktree の `Cargo.lock` で行い、Ruviform リポジトリへは書き込んでいない。機械可読 record は [`run-45834341661/pin-stereo-capability-check.json`](run-45834341661/pin-stereo-capability-check.json) に保存した。

| 確認項目 | 結果 |
| --- | --- |
| Synvelis main HEAD | `bff8114ed5056be82b6b3ce370c2141bf2e355d1` |
| pin 更新 PR | synvelis#679、merged `2026-11-26T13:50:05Z`、merge commit `bff8114ed5056be82b6b3ce370c2141bf2e355d1` |
| `Cargo.lock` Ruviform source | 36 件すべて `0479fa62e77e669a7bf84320d3d5418ffb949fc6`、single revision、40 hex |
| rf#1032 merge commit | `0479fa62e77e669a7bf84320d3d5418ffb949fc6` |
| ancestor 条件 | lock revision が rf#1032 merge commit と同値なので充足 |
| `MICIN_DEVICE_MAX_CHANNELS` | `2` |
| capture format | mono 互換 8 件に加えて stereo format 8 件があり、`KSAUDIO_SPEAKER_STEREO` を使用。mono-only ではない |
| capture data range | channel count 1 と 2 の data range が存在する |
| capture jack channel mask | `KSAUDIO_SPEAKER_STEREO` |
| raw file sha256 | `micinwavtable.h` = `9bde2339594d064fbd8917efeb9064ee08f7cdb6ad084d1acb45fe6db2186711`、`micintoptable.h` = `c24fbc5b7ae1aeca2393c9d5ed3950b0b5573d79a97cad1e4576b66823e5a856` |

upstream Ruviform run `45817964602` attempt `1` の stereo preserve Pass は upstream evidence であり、Synvelis consumer Pass へ転用しない。本 gate はその修正を含む pin で consumer を観測するための run である。

## Artifact 検収

GitHub artifact は upload-artifact v4 の immutable artifact で、workflow 側の retention 設定は 90 日である。artifact API が返した expires_at は両方とも `2027-02-24T13:58:31Z` だった。採取日時は 2026-11-26 で、run URL と artifact 名を一次参照先とする。

| artifact | id | archive sha256 | entry 数 | 欠損 |
| --- | ---: | --- | ---: | --- |
| `sv115-virtual-endpoint-consumer-45834341661-1` | `15124103366` | `35d4fd711339238126bdddcb4d79b7d2d131c0823cb5425d4bf4107c0dcf13fb` | 28 | なし |
| `sv115-virtual-endpoint-cleanup-45834341661-1` | `15124403047` | `6ec991d34cf11e23bd16c7b6b44fb225223dac9d1e5704b52a9ad9476b9baf30` | 13 | なし |

consumer artifact には `harness-stderr.log`、`harness-exit-code.txt`、`harness-stdout.log`、`provision-exit-code.txt`、`provision-state.json`、`provisioning/provision.log`、`provisioning/artifact-manifest.json`、provisioning stage logs、`provisioning/vaudio-cert.cer`、`readiness-gate.json`、`run-manifest.json`、`run-metadata/run-metadata-sv115-rcd01-virtual-endpoint-render-capture-recorder.json`、`ruviform-checkout-revision.txt`、`ruviform-lock-revision.txt` が含まれていた。cleanup artifact には `cleanup-pending.txt`、`artifact-manifest.json`、`cleanup-exit-code.txt`、cleanup stage logs、`unprovision-state.json`、`unprovision.log` が含まれていた。

artifact archive を download し、zip entry の inventory・size・CRC32・抽出ファイルとの byte 一致を検査した。欠損 entry と hash mismatch はなかった。entry 名・size・CRC32 の全件 inventory は [`run-45834341661/artifact-inspection.json`](run-45834341661/artifact-inspection.json) に保存した。容量の大きい log、driver package、cert binary は repository へコミットせず、artifact 名・id・run URL・expires_at・archive sha256 で参照する。

raw run log は `gh run view 45834341661 --repo scarecrow173/synvelis --log` で取得した。取得物は 424,054 bytes、sha256 は `cd71698aecd9ba961bbf27e6831919b288d8b46584fde2eeb8cbef195ec22c35` だった。raw log 全体は repository へコミットせず、GitHub run と sha256 で参照する。

## Terminal RunMetadata と provenance

run-metadata directory には terminal RunMetadata JSON がちょうど 1 件あった。artifact 内 file sha256 は `cc95aa7f0793b2081f97a2c55ff688262d081111806d7b0042fa8d4a2679c3ce` で、`run-manifest.json` に記録された metadata sha256 と一致した。機械可読 copy は [`run-45834341661/terminal-run-metadata.json`](run-45834341661/terminal-run-metadata.json) に保存した。

| 検証項目 | 値 / 期待値 | 結果 |
| --- | --- | --- |
| `result` | `fail` | consumer Fail |
| `repo` | `scarecrow173/synvelis` | 一致 |
| `sha` | `bff8114ed5056be82b6b3ce370c2141bf2e355d1` | 一致 |
| `level` | `ext-l3` | 一致 |
| `environment_kind` | `virtual-os-endpoint` | 一致 |
| `oracle` | `marker-rms-journal-commit-reload-v1` | 一致 |
| GitHub run evidence | `github-run:scarecrow173/synvelis:45834341661:1` | 存在 |
| Ruviform lock evidence | `ruviform-lock-rev:0479fa62e77e669a7bf84320d3d5418ffb949fc6` | 存在 |
| Ruviform checkout evidence | `ruviform-checkout-rev:0479fa62e77e669a7bf84320d3d5418ffb949fc6` | 存在 |
| lock rev / checkout rev | 両方 `0479fa62e77e669a7bf84320d3d5418ffb949fc6` | 一致 |

terminal RunMetadata の `observed` は次だった。

> `CANDIDATE_FAILED — no Ruviform pair candidate completed the Recorder round-trip: pair[0]: output timestamp regressions: 9`

harness exit code file は `101` で、harness stdout でも 1 test failed が確認できた。terminal metadata は 1 件だけで parse に成功し、provenance mismatch はなかった。したがってこの run は証拠不備ではない。`result=fail` をそのまま consumer Fail として扱う。

workflow verdict step が raw run log に出力した machine-readable record は [`run-45834341661/consumer-verdict.json`](run-45834341661/consumer-verdict.json) に抽出して保存した。内容は `verdict=consumer-fail`、`reason=terminal RunMetadata result=fail`、`terminal_metadata_count=1`、`consumer_upload_outcome=success`、`cleanup_step_outcome=success`、`cleanup_upload_outcome=success` だった。この record は artifact 内 terminal RunMetadata の `result=fail` と一致した。

## Stereo capture の観測と consumer verdict の分離

harness stderr では、render/capture candidate scan と negotiated format がいずれも `SampleRate(48000)` / `Stereo` / `F32` だった。初回 run の `in=1 out=2` とは異なり、rf#1029 stereo preserve 修正が provisioned endpoint capability に反映されたことを観測できた。

ただし、この観測は consumer Pass を意味しない。harness は output probe の timestamp regression を検出し、signal audit、multi-track sample/RMS oracle、journal verified prefix、commit/save/reload digest chain の評価へ進まずに pair candidate を失敗とした。したがって、stereo endpoint format の観測は rf#1029 修正の反映確認にはなるが、sample/byte/multi-track/clock/journal/commit/reload oracle の Pass へ転用できない。

## Provisioning readiness と cleanup

readiness gate は `ready=true`、`reason=OK` だった。provision state は `operation=provision`、`status=ok`、`exit_code=0` で、endpoint candidate は true、pair count は 1 だった。機械可読 copy は [`run-45834341661/readiness-gate.json`](run-45834341661/readiness-gate.json) と [`run-45834341661/provision-state.json`](run-45834341661/provision-state.json) に保存した。

観測された endpoint pair は次だった。

| direction | name | id |
| --- | --- | --- |
| render | `Ruviform Cable Input (Ruviform Virtual Audio Device)` | `{0.0.0.00000000}.{14fbd633-7ace-47bc-bb64-156a0b560e11}` |
| capture | `Ruviform Cable Output (Ruviform Virtual Audio Device)` | `{0.0.1.00000000}.{3108892b-20dc-4559-bd53-1a28c5f4ee82}` |

driver package provenance は provision state に残っている。`inf_sha256=5da93e51d9c7d772286df3287efe2f8e79b0385cbe558382597a1699f85c31b3`、`cat_sha256=e33b8e315a36162100276cc6f997c33ea571111c47f8ceb0b3cf0bfd73ecba58`、`sys_sha256=58ccf544eefc34c86c9666eac3e5921ca00c573adff8815d3fa49db65b5dd2c3`、publisher は `RvfVirtualAudio Probe (rf977)`、cert thumbprint は `DFEA171F272C8E8E44788131337BD3F0EC31AD21`、cert notAfter は `2026-11-28T14:04:30Z` だった。

cleanup は consumer verdict と分離して評価する。cleanup artifact の `cleanup-exit-code.txt` は `0`、unprovision state は `operation=unprovision`、`status=ok`、`exit_code=0` で、residue-check は `leftovers=0` だった。機械可読 copy は [`run-45834341661/unprovision-state.json`](run-45834341661/unprovision-state.json) と [`run-45834341661/cleanup-artifact-manifest.json`](run-45834341661/cleanup-artifact-manifest.json) に保存した。cleanup 成功は運用 verdict であり、consumer Fail を変えない。

## Fail の観測と責務境界

terminal RunMetadata の直接観測は `output timestamp regressions: 9` である。consumer harness の output probe は render callback timestamp の後退を数え、1 件以上で pair candidate を失敗とする。今回は 9 件だった。

この artifact だけでは、原因が Ruviform virtual endpoint の timestamp 供給、Synvelis consumer の stream 利用、または harness contract のいずれにあるかを確定できない。既存 Issue には同じ症状を直接追跡するものが見つからなかったため、責務切り分け用に `scarecrow173/ruviform` Issue `1034`（<https://github.com/scarecrow173/ruviform/issues/1034>）を起票した。一方で、判定権威である consumer harness が candidate 存在下で terminal `result=fail` を返した以上、acceptance vocabulary では consumer Fail である。workflow、harness、consumer oracle、endpoint selection、capture 判定は本 slice で変更していない。

## 初回 run との差異

初回 run `45797309944` attempt `1` では、Ruviform lock rev `87a7ba8ff3fbe6d4502bea27d152d7f74c105899` の capture endpoint capability が mono で、`multi-track stereo split requires >=2 channels (in=1 out=2)` により consumer Fail だった。今回 run では pin revision が `0479fa62e77e669a7bf84320d3d5418ffb949fc6` へ進み、render/capture とも stereo format が観測された。

したがって、初回 Fail の直接原因だった capture channel-width 不足は今回の観測では再現しなかった。一方で、consumer round-trip は別の timestamp 条件で失敗した。初回 run の evidence は歴史的観測としてそのまま残し、今回の run を独立 evidence として記録する。

## 受入条件行ごとの転記

今回の run は Windows virtual endpoint EXT-L3 の stereo preserve 修正後 rerun だけを扱う。他の受入条件行へこの結果を転用しない。

| Issue 受入条件行 | 本 run における verdict | 根拠 / 後続 |
| --- | --- | --- |
| 仮想優先の置換 DoD。Ruviform#628固定PCM/ClockをSynvelis本番Recorderで記録し独立Byte/Sample Oracle、複数Track、Clock/Queue/Recovery/VerifiedPrefixを検証 | **Fail**（Windows virtual endpoint EXT-L3 部分） | provisioned endpoint candidate 上で consumer harness が terminal RunMetadata `result=fail` を返した。stereo format は観測できたが、output timestamp regression により複数 Track・journal・commit・reload oracle は未観測。Synthetic/L1 case の既存 evidence をこの run で上書きしない |
| Mock InputStream/Clock Epoch/PCM/Mono Stereo Multi-track/Raw Native FrameとRecording Journal Verified Prefixの独立Fixtureでsample/byte fidelityを検証 | **NotRun（本 run では対象外）** | mock fixture case は provisioned manual run の対象ではない。既存 headless/L1 evidence と後続 gate で扱う |
| Input drop/rate change/device loss/queue full/Cancel/RT ErrorをFake/Injected Faultで再現しRecovery/安全停止を確認 | **NotRun（本 run では対象外）** | fault injection 拡張は対象外。virtual endpoint run の Fail を fault recovery evidence へ転用しない |
| Punch/Loop/Arm/Monitorとblock途中frame境界の一意性、Raw vs Software Monitorの非混入をHeadless/RTで確認 | **NotRun（本 run では対象外）** | Punch/Monitor/Arm 全 case は後続 Issue/slice。今回の run は Monitor off の 2 mono track caseの signal audit 前に失敗した |
| Windows/macOS/Linux既存CI Native Build/Link・Recording/RT/Project/WAV・必要real-device FeatureのCompile/Headless Testを対象SHAで確認 | **NotRun（本 run では対象外）** | main 対象 SHA の必須 CI / Daily CI 証拠は Coordinator が確認する。本 evidence は manual EXT-L3 run だけを記録する |
| Windows GUI-L2/EXT-L3。WindowsのRecorderのEXT-L3は仮想WASAPI Endpoint→本番Callbackを通し、RF #628固定PCMと独立Oracleを照合する | **Fail** | terminal RunMetadata `result=fail`、workflow verdict `consumer-fail`。EXT-L3 Pass evidence 候補ではない。物理 Audient iD4 の扱いは変更しない |
| macOS/Linux L1必須。Native Build/Link、Headless/RT/必要Featureを既存CIの同SHAで検証 | **NotRun（本 run では対象外）** | macOS/Linux L1 は別 CI evidence。今回の Windows manual run から転用しない |
| OS/機能別Evidence。実測/NotRun/Fail/NotRequiredを混同しない | **Fail と NotRun を区別して記録** | Windows virtual endpoint EXT-L3 は Fail。GUI Option B、fault injection、macOS/Linux、physical acceptance は本 run では NotRun/後続。証拠不備ではない |

## 修正の有無

workflow、harness、consumer oracle、endpoint selection、capture 判定、provisioning readiness、verdict ロジックは変更していない。`output timestamp regressions: 9` は oracle の Fail であり、それを消すための workflow/harness 修正や再実行は行わなかった。

今回追加したのは evidence と、rerun 実施により古くなった status 記述の最小更新だけである。docs index は更新しない。

## 正規の検証コマンド

ローカル検証は Microsoft Windows 11 x86_64（`10.0.26300`）で実施した。canonical 入口は mise である。cargo を直接実行した結果は使っていない。

| command | exit code | verdict | 要点 |
| --- | ---: | --- | --- |
| `mise run sync-ui-deps` | 0 | Pass（環境同期） | ignored な生成物 `ui/vendor/ruviform` を lock rev `0479fa62e77e669a7bf84320d3d5418ffb949fc6` へ同期した。commit 対象外のローカル生成物の更新であり、Ruviform リポジトリや `Cargo.lock` は変更していない |
| `mise run lint-actions` | 0 | Pass | actionlint pass。workflow ファイルは変更していないが、manual dispatch した workflow の static sanity を確認した |
| `mise run fmt-check` | 1 | Fail（validation infrastructure） | `cargo fmt --all -- --check` の起動が Windows `os error 206` で失敗し、file 単位の formatting verdict へ進まなかった。`sync-ui-deps` 後も同じ結果だった。本 slice は Rust code を変更していない。formatting 違反の証拠ではないが、canonical check の Pass でもない |
| `mise run check-ruviform-rev` | 0 | Pass | lockfile は 36 source / single rev `0479fa62e77e669a7bf84320d3d5418ffb949fc6`、同期後の `ui/vendor/ruviform/REV` も同じ revision で一致した |
| `mise exec -- prek run --all-files` | 0 | Pass | trailing whitespace、EOF、YAML/TOML、mixed line ending、BOM、private key、actionlint の全 hook pass |

## 保存した machine-readable records

保存した JSON copy は repository の text 規約に合わせて newline が正規化されている場合がある。byte 単位の一次参照は artifact archive と本文中の archive / artifact-entry sha256 とし、copy は provenance と acceptance 判定を確認するための機械可読 record として扱う。

| file | 内容 |
| --- | --- |
| [`run-45834341661/terminal-run-metadata.json`](run-45834341661/terminal-run-metadata.json) | consumer harness の terminal RunMetadata の機械可読 copy |
| [`run-45834341661/readiness-gate.json`](run-45834341661/readiness-gate.json) | provisioning readiness gate の機械可読 copy |
| [`run-45834341661/run-manifest.json`](run-45834341661/run-manifest.json) | pre-cleanup consumer evidence manifest の機械可読 copy |
| [`run-45834341661/provision-state.json`](run-45834341661/provision-state.json) | Ruviform provision primitive state の機械可読 copy |
| [`run-45834341661/provisioning-artifact-manifest.json`](run-45834341661/provisioning-artifact-manifest.json) | provisioning evidence manifest の機械可読 copy |
| [`run-45834341661/unprovision-state.json`](run-45834341661/unprovision-state.json) | Ruviform unprovision primitive state の機械可読 copy |
| [`run-45834341661/cleanup-artifact-manifest.json`](run-45834341661/cleanup-artifact-manifest.json) | cleanup evidence manifest の機械可読 copy |
| [`run-45834341661/consumer-verdict.json`](run-45834341661/consumer-verdict.json) | verdict step が raw run log へ出力した JSON の抽出 |
| [`run-45834341661/pin-stereo-capability-check.json`](run-45834341661/pin-stereo-capability-check.json) | dispatch 前の pin revision / stereo preserve fail-closed 確認 record |
| [`run-45834341661/artifact-inspection.json`](run-45834341661/artifact-inspection.json) | artifact inventory、archive sha256、保存 record の検収用索引 |

## 実施しなかったもの

| 項目 | status | 理由 / 後続 |
| --- | --- | --- |
| ruviform pin 更新 | NotRun / 対象外 | synvelis#28 の別 slice で merge 済みのものを前提確認に使った。本 slice では更新していない |
| workflow/harness/consumer oracle 修正 | NotRun / 対象外 | lease 外。今回の Fail を消す修正や oracle 緩和は行わない |
| 同じ target SHA/rev/capability での retry | NotRun | 1 回の canonical run を検収した。consumer Fail を消す目的の再実行は禁止されている |
| GUI Option B | NotRun / 後続 | 本 gate の対象外 |
| fault injection 拡張 | NotRun / 後続 | 本 gate の対象外 |
| macOS/Linux endpoint acceptance | NotRun / 後続 | 現行方針では macOS/Linux GUI-L2/EXT-L3 は Deferred。OS-L1 CI evidence は別途確認する |
| physical device 検証 | NotRun / 後続 | 通常必須から外す方針を維持する |
| local provisioning | NotRun / 対象外 | provision/unprovision script は GitHub-hosted ephemeral `windows-2025` runner 専用。local machine では実行していない |
| sv#115 close 判定 | NotRun / 対象外 | 本 gate は evidence 転記まで。close は Coordinator とユーザーが判断する |

## 残件

- canonical EXT-L3 Pass evidence は未取得である。`output timestamp regressions: 9` の責務を、Ruviform virtual endpoint timestamp capability、Synvelis consumer stream 利用、consumer harness contract のいずれにあるか切り分ける必要がある。切り分け用の Ruviform Issue `1034` を起票済みである。
- stereo format は観測できたが、sample/byte/multi-track/clock/journal/commit/reload oracle は未観測である。timestamp 条件を解消した後、default branch の対象 SHA に対する canonical provisioned run を別 gate で再実施する。今回の Fail を再 run の結果で遡って上書きしない。
- sv#115 の残り受入条件行（mock fixture、fault injection、Punch/Monitor/Arm、macOS/Linux L1、OS/機能別 Evidence の総合）は後続 slice/gate で扱う。
- merge 後 main SHA に対する必須 CI / Daily CI の統合検証は Coordinator が確認する。
- docs index（`docs/**/README.md`）は Coordinator が統合時に整備する。

## 判定

run `45834341661` attempt `1` は、provisioning readiness、consumer evidence artifact、cleanup artifact、provenance の検収には成功したが、terminal RunMetadata `result=fail` の **consumer Fail** である。EXT-L3 Pass でも NotRun でも Unsupported でも証拠不備でもない。stereo format の観測は rf#1029 修正の反映を示すが、Synvelis consumer round-trip の Pass ではない。本 evidence はこの Fail を canonical な観測として保存し、sv#115 の Windows virtual endpoint EXT-L3 受入が未達であることを示す。
