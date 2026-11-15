---
id: "EXP-Nis5H2kGCeimCOXZADO4C7"
type: "experiment"
status: "completed"
title: "Issue #115 rf#628 record-after-recovery — open-fault 後 E2E と stereo→2mono tap の実測"
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
    - "docs/impl/ir/issue-115-rf628-record-after-recovery.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-115-rf628-record-after-recovery.md"
    - "docs/test-specs/issue-115-rf628-record-after-recovery.md"
    - "docs/impl/exp/issue-115-rf628-consumer-oracle.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/issue-115-rf628-record-after-recovery.md"
  defers: []
  deferred-by: []
---

# Issue #115 rf#628 record-after-recovery — open-fault 後 E2E と stereo→2mono tap の実測

## 仮説

`ruviform-audio-test` の persistent open error
（`set_open_input_error`）は clear まで全 `open_input_stream` を
拒否し、失敗した open は device slot を残さない。したがって
rejection→clear の後、`RealDeviceRig::open` 内の
`InputSession::open` は recovery 後の re-open として成功し、
その session で通常どおり record→commit→reload できるはず。
また literal stereo device の mono(0)/mono(1) tap は SV-2 の
4ch→mono(1)/mono(3) と同機構で別 stream・別 staging media に
分離されるはず。

## 方法

- `rf628_record` を `rf628_record_with_pre_open` へ委譲 wrapper
  化し、`pre_open(&devices, &specs)` hook を `RealDeviceRig::open`
  直前に追加。SV-7 の hook 内で persistent error 設定→
  `InputSession::open` rejection（`InputBindError::Device`）を
  断言→clear。re-open は rig open 内の `InputSession::open` が
  担う。
- SV-7: record→drive→finish→commit→save→別 `ProjectController`
  reload を `assert_finish_complete`/`rf628_commit_reload`/
  `assert_scenario_stream`/`AlreadyApplied` で検証。
- SV-8: `channels: 2`・`Noise` scenario・`mono(0)`/`mono(1)` tap
  で SV-2 と同じ監査（別 media・oracle PCM・digest 分離・冪等）。

## Result

`cargo test -p synvelis-recording --locked --test rf628_oracle --
rf628_open_fault_recovery_commit_reload
rf628_stereo_device_two_mono_taps_commit_reload` — 2 pass /
0 fail。

- **open-fault 後 E2E は成立**: persistent error 下の
  `InputSession::open` は `InputBindError::Device` で defined
  rejection（SV-5c と同一分類）。clear 後の rig open 内
  `InputSession::open` が成功し、record→commit→reload が
  `Applied`/`AlreadyApplied`・`sources_committed = 1`・
  `recoverable` 空で完了。journal media は `pcm_reference_block`
  oracle と一致、`verified_sha256` は `sha2` 独立再計算と一致。
- **stereo→2mono tap 分離は成立**: literal stereo device の
  mono(0)/mono(1) tap が別 stream・別 staging media に分離し、
  各 stream が対応 channel の oracle PCM と一致。ch0/ch1 の
  `Noise` PCM は bit 単位で異なり digest が分離
  （`sources_committed = 2`・`AlreadyApplied`）。
- **既存経路の再利用で足りた**: recovery 専用の新経路・新
  assertion は不要だった — `rf628_record` への pre-open hook
  追加のみで SV-5c の後段を既存 helper の厳密さで網羅できた。

## Follow-ups

- `evidence/sv115/` case matrix の Multi-track (i) 行を実測へ
  更新（literal stereo→2mono を rf#628 lane でも網羅）。
- rf#629/physical/GUI/real-input lane は NotRun/NotRequired の
  まま。Issue #115 は open 継続。
