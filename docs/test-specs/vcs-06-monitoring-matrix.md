---
id: "TSPEC-KoKmM6h8CwFykztjanQvMC"
type: "test-spec"
status: "approved"
title: "VCS-06 monitoring-domain matrix tests"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
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
    - "docs/specs/vcs-06-monitoring-matrix.md"
    - "docs/designs/vcs-06-monitoring-matrix-design.md"
  derived-by: []
  verifies:
    - "docs/specs/vcs-06-monitoring-matrix.md"
    - "docs/designs/vcs-06-monitoring-matrix-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 monitoring-domain matrix tests

## Purpose

PR #318 で分類追加された monitor 宣言（`ObjectKind::
TrackMonitorDecl`/`PunchMarks`、`ComparisonDomain::Recording`）
について、VCS-06 ledger 用の固定 Case ID 付き実測行を
headless fixture で固定する。diff 分類は `Added`/`Removed`/
`Modified` の typed change として、apply は `Unsupported`
fail-closed 実測として、restore は `project.monitoring`
verbatim round-trip として記録する。実 GUI・consumer gate・
VCS Writer/3-way merge は対象外。

## Feature

Monitoring-domain VCS-06 evidence — monitor 宣言の
diff/apply/restore 実挙動を `vcs06_monitoring_matrix.rs` で
観測し `evidence/reports/vcs-06-fixture-matrix-ledger.md` の
Case 行として固定する。

## Rules

- **Rule VCS-DIFF-09 TrackMonitorDecl**: `SetTrackMonitor`
  upsert → `Added`（`stable_id == track.to_tagged()`・owner
  `Track` に重複 change 無し）、宣言 field 変更 → `Modified`
  （`arm`/`monitor`/`channel_map` の field evidence 付き）、
  `ClearTrackMonitor` → `Removed`。`domain() ==
  ComparisonDomain::Recording`。diff honesty: AA（同一
  monitoring）= `Equal`・0 changes、Recording 除外 spec で
  monitor-only 差分 = `Equal`・Recording のみ spec = `Different`、
  異 `project_id` = `Unknown` + `AmbiguousIdentity{CrossProject}`。
- **Rule VCS-DIFF-10 PunchMarks**: `{None,None}`→宣言 = `Added`
  + `affected_range = SemanticTimeRange::Musical [in,out)`、
  宣言→宣言（mark 変更）= `Modified` + `punch_in` field
  evidence、宣言→`{None,None}` = `Removed` + 消えた宣言の
  covered span。partial 宣言（片側 mark のみ）= `Added` +
  `affected_range == None`、inverted 宣言（`out < in`）=
  `Modified` + `affected_range == None` — 無効 range を捏造
  しない。`domain() == ComparisonDomain::Recording`。
- **Rule VCS-APPLY-07 monitoring kind fail-closed**: monitoring
  kind を含む selection で `target_fingerprint` と
  `apply_selected` の両経路が `ApplyError::Unsupported{kind,
  change: Modified, "object kind outside the apply surface"}`
  を返し、live `project.monitoring`・undo・journal（`Failed`
  記録）が不変であることを実測する。対応と偽装しない。
- **Rule VCS-RESTORE-07 monitoring verbatim**: `restore_as_copy`
  の publish bundle（`snapshots/<HEAD>` の `SnapshotEnvelope`）
  で `project.monitoring` が source project と verbatim 一致
  （`PartialEq` — `monitoring.tracks` + `monitoring.punch` の
  全 field）。fork `project_id` のみ差分。

## Examples

- `track_monitor_decl_add_modify_clear_are_typed_changes`
- `punch_marks_presence_transitions_are_typed_changes`
- `monitor_kind_selection_fails_apply_closed`
- `restore_as_copy_preserves_monitoring_verbatim`

## Guarantees

- monitor 宣言の add/modify/clear が ledger Case ID 粒度で
  typed change として独立観測される（`Unknown` を `Equal`
  にしない方針と整合 — monitor-only 差分は `Different`）。
- apply の `Unsupported` は実測 fail-closed として記録され、
  「monitoring kind は apply surface 外」という現行契約を
  変えないまま ledger が実態を反映する。
- restore の verbatim round-trip は fork identity 差分のみ
  （`project_id`）で `monitoring` の field-level 完全一致を
  検証する。

## Non-goals

- monitoring kind の apply surface 追加 — 別 slice。
- `vcs02_diff.rs` monitor 分類 fixture の重複（本 file は
  ledger 用の Case ID 粒度実測 — diff 規約の一次証拠は
  vcs02 に委譲）。
- `Removed` change の apply（全 kind で cascade design deferred —
  monitoring 固有ではない）。
- 実 device monitor / session 観測値 / RT graph — 非永続・外部
  gate。

## Risk

- apply 実測点が fingerprint CAS 段（`plan_changes` より前）で
  `change: Modified` を報告する — assert がこの実測を偽装
  しないよう variant・kind・detail を個別に検証する。
- monitor-only changeset が `Complete`+`Different` を満たさない
  と `NotApplicable` で先に失敗し `Unsupported` 経路に届かない
  — fixture を asset/processor 無しで構成する。
