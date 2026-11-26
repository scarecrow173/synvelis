# Issue #115 RCD-01 provisioned virtual endpoint consumer run evidence

対象 slice は、merge 済み manual-only consumer workflow の初回 provisioned manual run を観測し、artifact を検収して canonical evidence へ転記する follow-up gate である。本 evidence は workflow implementation validation ではなく、default branch の対象 SHA に対する provisioned Windows runner run の観測記録である。

結論は **consumer Fail** である。terminal RunMetadata は `result=fail` であり、workflow verdict も `consumer-fail` だった。provisioning、readiness、artifact upload、cleanup は成功したが、それらを Pass へ転用しない。初回 run で EXT-L3 Pass evidence は得られなかった。

## Run identity と dispatch

| 項目 | 値 |
| --- | --- |
| 記録日時 | 2026-11-26 |
| 実施者 | Orca dispatched worker `orca_session_id:4fed76de-89c3-4382-a0b7-b906a4243f95` |
| 対象 Issue | `scarecrow173/synvelis#115` |
| 対象 SHA | `a133050b682ea0a73f45359e122431ed59ede258`（dispatch 時点の main HEAD） |
| run id / attempt | `45797309944` / `1` |
| run URL | <https://github.com/scarecrow173/synvelis/actions/runs/45797309944> |
| workflow | `virtual-endpoint-consumer-acceptance` |
| event / ref | `workflow_dispatch` / `refs/heads/main` |
| run created / completed | `2026-11-26T10:00:55Z` / job completed `2026-11-26T10:07:51Z` |
| runner | GitHub-hosted `windows-2025`、runner OS `Windows`、arch `X64`、image `win25-vs2026`、image version `20261116.315.1` |
| Ruviform lock rev | `87a7ba8ff3fbe6d4502bea27d152d7f74c105899` |
| Ruviform checkout rev | `87a7ba8ff3fbe6d4502bea27d152d7f74c105899` |

dispatch は次のコマンドで行った。

```sh
gh workflow run virtual-endpoint-consumer-acceptance.yml \
  --repo scarecrow173/synvelis \
  --ref main \
  -f expected_sha=a133050b682ea0a73f45359e122431ed59ede258
```

run の `headSha`、workflow 内 `github_sha`、checkout 後 `target_sha`、`expected_sha` input、terminal RunMetadata `sha` はいずれも `a133050b682ea0a73f45359e122431ed59ede258` だった。watch は同一 run id に対する `gh run watch 45797309944 --repo scarecrow173/synvelis --exit-status` で行い、run を cancel せず最終 exit status `1` を確認した。端末ツールの出力上限で待機表示は途中で切れたが、同じ watch プロセスの出力から job 完了と exit status を確認した。

job conclusion は `failure` だった。これは acceptance verdict ではない。acceptance verdict は verdict step が terminal RunMetadata から評価した `consumer-fail` であり、両者を分けて記録する。

## Artifact 検収

GitHub artifact は upload-artifact v4 の immutable artifact で、workflow 側の retention 設定は 90 日である。artifact API が返した expires_at は両方とも `2027-02-24T10:00:56Z` だった。採取日時は 2026-11-26 で、run URL と artifact 名を一次参照先とする。

| artifact | id | archive sha256 | entry 数 | 欠損 |
| --- | ---: | --- | ---: | --- |
| `sv115-virtual-endpoint-consumer-45797309944-1` | `15105582472` | `633275365e78cef9a0ad2cdc81432d59b61548b19590bcad70334699c5c3be50` | 28 | なし |
| `sv115-virtual-endpoint-cleanup-45797309944-1` | `15106021714` | `2a5b53cd8b7ee3a7f7afca038719814eb9ecffc65548581fe7a4f11846c0323b` | 13 | なし |

consumer artifact の内容は `harness-stderr.log`、`harness-exit-code.txt`、`harness-stdout.log`、`provision-exit-code.txt`、`provision-state.json`、`provisioning/provision.log`、`provisioning/artifact-manifest.json`、provisioning stage logs、`provisioning/vaudio-cert.cer`、`readiness-gate.json`、`run-manifest.json`、`run-metadata/run-metadata-sv115-rcd01-virtual-endpoint-render-capture-recorder.json`、`ruviform-checkout-revision.txt`、`ruviform-lock-revision.txt` だった。

cleanup artifact の内容は `cleanup-pending.txt`、`artifact-manifest.json`、`cleanup-exit-code.txt`、cleanup stage logs、`unprovision-state.json`、`unprovision.log` だった。entry 名・size・CRC32 の全件 inventory は [`run-45797309944/artifact-inspection.json`](run-45797309944/artifact-inspection.json) に保存した。

