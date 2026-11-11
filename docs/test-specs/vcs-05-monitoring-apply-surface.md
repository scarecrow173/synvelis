---
id: "TSPEC-vcs05-monitoring-apply-surface"
type: "test-spec"
status: "approved"
title: "VCS-05 monitoring kinds apply surface fixtures"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "SPEC-vcs05-monitoring-apply-surface"
    - "DSGN-vcs05-monitoring-apply-surface"
  verifies:
    - "TASK-vcs05-monitoring-apply-surface"
  supersedes: []
  superseded_by: []
---

# VCS-05 monitoring kinds apply surface fixtures

## Feature: monitoring kinds on the apply surface

`ObjectKind::TrackMonitorDecl`/`PunchMarks` の `Added`/
`Modified` change が `apply_selected` の typed entity transplant
として適用され、`Removed` は fail-closed を維持することを
headless fixture で固定する。実 OS FS・実 Provider/plugin・
GUI/RT は対象外（NotRun）。

## Rules

- **T1 Added selection applies typed entities**: monitor 宣言を
  持たない before と宣言を持つ after の `Complete`/`Different`
  changeset で、`TrackMonitorDecl Added` + `PunchMarks Added`
  の 2 change selection が `ApplyOutcome::Applied` で commit
  され、live `project.monitoring.tracks[track]` が after の decl
  と一致、`monitoring.punch` が after の marks と一致する。
  1 apply = 1 undo step で undo は monitoring-empty 状態へ、
  redo は宣言を復元する。`target_fingerprint` は monitoring
  kind で解決する（`Unsupported` を返さない）。
  Example: `monitor_kind_added_selection_applies_typed_entities`
  （vcs06）・`monitor_decl_and_punch_apply_as_one_undo_step`
  （vcs05）。
- **T2 Modified selection applies typed entities**: S1（宣言
  あり）→ S2（field/mark 編集）の `Modified` selection が適用
  され、decl の `arm`/`monitor`/`channel_map` と punch mark が
  after 値になる。連続 apply の undo が 1 step ずつ S1 → S0
  monitoring-empty へ戻る。
  Example: `monitor_kind_modified_selection_applies_typed_entities`。
- **T3 Removed stays fail-closed**: 宣言済み live（S1）に対する
  S1→S0 の `Removed` selection は `ApplyError::Unsupported
  {kind, change: Removed, "removal changes are deferred (cascade
  design)"}` で拒否され、live `project.monitoring` と journal
  `Failed` record が honest に保たれる。`PunchMarks`・
  `TrackMonitorDecl` の両 kind で同一 verdict。
  Example: `monitor_kind_removed_selection_stays_fail_closed`。
- **T4 presence semantics mirror the classifier**: `PunchMarks`
  の entity 存在は `punch_in`/`punch_out` いずれか `Some` —
  全 `None` は absent（fingerprint `null`）。`TrackMonitorDecl`
  の entity 存在は `monitoring.tracks` の key 存在で、`stable_id`
  は `TrackId` tagged 形式を借用する（diff の keyed-synthetic
  規約と一致）。

## Guarantees

- 失敗経路（Unsupported/plan reject）で live `project.monitoring`・
  `live_revision`・undo 履歴・journal state が改竄されない。
- apply は `ProjectController` 単一 writer 経由で 1 undo step —
  receipt の `effects.single_undo_step`/`untouched.*` が真を
  報告する。
- monitoring kind を含まない既存 selection の apply 挙動・
  fingerprint・plan は不変（`vcs05_apply.rs` 既存 13 fixture が
  非回帰で pass）。

## Non-goals

- `Removed` の cascade apply（全 kind deferred）。
- `PluginInstance` の apply（#324 系の別 slice）。
- session 観測値・実 device monitor・実 GUI — 非永続/外部 gate。
