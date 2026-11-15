# RCD-01 Virtual L1 Oracle — Case Matrix (synvelis#115)

Issue `scarecrow173/synvelis#115`（RCD-01 実 Audio Device 録音統合）の
Required case を、検証 lane 別に分類する matrix。本 slice（Option A —
synthetic-L1 最終 Oracle の確立）が閉じるのは **synthetic-L1** と
**Evidence/Case Matrix** のみ。Issue は rf#629 仮想 endpoint EXT-L3・
physical・GUI（Option B）の gate が残るため **open 継続**。

> **2026-11-15 更新（rf#628-fixture consumer oracle slice）**:
> pinned ruviform が `ac10a2f7` → `abb9f372`（`ruviform-audio-test`
> `pipeline/` 収録済み最新 main）へ更新され、`rf#628-fixture` 列の
> deterministic fixture lane が `tests/rf628_oracle.rs` /
> `tests/rf628_oracle_rt_alloc.rs` で実測 Pass となった。各 cell の
> Pass は **deterministic-backend headless lane のみ**を意味し、
> real OS audio・GUI・physical・rf#629 の verdict を変更しない。
> 詳細は `issue-115-rf628-consumer-oracle-verification.md` と
> `rcd-01-virtual-l1-oracle-ledger.md` を参照。

## 分類 vocabulary

- **synthetic-L1**: fake port oracle（`MockPcm` 非依存の独立
  `oracle_sample` generator + scripted `OracleInputDevice`/
  `OracleInputStream`）による headless 実測 lane。本 slice の主題。
- **rf#628-fixture variant**: `scarecrow173/ruviform#628`
  （RF-VTEST-01 deterministic test runtime）の固定 PCM/Clock fixture を
  使う派生 case。pinned ruviform `abb9f372` に fixture 収録済み
  （`ruviform-audio-test`）。consumer oracle slice で実測。
- **rf#629 仮想 endpoint EXT-L3**: `scarecrow173/ruviform#629`
  （RF-VTEST-02 Windows 仮想 endpoint）— OS が列挙・Open できる仮想
  WASAPI endpoint → 本番 Callback を通す EXT-L3 case。
- **physical**: 物理 Audio Device（Audient iD4 等）の実機検証。
- **GUI（Option B）**: `synvelis-app` 層の Recorder command/UI wire
  （EXT-L3/GUI-L2）。

## Required case × lane matrix