artifact の実物を download して自分で parse し、verdict step の summary だけへ依存しない検収を行った。run log 全体は size が大きいため repository へコミットせず、GitHub run を一次参照先とする。取得した raw run log の sha256 は `121570510bf06b1cf06b8590548feb79f71d60be77256c256d5956fe0cc416f9` だった。

## Terminal RunMetadata と provenance

run-metadata directory には terminal RunMetadata JSON がちょうど 1 件あった。file sha256 は `ae00665f012408420a4a49afa6d4047f5a81cd2f4eab06524d428629b57a4e22` で、`run-manifest.json` に記録された metadata sha256 と一致した。機械可読 copy は [`run-45797309944/terminal-run-metadata.json`](run-45797309944/terminal-run-metadata.json) に保存した。

| 検証項目 | 値 / 期待値 | 結果 |
| --- | --- | --- |
| `result` | `fail` | consumer Fail |
| `repo` | `scarecrow173/synvelis` | 一致 |
| `sha` | `a133050b682ea0a73f45359e122431ed59ede258` | 一致 |
| `level` | `ext-l3` | 一致 |
| `environment_kind` | `virtual-os-endpoint` | 一致 |
| `oracle` | `marker-rms-journal-commit-reload-v1` | 一致 |
| GitHub run evidence | `github-run:scarecrow173/synvelis:45797309944:1` | 存在 |
| Ruviform lock evidence | `ruviform-lock-rev:87a7ba8ff3fbe6d4502bea27d152d7f74c105899` | 存在 |
| Ruviform checkout evidence | `ruviform-checkout-rev:87a7ba8ff3fbe6d4502bea27d152d7f74c105899` | 存在 |
| lock rev / checkout rev | 両方 `87a7ba8ff3fbe6d4502bea27d152d7f74c105899` | 一致 |

terminal RunMetadata の `observed` は次だった。

> `CANDIDATE_FAILED — no Ruviform pair candidate completed the Recorder round-trip: pair[0]: multi-track stereo split requires >=2 channels (in=1 out=2)`

harness exit code file は `101` で、harness stdout でも 1 test failed が確認できた。terminal metadata は 1 件だけで parse に成功し、provenance mismatch はなかった。したがってこの run は証拠不備ではない。`result=fail` をそのまま consumer Fail として扱う。

workflow verdict step が raw run log に出力した machine-readable record は [`run-45797309944/consumer-verdict.json`](run-45797309944/consumer-verdict.json) に抽出して保存した。内容は `verdict=consumer-fail`、`reason=terminal RunMetadata result=fail`、`terminal_metadata_count=1`、`consumer_upload_outcome=success`、`cleanup_step_outcome=success`、`cleanup_upload_outcome=success` だった。

## Provisioning readiness と cleanup

readiness gate は `ready=true`、`reason=OK` だった。provision state は `operation=provision`、`status=ok`、`exit_code=0` で、endpoint candidate は true、pair count は 1 だった。機械可読 copy は [`run-45797309944/readiness-gate.json`](run-45797309944/readiness-gate.json) と [`run-45797309944/provision-state.json`](run-45797309944/provision-state.json) に保存した。

観測された endpoint pair は次だった。

| direction | name | id |
| --- | --- | --- |
| render | `Ruviform Cable Input (Ruviform Virtual Audio Device)` | `{0.0.0.00000000}.{c7136f16-fb0d-4b35-a310-8ef09628c2cd}` |
| capture | `Ruviform Cable Output (Ruviform Virtual Audio Device)` | `{0.0.1.00000000}.{43080967-2455-40bd-99e8-83708ee86977}` |

driver package provenance は provision state に残っている。`inf_sha256=5da93e51d9c7d772286df3287efe2f8e79b0385cbe558382597a1699f85c31b3`、`cat_sha256=ab83383362ba7ac00d3327b09a93fb95de41cced0fcd7dc66004cbe8af378c4e`、`sys_sha256=66b6ea0c45e8f1011bb0a952412607ec7a85e69f2fb710ae34cad57f40a57270`、publisher は `RvfVirtualAudio Probe (rf977)`、cert thumbprint は `5132ED7D65D89227B4C5EC1678CE8235783DB9EF`、cert notAfter は `2026-11-28T10:06:21Z` だった。

cleanup は consumer verdict と分離して評価する。cleanup artifact の `cleanup-exit-code.txt` は `0`、unprovision state は `operation=unprovision`、`status=ok`、`exit_code=0` で、residue-check は `leftovers=0` だった。機械可読 copy は [`run-45797309944/unprovision-state.json`](run-45797309944/unprovision-state.json) と [`run-45797309944/cleanup-artifact-manifest.json`](run-45797309944/cleanup-artifact-manifest.json) に保存した。cleanup 成功は運用 verdict であり、consumer Fail を変えない。

## Fail の直接原因と責務境界

