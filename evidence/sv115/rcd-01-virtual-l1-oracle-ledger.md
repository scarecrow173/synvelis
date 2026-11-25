# RCD-01 Virtual L1 Oracle — Evidence Ledger (synvelis#115)

Headless fixture evidence for the `synvelis-recording` slice of
Issue #115 (RCD-01 実 Audio Device 録音統合 — closeout/oracle
slice, Option A — synthetic-L1 最終 Oracle の確立).
Every row records an independently observed Expected/Observed
pair from `crates/synvelis-recording/tests/rcd01_oracle.rs`.
`Pass` here means *the stated headless/integration-test lane
only* — it never implies real OS audio-device capture, real
WASAPI callback, real GUI, other-OS behavior, or product
acceptance. Those lanes are `NotRun`/`Deferred`/`NotRequired`
with reasons, per `POLICIES.md`.

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis branch | `scarecrow173/sv115-rcd01-oracle` |
| OS | Windows (win32) |
| Toolchain | rustc 1.99.1 (58cf8f9c3 2026-10-13) / cargo 1.99.1 (mise pin 系) |
| Pinned ruviform rev | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`check-ruviform-rev` で確認） |
| Fixture | scripted oracle `OracleInputDevice`/`OracleInputStream`（`MockPcm` 非依存の独立 `oracle_sample` generator、emission-log 記録、`jump_to` discontinuity 注入）→ `RealDeviceRig::open`(`&dyn Device`) → `InputSession`/`InputAdapter` → 実 `StreamWriter` → staging media + journal replay → `commit_session` → `DirectoryBundleStore` save → 別 `ProjectController::load` |
| Lane | headless integration test のみ（`cargo test -p synvelis-recording --test rcd01_oracle`、補助証拠; canonical = `mise run test synvelis-recording`） |

Status vocabulary: `Pass` = measured pass in the stated lane;
`Fail` = measured failure; `NotRun` = lane exists but was not
measured here; `Unsupported` = capability genuinely unavailable;
`Deferred` = intentionally out of scope for this slice;
`NotRequired` = lane intentionally not a gate for this slice.

## RCD01-OR-01 — 独立 oracle generator（`MockPcm` 非依存）

Fixture: `oracle_sample_golden_consts`・module doc・import/call
graph。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| generator 構造 | `MockPcm`（3 項 `wrapping_mul`/XOR + `>>48`→u16→`/65536`）と mixer・定数・bit window・f32 射影を共有しない独立 deterministic function | `oracle_sample` は splitmix64 連鎖相当 hash + 2 回 fmix64-style avalanche + `f64`→`[-1,1)` f32 射影で別構造 | Pass |
| golden const pin | `ORACLE_GOLDEN`（`(frame,channel,seed,f32 bits)`）で出力 bit pattern を pin | 全 const 一致 | Pass |
| 期待値構成の独立性 | oracle 経路は `MockPcm`・`verify_capture_pcm`・`verify_tap_log`・`verify_log`・`MockInput`/`block_pcm`・`synvelis_recording::mock::*` を期待値構成に呼ばない | emission-log 検証 + `sha2` 再計算のみ。input 生成も `oracle_sample` 由来で「入力生成」と「期待値検算」が同じ関数を経由しない | Pass |
| sentinel | 未供給 frame sentinel `u32::MAX` が有限 oracle 出力 `[-1,1)` と衝突しない | 衝突なし（NaN bit pattern） | Pass |

## RCD01-OR-02 — E2E byte/sample fidelity（stereo identity）

Fixture: `oracle_stereo_identity_commit_reload`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| stereo identity E2E | scripted oracle stereo device → `InputSession`/`InputAdapter` → 実 `StreamWriter` → staging media + journal replay → `commit_session` → save → 別 `ProjectController::load` で oracle sample と bit/sample 一致 | 一致 | Pass |
| digest 整合 | `Source.content_digest`/`Asset.digest` が finalized ref の raw fold digest（staging media verified prefix）と整合、reload 後も不変 | 一致 | Pass |
| byte resolve | journal `ChunkRecord.media_offset`/`spans[].media_offset` で index した staging media f32le bytes が emission log の `f32` bit pattern と bitwise 一致 | 一致 | Pass |
| `verified_sha256` | journaled verified prefix bytes の SHA-256 を test 内 `sha2` で独立再計算して `sha256:<64hex>` と照合 | 一致 | Pass |
| idempotency | reload 後 `CommitStatus::AlreadyApplied`・`applied_commits` が同一 | 一致 | Pass |

