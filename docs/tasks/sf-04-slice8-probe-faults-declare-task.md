---
id: "TASK-9vK4mP2nQ8wX5zB7jH3fY6c"
type: "task"
status: "in_progress"
title: "SF-04 slice8 タスク — SF-PROBE-05 fault fixtures + 不連続宣言点"
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
  implements:
    - "docs/plans/sf-04-slice8-probe-faults-declare-plan.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-04-slice8-probe-faults-declare-plan.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/sf-04-slice8-probe-faults-declare.md"
  defers: []
  deferred-by: []
---

# SF-04 slice8 タスク — SF-PROBE-05 fault fixtures + 不連続宣言点

## Work Items

- [x] T1 `RoutingEngine::declare_probe_discontinuity` 追加
  （`self.probes.bump_clock_epoch()` delegate、doc 付き）。
- [x] T2 `SessionRoutingEngine::declare_probe_discontinuity`
  passthrough（`crates/synvelis-app/src/engine.rs`）。
- [x] T3 `crates/synvelis-player/tests/
  sf04_s8_probe_faults.rs` 新規 — F1–F5。
- [x] T4 `crates/synvelis-player/tests/
  sf04_s9_probe_discontinuity_declare.rs` 新規 — F6–F10。
- [x] T5 `probe.rs` `PendingClose` — drain batch 内の segment
  close を batch 末尾へ遅延（fixture が露呈した実ギャップの
  修正）。
- [x] T6 既存 `sf04_*` test 非回帰確認（25/25 green）。
- [x] T7 canonical 検証 9 lane 全実行（fmt-check/check/lint/
  test/test-rt/check-app-native/lint-app-native/test-msrv/
  build — 全 green。`test` 初回は環境要因の disk-full
  （os error 112・synvelis-library cache test のみ）で
  失敗したが、容量回復後の再実行で全 pass — コード起因
  ではない）。
- [ ] T8 Conventional Commit → push → 日本語 PR →
  Issue #107 コメント → `worker_done`。

## Notes

- T5 は task spec の許可条件「fixture が実ギャップを露呈した
  場合のみ」に合致 — `BudgetExhausted` は最終 chunk と
  `CaptureStopped` status が必ず同 drain batch で生成される
  ため、telemetry-first 配信順序で close 前 chunk が orphan
  される構造的欠陥。impl record に根拠を記録。
- 外部 gate（実機・#109 Oracle・#108 UI・`syn.probe.compare`
  app op・SF-PROBE-08/09・consumer 照合）は NotRun —
  Pass 扱いしない。
