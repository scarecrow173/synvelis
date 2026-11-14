---
id: "TASK-034XF53zzlHnxfrP66Ueaf"
type: "task"
status: "done"
title: "VCS-06 variant-domain matrix"
created: "2026-11-14"
updated: "2026-11-15"
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
    - "docs/plans/vcs-06-variant-matrix-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/vcs-06-variant-matrix-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-06-variant-matrix-plan.md"
    - "docs/specs/vcs-06-variant-matrix.md"
    - "docs/designs/vcs-06-variant-matrix-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-06-variant-matrix.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 variant-domain matrix

## Work

`crates/synvelis-project/tests/vcs06_variant_matrix.rs` を新規
追加し、schema v14 `Project::variant_sets`（sv#278 CB-01）の
diff/restore 実挙動を 3 tests で固定する。`src/` 変更なし・
正直記録型 — silent `Equal`/silent drop/restore fail-closed を
観測値のまま pin する。
`evidence/reports/vcs-06-fixture-matrix-ledger.md` に
VCS-DIFF-11（`Blocked`）・VCS-APPLY-10（`Blocked`）・
VCS-RESTORE-10（`Fail`）行を実測値で追加し、Environment
ブロックを現在値（`SCHEMA_VERSION = 14`・merged `origin/main`・
現行 ruviform pin）へ更新する。
`docs/discovery/variant-sets-vcs-diff-gap.md` に gap 記録を
作成し、follow-up Issue #457（variant diff adapter）・
#458（restore defect）を起票する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/vcs-06-variant-matrix-plan.md`
- `docs/specs/vcs-06-variant-matrix.md`
- `docs/designs/vcs-06-variant-matrix-design.md`
- `docs/test-specs/vcs-06-variant-matrix.md`
- `docs/discovery/variant-sets-vcs-diff-gap.md`

## Dependencies

- 先行 slice（CB-01 variant model + VCS-06 matrix）:
  `Project::variant_sets`・`fork_variant_set`/`adopt_branch`・
  `FakeDomainAdapter`/`InMemoryBranchContentStore`・
  `Vcs06_*` matrix helper 契約は merge 済み（sv#278・
  `vcs06_monitoring_matrix.rs`/`vcs06_plugin_instance_matrix.rs`）。
- `fork_project` の `project_id` のみ再採番・
  `validate_set_shape` の origin `project_id` 不変条件 —
  restore fail-closed の実測根拠。

## Affected Files

- `crates/synvelis-project/tests/vcs06_variant_matrix.rs`
  （新規）
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`（更新）
- `docs/discovery/variant-sets-vcs-diff-gap.md`（新規）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  vcs-06-variant-matrix*.md`（新規）
- `docs/*/README.md` index（doc tooling による再生成）

## Done When

- [x] `vcs06_variant_matrix.rs` 3 tests が外部観測契約を検証
  し pass（`Equal`/`Complete` pin・mixed silent drop pin・
  restore `Validation` fail-closed）
- [x] ledger の VCS-DIFF-11（`Blocked`）・VCS-APPLY-10
  （`Blocked`）・VCS-RESTORE-10（`Fail`）行が実測値で記録
  される
- [x] `docs/discovery/variant-sets-vcs-diff-gap.md` gap 記録 +
  doc チェーン + index 再生成
- [x] follow-up Issue #457（variant diff adapter）・
  #458（restore defect）起票
- [x] `git fetch origin main && git merge origin/main` 後の
  canonical `mise run` 検証が pass（`test-msrv` は Daily CI
  委譲で NotRun）
- [x] push → PR 作成（Refs synvelis#97、Issue は Open のまま —
  scarecrow173/synvelis#460）

## Verification

- [x] `cargo test -p synvelis-project --test
  vcs06_variant_matrix`（補助）— 3 pass
- [x] `cargo clippy -p synvelis-project --all-targets --
  -D warnings`（補助）— clean
- [x] `cargo fmt --all -- --check`（補助）— clean
- [x] `mise run fmt-check check lint test test-rt build`
  （canonical、全 exit 0。`test-msrv` は Daily CI 委譲で
  NotRun — impl-ir に記録）
