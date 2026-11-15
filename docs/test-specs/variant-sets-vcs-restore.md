---
id: "TSPEC-034XI5DgSYUwdugxxX2rmz"
type: "test-spec"
status: "approved"
title: "variant_sets restore-as-copy verbatim provenance fixture"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/458"
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
  related:
    - "docs/specs/vcs-06-variant-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/variant-sets-vcs-restore.md"
  derived-by: []
  verifies:
    - "docs/specs/variant-sets-vcs-restore.md"
    - "docs/designs/variant-sets-vcs-restore.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata: {}
---

# variant_sets restore-as-copy verbatim provenance fixture

## Purpose

synvelis#97 が `Fail` として正直 pin した `restore_as_copy` の
variant origin identity defect（`docs/discovery/variant-sets-
vcs-diff-gap.md` #458 節・ledger `VCS-RESTORE-10`）が
synvelis#458 で閉じたことを、headless fixture で恒久的に保証する。
persisted 境界の provenance 規則を退行させる変更（same-project
検査の persisted への再適用、nested identity の rewrite、
`stable_id_counts` の variant entry 除去）は本 spec の失敗とし
て検出する。

## Feature

`restore_as_copy` は `variant_sets` を含む project を新しい
`project_id` の独立 copy として publish し、`VariantSet`/
`Branch` の stable ID・revision・`BranchContentRef`・
`last_adopt`・`origin`/`created_from` provenance を verbatim
保持する。persisted/load oracle（`validate_persisted`）は
restore 産 project の cross-project provenance を受理し、
live op 境界（`VariantOp::Fork::validate`）は same-project
不変条件を維持する。

## Rules

- restore は `RestoreOutcome::Published` を返し、destination
  bundle（HEAD → snapshot envelope）を publish する — publish
  前 `RestoreError::Validation` で fail-closed しない。
- published envelope の `project.variant_sets` は source project
  の `variant_sets` と **verbatim 一致**（map key の
  `VariantSetId`、nested `BranchId`、`VariantSetRevision`/
  `BranchRevision`/`BranchContentRef`/`BranchLineage`/
  `last_adopt`/`origin_objects`/`scope`/`adopted` 全 field）。
- `VariantSet::origin`・全 `Branch::created_from` の
  `SnapshotIdentity.project_id` は **source project を指し続ける**
  — provenance は rewrite されない（`project_id` のみの再採番は
  top-level `Project::project_id` に限る）。
- `envelope.project_id`・`envelope.project.project_id` は
  `req.new_project_id` に一致し、source とは異なる。
- `receipt.stable_ids.counts` は `"variant-set"`・`"branch"`
  entry を持ち、source の entity 数と一致する。
- `envelope.project.validate_references()`（load oracle）は
  restore 産 project で `Ok` — open/load で拒否されない。
- live `VariantOp::Fork` は `origin`/`created_from` の
  same-project 不変条件を維持する（`SameProject` 規則）。
- `variant_sets` 空 project の restore・apply surface の
  fail-closed pin は不変。

## Examples

- Given track + forked variant set（adopted candidate 済み・
  `last_adopt` record あり）の live project、When
  `restore_as_copy` を別 `new_project_id` で実行、Then receipt
  は `Published`、destination の envelope readback で
  `variant_sets` が source と verbatim 一致、`project_id` のみ
  `new_project_id`、`origin`/`created_from` は source
  `project_id` を指し続け、`stable_ids.counts["variant-set"]==1`・
  `["branch"]==2`。
- Given persisted project file が foreign provenance の
  variant set を持つ（restore 産相当）、When load oracle
  （`validate_references` → `validate_persisted`）を実行、Then
  provenance `project_id` 不一致では拒否されない（構造不変条件
  違反は従来どおり拒否）。
- Given live project、When `VariantOp::Fork` が same-project で
  ない `origin` を持つ set を挿入しようとする、Then
  `SameProject` 規則で拒否される（live 境界の不変条件維持）。

## Guarantees

- silent fail-closed（`RestoreError::Validation` での publish
  前拒否）への回帰がない。
- provenance の rewrite（nested `SnapshotIdentity.project_id`
  の再採番）を行わない — authored provenance の毀損なし。
- live op 境界の same-project 不変条件が維持される。
- `variant_sets` 空 project・既存 VCS-05 restore 契約
  （staging/resume/fail-closed path）・apply fail-closed pin
  （`variant_changes_stay_fail_closed_on_apply_surface`）は
  非回帰。

## Non-goals

- `BranchContentStore` payload bytes の fork 先への複製 —
  restore 産 project の `Captured` branch は
  `CandidateUnavailable` になりうる既知の capability 限界
  （別設計・#457 Deferred）。
- apply surface の variant transplant（別 Issue）。
- variant diff adapter の変更（#457 scope・確定済み）。
- real GUI/OS/consumer lane。

## Risk

保証を落とすと (1) variant 利用 project の restore-as-copy が
再び全滅する capability gap が再発し、(2) provenance rewrite
を行う誤修復が authored provenance を静かに毀損し、(3)
`stable_id_counts` の欠落が receipt の verbatim 証拠を
不完全にする。ledger `VCS-RESTORE-10` が再び `Fail`/誤 `Pass`
に戻る。