harness stderr では、render endpoint は `Stereo`、capture endpoint は `Mono` と negotiate されていた。harness は stereo input を 2 mono track へ分ける case を要求するため、input channel 1 / output channel 2 の組合せでは round-trip を開始せず、terminal RunMetadata `result=fail` を記録した。

read-only に確認した Ruviform pin revision `87a7ba8ff3fbe6d4502bea27d152d7f74c105899` では、`drivers/rvf-virtual-audio/micinwavtable.h` が `MICIN_DEVICE_MAX_CHANNELS 1` を定義し、`drivers/rvf-virtual-audio/minipairs.h` の capture miniport が同じ値を使う。一方、render side の speaker table は max channels 2 である。この source 上の channel capability は、run で観測された render stereo / capture mono と一致する。

したがって直接原因は、現行 Ruviform virtual capture endpoint が stereo capture を提供しないことにあると判断できる。これは Ruviform 側の driver capability に関わる問題であり、本 follow-up gate の lease では修正しない。`virtual_endpoint.rs` の oracle、endpoint selection、capture 判定、production Recorder 呼出し、既存 Pass/Fail/NotRun 条件も変更しない。同じ capability のまま再 run しても同じ Fail になる見込みなので、本 gate では validation run を行わない。

この原因分析は consumer verdict を NotRun へ変える根拠ではない。readiness は ok、endpoint pair candidate は存在し、candidate に対する consumer harness は terminal RunMetadata `result=fail` を返した。検証仕様の語彙では consumer Fail である。

## 受入条件行ごとの転記

今回の run は Windows virtual endpoint EXT-L3 の初回 provisioned 観測だけを扱う。他の受入条件行へこの結果を転用しない。

| Issue 受入条件行 | 本 run における verdict | 根拠 / 後続 |
| --- | --- | --- |
| 仮想優先の置換 DoD。Ruviform#628固定PCM/ClockをSynvelis本番Recorderで記録し独立Byte/Sample Oracle、複数Track、Clock/Queue/Recovery/VerifiedPrefixを検証 | **Fail**（Windows virtual endpoint EXT-L3 部分） | provisioned endpoint candidate 上で consumer harness が terminal RunMetadata `result=fail` を返した。stereo capture 不足で recording round-trip 自体へ進めず、複数 Track・journal・commit・reload oracle は未観測。Synthetic/L1 case の既存 evidence をこの run で上書きしない |
| Mock InputStream/Clock Epoch/PCM/Mono Stereo Multi-track/Raw Native FrameとRecording Journal Verified Prefixの独立Fixtureでsample/byte fidelityを検証 | **NotRun（本 run では対象外）** | mock fixture case は provisioned manual run の対象ではない。既存 headless/L1 evidence と後続 gate で扱う |
| Input drop/rate change/device loss/queue full/Cancel/RT ErrorをFake/Injected Faultで再現しRecovery/安全停止を確認 | **NotRun（本 run では対象外）** | fault injection 拡張は対象外。virtual endpoint run の Fail を fault recovery evidence へ転用しない |
| Punch/Loop/Arm/Monitorとblock途中frame境界の一意性、Raw vs Software Monitorの非混入をHeadless/RTで確認 | **NotRun（本 run では対象外）** | Punch/Monitor/Arm 全 case は後続 Issue/slice。今回の run は Monitor off の 2 mono track caseを開始前に失敗した |
| Windows/macOS/Linux既存CI Native Build/Link・Recording/RT/Project/WAV・必要real-device FeatureのCompile/Headless Testを対象SHAで確認 | **NotRun（本 run では対象外）** | main 対象 SHA の必須 CI / Daily CI 証拠は Coordinator が確認する。本 evidence は manual EXT-L3 run だけを記録する |
| Windows GUI-L2/EXT-L3。WindowsのRecorderのEXT-L3は仮想WASAPI Endpoint→本番Callbackを通し、RF #628固定PCMと独立Oracleを照合する | **Fail** | terminal RunMetadata `result=fail`、workflow verdict `consumer-fail`。EXT-L3 Pass evidence 候補ではない。物理 Audient iD4 の扱いは変更しない |
| macOS/Linux L1必須。Native Build/Link、Headless/RT/必要Featureを既存CIの同SHAで検証 | **NotRun（本 run では対象外）** | macOS/Linux L1 は別 CI evidence。今回の Windows manual run から転用しない |
| OS/機能別Evidence。実測/NotRun/Fail/NotRequiredを混同しない | **Fail と NotRun を区別して記録** | Windows virtual endpoint EXT-L3 は Fail。GUI Option B、fault injection、macOS/Linux、physical acceptance は本 run では NotRun/後続。証拠不備ではない |

## 修正の有無

