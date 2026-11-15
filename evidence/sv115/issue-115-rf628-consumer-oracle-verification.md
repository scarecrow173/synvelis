# Evidence — synvelis#115 rf#628 consumer oracle: deterministic fixture 駆動の consumer-side 検証

Verification record for the `rf#628-fixture` consumer lane of Issue
#115 (RCD-01 実 Audio Device 録音統合). This slice bumps the pinned
ruviform revision to `abb9f372` (first main carrying
`crates/ruviform-audio-test` with the `pipeline/` deterministic
runtime), adds `ruviform-audio-test` as a `synvelis-recording`
dev-dependency, and drives the **production** recording chain —
`InputSession`/`InputAdapter` → `RealDeviceRig` → `StreamWriter` →
staging media + journal → `commit_session` → `DirectoryBundleStore`
save → separate `ProjectController::load` — with the scripted
deterministic backend instead of the test-local fake device used by
the prior synthetic-L1 slice (`rcd01_oracle.rs`).

`Pass` below means *the stated headless/deterministic-fixture lane
only* — it never implies real OS audio-device capture, real WASAPI
callback, the rf#629 virtual endpoint, real GUI, physical hardware,
other-OS behavior, or product acceptance. Those lanes stay
`NotRun`/`NotRequired`/`Deferred` with named owners, per
`POLICIES.md`.

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis branch | `scarecrow173/sv115-rf628-oracle` |
| Base commit | `ae0dd2b6f6b7fee96e0b8e36902f6cd03fced801` (= `origin/main` at dispatch) |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.1 (58cf8f9c3 2026-10-13) / cargo 1.99.1 via mise |
| Pinned ruviform rev | `abb9f372d3ee95ee824b3c4f7dc68e8083ce2774` — `check-ruviform-rev` reports 28 lockfile sources at this rev, `ui/vendor/ruviform` REV match (`sync-ui-deps` applied) |
| Fixture | `ruviform_audio_test`: `ScriptedBackend`/`ScriptedDeviceHandle`/`DeviceSpec`/`InputSpec`/`StreamScript`/`StreamFault`/`PumpReport`, `PcmFixture`/`PcmPattern`/`PcmScenario`/`PcmSegment`, `VirtualClock`/`VirtualClockDriver`, independent `oracle::pcm_reference_block` + `pcm_assert_eq` (`Exact` / `AbsEpsilon(1e-5)` for transcendental patterns) |
| Production path under test | `RealDeviceRig::open(&dyn Device)` → `InputSession`/`InputAdapter` (`pump_input_next` = backend pump + scenario fill + data callback) → `StreamWriter` → staging media/journal → finalize → `commit_session` → save → reload |
| Lane | headless integration test のみ（`cargo test -p synvelis-recording --test rf628_oracle --test rf628_oracle_rt_alloc`、補助証拠; canonical = `mise run test synvelis-recording` / `mise run test-rt`） |

Status vocabulary: `Pass` = measured pass in the stated lane;
`Fail` = measured failure; `NotRun` = lane exists but was not
measured here; `Unsupported` = capability genuinely unavailable;
`Deferred` = intentionally out of scope for this slice;
`NotRequired` = lane intentionally not a gate for this slice.

## Canonical lane (mise tasks)

All lanes run via `mise run <task>` in this worktree on
2026-11-15 (JST). Only lanes actually executed are listed as
`Pass`; unexecuted lanes are `NotRun`.

