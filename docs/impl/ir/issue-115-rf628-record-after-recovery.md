---
id: "IMPL-ptnUWquRtwhumn0SE8jb8W"
type: "impl"
status: "completed"
title: "Issue #115 rf#628 record-after-recovery — synvelis#115 slice"
created: "2026-11-16"
updated: "2026-11-16"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "docs/specs/issue-115-rf628-record-after-recovery.md"
      - type: "file"
        path: "docs/designs/issue-115-rf628-record-after-recovery.md"
      - type: "file"
        path: "docs/plans/issue-115-rf628-record-after-recovery.md"
      - type: "file"
        path: "docs/tasks/issue-115-rf628-record-after-recovery.md"
      - type: "file"
        path: "docs/test-specs/issue-115-rf628-record-after-recovery.md"
      - type: "file"
        path: "docs/impl/ir/issue-115-rf628-record-after-recovery.md"
      - type: "file"
        path: "docs/impl/exp/issue-115-rf628-record-after-recovery.md"
      - type: "file"
        path: "evidence/sv115/issue-115-rf628-record-after-recovery-verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/tests/rf628_oracle.rs"
      - type: "file"
        path: "evidence/sv115/rcd-01-virtual-l1-oracle-case-matrix.md"
      - type: "file"
        path: "evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-115-rf628-record-after-recovery.md"
    - "docs/specs/issue-115-rf628-record-after-recovery.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-115-rf628-record-after-recovery.md"
    - "docs/plans/issue-115-rf628-record-after-recovery.md"
    - "docs/test-specs/issue-115-rf628-record-after-recovery.md"
    - "docs/impl/exp/issue-115-rf628-record-after-recovery.md"
    - "docs/impl/ir/issue-115-rf628-consumer-oracle.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-115-rf628-record-after-recovery.md"
  derived-by: []
  verifies:
    - "docs/test-specs/issue-115-rf628-record-after-recovery.md"
  verified-by:
    - "docs/test-specs/issue-115-rf628-record-after-recovery.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-115-rf628-record-after-recovery.md"
    rejected: []
---

# Issue #115 rf#628 record-after-recovery — Implementation Record

## Summary

`synvelis#115` の `rf#628-fixture` lane residual gap closure slice。
PR #486（`ac5200c`）の consumer oracle が正直に残した 2 gap —
SV-5c の open-fault re-open 後の record/commit/reload 未検証
（SV-7）と literal stereo→2mono tap variant 未網羅（SV-8）— を
`tests/rf628_oracle.rs` への test 追加のみで閉じた。
production `src/`・`tests/common/mod.rs`・依存・ruviform pin は
非変更（test-only）。

## Context

- `rf628_open_fault_rejected_then_recovers`（SV-5c）は
  persistent open error → `InputBindError::Device` defined
  rejection → clear → re-open 成功までを検証したが、re-open 後
  session を drop するのみで record 以降は未検証と verification
  record が明示していた。
- case matrix は literal stereo→2mono 構成を synthetic-L1 lane
  のみの網羅と正直に記録しており、rf#628 lane での対応 case が
  欠けていた。
- `RealDeviceRig::open`/`open_rig_core` は `&dyn Device` 経路で
  `InputSession::open`/`add` を内部実行するため、open fault の
  注入→rejection→clear を rig open 前に仕込めば、re-open 成功
  そのものを本番 rig 構築経路で観測できる。

## Implementation

### Changes

