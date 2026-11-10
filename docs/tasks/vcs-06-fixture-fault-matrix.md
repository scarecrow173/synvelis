---
id: "TASK-vcs06-fixture-fault-matrix"
type: "task"
status: "in-progress"
title: "VCS-06 fixture-level fault matrix + evidence ledger"
created: "2026-11-13"
updated: "2026-11-13"
owners: []
relations:
  source:
    - "SPEC-vcs06-fixture-fault-matrix"
    - "TSPEC-vcs06-fixture-matrix"
  plan: []
  design: []
  supersedes: []
  superseded_by: []
---

# VCS-06 fixture-level fault matrix (synvelis#97)

- [x] 既存 vcs01/02/03/05 fixture の Case マッピング調査
- [x] `vcs06_diff_matrix.rs`（5 tests）
- [x] `vcs06_apply_matrix.rs`（8 tests）
- [x] `vcs06_restore_matrix.rs`（8 tests）
- [x] `vcs06_git_matrix.rs`（3 tests）
- [x] `evidence/reports/vcs-06-fixture-matrix-ledger.md`
- [x] `docs/specs|test-specs|tasks` チェーン
- [ ] `mise run fmt-check/check/lint/test/test-rt` canonical 検証
- [ ] origin/main merge → 再検証 → commit/push/PR

対象外: VCS-UI-01..05、実 GUI(#114)、consumer(#219/#294)、CI(#83)、
他 OS、VCS Writer/3-way Merge、追加 provider。
