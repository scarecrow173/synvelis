---
id: "IMPL-034XIhKQqkDOu2aMdhyjkE"
type: "impl"
status: "completed"
title: "variant_sets VCS restore-as-copy provenance 修復 (synvelis#458)"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/458"
  changes:
    added:
      - type: "file"
        path: "docs/specs/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/designs/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/plans/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/tasks/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/test-specs/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/impl/ir/variant-sets-vcs-restore.md"
      - type: "file"
        path: "docs/impl/exp/variant-sets-vcs-restore.md"
      - type: "file"
        path: "evidence/sv458/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/variant/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/src/restore/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_variant_matrix.rs"
      - type: "file"
        path: "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/variant-sets-vcs-restore.md"
    - "docs/plans/variant-sets-vcs-restore.md"
    - "docs/designs/variant-sets-vcs-restore.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/variant-sets-vcs-restore.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
    - "docs/impl/ir/variant-sets-vcs-diff.md"
    - "docs/impl/ir/cb-01-variant-set.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/variant-sets-vcs-restore.md"
    - "docs/plans/variant-sets-vcs-restore.md"
  derived-by: []
  verifies:
    - "docs/test-specs/variant-sets-vcs-restore.md"
  verified-by:
    - "docs/test-specs/variant-sets-vcs-restore.md"
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/457"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# variant_sets VCS restore-as-copy provenance 修復 (synvelis#458)

## Summary

Issue #458（VCS-05 restore umbrella）の実装 slice として、
`variant_sets` を含む project の `restore_as_copy` が staged
envelope 検証で `RestoreError::Validation` fail-closed していた
defect を修復した。`fork_project` が top-level `project_id`
のみ再採番し nested `VariantSet::origin`/`Branch::created_from`
の `SnapshotIdentity` を verbatim 保持するのは CB-01 の authored
provenance 意味論どおり正しいため、`validate_set_shape` を
`ProvenanceRule`（`SameProject`/`Persisted`）で境界分割した。
live op 境界（`VariantOp::Fork::validate`）は same-project を
維持し、persisted/load oracle（`validate_persisted` →
`validate_references`）は source-project provenance を受理する。
あわせて `stable_id_counts` に `variant-set`/`branch` を追加し、
VCS-RESTORE-10 pin を `Published` + envelope verbatim readback
の成功系へ flip した。

## Context

synvelis#97（VCS-06 fixture matrix）の ledger が
`VCS-RESTORE-10 Fail` として実測 pin した defect：
`fork_project`（`restore/mod.rs`）は
`snapshot.projection.project().clone()` + `project_id` 再採番のみ
で、nested provenance の `SnapshotIdentity.project_id` を書き
換えない。`validate_staged` → `Project::validate_references` →
`variant::engine::validate_persisted` → `validate_set_shape` が
`origin.project_id == project.project_id` /
`created_from.project_id == project.project_id` を要求し、
restore が publish 前に fail-closed していた。
`SnapshotIdentity` は `{project_id, source, locator, digest, ...}`
で digest/locator が source snapshot bytes を指す authored
参照であり、`project_id` だけを書き換えると存在しない snapshot
への参照を捏造して provenance を毀損する。設計判断は
`docs/designs/variant-sets-vcs-restore.md`（D1–D4）に確定済み。

## Implementation

### Changes

- `variant/engine.rs`: `pub(crate) enum ProvenanceRule {
  SameProject, Persisted }` を追加。`validate_set_shape` が
  `provenance` 引数を取り、`created_from.project_id` /
  `set.origin.project_id` の same-project 検査のみ
  `SameProject` 時に限定（構造検査は両 rule で維持）。
  `VariantOp::Fork::validate` → `SameProject`、
  `validate_persisted` → `Persisted`。
- `restore/mod.rs`: `stable_id_counts` に `variant-set`（set 数）
  と `branch`（全 set の branch 合計）を追加。`fork_project`
  は変更なし（provenance verbatim が設計意図）。
- `tests/vcs06_variant_matrix.rs`: VCS-RESTORE-10 を
  `restore_as_copy_preserves_variant_sets_verbatim` へ flip —
  `RestoreOutcome::Published`・stable receipt の
  `variant-set`/`branch` count・HEAD→snapshot→envelope
  readback で `variant_sets` map 全体の verbatim 等価、
  `last_adopt`/`origin`/`created_from` が source project を
  指すこと、`envelope.project.validate_references()` 成功、
  source project 非破壊を assert。
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`:
  `VCS-RESTORE-10` Fail→Pass flip、新 test 名・Env block・
  BranchContentStore capability 限界の正直記録。
- `evidence/sv458/verification.md`: 実測検証記録。

### Decisions During Implementation

- 設計 (a) nested identity 再採番は棄却 — `SnapshotIdentity` の
  `project_id` のみ書き換えると digest/locator が指す source
  snapshot との整合が崩れ、authored provenance を falsify する
  （design D1/D2、CB-01 origin 意味論と整合）。
- 設計 (b) persisted 境界の規則見直しを採用 — live op と
  load oracle で provenance 要件を分離する `ProvenanceRule`
  を導入（design D3）。`VariantOp::Fork` は live identity を
  by-construction で刻むため `SameProject` で従来どおり strict。
- `stable_id_counts` の `variant-set`/`branch` 追加は receipt
  の verbatim 検証に必要な最小範囲（Issue AC の readback 検証と
  セット）。

## Related Experiments

### Adopted

`docs/impl/exp/variant-sets-vcs-restore.md` — nested 再採番 vs
validate 規則見直しの 2 案比較と CB-01 origin 意味論の検証
メモ。結論は design D1–D4 と一致したため採用。

### Rejected

なし（exp 自体が採用済み設計の裏付け）。

## Validation

- `mise exec -- cargo test -p synvelis-project --test
  vcs06_variant_matrix` — 3 tests pass（新 pin 含む）。
- `mise run test synvelis-project` — 全 test pass
  （`variant_changes_stay_fail_closed_on_apply_surface` 等の
  非回帰 pin 含む）。
- canonical 検証（`mise run fmt-check`/`check`/`lint`/`test`/
  `test-rt`/`build`/`check-full`/`prek`）は PR 本文の検証表に
  実測値を記録。`evidence/sv458/verification.md` に証跡。

## Risks

- `BranchContentRef::Captured.payload_digest` の payload bytes は
  `BranchContentStore`（project bundle 外）にあり、restore は
  fork 先へ bytes を複製しない — 復元 project で candidate
  操作を行うと `CandidateUnavailable` になりうる既知の
  capability 限界（#457 Deferred・本 slice 範囲外、ledger に
  正直記録）。
- persisted 境界の provenance 緩和は load oracle 全体に効く —
  `validate_persisted` の構造検査（revision・scope・pairwise
  conflict 等）は維持されるが、foreign provenance を持つ他の
  persisted 経路（将来の import 等）でも受理される。設計上
  provenance は write-only 参照であり意図どおり。

## Follow-ups

- `BranchContentStore` の fork 先への payload bytes 複製・
  apply surface の variant transplant（#457 Deferred・別 Issue）。
- variant diff の apply 対応（`VCS-APPLY-10` は `Blocked` の
  まま）。