- **`crates/synvelis-recording/tests/rf628_oracle.rs`**:
  - `rf628_record` を `rf628_record_with_pre_open(…, |_, _| {})`
    へ委譲する薄い wrapper 化し、本体を
    `rf628_record_with_pre_open` へ移管。device/spec 構築後・
    `RealDeviceRig::open` 直前に `pre_open(&devices, &specs)` を
    1 度呼ぶ hook を追加（既存 7 呼出しは signature・挙動不変）。
  - SV-7 `rf628_open_fault_recovery_commit_reload`: hook 内で
    `set_open_input_error(Some(DeviceError::Backend(
    "permission_denied")))` → `InputSession::open` が
    `Err(InputBindError::Device(_))` → `set_open_input_error(None)`。
    その後の rig open 内 `InputSession::open` が re-open を兼ね、
    record→finish→commit→save→別 `ProjectController` reload を
    `assert_finish_complete`/`rf628_commit_reload`/
    `assert_scenario_stream`/`AlreadyApplied` で SV-1 と同じ
    厳密さで検証。
  - SV-8 `rf628_stereo_device_two_mono_taps_commit_reload`:
    `channels: 2`・`contiguous_scenario(2, Noise)`・
    `taps: [(0, mono(0)), (1, mono(1))]` で literal stereo→2mono
    を構成し、SV-2 と同じ監査（別 media・per-channel oracle・
    digest 分離・commit→reload・冪等）を実施。
- **`evidence/sv115/`**: case matrix の Multi-track (i) 行・
  fault 系行を実測へ更新、ledger の lane summary を SV-1〜SV-8
  実測へ整合、本 slice の検証記録を新規追加。
- **doc chain**: slug `issue-115-rf628-record-after-recovery`
  の spec/design/plan/task/test-spec/impl-ir/impl-exp +
  `docs/*/README.md` index 再生成。

### Decisions During Implementation

- **pre-open hook 方式の採用**: 手動 `InputSession::open` で
  `(session, ends)` を取得し `RealDeviceRig` を手組みする案は、
  private `open_rig_core`/`staging_streams` の複製（~50 行・
  ドリフト温床）を要するため不採用。rig open 内の
  `InputSession::open` が recovery re-open を兼ねる設計とし、
  本番 rig 構築経路をそのまま通した。
- **SV-5c test は非変更**: rejection→clear→re-open 成功の
  既存 verdict を維持し、SV-7 は後段を通す別 test として追加
  （gap を閉じるが既存証拠を書き換えない）。
- **`rf628_oracle_rt_alloc.rs` 非変更**: SV-7/SV-8 は global
  allocator 統計と無関係の functional test であり、単一計測
  テスト規約の binary へは置かない。`rt_` test 追加もないため
  `test-rt` は条件非該当（NotRun 記録）。

## Related Experiments

- `docs/impl/exp/issue-115-rf628-record-after-recovery.md` —
  open-fault lifecycle（persistent error の評価順・clear 後の
  re-open）と stereo→2mono tap 分離の実測記録。

## Validation

- `cargo test -p synvelis-recording --locked --test rf628_oracle
  -- rf628_open_fault_recovery_commit_reload
  rf628_stereo_device_two_mono_taps_commit_reload` — 2 pass /
  0 fail（補助証拠）。
- `mise run fmt-check` / `check synvelis-recording` /
  `lint synvelis-recording` / `test synvelis-recording`（
  `rf628_oracle` 11 tests 全 pass）/ `prek run --all-files` —
  全て exit 0。
- `mise run test-rt` — NotRun（rt_alloc file 非変更・`rt_` test
  追加なしで条件非該当）。
- 実測値の一覧は検証記録
  `evidence/sv115/issue-115-rf628-record-after-recovery-verification.md`
  を参照。

## Risks

- `pre_open` hook が将来の rig 構築変更で呼出し位置を失う
  リスク → doc comment で位置契約を明記。rejection 未発生時は
  rig open の `.expect("session open")` が panic するため silent
  pass しない。
- open fault の残留 state → backend `open_input` の fault 評価
  順を `stream.rs` で確認済み（persistent は消費されず clear
  まで全 open を拒否、`d.open_input` slot は失敗時に触らない）。

## Follow-ups

- `scarecrow173/ruviform#629` 仮想 WASAPI endpoint EXT-L3・
  physical・GUI（Option B）・macOS/Linux L1 CI — NotRun/
  NotRequired のまま、Issue #115 open 継続の根拠。
- 本 slice の merge 後 SHA に対する Daily Full CI 証拠は
  Coordinator 管理の外部 gate。
