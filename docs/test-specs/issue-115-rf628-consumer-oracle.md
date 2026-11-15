---
id: "TSPEC-FBuBxX4T4QwzjTi0GVihWW"
type: "test-spec"
status: "approved"
title: "Issue #115 rf#628 consumer oracle test-spec"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-115-rf628-consumer-oracle.md"
    - "docs/designs/issue-115-rf628-consumer-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-01-virtual-l1-oracle.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-115-rf628-consumer-oracle.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-115-rf628-consumer-oracle.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #115 rf#628 consumer oracle test-spec

## Purpose

`crates/synvelis-recording/tests/rf628_oracle.rs`（SV-1〜SV-5）と
`tests/rf628_oracle_rt_alloc.rs`（SV-6）が保証すべきことを定義する
test-spec。scripted deterministic backend の成功を実 OS device・
WASAPI callback・GUI・physical の証明にしない
（deterministic-backend headless lane の Pass を他 lane の Pass に
転用しない）。

## Feature

`ruviform-audio-test` の `ScriptedBackend`/`PcmScenario`/
`StreamFault`/`VirtualClock`/`oracle` を本番 `InputSession`/
`InputAdapter` → `StreamWriter` → staging media/journal →
`commit_session` → save → reload 経路へ通す consumer-oracle。
期待値は `pcm_reference_block`（`PcmFixture::fill` 非依存の独立
再実装）で構成し `pcm_assert_eq` で照合する。

## Rules

- **R1 fixture metadata/case identity**: `PumpReport` の
  direction/callback_index/position/reported_frame_position/
  clock_domain/gap_frames/errors_fired/invalidated が scenario 設計
  どおりであることを記録・断言する（case 識別の証拠）。
- **R2 PCM fidelity**: journal span が指す staging media の f32le
  bytes を segment 単位の `pcm_reference_block` 期待 run と照合。
  `Sine`/`Sweep` は `AbsEpsilon(1e-5)`、他 pattern は `Exact`。
  `pcm_crc32` も各 chunk で検算する。
- **R3 multi-track/clock**: (i) multichannel device の複数 mono tap
  が別 stream・別 staging media に per-channel PCM で分離、
  (ii) same-clock 2 device が `InputSession::add` を通り別 seed の
  PCM を別 stream に記録、(iii) 別 clock domain は
  `ClockDomainMismatch` で defined rejection。
- **R4 fault/verified prefix**: `PcmScenario` gap（silence+Xrun）・
  `FramePosition` jump・`Error(ClockDiscontinuity)`・`vanish` で
  fail-stop し、journal verified prefix が scenario oracle と bit
  一致 + `verified_sha256` 再計算 pin + commit 層 recoverable
  分類（Source 非昇格）を commit→reload 越しに検証。
  `OpenFaults` は `InputBindError::Device` の defined rejection と
  clear 後 recovery を検証。
- **R5 RT 契約**: scripted `pump_input_next`（backend pump +
  scenario fill + data callback）1 回分が heap 割当・dealloc を
  行わないことを warm-up 済み `Region` で計測（別 binary・単一
  テスト・`rt_` prefix）。
- **R6 honesty**: 実行した lane のみ Pass と記録。rf#629/physical/
  GUI/real-input lane の verdict を変更しない。

## Examples（test ↔ rule 対応）

| Test | Rule |
| --- | --- |
| `rf628_stereo_scenario_commit_reload` | R1+R2（SV-1 stereo 3-segment E2E） |
| `rf628_multichannel_two_mono_taps_commit_reload` | R3(i)（SV-2 4ch→2 mono tap） |
| `rf628_two_devices_same_clock_commit_reload` | R3(ii)（SV-3 same-clock 2 device） |
| `rf628_clock_domain_mismatch_rejected` | R3(iii)（SV-3 負側） |
| `rf628_pcm_gap_verified_prefix` | R4（SV-4 scenario gap→Xrun） |
| `rf628_frame_position_fault_verified_prefix` | R4（SV-5a position jump） |
| `rf628_clock_discontinuity_error_fault` | R4（SV-5b epoch fault bit） |
| `rf628_open_fault_rejected_then_recovers` | R4（SV-5c open fault） |
| `rf628_device_vanish_fails_stop` | R4（SV-5d device loss） |
| `rt_scripted_input_callback_is_alloc_and_drop_free` | R5（SV-6） |