## RCD01-OR-03 — E2E byte/sample fidelity（multichannel identity）

Fixture: `oracle_multichannel_identity_commit_reload`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| multichannel identity E2E | multichannel oracle device → 同上 E2E chain で全 channel の oracle sample bit/sample 一致 | 一致 | Pass |
| `verified_sha256` | `sha2` 独立再計算照合 | 一致 | Pass |

## RCD01-OR-04 — multi-track (i): 1 stereo → 2 mono tap 分離

Fixture: `oracle_stereo_device_two_mono_taps_commit_reload`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| tap 分離 | 1 stereo oracle stream の 2 mono tap（別 `ChannelMap`）が別 stream・別 staging media に oracle 値で分離 | 一致 | Pass |
| per-tap fidelity | 各 tap の staging media が対応 channel の oracle sample と bitwise 一致、commit→reload まで維持 | 一致 | Pass |
| `verified_sha256` | `sha2` 独立再計算照合 | 一致 | Pass |

## RCD01-OR-05 — multi-track (ii): same-clock 2 device

Fixture: `oracle_two_devices_same_clock_commit_reload`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| same-clock 束ね | same-clock 2 oracle device が `InputSession::add` で束ねられ別 stream に記録 | 一致 | Pass |
| 別 seed 分離 | 各 device が別 seed の oracle PCM を持ち、各 stream が対応 seed の oracle sample と bitwise 一致 | 一致 | Pass |
| `verified_sha256` | `sha2` 独立再計算照合 | 一致 | Pass |

## RCD01-OR-06 — fault: input discontinuity fail-stop verified prefix

Fixture: `oracle_input_discontinuity_verified_prefix`・
`oracle_missing_finalized_mixed_session`。
precedent: `fake_input_drop_fails_stop_and_preserves_verified_prefix`・
`assert_fail_stop_prefix`（`tests/device_recording.rs`）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| discontinuity 検出 | oracle stream `jump_to` で position gap 注入 → `FAULT_INPUT_DISCONTINUITY` → fail-stop | 一致 | Pass |
| verified prefix fidelity | journal verified prefix bytes が oracle prefix と bit 一致（連続捏造・偽装なし） | 一致 | Pass |
| `verified_sha256` | journaled verified prefix bytes の SHA-256 を `sha2` で独立再計算して照合 | 一致 | Pass |
| writer outcome | durable frontier として `Complete`（partial media tail は journal 対象外） | `Complete`（precedent 準拠） | Pass |
| recoverable 分類 | session-scoped capture-fault marker により faulted stream が `Source` へ昇格せず honest に recoverable 分類、commit→reload 越しに維持 | 一致 | Pass |
| mixed session | 健全 stream 混在 session で健全側の commit→reload oracle fidelity も通る | 一致 | Pass |

## Lane summary

| Lane | Status | Note |
| --- | --- | --- |
| synthetic-L1（本 slice・fake port oracle） | Pass | 全 case・全 stream/track で Expected/Observed 記録済み |
| rf#628-fixture variant | Pass（2026-11-15 consumer oracle slice + 2026-11-16 residual gap closure） | `scarecrow173/ruviform#628` の deterministic runtime が pinned ruviform `abb9f372` の `ruviform-audio-test` として収録され、`tests/rf628_oracle.rs`/`rf628_oracle_rt_alloc.rs` で SV-1〜SV-8 を実測（SV-7 open-fault recovery E2E・SV-8 literal stereo→2mono は 2026-11-16 slice で追加）。case 行の詳細は `issue-115-rf628-consumer-oracle-verification.md` と `issue-115-rf628-record-after-recovery-verification.md` を参照（deterministic-backend headless lane のみ） |
| consumer virtual endpoint EXT-L3 | NotRun | harness 実装済みだが本 host に Ruviform endpoint pair 未 provisioning（`NOT_PROVISIONED`、2026-11-25 実測 exit 0）。upstream ruviform#1001 の base round-trip Pass とは分離して記録 |
| physical HW | NotRequired | 追加物理検証は通常必須外（POLICIES.md §2.1）。iD4 の real-input opt-in lane は上記のとおり構造検証として実測 Pass（oracle-grade PCM fidelity ではない） |
| `mise run test-recording-real-input` | Pass | opt-in HW lane を実行。WASAPI 上の Audient iD4（Loop-back 1/2・Analogue 1/2）を実測し 10/10 pass（構造・continuity 検証。real PCM の内容は deterministic oracle で検証不能のため structural verification が Pass 権威 — `real_deterministic_pcm_source_records_not_run` が正直に NotRun を記録）。診断的 `--nocapture` 再実行で `real_long_capture_monitors_frame_continuity` が device/driver continuity fault（bits 0x0020、verified prefix は journal/media intact と自己分類）を 1 度観測する flaky 挙動あり、直後の再実行は 10/10 pass — real-HW 特性であり本 slice の変更とは無関係 |
| canonical blob publish 脚 | Deferred | `synvelis-player` 管轄。既存証拠 `crates/synvelis-player/tests/rcd02_blob_digest.rs`・`evidence/sv116/rcd-02-blob-sha256-ledger.md` を参照。本 slice で Pass 転用しない |


