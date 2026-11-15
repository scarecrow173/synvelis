---
id: "PLAN-034XI5CIhuhflxLfURQcJq"
type: "plan"
status: "approved"
title: "variant_sets restore-as-copy origin identity 修復 slice"
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
  implements:
    - "docs/designs/variant-sets-vcs-restore.md"
  implemented-by:
    - "docs/tasks/variant-sets-vcs-restore.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/variant-sets-vcs-restore.md"
    - "docs/plans/variant-sets-vcs-diff.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/variant-sets-vcs-restore.md"
  derived-by:
    - "docs/tasks/variant-sets-vcs-restore.md"
  verifies: []
  verified-by:
    - "docs/test-specs/variant-sets-vcs-restore.md"
  references: []
  defers: []
  deferred-by: []
metadata: {}
---

# variant_sets restore-as-copy origin identity 修復 slice Implementation Plan

## Goal

`restore_as_copy` が `variant_sets` を含む project を publish 前
validation で fail-closed する defect（synvelis#458・VCS-
RESTORE-10 `Fail`）を、persisted 境界の provenance 規則見直しで
修復し、variant set を verbatim 保持した copy を publish する。

## Scope

単一 implementation stream。design
`variant-sets-vcs-restore`（D1–D6）の決定をそのまま実装する。
`src/diff/`・`src/apply/`・`BranchContentStore` 経路は非変更。

## Upstream Documents

- `docs/specs/variant-sets-vcs-restore.md`
- `docs/designs/variant-sets-vcs-restore.md`
- `docs/discovery/variant-sets-vcs-diff-gap.md`（#458 節）

## Design Inputs

- `docs/designs/variant-sets-vcs-restore.md`（approved・D1–D6）

## File Map

| file | 変更 |
| --- | --- |
| `crates/synvelis-project/src/variant/engine.rs` | `ProvenanceRule` enum 追加、`validate_set_shape` に規則引数、`VariantOp::Fork::validate` → `SameProject`、`validate_persisted` → `Persisted` |
| `crates/synvelis-project/src/restore/mod.rs` | `stable_id_counts` に `variant-set`/`branch` 追加（`fork_project` は不変） |
| `crates/synvelis-project/tests/vcs06_variant_matrix.rs` | restore pin flip: `..._fails_closed_on_variant_set_origin_mismatch` → `restore_as_copy_preserves_variant_sets_verbatim`（Published + envelope readback） |
| `evidence/reports/vcs-06-fixture-matrix-ledger.md` | VCS-RESTORE-10 `Fail`→`Pass` flip、env 記録、`BranchContentStore` capability 限界の追記 |
| `evidence/sv458/` | verification evidence（新規） |

## Implementation Sequence

- [x] `engine.rs`: `ProvenanceRule` enum + `validate_set_shape`
  規則引数 + 2 call site 更新
- [x] `restore/mod.rs`: `stable_id_counts` 拡張
- [x] `vcs06_variant_matrix.rs`: test flip（red→green 確認）
- [x] ledger `VCS-RESTORE-10` flip + env 更新
- [x] `evidence/sv458/` verification evidence
- [x] docs index 再生成（doc-maintenance）

## Dependencies and Risks

- cross-crate `validate_references` consumer
  （`synvelis-render`/`synvelis-gateway`/`synvelis-workspace`）—
  規則緩和は受理拡大方向のみ。`check-full` で非回帰確認。
- `BranchContentRef::Captured` payload bytes は `BranchContentStore`
  外 — restore 産 project の candidate branch は
  `CandidateUnavailable` になりうる既知限界（修復しない・
  ledger に記録）。
- `VariantOp` は `pub(crate)` — `ProvenanceRule` も `pub(crate)`
  に留め public API を増やさない。

## Task Breakdown

- 単一 task（`docs/tasks/variant-sets-vcs-restore.md`）— 変更面が
  小さく依存が直列のため分割しない。

## Verification

| 検証 | コマンド |
| --- | --- |
| fmt | `mise run fmt-check` |
| check | `mise run check synvelis-project` |
| lint | `mise run lint synvelis-project` |
| test | `mise run test synvelis-project` |
| build | `mise run build synvelis-project` |
| RT | `mise run test-rt` |
| cross-crate | `mise run check-full` |
| hooks | `mise exec -- prek run --all-files` |

`*-msrv` 系・`*-full` 系（`check-full` 以外）は Daily Full CI
委譲で NotRun。

## Open Questions / Gaps

- なし — design D1–D6 で全論点確定済み。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（dispatched worker・単一
  stream）
- Independent work streams: none
- User approval required before dispatch: review-protocol blocker
  0 を承認として扱う（共通契約）

## Review Handoff

- reviewer focus: `validate_set_shape` の規則引数が live op 境界
  （`SameProject`）と persisted oracle（`Persisted`）で正しく
  使い分けられているか、`stable_id_counts` の key 規約、pin
  flip の assert が VCS-RESTORE-10 期待形（verbatim +
  `project_id` のみ再採番 + provenance source 指し継続）を
  過不足なく覆うか。
