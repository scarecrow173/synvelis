---
id: "SPEC-034XI2qQNGjpsbqpLe06vQ"
type: "spec"
status: "approved"
title: "variant_sets restore-as-copy origin identity 修復 — persisted 境界の provenance 規則 (synvelis#458)"
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
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
    - "docs/specs/creative-branch.md"
    - "docs/specs/vcs-05-apply-restore-headless.md"
    - "docs/specs/vcs-06-variant-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
  derived-by:
    - "docs/designs/variant-sets-vcs-restore.md"
  verifies: []
  verified-by:
    - "docs/test-specs/variant-sets-vcs-restore.md"
  references: []
  defers: []
  deferred-by: []
metadata: {}
---

# variant_sets restore-as-copy origin identity 修復 — persisted 境界の provenance 規則 (synvelis#458)

## Context

synvelis#97（VCS-06 fixture matrix）が `vcs06_variant_matrix.rs` の
`restore_as_copy_fails_closed_on_variant_set_origin_mismatch` で
実測 Fail として pin した defect: `variant_sets` を含む project の
`restore_as_copy` は、publish 前の staged-envelope
`validate_references` → `validate_persisted` → `validate_set_shape`
が「variant origin の `project_id` は enclosing project と一致すべ
き」不変条件に違反するとして `RestoreError::Validation` で
fail-closed する。`restore::fork_project` は
`snapshot.projection.project().clone()` に `project_id` の再採番
のみを行い、nested `VariantSet::origin`・全 `Branch::created_from`
の `SnapshotIdentity` を書き換えないため、schema v14 で variant
set を持つ project は restore-as-copy が一切成功しない
capability gap である（silent corruption はない — staging 保持・
destination 不在・source 不変の fail-closed）。

`docs/discovery/variant-sets-vcs-diff-gap.md` の設計論点は、修復が
(a) `fork_project` で nested `SnapshotIdentity` を再帰再採番するか、
(b) `validate_set_shape` の same-project 規則を persisted 境界で
見直すかの二択であり、`SnapshotIdentity` が「どの snapshot bytes
を指すか」の provenance 参照（`digest` は source snapshot bytes、
`locator` は variant により `project_id` を内包）である以上、
`project_id` のみの書き換えは存在しない snapshot への虚偽参照を
捏造し authored provenance を毀損する。本 spec は (b) —
persisted/load oracle での規則見直し・provenance verbatim 維持 —
を要件として固定する。設計判断の確定は
`docs/designs/variant-sets-vcs-restore.md` が担う。

## Requirements

- `variant_sets` を含む project の `restore_as_copy` は
  `RestoreOutcome::Published` で成功し、destination bundle の
  envelope が publish される — silent fail-closed を解消する。
- persisted/load oracle（`Project::validate_references` →
  `variant::engine::validate_persisted`）は、`VariantSet::origin`・
  `Branch::created_from` の `SnapshotIdentity.project_id` が
  enclosing `project.project_id` と異なることを**拒否しない**。
  これらは authored provenance であり、restore-as-copy 産の
  project では source project を指し続けるのが正しい。
- live op 境界（`VariantOp::Fork::validate`）では same-project
  要件を維持する — live fork は by-construction で live identity
  を刻むため、規則は冗長な防御として残すか、根拠を明記して落とす
  かを design で確定する。
- restore 成功時、issued `VariantSetId`/`BranchId`・branch
  revision・`BranchContentRef`・`last_adopt`・`origin`/
  `created_from` は **verbatim 保持**され、再採番されるのは
  top-level `project_id` のみであることが envelope readback で
  検証される。
- `stable_id_counts`（`StableIdReport`・policy `"preserve"`）は
  `variant-set`・`branch` の entity count を報告する — 現状これらを
  欠落しており verbatim 検証の receipt 証拠にならない。
- `variant_sets` が空の project の restore は非回帰。
- `src/diff/`（apply fail-closed pin `variant_changes_stay_
  fail_closed_on_apply_surface`・`supported_kind`）・`src/apply/`
  （variant transplant）・`BranchContentStore` の fork 先への
  bytes 複製は変更しない — いずれも別 Issue/別設計の scope。

## Acceptance Criteria

- `variant_sets` を含む project の `restore_as_copy` が
  `Published` を返し、publish 前 fail-closed の
  `RestoreError::Validation`（`origin belongs to a different
  project`）が解消されることが test で検証される。
- envelope readback で、復元 project の `variant_sets` が source
  と verbatim 一致（`VariantSetId`/`BranchId`・revision・
  `BranchContentRef`・`last_adopt`・`origin`/`created_from` の
  `project_id` を含む）し、`project_id` のみ `new_project_id` に
  再採番されていることが assert される。
- persisted project が load oracle（`validate_references`）を
  通過する — restore 産 project が open/load で拒否されない。
- `variant_sets` 空 project の restore・既存 VCS-05 契約
  （staging/resume/fail-closed path）が非回帰。
- 修復方針（nested identity 再採番 vs validate 規則見直し）が
  design doc で確定し、CB-01 の origin 意味論
  （`docs/specs/creative-branch.md` CB-R-01-2）と整合する。
- `BranchContentRef::Captured.payload_digest` の payload bytes は
  `BranchContentStore` 外であり、restore 産 project の candidate
  branch は `decode_candidate` で `CandidateUnavailable` になり
  うる既知の capability 限界を ledger/証拠に正直記録する
  （本 slice で修復しない）。

## Sources and Relations

- `docs/discovery/variant-sets-vcs-diff-gap.md` — gap 実測と
  `SnapshotIdentity` 意味論の設計論点（#458 節）。
- `docs/specs/creative-branch.md`（CB-01・#278）— `origin`/
  `created_from` の `SnapshotIdentity` 再利用契約（CB-R-01-2）。
- `docs/designs/creative-branch-design.md` — `origin` は fork
  origin の fixed snapshot identity という provenance 契約。
- synvelis#97 VCS-06 ledger `VCS-RESTORE-10` `Fail` 行 — 実測 pin。
- synvelis#457 — variant diff 分類（`created_from`/`content` は
  `Replaced` field class として provenance 観測済み）。
