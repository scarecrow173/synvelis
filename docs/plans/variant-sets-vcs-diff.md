---
id: "PLAN-034XGQcU2c2ctkSQ1qVvWi"
type: "plan"
status: "approved"
title: "variant_sets VCS semantic diff 分類 slice"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/457"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/variant-sets-vcs-diff.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/variant-sets-vcs-diff.md"
    - "docs/plans/plugin-instances-vcs-diff-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/variant-sets-vcs-diff.md"
  derived-by:
    - "docs/tasks/variant-sets-vcs-diff.md"
  verifies: []
  verified-by:
    - "docs/test-specs/variant-sets-vcs-diff.md"
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/458"
  deferred-by: []
metadata: {}
---

# variant_sets VCS semantic diff 分類 slice Implementation Plan

## Goal

`Project::variant_sets`（schema v14）を VCS-02 semantic diff の
compare surface に載せ、variant-only 差分の silent `Equal`/
`Complete` と mixed diff の silent drop を typed change で
閉じる（synvelis#457）。

## Scope

単一 implementation stream。design
`variant-sets-vcs-diff`（D1–D9）の決定をそのまま実装する。

## File map

| file | 変更 |
| --- | --- |
| `crates/synvelis-project/src/diff/types.rs` | `ComparisonDomain::Variants` 末尾 append、`ComparisonDomains::all()`、`ObjectKind::VariantSet`/`VariantBranch` 末尾 append、`domain()`、`DIFF_POLICY_VERSION` 3→4 |
| `crates/synvelis-project/src/diff/engine.rs` | `impl_entity_key!` に `VariantSetId`/`BranchId`、`raw_kind_map` に variant set/branch 登録 |
| `crates/synvelis-project/src/diff/adapters.rs` | `classify_variants`、`VariantsAdapter`（set `diff_map` + branch nested-global diff）、`default_adapters` 登録、`classify_field` domain arm |
| `crates/synvelis-project/src/render_input.rs` | `VariantSet` keyed / `VariantBranch` nested-special の canonical value 解決（inventory 非搭載 — D6） |
| `crates/synvelis-project/tests/vcs07_variant_diff.rs` | 新規 7 test |
| `crates/synvelis-project/tests/vcs06_variant_matrix.rs` | 2 diff pin flip（restore test untouched） |
| `crates/synvelis-project/tests/vcs02_diff.rs` | `DIFF_POLICY_VERSION` pin 3→4 + コメント整合 |
| `evidence/reports/vcs-06-fixture-matrix-ledger.md` | VCS-DIFF-11 flip、env/test 一覧更新 |
| `evidence/sv457/` | verification evidence |

## Dependency graph

types → engine → adapters → render_input → tests → ledger/evidence。
apply surface・variant engine・restore は非変更（fail-closed pin
のみ）。

## Risk register

| risk | 対応 |
| --- | --- |
| cross-crate `ObjectKind`/`ComparisonDomain` consumer 破壊 | `check-full` で workspace compile 確認（`matches!`/field 型使用のみ検証済み） |
| field class の過剰/過小分類 | test-spec の field-level pin + review-protocol |
| `render_input` inventory 判断誤り | D6 根拠を design に明記、canonical value 契約は維持 |

## Verification matrix

| 検証 | コマンド |
| --- | --- |
| fmt | `mise run fmt-check` |
| check | `mise run check synvelis-project` |
| lint | `mise run lint synvelis-project` |
| test | `mise run test synvelis-project` |
| build | `mise run build synvelis-project` |
| cross-crate | `mise run check-full` |
| RT | `mise run test-rt` |
| hooks | `mise exec -- prek run --all-files` |

`*-msrv` 系は Daily CI 委譲で NotRun。