| Command | Expected | Observed | Status |
| --- | --- | --- | --- |
| `mise run fmt-check` | exit 0 | exit 0 | Pass |
| `mise run check synvelis-recording` | exit 0 | exit 0 | Pass |
| `mise run lint synvelis-recording` | exit 0 | exit 0 (7 clippy findings in new test file fixed: similar names, doc backticks, checked `u16` cast, `manual_let_else` ×2; re-run clean) | Pass |
| `mise run test synvelis-recording` | exit 0 | exit 0 — all test binaries ok incl. `rf628_oracle` 9 tests, `rf628_oracle_rt_alloc` 1 test | Pass |
| `mise run test-rt` | exit 0 (`rt_` filter) | exit 0 — workspace `rt_`-matching tests all ok incl. `rt_scripted_input_callback_is_alloc_and_drop_free` | Pass |
| `mise run check-headless-deps` | exit 0 | exit 0 — no GUI framework deps in 12 headless crates | Pass |
| `mise run check-ruviform-rev` | exit 0 | exit 0 — `abb9f372…` resolved, 28 lockfile sources, REV match | Pass |
| `node tools/check-ruviform-rev.mjs --require-vendor` | exit 0 | exit 0 (via `sync-ui-deps` 後の vendor 再同期確認) | Pass |
| `mise exec -- prek run --all-files` | exit 0 | exit 0 — all hooks Passed/Skipped | Pass |
| `cargo test -p synvelis-recording --test rf628_oracle --test rf628_oracle_rt_alloc` | 10 pass / 0 fail | 10 pass / 0 fail（補助証拠 — canonical lane は上記 mise task） | Pass |
| `mise run test-recording-real-input` | — | not run in this slice（real-HW opt-in lane; 前 slice の iD4 10/10 構造検証履歴は `rcd-01-virtual-l1-oracle-ledger.md` を参照、本 slice で再実行・Pass 転用しない） | NotRun |
| `mise run build` / `test-msrv` / `*-full` / 他 package lane | — | not run（変更面は `synvelis-recording` dev-dep + docs/evidence; `*-msrv`/`*-full` は Daily Full CI delegated） | NotRun |

## Fixture case rows（`tests/rf628_oracle.rs` / `rf628_oracle_rt_alloc.rs`）

`BLOCK = 500` frames/callback、`SCENARIO_END = 16384`、drive は
`VirtualClockDriver` 追従 + 末尾端数を `StreamFault::Frames` で
厳密一致。Expected は `pcm_reference_block`（`PcmFixture::fill`
非依存の独立再実装）を segment 単位で構成し、Observed は journal
span が指す staging media の f32le bytes。`verified_sha256` は
journaled verified prefix bytes を test 内 `sha2` で独立再計算して
照合（全 commit/reload case で実施）。

### RF628-OR-01 — SV-1: stereo scenario E2E commit/reload

Fixture: `rf628_stereo_scenario_commit_reload`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| stereo E2E | Noise→Impulse→Sine 3 segment scenario → scripted backend → 本番 chain → staging media + journal → commit → save → 別 load で segment 単位 oracle PCM と一致 | 一致（`Sine` は `AbsEpsilon(1e-5)`、他は `Exact` bitwise） | Pass |
| `PumpReport` metadata | direction/callback_index/position/reported_frame_position が連続、`clock_domain = ClockDomainId::new(7)`、`gap_frames = 0`、`errors_fired = 0` | 全 callback で一致 | Pass |
| digest/idempotency | `Source.content_digest`/`Asset.digest` が verified prefix raw fold digest と整合、reload 後 `AlreadyApplied`/`NothingCommitted` の冪等分類 | 一致 | Pass |

### RF628-OR-02 — SV-2: multichannel → 2 mono tap 分離

Fixture: `rf628_multichannel_two_mono_taps_commit_reload`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| tap 分離 | 4ch device の mono(1)/mono(3) tap が別 stream・別 staging media に per-channel PCM で分離 | 一致（各 tap が対応 channel の oracle PCM と bitwise 一致、commit→reload 維持） | Pass |

### RF628-OR-03 — SV-3: same-clock 2 device + clock domain rejection

