---
id: "TSPEC-KV4kCWrFJsrB7hfC0dcpAp"
type: "test-spec"
status: "approved"
title: "Issue #115 rf#628 record-after-recovery test-spec"
created: "2026-11-16"
updated: "2026-11-16"
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
    - "docs/specs/issue-115-rf628-record-after-recovery.md"
    - "docs/designs/issue-115-rf628-record-after-recovery.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-115-rf628-consumer-oracle.md"
    - "docs/test-specs/rcd-01-virtual-l1-oracle.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-115-rf628-record-after-recovery.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-115-rf628-record-after-recovery.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #115 rf#628 record-after-recovery test-spec

## Purpose

`crates/synvelis-recording/tests/rf628_oracle.rs` に追加する
SV-7（open-fault recovery 後の record/commit/reload）と SV-8
（literal stereo→2mono tap variant）が保証すべきことを定義する
test-spec。先行 slice（`issue-115-rf628-consumer-oracle`）の
R1〜R6 を継承し、recovery 後 E2E と stereo tap 構成の 2 gap を
閉じる。scripted deterministic backend の成功を実 OS device・
WASAPI callback・GUI・physical の証明にしない
（deterministic-backend headless lane の Pass を他 lane の Pass に
転用しない）。

## Feature

SV-5c が残した open-fault re-open 後の未検証区間を、既存の
`RealDeviceRig::open`/`InputSession`/`StreamWriter`/journal/
commit/save/reload 経路で通す。期待値は `pcm_reference_block`
（`PcmFixture::fill` 非依存の独立再実装）で構成し
`pcm_assert_eq` で照合する。SV-8 は literal stereo device の
両 channel を別 mono tap・別 stream・別 staging media に分離する
構成を検証する。

## Rules

- **R7 open-fault recovery E2E**: persistent open error
  （`set_open_input_error`）下で `InputSession::open` が
  `InputBindError::Device` の defined rejection を返し、error
  clear 後に同じ device/spec で `InputSession::open` が成功
  （re-open）し、その session で record→drive→finish→commit→
  save→別 `ProjectController` reload まで完了する。re-open 後の
  PCM は scenario oracle と一致し、`verified_sha256` は `sha2`
  独立再計算と一致、commit は `Applied` + `AlreadyApplied`
  冪等分類を満たす。
- **R8 literal stereo→2mono tap**: 2ch（stereo）device の
  `ChannelMap::mono(0)`/`mono(1)` tap が別 stream・別 staging
  media に per-channel PCM で分離され、各 stream が対応 channel
  の oracle PCM と一致、commit→reload・digest 連鎖・冪等分類を
  満たす（4ch→mono(1)/mono(3) の SV-2 とは別構成）。
- **R9 非回帰**: `rf628_record` の pre-open hook 化は既存
  呼出しを no-op hook で維持し、SV-1〜SV-6 の全 test が
  変更なく pass する。recovery 専用の新経路・新 assertion 基盤
  を発明しない（既存 helper の再利用）。
- **R6 honesty（継承）**: 実行した lane のみ Pass と記録。
  rf#629/physical/GUI/real-input lane の verdict を変更しない。

## Examples（test ↔ rule 対応）

| Test | Rule |
| --- | --- |
| `rf628_open_fault_recovery_commit_reload` | R7+R9（SV-7 recovery E2E） |
| `rf628_stereo_device_two_mono_taps_commit_reload` | R8（SV-8 literal stereo→2mono） |

先行 slice 対応: SV-1〜SV-6 の R1〜R5 は
`docs/test-specs/issue-115-rf628-consumer-oracle.md` を参照
（本 slice で変更しない）。