## 2026-11-25 consumer virtual endpoint harness update

`tests/virtual_endpoint.rs` に Ruviform Cable Input/Output を対象とする
consumer virtual-WASAPI EXT-L3 harness が追加された。upstream ruviform#1001
/ PR #1008（merge `f332e462`）では SysVAD 派生 `Ruviform Cable Input/Output`
の install→実 WASAPI render→capture round-trip が GitHub-hosted Windows で
Pass 済みだが、本 slice を実行した開発 host には Ruviform endpoint pair が
provisioning されていない。`mise run test-recording-virtual-endpoint` は
`result=not-run` / `NOT_PROVISIONED` の RunMetadata JSON を出力して exit 0
を実測した。したがって consumer EXT-L3 round-trip は NotRun であり、Pass では
ない。詳細は `issue-115-virtual-endpoint-verification.md`。

## 未検証の項目（NotRun / Unsupported / Deferred）

- **rf#628 fixture variant**: `scarecrow173/ruviform#628`（RF-VTEST-01
  deterministic test runtime）の fixture は pinned ruviform `abb9f372`
  の `ruviform-audio-test` に収録済みで、consumer oracle slice
  （2026-11-15）により deterministic-backend headless lane で実測
  Pass。residual gap closure slice（2026-11-16）で SV-7
  open-fault recovery E2E・SV-8 literal stereo→2mono を追加し
  SV-1〜SV-8 を実測。case 行は
  `issue-115-rf628-consumer-oracle-verification.md` と
  `issue-115-rf628-record-after-recovery-verification.md` を参照。
  real OS audio・GUI・physical lane の verdict は変更しない。
- **consumer virtual endpoint EXT-L3**: harness は実装済みだが、本 host
  に Ruviform Cable Input/Output pair が provisioning されていないため
  NotRun(NOT_PROVISIONED)。upstream ruviform#1001 / PR #1008 の base
  round-trip Pass は upstream の証拠であり、Synvelis consumer EXT-L3 の
  Pass へ転用しない — Issue #115 open 継続の根拠。
- **physical HW**: 追加物理検証は通常必須外（NotRequired）。iD4 10/10
  は既存履歴のまま。
- **GUI（Option B）**: `synvelis-app` 層の Recorder command/UI wire の
  headless lane は `sv114-gui-option-b-recorder-wire` slice
  （synvelis#114）で実装・実測済み — `syn.recorder.*` lifecycle
  ops・fake `Device`/`InputStream` E2E・capabilities/close_veto/
  recording_info の実態一致を `evidence/sv114/
  recorder-wire-option-b-headless.md` に記録。実 device 列挙・
  実 GUI-L2・EXT-L3 は引き続き Deferred/NotRun であり、本 slice
  の headless Pass をそれらの verdict に転用しない。
- **canonical blob publish 脚**: `synvelis-player` 管轄の実機経路。
  本 slice は digest adoption 契約の範囲で raw fold digest → staging
  media verified prefix bytes を解決し、`verified_sha256` を `sha2`
  で独立再計算して pin。`publish_recorded` の実機経路は既存証拠を参照。
- **`mise run test-recording-real-input`**: opt-in HW lane を実行し
  Audient iD4（WASAPI）で 10/10 pass を実測。ただしこれは構造・
  continuity 検証の Pass であり、real PCM 内容の deterministic oracle
  検証は不能（`real_deterministic_pcm_source_records_not_run` が
  NotRun を正直に記録）。oracle-grade PCM fidelity は synthetic-L1
  lane のみを権威とする。

## audit「最終 Oracle を明確化」への回答

最終 oracle = test 内独立 generator（`oracle_sample`）による
sample/byte 照合（emission-log + staging media + journaled prefix +
`verified_sha256` `sha2` 再計算）であり、canonical publish 脚・
rf#628/rf#629・物理 lane は各々の状態を case matrix で明示する。
synthetic-L1 の Pass を real-device/GUI/OS lane の Pass に転用しない。
