# rf#1029 pin update verification — synvelis

## 基本情報

- 作業日: 2026-11-26（JST）
- 対象 Issue: `scarecrow173/synvelis#28`（常設 tracker。close しない）
- branch: `chore/28-ruviform-pin-rf1029`
- synvelis base SHA: `34e69fb27774f41f6aded1a24320be90ffd7477a`（検証時点でも `origin/main` と同一。最新 main の取り込みは不要と確認）
- before ruviform rev: `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`
- candidate ruviform rev: `0479fa62e77e669a7bf84320d3d5418ffb949fc6`（検証時点でも ruviform `origin/main` HEAD と同一）
- OS: Microsoft Windows 11 x86_64
- すべてローカル検証。merge 後 main SHA の CI による統合検証の代替ではない。

## canonical lane 結果

実行コマンドはすべて synvelis ルートから。各 lane の exit code と開始/終了時刻は
`verification/lane-results.tsv` を参照。compiler 出力の生ログは prek の
whitespace/EOL 正規化で改変されないよう commit 対象外とし、コマンド出力の要点と
exit code を本ファイルへ転記する方針は既存 `evidence/sv28/` の markdown 証拠と
同じである。

| lane | command | exit | verdict | 要点 |
| --- | --- | ---: | --- | --- |
| sync-ui-deps | `mise run sync-ui-deps` | 0 | Pass | `synced ruviform ui/* @ 0479fa62e7` |
| check-ruviform-rev | `mise run check-ruviform-rev` | 0 | Pass | 36 lockfile sources、lockfile-primary、REV match |
| check-full | `mise run check-full` | 0 | Pass | `cargo check --workspace --all-targets --locked` 完了 |
| lint synvelis-recording | `mise run lint synvelis-recording` | 0 | Pass | clippy `-D warnings` 完了 |
| test synvelis-recording | `mise run test synvelis-recording` | 0 | Pass | 全 test pass |
| lint-recording-real-input | `mise run lint-recording-real-input` | 0 | Pass | real-device feature 有効 compile/lint 非回帰（実機実行ではない） |
| lint synvelis-app | `mise run lint synvelis-app` | 0 | Pass | clippy `-D warnings` 完了 |
| test synvelis-app | `mise run test synvelis-app` | 0（再実行） | Pass | 初回 exit 101 は L1 runner artifact 未 build が原因。後述の切り分けを参照 |
| test-rt | `mise run test-rt` | 0 | Pass | RT no-alloc/no-block contract test pass |
| check-headless-deps | `mise run check-headless-deps` | 0 | Pass | headless dependency 契約非回帰 |
| check-ui | `mise run check-ui` | 0 | Pass | UI typecheck 非回帰 |
| test-ui | `mise run test-ui` | 0 | Pass | UI test pass |
| build-ui | `mise run build-ui` | 0 | Pass | frontend build 成功。dist は生成物で commit しない |
| test-tim03-l1（追加 lane） | `mise run test-tim03-l1` | 0 | Pass | L1 runner build + tim03 oracle 2 test pass |
| test-pc-c4-l1（追加 lane） | `mise run test-pc-c4-l1` | 0 | Pass | L1 runner build + pc_c4 real ABI binding 2 test pass |
| prek | `mise exec -- prek run --all-files` | 0 | Pass | trailing whitespace、EOF、YAML/TOML、mixed line ending、BOM、private key、actionlint の全 hook pass。auto-fix 差分なし。staging 後の対象ファイルを含む最終 tree で 2026-11-26 22:30 JST 前後に複数回実行し、いずれも exit 0 |

## `test synvelis-app` 初回失敗の切り分け

初回実行（22:13–22:15、exit 101）では `pc_c4_l1_vst3_real_abi_binding` と
`pc_c4_l1_clap_real_abi_binding` の 2 test のみが失敗した。panic message は
`L1 runner missing at target\debug\synvelis-plugin-test-*-l1-runner.exe — run mise run test-pc-c4-l1 (or build synvelis-plugin-test-l1-runner)` であり、
test harness が要求する別 crate の runner 実行ファイルが fresh worktree の
`target/debug` に存在しなかったことが直接原因である。

切り分けの根拠:

1. 失敗したのは runner artifact に依存する 2 test のみで、他の全 test file は pass。
2. upstream diff range に `ruviform-plugin` 系や plugin ABI 契約の差分はなく、
   pin 起因の API/behavior 変更と結びつかない。
3. test 自身の panic message が runner build を前提条件として明示している。
4. canonical lane `mise run test-tim03-l1` / `mise run test-pc-c4-l1`（runner を
   build する正規入口）を実行した後の `mise run test synvelis-app` 再実行は
   exit 0 で全 test pass。

したがってこの失敗は pin 起因の製品 regression ではなく、fresh worktree での
test 前提 artifact 不足と分類する。oracle を緩めず、前提を正規 lane で満たして
再検証した。両 L1 lane 自体も本 slice の追加 canonical lane として pass を記録する。

## 補助証拠（canonical = mise ではない）

| コマンド | 用途 | 結果 |
| --- | --- | --- |
| `git ls-remote`（ruviform / synvelis） | dispatch 時・検証時の HEAD 再測定 | ruviform `0479fa62…`、synvelis `34e69fb2…` で不変 |
| `git merge-base --is-ancestor` | rf#1029 ancestor 確認 | exit 0（candidate は merge commit と同値） |
| `git diff` / `git log`（ruviform range） | 17 commits の差分評価 | `ruviform-0479fa62-diff-review.md` |
| `mise exec -- cargo update -p ruviform-media-core --precise 0479fa62…` | scoped lock 更新 | 36 source fragment のみ差分。無関係依存 drift なし |

## 実行しない gate（NotRun / Deferred）

| gate | state | 理由 |
| --- | --- | --- |
| `mise run test-recording-real-input` | NotRun | 実 audio device 依存。本 slice の必須検証に含めない |
| `mise run test-recording-virtual-endpoint` | NotRun | provisioned endpoint 環境依存。環境不在時の exit 0/NotRun を Pass にしない |
| synvelis#115 canonical EXT-L3 再 run | NotRun / 後続 slice | rf#1029 contract を含む main 対象 SHA での provisioned consumer run は後続 slice |
| provisioned endpoint workflow / manual workflow dispatch | NotRun | 本 slice では実行しない |
| 実 GUI・実 OS Window・実 Plugin Editor/Native Dock・物理機材・市販 plugin | NotRun | 本 slice の必須検証に含めない |
| ruviform #219/#294・AudioLens #127 cross-consumer tuple | NotRun / 後続 slice | 本 slice では提出しない |
| merge 後 main SHA の 3OS CI / Daily Full CI | NotRun | Coordinator が統合検証として確認 |
| Windows GUI-L2/EXT-L3 owner 別収集 | Deferred | 製品 Owner slice の責務 |

upstream rf#1029 provisioned Pass（run `45817964602`）は upstream evidence であり、
synvelis consumer Pass へ転用しない。

## rollback

- 更新前 rev `87a7ba8f…` への未コミット rollback 手順は
  `ruviform-0479fa62-diff-review.md` に記録。
- rollback 実行: **NotRun（採用のため不要）**。
