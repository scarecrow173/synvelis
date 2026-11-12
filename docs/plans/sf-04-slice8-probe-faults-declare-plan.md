---
id: "PLAN-3tM8kN1xC5wQ7rV2jH6fY9b"
type: "plan"
status: "approved"
title: "SF-04 slice8 実装計画 — SF-PROBE-05 fault fixtures + 不連続宣言点"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-04-slice8-probe-faults-declare-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice8 実装計画 — SF-PROBE-05 fault fixtures + 不連続宣言点

## Goal

Issue #107 の bounded deliverable 2 件を 1 PR で完了する:
(a) `sf04_s8_probe_faults.rs` で SF-PROBE-05 の fault 経路を
fail-closed 証拠として固定、(b) `declare_probe_discontinuity`
API + app passthrough + `sf04_s9_probe_discontinuity_declare.rs`
で seek 以外の不連続宣言点を実装・証明する。あわせて
fixture が露呈した `probe.rs` drain の segment close 順序
ギャップを `PendingClose` で修正する。

## File Map

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-player/src/routing_engine.rs` | `declare_probe_discontinuity` 追加（`probes.bump_clock_epoch` へ delegate、doc 付き） |
| `crates/synvelis-app/src/engine.rs` | `SessionRoutingEngine::declare_probe_discontinuity` passthrough 追加 |
| `crates/synvelis-player/src/probe.rs` | `PendingClose` 追加 — drain batch 内の segment close を batch 末尾へ遅延 |
| `crates/synvelis-player/tests/sf04_s8_probe_faults.rs` | 新規 5 test |
| `crates/synvelis-player/tests/sf04_s9_probe_discontinuity_declare.rs` | 新規 5 test |
| `docs/specs/sf-04-slice8-probe-faults-declare.md` | 新規 |
| `docs/designs/sf-04-slice8-probe-faults-declare-design.md` | 新規 |
| `docs/plans/sf-04-slice8-probe-faults-declare-plan.md` | 新規（本書） |
| `docs/tasks/sf-04-slice8-probe-faults-declare-task.md` | 新規 |
| `docs/test-specs/sf-04-slice8-probe-faults-declare.md` | 新規 |
| `docs/impl/ir/sf-04-slice8-probe-faults-declare.md` | 新規 |

変更禁止領域（他 Worker 占有）は spec の Out of Scope を参照。

## Sequence

1. `RoutingEngine::declare_probe_discontinuity` + app
   passthrough 実装 → `cargo check -p synvelis-player
   -p synvelis-app`。
2. `sf04_s8_probe_faults.rs` 実装。fixture 設計中に
   `BudgetExhausted` segment が `Unknown`/`frame_range=None`
   になる drain 順序ギャップを確認 → `probe.rs` に
   `PendingClose` を実装 → s8 test green。
3. `sf04_s9_probe_discontinuity_declare.rs` 実装（s3 clip
   fixture + s7 bridge fixture 再利用）。
4. 既存 `sf04_*` test 群（slice1–7・probe adapter 他）を実行
   し `probe.rs` 変更の非回帰を確認。
5. `docs/` チェーン作成。
6. canonical 検証:
   `mise run fmt-check` → `check` → `lint` → `test` →
   `test-rt` → `check-app-native` → `lint-app-native` →
   `test-msrv` → `build`。
7. `git diff` で許可範囲のみの変更を確認 → Conventional
   Commit → push → 日本語 PR → Issue #107 コメント →
   `worker_done`。

## Risks

- `PendingClose` が既存 segment 期待値（`ended_by`/epoch/
  completeness）を変え得る → s3/s7/s8/s9 + 既存 sf04 test で
  非回帰確認済み（25/25 green）。
- `probe.rs` 変更は許可条件「fixture が実ギャップを露呈」に
  合致 — `BudgetExhausted` の最終 chunk が必ず同 batch の
  status と共に生成される構造が根拠。impl record に明記。
- 他 Worker 並行作業との競合 → 変更は許可 file のみ、
  commit 前に `git status`/`diff` で確認。

## Verification

- targeted: `cargo test -p synvelis-player --test
  sf04_s8_probe_faults`（5/5）、`--test
  sf04_s9_probe_discontinuity_declare`（5/5）、既存
  `sf04_*` 25/25 green。
- canonical: 上記 9 lane 全実行。結果は impl record・PR・
  worker 報告に記録。
- 外部 gate（実機・#109・#108・app op・SF-PROBE-08/09・
  consumer 照合）は NotRun — Pass 扱いしない。
