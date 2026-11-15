---
id: "PLAN-2NkiwJVohdkMbtz01hznrR"
type: "plan"
status: "approved"
title: "Issue #115 rf#628 record-after-recovery Plan"
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
    - "docs/specs/issue-115-rf628-record-after-recovery.md"
    - "docs/designs/issue-115-rf628-record-after-recovery.md"
  implemented-by:
    - "docs/tasks/issue-115-rf628-record-after-recovery.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-115-rf628-consumer-oracle.md"
    - "docs/plans/rcd-01-virtual-l1-oracle.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-115-rf628-record-after-recovery.md"
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
---

# Issue #115 rf#628 record-after-recovery Plan

## Scope split

単一 slice（`rf#628-fixture` lane の residual gap closure —
SV-7 record-after-recovery + SV-8 literal stereo→2mono）。Issue
#115 の残り gate（rf#629 仮想 endpoint EXT-L3・physical・GUI
Option B・macOS/Linux L1 CI）は本 slice 対象外で、Issue は
open 継続。

## File map

| File | 変更 | 内容 |
| --- | --- | --- |
| `crates/synvelis-recording/tests/rf628_oracle.rs` | modified | `rf628_record` を `rf628_record_with_pre_open` 委譲 wrapper 化 + `pre_open` hook 呼出し追加。SV-7 `rf628_open_fault_recovery_commit_reload`・SV-8 `rf628_stereo_device_two_mono_taps_commit_reload` の 2 test を末尾に追加（計 11 test） |
| `evidence/sv115/rcd-01-virtual-l1-oracle-case-matrix.md` | modified | Multi-track (i) 行の rf#628-fixture cell を実測へ更新（literal stereo→2mono 追加）、fault 行または新規行に open-fault recovery E2E を反映 |
| `evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md` | modified | lane summary の rf#628-fixture 注記を SV-1〜SV-8 実測へ整合 |
| `evidence/sv115/` | added | 本 slice の検証記録（新規ファイル） |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/issue-115-rf628-record-after-recovery.md` | added | doc chain（slug `issue-115-rf628-record-after-recovery`） |
| `docs/*/README.md` | generated | `doc-maintenance`/`impl-doc` index 再生成（手編集禁止） |

`tests/common/mod.rs`・production `src/`・`Cargo.toml`・
`Cargo.lock`・ruviform pin は非変更。

## Dependency graph

```
rf628_record pre_open hook refactor（既存 test 非回帰）
  → SV-7/SV-8 test 追加 → mise 正規検証
  → evidence 更新 → doc chain → review-protocol → commit/push/PR
```

## Risk register

| Risk | 対応 |
| --- | --- |
| hook 位置ミスで open fault が rig open に間に合わない | `pre_open` を `RealDeviceRig::open` 直前に配置。rejection 未発生なら test が panic |
| 失敗 open の残留 state | backend `open_input` の fault 評価順（persistent → once → DeviceBusy）を `stream.rs` で確認済み。clear 後の open は新規 slot を正常構築 |
| SV-8 tap PCM が一致し分離立証不能 | `Noise` の channel 次元 hash で ch0/ch1 は bit 単位に異なる（SV-2 先例）。digest `assert_ne` で確認 |
| `rf628_record` refactor の非回帰破壊 | no-op hook 委譲のみ。`mise run test synvelis-recording` で全 test 確認 |
| rt_alloc binary への誤配置 | SV-7/SV-8 は `rf628_oracle.rs` のみ。`rf628_oracle_rt_alloc.rs` は単一計測テスト規約のため非変更・`rt_` test 追加なし → `test-rt` 条件非該当で NotRun 記録 |

## Rollback strategy

`rf628_oracle.rs` の変更は wrapper 化 + 末尾 test 追加のみ。
問題時は test 追加部と hook を revert すれば PR #486 状態へ
戻る（evidence/docs は別 commit で論理分割）。

## Verification matrix

| 検証 | 種別 | 期待 |
| --- | --- | --- |
| `cargo test -p synvelis-recording --test rf628_oracle`（補助証拠） | 補助 | 11 test pass・新規 2 test 名で実走確認 |
| `mise run fmt-check` | 正規 | pass |
| `mise run check synvelis-recording` | 正規 | pass |
| `mise run lint synvelis-recording` | 正規 | pass（-D warnings） |
| `mise run test synvelis-recording` | 正規 | pass（非回帰含む・新規 test 名で確認） |
| `mise run test-rt` | 条件付き | `rf628_oracle_rt_alloc.rs` 非変更・`rt_` test 追加なしのため条件非該当 → NotRun 記録 |
| `mise exec -- prek run --all-files` | 正規 | pass |
| doc index（`index plan`/`index apply`、`sync_impl_index`） | 正規 | in-sync 化 |
| `test-recording-real-input`/`test-msrv`/`*-full` | — | 実行しない → NotRun 記録 |
