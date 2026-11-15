# Evidence — synvelis#115 rf#628 record-after-recovery: open-fault 後 E2E と literal stereo→2mono tap

Verification record for the `issue-115-rf628-record-after-recovery`
slice of Issue #115 (RCD-01 実 Audio Device 録音統合). This slice
closes the two residual gaps honestly left by the rf#628 consumer
oracle slice (PR #486 / `ac5200c`):

- **SV-7** — SV-5c (`rf628_open_fault_rejected_then_recovers`) verified
  open-fault defined rejection → clear → re-open success but stopped
  there; the re-opened session was dropped without record/commit/
  reload. This slice adds
  `rf628_open_fault_recovery_commit_reload`, which drives the
  re-opened session through the full production path.
- **SV-8** — the rf#628 lane covered multichannel→2 mono taps only as
  4ch→mono(1)/mono(3); the literal stereo→mono(0)/mono(1) shape was
  covered only by the synthetic-L1 lane. This slice adds
  `rf628_stereo_device_two_mono_taps_commit_reload`.

`Pass` below means *the stated headless/deterministic-fixture lane
only* — it never implies real OS audio-device capture, real WASAPI
callback, the `scarecrow173/ruviform#629` virtual endpoint, real GUI,
physical hardware, other-OS behavior, or product acceptance. Those
lanes stay `NotRun`/`NotRequired`/`Deferred` with named owners, per
`POLICIES.md`.

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis branch | `feat/115-sv5c-reopen` |
| Base commit | `ac5200cd457551b839d379026404cb8b9e76d7f6` (= `origin/main` at dispatch, PR #486 squash merge) |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.1 (58cf8f9c3 2026-10-13) / cargo 1.99.1 via mise |
| Pinned ruviform rev | `abb9f372d3ee95ee824b3c4f7dc68e8083ce2774` — unchanged by this slice (test-only; no `Cargo.lock`/manifest diff) |
| Fixture | `ruviform_audio_test`: `ScriptedBackend`/`ScriptedDeviceHandle` (`set_open_input_error` persistent fault), `PcmFixture`/`PcmPattern`/`PcmScenario`, `VirtualClock`/`VirtualClockDriver`, independent `oracle::pcm_reference_block` + `pcm_assert_eq` |
| Production path under test | `RealDeviceRig::open(&dyn Device)` → `InputSession`/`InputAdapter` → `StreamWriter` → staging media/journal → finalize → `commit_session` → `DirectoryBundleStore` save → separate `ProjectController::load` |
| Lane | headless integration test のみ（canonical = `mise run test synvelis-recording`；補助証拠 = focused `cargo test`） |

Status vocabulary: `Pass` = measured pass in the stated lane;
`Fail` = measured failure; `NotRun` = lane exists but was not
measured here; `Unsupported` = capability genuinely unavailable;
`Deferred` = intentionally out of scope for this slice;
`NotRequired` = lane intentionally not a gate for this slice.

## Canonical lane (mise tasks)

All lanes run via `mise run <task>` in this worktree on 2026-11-16
(JST). Only lanes actually executed are listed as `Pass`; unexecuted
lanes are `NotRun`.

| Command | Expected | Observed | Status |
| --- | --- | --- | --- |
| `mise run fmt-check` | exit 0 | exit 0（初回は新規 test の整形差分で fail → `cargo fmt -p synvelis-recording` で修正後 pass） | Pass |
| `mise run check synvelis-recording` | exit 0 | exit 0 | Pass |
| `mise run lint synvelis-recording` | exit 0 | exit 0 | Pass |
| `mise run test synvelis-recording` | exit 0 | exit 0 — all test binaries ok。`rf628_oracle` 11 tests 全 pass（`rf628_open_fault_recovery_commit_reload`・`rf628_stereo_device_two_mono_taps_commit_reload` を含む）、`rf628_oracle_rt_alloc` 1 test pass | Pass |
| `mise run test-rt` | — | not run — `rf628_oracle_rt_alloc.rs` 非変更・`rt_` prefix test 追加なしのため条件非該当（plan 記載どおり NotRun） | NotRun |
| `mise exec -- prek run --all-files` | exit 0 | exit 0 — all hooks Passed/Skipped | Pass |
| `cargo test -p synvelis-recording --locked --test rf628_oracle -- rf628_open_fault_recovery_commit_reload rf628_stereo_device_two_mono_taps_commit_reload` | 2 pass / 0 fail | 2 pass / 0 fail（補助証拠 — canonical lane は上記 mise task） | Pass |
| `mise run test-recording-real-input` | — | not run in this slice（real-HW opt-in lane; 前 slice の iD4 10/10 構造検証履歴は `rcd-01-virtual-l1-oracle-ledger.md` を参照、本 slice で再実行・Pass 転用しない） | NotRun |
| `mise run test-msrv` / `*-full` / 他 package lane / `sync-ui-deps`・`check-ruviform-rev` | — | not run（`*-msrv`/`*-full` は Daily Full CI delegated; ruviform pin 不変のため sync/check 不要） | NotRun |

## Fixture case rows（`tests/rf628_oracle.rs`）

`BLOCK = 500` frames/callback、drive/finish/commit/reload/独立
oracle 照合の枠組みは consumer oracle slice
（`issue-115-rf628-consumer-oracle-verification.md`）と同一。
本 slice の 2 case のみ記録する。

### RF628-OR-10 — SV-7: open fault recovery 後の record/commit/reload

Fixture: `rf628_open_fault_recovery_commit_reload`。

`rf628_record` を `rf628_record_with_pre_open(…, |_, _| {})` へ
委譲する薄い wrapper 化し、device/spec 構築後・
`RealDeviceRig::open` 直前に `pre_open(&devices, &specs)` hook を
1 度呼ぶ形にした（既存 7 呼出しは signature・挙動不変）。SV-7 の
hook 内で persistent open error を設定し、defined rejection を
断言してから clear する。re-open は rig open 内の
`InputSession::open` が兼ね、その session で record→finish→
commit→save→別 `ProjectController` reload まで通す。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| defined rejection | `set_open_input_error(Some(DeviceError::Backend("permission_denied")))` 下で `InputSession::open` が `InputBindError::Device` を返す（crash/`Failed` 昇格と区別、SV-5c と同一分類） | `Err(InputBindError::Device(_))` を hook 内で確認 | Pass |
| recovery re-open | clear 後に同じ device/spec で rig open 内 `InputSession::open` が成功する | `RealDeviceRig::open` が panic なく完了 | Pass |
| record E2E | recovery session で deterministic PCM を record→finish | `assert_finish_complete` 相当の完走を確認 | Pass |
| commit/reload | `commit_session` → `Applied`・`sources_committed = 1`・`recoverable` 空 → save → 別 `ProjectController::load` | `rf628_commit_reload` 経由で確認、再 commit は `AlreadyApplied` | Pass |
| oracle 照合 | journal media が `pcm_reference_block` と一致、`verified_sha256` が `sha2` 独立再計算と一致 | `assert_scenario_stream`（original + reloaded 両 controller）で確認 | Pass |

### RF628-OR-11 — SV-8: literal stereo device → 2 mono tap

Fixture: `rf628_stereo_device_two_mono_taps_commit_reload`。

`channels: 2`・`contiguous_scenario(2, PcmPattern::Noise)`・
`taps: [(0, ChannelMap::mono(0)), (1, ChannelMap::mono(1))]` で
literal stereo→2mono を構成し、SV-2（4ch→mono(1)/mono(3)）と同じ
監査を実施。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| stream 分離 | ch0→stream A・ch1→stream B が別 stream・別 staging media path | 2 stream が独立 finalize、media path 相違を確認 | Pass |
| PCM oracle | 各 stream が対応 channel の `pcm_reference_block`（Noise, `Exact`）と一致 | `assert_scenario_stream` で両 stream 照合 | Pass |
| channel 分離 | ch0/ch1 の Noise PCM が bit 単位で異なり digest が分離 | per-channel PCM/digest の相違を確認（`sources_committed = 2`） | Pass |
| commit/reload 冪等 | commit→save→別 controller reload→`AlreadyApplied` | SV-2 と同一手順で確認 | Pass |

## Lane summary

| Lane | Status | Note |
| --- | --- | --- |
| rf#628-fixture consumer oracle（本 slice・`ruviform-audio-test` scripted backend） | Pass | SV-1〜SV-8 全 case を deterministic-backend headless lane で実測（SV-7/SV-8 は本 slice で追加）。deterministic fixture lane のみを権威とする |
| synthetic-L1（test-local fake device） | Pass | `rcd01_oracle.rs`（`rcd-01-virtual-l1-oracle-ledger.md` 参照）。本 slice は置き換えではなく rf#628 lane の gap closure |
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
  本 slice の変更（test file + docs/evidence）とは無関係であり、
  再実行・Pass 転用はしない。
- **`mise run test-rt`**: 条件非該当のため NotRun
  （`rf628_oracle_rt_alloc.rs` 非変更・`rt_` prefix test 追加なし）。
  既存 SV-6 計測は `mise run test synvelis-recording` 内の
  `rf628_oracle_rt_alloc` binary で pass を確認済み。
- **synthetic-L1 lane の open-fault recovery**: `rcd01_oracle.rs`
  には open fault case が存在しない（NotRun）。rf#628 lane の
  SV-7 が deterministic-backend lane の権威。

## audit「open-fault 後の本番経路 E2E」への回答

persistent open error → defined `InputBindError::Device` rejection
→ clear → 同一 device/spec での re-open → record → finish →
commit → save → 別 `ProjectController` reload までを、production
`RealDeviceRig`/`InputSession`/`InputAdapter`/`StreamWriter`/
journal/commit 経路で実測した。re-open は rig open 内の
`InputSession::open` が担い、recovery 専用の新経路・新 assertion
基盤は不要だった。literal stereo→2mono tap も SV-2 と同機構で
別 stream・別 staging media・per-channel oracle/digest 分離を
確認した。fixture lane の Pass は deterministic-backend headless
lane のみを意味し、rf#629/physical/GUI/real-input lane の verdict
を変更しない。Issue #115 は rf#629 gate が残るため **open 継続**。
