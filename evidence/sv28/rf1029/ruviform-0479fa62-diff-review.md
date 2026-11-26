# rf#1029 pin update diff review — ruviform 87a7ba8f → 0479fa62

## 基本情報

- 作業日: 2026-11-26（JST）
- 対象 Issue: `scarecrow173/synvelis#28`（常設 tracker。close しない）
- branch: `chore/28-ruviform-pin-rf1029`
- synvelis base SHA: `34e69fb27774f41f6aded1a24320be90ffd7477a`（dispatch 時点 `origin/main`）
- before ruviform rev: `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`
- candidate ruviform rev: `0479fa62e77e669a7bf84320d3d5418ffb949fc6`
- range: `87a7ba8f..0479fa62`、17 commits
- OS: Windows 11 x86_64

## dispatch 時再測定

補助証拠（canonical = mise ではない read-only 調査）として次を再測定した。

| 項目 | 結果 |
| --- | --- |
| `git ls-remote` ruviform main | `0479fa62e77e669a7bf84320d3d5418ffb949fc6`（起稿時点 snapshot と一致） |
| `git ls-remote` synvelis main | `34e69fb27774f41f6aded1a24320be90ffd7477a` |
| `git merge-base --is-ancestor 0479fa62 0479fa62` | exit 0（candidate は rf#1029 merge commit そのもの） |
| before lockfile ruviform sources | 36 件すべて `87a7ba8f…` |
| commit 件数 | 17 |
| upstream 全体 diff | 31 files, +4,320 / −544 |
| upstream `Cargo.toml` diff | なし |

candidate は rf#1029 merge commit `0479fa62e77e669a7bf84320d3d5418ffb949fc6` と
同値であり、ancestor 条件を満たす。descendant ではないため range 評価は
`87a7ba8f..0479fa62` で確定する。

## 17 commits の分類

| 分類 | 主な commit | 内容 |
| --- | --- | --- |
| rf#1029 stereo capture | `53ebccd3` 他、merge PR #1032 (`0479fa62`) | virtual capture endpoint の stereo format range/default/jack metadata、mono capture silent pass 防止、downmix/unsupported width 分類、provisioned Windows evidence |
| rf#1026 WASAPI input forward gap | `0fb97d1b` 系 merge | forward gap を `Xrun` + `ClockDiscontinuity` へ分類。backward jump/reset、flag-only は `ClockDiscontinuity` のみ |
| rf#1009 xrun injection/harness | driver copy/injection、probe workflow、`Invoke-XrunCases.ps1`、harness tests | upstream capability 追加。consumer 直接依存ではない |
| rf#1015 structured proof | `ruviform-io-structured` CSV decision core、`ruviform-ui/src/ipc.rs` `valid_op` 再構成、Kani proof | semantics preserving の内部再構成が主 |
| docs/evidence/meta | 各 evidence、docs、upstream `mise.toml` | consumer 非該当 |

## crate 別依存木交差

| upstream crate | synvelis lockfile | 評価 |
| --- | --- | --- |
| `ruviform-audio-device` | 存在（`synvelis-recording` は optional `real-device` feature 経由） | compile API 破壊なし。WASAPI input runtime contract 差分あり（後述） |
| `ruviform-ui` | 存在（`synvelis-app` / UI vendor 経由） | `src/ipc.rs` `valid_op` の内部再構成。公開 API 差分なし |
| `ruviform-io-structured` | **不存在** | 依存木外。consumer 影響なし |
| driver `rvf-virtual-audio` | Cargo dependency ではない | capability 差分として記録のみ |

公開 API 面では、range 内の `pub` 追加/除去は検出されなかった。残る consumer
影響は runtime behavior の rf#1026 分類であり、次節で評価する。

## rf#1029 stereo capture contract

upstream ADR/evidence の要点:

- 旧 virtual capture endpoint は driver format range/topology が mono で、stereo
  content が downmix されていた。修正後は capture endpoint が stereo を expose し、
  stereo-to-stereo loopback で左右独立内容を保持する。
- upstream provisioned evidence は run `45817964602` attempt 1 で render/capture
  とも stereo 48 kHz/2ch/F32、independent-tone oracle Pass を記録したと述べる。
  evidence 実走時の head SHA は `65572966ed0ca301ba3a76dbbe35977ea856f9c2` で、
  merge commit `0479fa62` 上の実走ではない。
- upstream ADR は Synvelis consumer oracle/harness/workflow を変更していないと
  明記する。synvelis#115 再評価は後続 gate であり本 slice の deliverable ではない。

consumer 側の扱い: 本 slice は contract を含む rev を lock に取り込むだけであり、
synvelis の stereo 要求 oracle を緩めない。upstream provisioned Pass を synvelis
consumer Pass へ転用しない。

## rf#1026 WASAPI input forward gap の consumer 影響

upstream 契約（`ruviform-audio-device` WASAPI input）:

- 同一 capture epoch 内の forward packet-position gap: `DeviceError::Xrun` に
  続いて `DeviceError::ClockDiscontinuity` を報告し、resynchronize する。
- backward jump、position reset、flag-only discontinuity: `ClockDiscontinuity`
  のみ（xrun なし）。
- zero PCM で位置が連続: error なし。