| Required case（Issue 受入条件） | synthetic-L1（本 slice） | rf#628-fixture | rf#629 仮想 endpoint EXT-L3 | physical | GUI（Option B） |
| --- | --- | --- | --- | --- | --- |
| 独立 PCM oracle（`MockPcm` 非依存 generator で sample/byte 検算） | **Pass**（`oracle_sample` + emission-log + golden const） | **Pass**（`pcm_reference_block` — `PcmFixture::fill` 非依存の独立再実装 + `pcm_assert_eq`） | NotRun | NotRequired | NotRun |
| E2E byte/sample fidelity（mono/stereo/multichannel、commit→reload→digest resolve→bitwise 照合） | **Pass**（`oracle_stereo_identity`/`oracle_multichannel_identity`） | **Pass**（`rf628_stereo_scenario_commit_reload`/`rf628_multichannel_two_mono_taps_commit_reload`） | NotRun | NotRequired | NotRun |
| Multi-track (i): 1 stereo → 2 mono tap 別 stream・別 staging media 分離 | **Pass**（`oracle_stereo_device_two_mono_taps`） | **Pass**（`rf628_multichannel_two_mono_taps_commit_reload` — 4ch→mono(1)/mono(3) tap。literal な stereo→2mono 構成は synthetic-L1 lane のみで網羅） | NotRun | NotRequired | NotRun |
| Multi-track (ii): same-clock 2 device が別 stream に別 seed PCM | **Pass**（`oracle_two_devices_same_clock`） | **Pass**（`rf628_two_devices_same_clock_commit_reload` + 別 domain rejection） | NotRun | NotRequired | NotRun |
| `verified_sha256` 独立再計算照合（`sha2`、journaled verified prefix） | **Pass**（全 fidelity/fault case で pin） | **Pass**（全 commit/reload case で `sha2` 再計算照合） | NotRun | NotRequired | NotRun |
| Fault: input discontinuity → fail-stop → verified prefix oracle bit 一致 + commit 層 recoverable 分類（`Source` 非昇格） | **Pass**（`oracle_input_discontinuity`/`oracle_missing_finalized_mixed_session`） | **Pass**（scenario gap/`FramePosition`/`ClockDiscontinuity`/vanish の 4 系統で verified prefix + recoverable 分類。open fault は record 前の defined rejection で本 row の prefix 契約対象外） | NotRun | NotRequired | NotRun |
| Punch/Loop/Arm/Monitor・block 途中 frame 境界の一意性、Raw vs Software Monitor 非混入 | NotRun（本 slice 対象外 — 別 slice/Issue） | NotRun | NotRun | NotRequired | NotRun |
| Windows/macOS/Linux 既存 CI Native Build/Link・Recording/RT/Project/WAV・必要 real-device Feature compile/headless | NotRun（CI lane — merge 後対象 SHA の CI 証拠で確認） | NotRun | NotRun | NotRequired | NotRun |
| Windows GUI-L2/EXT-L3（仮想 WASAPI endpoint → 本番 Callback → Synvelis 独立 oracle 照合） | NotRun | NotRun | **NotRun(NOT_PROVISIONED)** | NotRequired | NotRun |
| macOS/Linux L1（Native Build/Link・Headless/RT/必要 Feature を既存 CI 同 SHA で検証） | NotRun（CI lane） | NotRun | NotRun | NotRequired | NotRun |
| OS/機能別 Evidence（実測/NotRun/Fail/NotRequired を混同しない） | **Pass**（本 ledger + case matrix で分類記録） | **Pass**（`issue-115-rf628-consumer-oracle-verification.md` で分類記録） | NotRun | NotRequired | NotRun |

## 補助 lane（本 slice で実行・記録）

| Lane | Status | Note |
| --- | --- | --- |
| `mise run test-recording-real-input` | Pass（構造検証） | opt-in HW lane を実行。Audient iD4（WASAPI）で 10/10 pass。real PCM 内容の deterministic oracle 検証は不能のため structural verification が Pass 権威（`real_deterministic_pcm_source_records_not_run` が NotRun を記録）。oracle-grade PCM fidelity は synthetic-L1 lane のみ |
| canonical blob publish 脚 | Deferred | `synvelis-player` 管轄。既存証拠 `crates/synvelis-player/tests/rcd02_blob_digest.rs`・`evidence/sv116/rcd-02-blob-sha256-ledger.md` を参照。本 slice で Pass 転用しない。digest adoption 契約の範囲で raw fold digest → staging media verified prefix bytes を解決する方式を採用 |

## 正直記録の原則

- synthetic-L1 と rf#628-fixture の Pass は **headless deterministic
  lane のみ**を意味し、real OS audio-device capture・real WASAPI
  callback・real GUI・other-OS filesystem 挙動・product acceptance
  を一切含意しない。
- rf#628-fixture variant は pinned ruviform `abb9f372` の
  `ruviform-audio-test` により consumer oracle slice で実測 Pass
  （deterministic-backend lane）。rf#629 仮想 endpoint EXT-L3 は
  hard dep だが本 slice 対象外（環境未構築 = NOT_PROVISIONED）。
  いずれの Pass も他 lane へ転用しない。
- physical は追加検証が通常必須外（NotRequired）。iD4 10/10 は既存
  履歴のまま。
- canonical blob publish 脚は `synvelis-player` 管轄の既存証拠を参照
  し、本 slice で Pass 転用しない。
- Issue #115 は上記 gate が残るため **open 継続**。本 slice が閉じるのは
  (a) 独立 PCM oracle と (b) Required case 分類の evidence/case matrix
  のみ。
