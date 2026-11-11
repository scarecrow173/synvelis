---
id: "TASK-4Sfq6WWBaX1avImCs5OcLh"
type: "task"
status: "done"
title: "VCS-06 monitoring-domain matrix"
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
  implements:
    - "docs/plans/vcs-06-monitoring-matrix-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/vcs-06-monitoring-matrix-plan.md"
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
    - "docs/test-specs/vcs-06-monitoring-matrix.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 monitoring-domain matrix

## Work

`crates/synvelis-project/tests/vcs06_monitoring_matrix.rs` を新規
追加し、PR #318 で分類追加された monitor 宣言（
`ObjectKind::TrackMonitorDecl`/`PunchMarks`、
`ComparisonDomain::Recording`）の diff/apply/restore 実挙動を
4 tests で固定する。`src/` 変更なし。
`evidence/reports/vcs-06-fixture-matrix-ledger.md` に
VCS-DIFF-09/10・VCS-APPLY-07・VCS-RESTORE-07 行を実測値で追加し、
Environment ブロックを現在値（`SCHEMA_VERSION = 13`・
merged `origin/main`・現行 ruviform pin）へ更新する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/vcs-06-monitoring-matrix-plan.md`
- `docs/specs/vcs-06-monitoring-matrix.md`
- `docs/designs/vcs-06-monitoring-matrix-design.md`
- `docs/test-specs/vcs-06-monitoring-matrix.md`

## Dependencies

- 先行 slice（monitor persistence + VCS diff monitor 分類）:
  `Project::monitoring` POD・`SetTrackMonitor`/
  `ClearTrackMonitor`/`SetPunchMarks` op・
  `ObjectKind::TrackMonitorDecl`/`PunchMarks` 分類は merge 済み
  （PR #318 / `a218944b`）。
- VCS-06 matrix の既存 helper 契約（`controller`/`snap_live`/
  `compare`/`Src`/`intent_for`/`MapBlobs`/`request`）。

## Affected Files

- `crates/synvelis-project/tests/vcs06_monitoring_matrix.rs`
  （新規）
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`（更新）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  vcs-06-monitoring-matrix*.md`（新規）
- `docs/*/README.md` index（doc tooling による再生成）

## Done When

- [x] `vcs06_monitoring_matrix.rs` 4 tests が外部観測契約を検証
  し pass
- [x] ledger の VCS-DIFF-09/10・VCS-APPLY-07・VCS-RESTORE-07
  行が実測値で記録される
- [x] doc チェーン + index 再生成
- [x] `git fetch origin main && git merge origin/main` 後の
  canonical `mise run` 検証が pass
- [ ] push → PR 作成（Refs synvelis#97、Issue は Open のまま）

## Verification

- [x] `cargo test -p synvelis-project --test
  vcs06_monitoring_matrix`（補助）
- [x] `cargo clippy -p synvelis-project --all-targets --
  -D warnings`（補助）
- [x] `cargo fmt --all -- --check`（補助）
- [x] `mise run fmt-check check lint test test-rt test-msrv`
  （canonical、全 exit 0）