synvelis 側の既存写像（`crates/synvelis-recording/src/input.rs`）:

- `DeviceError::Xrun` → `FAULT_INPUT_DISCONTINUITY`（input discontinuity）
- `DeviceError::ClockDiscontinuity` → `FAULT_EPOCH_DISCONTINUITY`（epoch discontinuity）
- `FaultLatch` は bit ごとの冪等 latch で、fail-stop 後の後続 callback は無効化される。

評価: forward gap 1 回で両 bit が立つのは重複副作用ではなく、capture loss
（xrun）と position epoch 再同期（clock discontinuity）という別の事実をそれぞれ
正しく latch した結果である。fail-stop と verified prefix の契約は変わらない。
したがって `input.rs` / `tests/input_adapter.rs` の source 適応は**不要**と判断した。
stereo multi-track requirement、capture channel-width oracle、fault 分類の意味は
弱めていない。`tests/virtual_endpoint.rs`（lease 外）も変更しない。

## rf#1009 xrun injection/harness

upstream evidence（`evidence/rf1009/`）の要点:

- driver `Parameters` key の `RvfXrunScenario` / `RvfXrunStartPacket` /
  `RvfXrunPacketCount` を capture stream 作成時に読み、scenario 1（notification
  抑止による capture-overflow-drop）と scenario 2（content zero-fill）を注入する。
  対象 pin は `eMicInDevice` + `KSPIN_WAVEIN_HOST` + `SystemCapturePin` に限定される。
- harness は typed `DeviceError::Xrun` / `ClockDiscontinuity` / position gap /
  PCM zero-run を観測し、`rf1009-xrun-observation/v1` / `rf1009-xrun-case/v1`
  JSON を出す。upstream の deterministic oracle/schema test は harness-level で
  pass したが、provisioned runtime evidence は upstream でも NotRun のままである。
- `Invoke-XrunCases.ps1` と workflow stage は additive で、upstream でも driver
  build は WDK 未装備環境では NotRun と記録されている。

consumer 側の扱い: synvelis は workflow/tool/harness を追加しない。fault injection
matrix の採用可否は後続 slice の**未採用・候補**として記録する。

## rf#1015 structured proof / valid_op refactor

- `ruviform-io-structured` は synvelis lockfile に存在せず依存木外。CSV limit
  decision core の抽出と `BoundedRead` 内部簡素化は公開 behavior を変えないと
  upstream evidence が述べる。
- `ruviform-ui/src/ipc.rs` の `valid_op` は byte predicate へ再構成されたが、
  公開 signature と wire grammar の意図的変更はなく、Kani proof 追加が主である。
  synvelis-app の headless lint/test で consumer 非回帰を確認する。
- proof 自体は synvelis で採用・転用しない。

## lockfile 差分

scoped `cargo update -p ruviform-media-core --precise 0479fa62…`（mise 経由、
補助証拠。canonical 検証は mise lane）で更新した結果:

- `Cargo.lock` diff は ruviform source fragment の `-`/`+` 72 行（36 件 × 2）のみ。
- 全 36 source が `0479fa62e77e669a7bf84320d3d5418ffb949fc6` の単一 SHA。
- 無関係な crates.io 依存 73 件は不変。新規 package・依存リスト変更なし。
- `Cargo.toml` 差分なし。恒久 `rev` は書いていない。
- `mise run sync-ui-deps`: `synced ruviform ui/* @ 0479fa62e7`。
- `mise run check-ruviform-rev`: 36 lockfile sources、lockfile-primary、REV match。

## 採否判断

**採用**。理由:

1. candidate は rf#1029 stereo capture preserve contract を含む merge commit そのもの。
2. lock diff は scoped で、無関係 dependency drift がない。
3. consumer 影響は rf#1026 runtime 分類のみで、既存 fault 写像が契約と整合し、
   lease 内 source 適応は不要と判断できた。
4. 必須 canonical lane の結果は `verification.md` に記録する（本ファイル作成時点で
   実行中のものは最終結果で更新する）。

## rollback 記録

- 更新前 rev: `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`
- target rev: `0479fa62e77e669a7bf84320d3d5418ffb949fc6`
- 手順（未コミット時）: `mise exec -- cargo update -p ruviform-media-core --precise 87a7ba8ff3fbe6d4502bea27d152d7f74c105899` で lock を戻し、必要なら `git checkout -- Cargo.lock`。その後 `mise run sync-ui-deps` / `mise run check-ruviform-rev` で一致確認。
- 手順（push 後）: force push せず revert PR または新規 dependency rollback PR。
- 実行結果: **NotRun（採用のため不要）**。

## 実行しない gate（後続 slice）

- `mise run test-recording-real-input`、`mise run test-recording-virtual-endpoint`
- synvelis#115 canonical EXT-L3 再 run、provisioned endpoint workflow、manual workflow dispatch
- 実 GUI、実 OS Window、実 Plugin Editor/Native Dock、物理機材、市販 plugin
- ruviform #219/#294・AudioLens #127 への cross-consumer tuple 提出
- merge 後 main SHA の 3OS CI / Daily Full CI（Coordinator が統合検証）

これらは `NotRun` / `Deferred` であり、本 slice のローカル Pass と混同しない。
