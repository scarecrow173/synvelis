---
id: "TASK-9cHuFK1tnSWxWkEqEnjMYm"
type: "task"
status: "done"
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
  implements:
    - "docs/plans/issue-115-rf628-record-after-recovery.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-115-rf628-record-after-recovery.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-115-rf628-record-after-recovery.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #115 rf#628 record-after-recovery — synvelis#115 slice

## Tasks

- [x] T1: `rf628_record` を `rf628_record_with_pre_open` 委譲
  wrapper 化し、device/spec 構築後・`RealDeviceRig::open` 直前に
  `pre_open(&devices, &specs)` hook を追加（既存呼出しは no-op
  hook で非回帰）。
- [x] T2: SV-7 `rf628_open_fault_recovery_commit_reload` —
  hook 内で persistent open error → `InputSession::open` が
  `InputBindError::Device` rejection → clear → rig open 内
  re-open → record→finish→commit→save→別 controller reload を
  SV-1 と同じ oracle 厳密さで検証。
- [x] T3: SV-8 `rf628_stereo_device_two_mono_taps_commit_reload` —
  literal stereo device → mono(0)/mono(1) の 2 mono tap・別
  stream・別 staging media を commit→reload まで検証。
- [x] T4: `evidence/sv115/` case matrix・ledger・本 slice 検証記録
  を実測で更新。
- [x] T5: doc chain 7 点 + `docs/*/README.md` index 再生成。
- [x] T6: 正規検証（fmt-check/check/lint/test
  synvelis-recording/prek）+ review-protocol + commit/push/PR
  （`Refs #115`）。

## Notes

- `tests/common/mod.rs`・production `src/`・依存・pin・
  `rf628_oracle_rt_alloc.rs` は非変更。`RealDeviceRig::open` の
  `&dyn Device` 経路をそのまま再利用し、rig open 内の
  `InputSession::open` が recovery re-open を兼ねる設計
  （design D1/D2）。
- `test-rt` は `rf628_oracle_rt_alloc.rs` 非変更・`rt_` test
  追加なしのため条件非該当（NotRun 記録）。
- SV-5c `rf628_open_fault_rejected_then_recovers` は変更せず、
  SV-7 は re-open 後を通す別 test として追加（既存 verdict を
  書き換えず gap を閉じる）。
