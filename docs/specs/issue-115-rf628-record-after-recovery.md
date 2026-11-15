---
id: "SPEC-x2yrcR6anjkQ3O33VJ99WF"
type: "spec"
status: "approved"
title: "Issue #115 rf#628 record-after-recovery — synvelis#115 slice"
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
  implemented-by:
    - "docs/designs/issue-115-rf628-record-after-recovery.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-115-rf628-consumer-oracle.md"
    - "docs/specs/rcd-01-virtual-l1-oracle.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  defers: []
  deferred-by: []
---

# Issue #115 rf#628 record-after-recovery — synvelis#115 slice

## 背景

`synvelis#115`（RCD-01 実 Audio Device 録音統合）の仮想優先置換 DoD
に対し、先行 slice（PR #486 squash merge `ac5200c`、doc chain slug
`issue-115-rf628-consumer-oracle`）が `ruviform-audio-test`
deterministic scripted backend を本番 `InputSession`/`InputAdapter`
→ `StreamWriter` → staging media/journal → `commit_session` →
save → reload 経路へ通す consumer oracle lane（SV-1〜SV-6）を
統合した。しかし同 slice の verification record
（`evidence/sv115/issue-115-rf628-consumer-oracle-verification.md`
RF628-OR-07 recovery 行）は 2 件の residual gap を正直に記録した
まま残している。

- **SV-7（record-after-recovery 未検証）**: SV-5c
  `rf628_open_fault_rejected_then_recovers` は persistent open
  error → `InputSession::open` の defined rejection
  （`InputBindError::Device`）→ error clear → 同じ device/spec で
  re-open 成功までを検証したが、re-open 後 session を drop する
  のみで **record → drive → finish → commit → save → 別
  `ProjectController` reload は未検証**。
- **SV-8（literal stereo→2mono tap variant 未網羅）**: 現行
  `rf628_multichannel_two_mono_taps_commit_reload` は 4ch device
  の mono(1)/mono(3) tap で代替しており、literal な stereo
  device → `ChannelMap::mono(0)`/`mono(1)` の 2 mono tap 構成は
  synthetic-L1 lane（`oracle_stereo_device_two_mono_taps`）のみが
  網羅（case matrix の正直な記録どおり）。

本 slice はこの 2 gap を test + evidence + docs で閉じる
residual gap closure。production `src/`・依存・ruviform pin は
変更しない（test-only）。

## スコープ

- `crates/synvelis-recording/tests/rf628_oracle.rs` に新規 test を
  追加する。
  - SV-7 `rf628_open_fault_recovery_commit_reload`: persistent
    open error → defined rejection → clear → re-open（rig open
    内 `InputSession::open` が兼ねる）→ その session で
    record→drive→finish→commit→save→別 `ProjectController`
    reload まで、独立 `pcm_reference_block` oracle 照合・
    `verified_sha256` の `sha2` 独立再計算・commit/reload 分類を
    SV-1 正常系と同じ厳密さで検証。
  - SV-8 `rf628_stereo_device_two_mono_taps_commit_reload`:
    literal stereo device → mono(0)/mono(1) の 2 mono tap・別
    stream・別 staging media 構成を commit→reload まで検証
    （SV-2 と同構造の stereo variant）。
- `rf628_record` に rig open 前 hook
  （`rf628_record_with_pre_open`）を追加し、open fault の
  注入→rejection→clear を既存 rig 構築経路の直前に仕込める
  ようにする（既存呼出しは no-op hook で非回帰）。
- `evidence/sv115/` の case matrix・ledger・本 slice の検証記録を
  実測 verdict で更新する。
- doc chain（slug `issue-115-rf628-record-after-recovery`）を
  spec→design→plan→task→test-spec→impl/ir→impl/exp で作成する。

## 非スコープ（正直な境界）

- 実 OS audio device・WASAPI callback・GUI-L2・physical hardware・
  `scarecrow173/ruviform#629` 仮想 endpoint EXT-L3 — 本 slice
  対象外、各 lane は現行 verdict のまま NotRun/NotRequired。
- `test-recording-real-input` opt-in HW lane・`*-msrv`・`*-full` —
  変更しない / Daily Full CI delegated。
- SV-9 device-injected MIDI・SV-10 transport MIDI mapping・
  Punch/Loop/Arm/Monitor oracle lane 重複 — 別 slice/Issue。
- production `src/`・`tests/common/mod.rs`・`Cargo.toml`・
  `Cargo.lock`・ruviform pin — 変更しない。
- Issue #115 は open 継続（PR は `Refs #115`、`Closes` 禁止）。

## 受入条件（本 slice）

- SV-7/SV-8 が deterministic-backend headless lane として実測
  Pass し、証拠が case matrix/ledger/検証記録に記録される。
- scripted-backend の Pass を実 OS device/GUI/physical の Pass に
  転用しない。
- production `src/`・依存・pin 変更なし（test-only）。