workflow と harness の artifact/provenance 出力は、今回の consumer Fail の判定に必要な terminal RunMetadata、readiness、run manifest、artifact upload、cleanup record を残せた。fail の原因は metadata output directory や env provenance の不備ではない。したがって本 PR では `.github/workflows/virtual-endpoint-consumer-acceptance.yml` と `crates/synvelis-recording/tests/virtual_endpoint.rs` を変更しない。

今回追加したのは evidence と、run 実施により古くなった status 記述の最小更新だけである。docs index は更新しない。

## 保存した machine-readable records

保存した JSON copy は repository の text 規約に合わせて newline が正規化されている場合がある。byte 単位の一次参照は artifact archive と本文中の archive / artifact-entry sha256 とし、copy は provenance と acceptance 判定を確認するための機械可読 record として扱う。

| file | 内容 |
| --- | --- |
| [`run-45797309944/terminal-run-metadata.json`](run-45797309944/terminal-run-metadata.json) | consumer harness の terminal RunMetadata の機械可読 copy |
| [`run-45797309944/readiness-gate.json`](run-45797309944/readiness-gate.json) | provisioning readiness gate の機械可読 copy |
| [`run-45797309944/run-manifest.json`](run-45797309944/run-manifest.json) | pre-cleanup consumer evidence manifest の機械可読 copy |
| [`run-45797309944/provision-state.json`](run-45797309944/provision-state.json) | Ruviform provision primitive state の機械可読 copy |
| [`run-45797309944/provisioning-artifact-manifest.json`](run-45797309944/provisioning-artifact-manifest.json) | provisioning evidence manifest の機械可読 copy |
| [`run-45797309944/unprovision-state.json`](run-45797309944/unprovision-state.json) | Ruviform unprovision primitive state の機械可読 copy |
| [`run-45797309944/cleanup-artifact-manifest.json`](run-45797309944/cleanup-artifact-manifest.json) | cleanup evidence manifest の機械可読 copy |
| [`run-45797309944/consumer-verdict.json`](run-45797309944/consumer-verdict.json) | verdict step が raw run log へ出力した JSON の抽出 |
| [`run-45797309944/artifact-inspection.json`](run-45797309944/artifact-inspection.json) | artifact inventory、archive sha256、保存 record の検収用索引 |

容量の大きい log、driver package、cert binary は repository へコミットせず、artifact 名・run URL・retention・archive sha256 で参照する。`vaudio-cert.cer` は public cert であっても evidence 転記の必須対象ではないためコミットしなかった。

## 実施しなかったもの

| 項目 | status | 理由 / 後続 |
| --- | --- | --- |
| workflow/harness 修正後 validation run | NotRun | lease 内で直せる metadata/provenance 不備ではなく、同じ channel capability では再 run も同じ Fail になる見込みのため |
| GUI Option B | NotRun / 後続 | 本 gate の対象外 |
| fault injection 拡張 | NotRun / 後続 | 本 gate の対象外 |
| macOS/Linux endpoint acceptance | NotRun / 後続 | 現行方針では macOS/Linux GUI-L2/EXT-L3 は Deferred。OS-L1 CI evidence は別途確認する |
| physical device 検証 | NotRun / 後続 | 通常必須から外す方針を維持する |
| local provisioning | NotRun / 対象外 | provision/unprovision script は GitHub-hosted ephemeral `windows-2025` runner 専用。local machine では実行していない |
| sv#115 close 判定 | NotRun / 対象外 | 本 gate は evidence 転記まで。close は Coordinator とユーザーが判断する |

## 残件

- canonical EXT-L3 Pass evidence は未取得である。Ruviform virtual capture endpoint が stereo capture を提供できるようにする契約・実装の整理が必要である。これは Ruviform 側の責務であり、本 PR では修正しない。
- consumer harness が stereo input に進めるようになった後、default branch の対象 SHA に対する canonical provisioned run を再実施し、別 evidence として記録する。今回の Fail を再 run の結果で遡って上書きしない。
- sv#115 の残り受入条件行（mock fixture、fault injection、Punch/Monitor/Arm、macOS/Linux L1、OS/機能別 Evidence の総合）は後続 slice/gate で扱う。
- merge 後 main SHA に対する必須 CI / Daily CI の統合検証は Coordinator が確認する。
- docs index（`docs/**/README.md`）は Coordinator が統合時に整備する。

## 判定

run `45797309944` attempt `1` は、provisioning readiness、consumer evidence artifact、cleanup artifact、provenance の検収には成功したが、terminal RunMetadata `result=fail` の **consumer Fail** である。EXT-L3 Pass でも NotRun でも証拠不備でもない。本 evidence はこの Fail を canonical な観測として保存し、sv#115 の Windows virtual endpoint EXT-L3 受入が未達であることを示す。