Fixture: `rf628_two_devices_same_clock_commit_reload`・
`rf628_clock_domain_mismatch_rejected`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| same-clock 束ね | shared `VirtualClock`（同 `ClockDomainId`）の 2 device が `InputSession::add` を通り別 stream に記録 | 一致 | Pass |
| 別 seed 分離 | 各 device が別 seed の `PcmScenario` を持ち各 stream が対応 oracle PCM と一致 | 一致 | Pass |
| clock domain mismatch | 別 `VirtualClock`（epoch 8 → 別 domain）の device は `ClockDomainMismatch` で defined rejection | `InputSession::add` が `Err` を返すことを確認 | Pass |

### RF628-OR-04 — SV-4: scenario gap → verified prefix

Fixture: `rf628_pcm_gap_verified_prefix`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| gap 検出 | `PcmScenario` の未供給区間（frame 2000–2499 = 1 callback 分 500 frame）で backend が silence + Xrun を報告（`gap_frames = 500`/callback、`errors_fired >= 1`） | `PumpReport` で確認、consumer 側 `FAULT_INPUT_DISCONTINUITY` latch → fail-stop | Pass |
| verified prefix | journal verified prefix = gap 直前 2000 frame が oracle と bit 一致、`verified_sha256` 再計算一致 | 一致 | Pass |
| commit 分類 | faulted stream が `Source` 非昇格・recoverable 分類（`NothingCommitted` + `recoverable.len() = 1`）、commit→reload 越しに維持 | 一致 | Pass |

### RF628-OR-05 — SV-5a: frame-position discontinuity

Fixture: `rf628_frame_position_fault_verified_prefix`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| position jump | `StreamFault::FramePosition` で callback 6 の position を 3300 に jump → report に反映、engine `first_input_frame` 不連続で fail-stop | `FAULT_INPUT_DISCONTINUITY` latch、verified prefix = 3000 frame（jump 直前） | Pass |

### RF628-OR-06 — SV-5b: `Error(ClockDiscontinuity)` epoch fault

Fixture: `rf628_clock_discontinuity_error_fault`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| epoch fault 写像 | `StreamFault::Error(DeviceError::ClockDiscontinuity)` が error callback 経由で `FAULT_EPOCH_DISCONTINUITY` に写像され fail-stop | fault_bits 実測で `FAULT_EPOCH_DISCONTINUITY` bit を確認、verified prefix = 2000 frame（fault_at=4 × BLOCK=500） | Pass |

### RF628-OR-07 — SV-5c: open fault rejection → recovery

Fixture: `rf628_open_fault_rejected_then_recovers`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| defined rejection | `set_open_input_error` persistent error で `InputSession::open` が `InputBindError::Device` を返す（crash/`Failed` 昇格と区別） | `Err(InputBindError::Device(_))` を確認 | Pass |
| recovery | error clear 後に同じ device/spec で `InputSession::open` が成功する（transient の正直な分類） | re-open 成功を確認。**re-open 後の record/commit/reload は本 test の対象外（未検証）** | Pass |

### RF628-OR-08 — SV-5d: device vanish fail-stop

Fixture: `rf628_device_vanish_fails_stop`。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| device loss | 1500 frame 正常供給後 `vanish()` → `pump_input_next` が defined error、`StreamInvalidated` → `FAULT_INPUT_DISCONTINUITY` latch → fail-stop | fault_bits 実測で確認 | Pass |
| verified prefix | prefix = 1500 frame（vanish_at=3 × BLOCK=500）保持、fail-stop 後の追加 pump で prefix 非伸長、commit は `NothingCommitted` + recoverable 分類 | 一致 | Pass |

### RF628-OR-09 — SV-6: scripted callback RT no-alloc

Fixture: `rt_scripted_input_callback_is_alloc_and_drop_free`
（`tests/rf628_oracle_rt_alloc.rs`、別 binary・`rt_` prefix・
warm-up 済み `Region`）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| no alloc/dealloc | warm-up 8 callback 後の `pump_input_next` 1 回分（backend pump + scenario fill + data callback）が heap 割当・dealloc を行わない | allocations = 0・deallocations = 0（`stats_alloc` `Region` 実測）、drain 後 fault bits = 0 | Pass |

