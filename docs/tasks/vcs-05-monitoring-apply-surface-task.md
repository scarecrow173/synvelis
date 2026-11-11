---
id: "TASK-vcs05-monitoring-apply-surface"
type: "task"
status: "done"
title: "VCS-05 monitoring kinds apply surface 拡張 task"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "ISSUE-96"
    - "PLAN-vcs05-monitoring-apply-surface"
  verified_by:
    - "TSPEC-vcs05-monitoring-apply-surface"
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-05 monitoring kinds apply surface 拡張 task

## Tasks

- [x] T1 `apply/entity.rs`: `supported_kind` に
  `TrackMonitorDecl`/`PunchMarks` 追加、`entity_value` の
  keyed-synthetic view + declaration-presence view、`write_entity`
  の独自 arm（`keyed_insert!` 非使用）。
- [x] T2 `apply/mod.rs`: module doc の scope-out 文言更新。
- [x] T3 `tests/vcs05_apply.rs`: monitoring Added apply +
  1 undo step fixture 追加。
- [x] T4 `tests/vcs06_monitoring_matrix.rs`: VCS-APPLY-07 を
  supported-apply（Added/Modified）+ Removed fail-closed の
  実測へ再構成。
- [x] T5 `evidence/reports/vcs-06-fixture-matrix-ledger.md`:
  VCS-APPLY-07 行 + Environment ブロック更新。
- [x] T6 doc chain 6 文書作成。
- [x] T7 `origin/main` merge + `mise run` canonical 検証。

## Deferred（Issue へ引き継ぎ）

- `Removed` change の cascade apply（全 kind）→ 別設計
- `PluginInstance` の apply surface → #324 系の別 slice
- recording/comping/SMF/clock-epoch 領域の apply → scope-out
- 実 OS FS durability / 実 Provider / plugin host / GUI / RT /
  cross-consumer → #97/#294