## Lane summary

| Lane | Status | Note |
| --- | --- | --- |
| rf#628-fixture consumer oracle（本 slice・`ruviform-audio-test` scripted backend） | Pass | SV-1〜SV-6 全 case・全 stream で Expected/Observed 記録済み。deterministic fixture lane のみを権威とする |
| synthetic-L1（前 slice・test-local fake device） | Pass | `rcd01_oracle.rs` 10 tests（`rcd-01-virtual-l1-oracle-ledger.md` 参照）。本 slice はその置き換えではなく fixture runtime 経由の独立 lane |
| rf#629 仮想 endpoint EXT-L3 | NotRun | `scarecrow173/ruviform#629` OPEN、OS 列挙/Open 可能な仮想 WASAPI endpoint 環境未構築（NOT_PROVISIONED、hard dep・本 slice 対象外） |
| physical HW | NotRequired | 追加物理検証は通常必須外（POLICIES.md §2.1）。iD4 10/10 は前 slice の構造検証履歴のまま |
| GUI（Option B） | NotRun | `synvelis-app` 層の Recorder command/UI wire は本 slice 対象外（Deferred） |
| canonical blob publish 脚 | Deferred | `synvelis-player` 管轄。既存証拠 `crates/synvelis-player/tests/rcd02_blob_digest.rs`・`evidence/sv116/rcd-02-blob-sha256-ledger.md` を参照。本 slice で Pass 転用しない |
| macOS/Linux L1 CI・`*-msrv`/`*-full` | NotRun | Daily Full CI delegated — merge 後対象 SHA の CI 証拠で確認 |

## 未検証の項目（NotRun / Unsupported / Deferred）

- **rf#629 仮想 endpoint EXT-L3**: `scarecrow173/ruviform#629`
  （RF-VTEST-02 Windows 仮想 endpoint）OPEN。OS が列挙・Open できる
  仮想 WASAPI endpoint → 本番 Callback の環境が未構築のため
  NotRun(NOT_PROVISIONED)。hard dep だが本 slice 対象外 — Issue #115
  open 継続の根拠。
- **physical HW**: 追加物理検証は通常必須外（NotRequired）。
- **GUI（Option B）**: `synvelis-app` 層の Recorder command/UI wire
  は本 slice 対象外（Deferred）。
- **canonical blob publish 脚**: `synvelis-player` 管轄の実機経路。
  本 slice は staging media verified prefix bytes を digest adoption
  契約の範囲で解決し `verified_sha256` を `sha2` で独立再計算して
  pin。`publish_recorded` の実機経路は既存証拠を参照。
- **`mise run test-recording-real-input`**: 本 slice では未実行
  （NotRun）。前 slice の iD4 構造検証 Pass は履歴として参照するが
  本 slice の変更（dev-dep 追加 + test file + docs）とは無関係で
  あり、再実行・Pass 転用はしない。
- **ruviform 側の残 gap**: `RunMetadata` JSON serializer 欠如・
  `InputPump` drift リスクは ruviform 側の既知論点として
  `docs/impl/exp/issue-115-rf628-consumer-oracle.md` に記録。
  本 slice では恒久 workaround を作らない。

## audit「fixture variant を本番経路で検証」への回答

rf#628 fixture variant は `ruviform-audio-test` の scripted
deterministic backend を `&dyn Device` 経由で本番
`InputSession`/`RealDeviceRig`/`StreamWriter`/journal/commit/save/
reload 経路へ通し、独立 `pcm_reference_block` oracle + `PumpReport`
metadata + `verified_sha256` `sha2` 再計算で SV-1〜SV-6 を実測した。
fixture lane の Pass は deterministic-backend headless lane のみを
意味し、rf#629/physical/GUI/real-input lane の verdict を変更しない。
Issue #115 は rf#629 gate が残るため **open 継続**。
