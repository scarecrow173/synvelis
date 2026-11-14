---
id: "TSPEC-034XF2IDsJG8izxwu3nyhl"
type: "test-spec"
status: "approved"
title: "VCS-06 variant-domain matrix tests"
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
    - "docs/specs/vcs-06-variant-matrix.md"
    - "docs/designs/vcs-06-variant-matrix-design.md"
  derived-by: []
  verifies:
    - "docs/specs/vcs-06-variant-matrix.md"
    - "docs/designs/vcs-06-variant-matrix-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 variant-domain matrix tests

## Purpose

schema v14 で永続化 domain として導入された
`Project::variant_sets`（sv#278 CB-01）について、VCS-06
ledger 用の固定 Case ID 付き実測行を headless fixture で固定
する。本 slice は「正直記録型」— compare surface が
`variant_sets` を観測不能であること（silent `Equal` /
silent drop）と `restore_as_copy` の fail-closed 拒否を
観測値のまま pin し、ledger では `Blocked`/`Fail` として
記録する。実 GUI・consumer gate・VCS Writer/3-way merge は
対象外。

## Feature

Variant-domain VCS-06 evidence — `variant_sets` の
diff/restore 実挙動を `vcs06_variant_matrix.rs` で観測し
`evidence/reports/vcs-06-fixture-matrix-ledger.md` の
Case 行として固定する。

## Rules

- **Rule VCS-DIFF-11 variant_sets compare-surface gap**:
  variant-set の fork（`fork_variant_set` による set 作成）
  のみ差分、および `adopt_branch` による mutation のみ差分の
  snapshot 対を `compare_snapshots` で比較したとき、観測値が
  `overall = Equal`・`completeness = Complete`・0 changes・
  0 unresolved であることを pin する。`identity.digest` 差と
  `variant_sets` map の実差（`len`/`id`/`revision`/`adopted`）
  を assert し AA fast path でないことを確認する — adapter
  が走っても観測域に variant state が無いことを固定。
- **Rule VCS-DIFF-11 mixed silent drop**: variant set +
  track rename の mixed diff で `overall = Different` だが
  changes は `ObjectKind::Track`・`ChangeKind::Modified`・
  `name` field evidence の 1 件のみ — variant 差分は emit
  されないことを pin する。
- **Rule VCS-APPLY-10 variant apply surface 不存在**:
  `compare_snapshots` が variant-kind `SemanticChange` を
  emit しないため apply 対象の change object が存在しない
  — ledger には honest `Blocked` として記録し、apply 行を
  Pass にしない（fixture は apply を実行しない — diff が
  emit 0 件であることが apply surface 不存在の根拠）。
- **Rule VCS-RESTORE-10 restore-as-copy fail-closed**:
  `variant_sets` を含む project の `restore_as_copy` が
  `Err(RestoreError::Validation)` で fail-closed し、error
  detail が `origin belongs to a different project` を含み、
  destination が publish されず（`dest` 不在）staging が
  recovery record として保持される（`staging` dir 存在）
  ことを assert する。silent corruption/publish がないことを
  固定。envelope readback による verbatim 確認は restore
  成功が前提のため本 slice では未実施（#458 修復後の
  follow-up）。

## Examples

- `variant_set_presence_and_mutation_are_invisible_to_diff`
- `mixed_diff_reports_track_change_but_drops_variant_set`
- `restore_as_copy_fails_closed_on_variant_set_origin_mismatch`

## Guarantees

- variant-set の作成・mutation のみ差分が ledger Case ID
  粒度で `Equal`/`Complete`・0 changes として独立観測され、
  compare-surface gap が `Blocked` として記録される
  （silent `Equal` を Pass に繰り上げない）。
- mixed diff で variant 差分が silent drop されることが
  pin され、consumer が「一部しか変わっていない」と誤認
  し得ることが記録される。
- restore の fail-closed は publish 前 validation
  （`validate_set_shape` の origin `project_id` 不変条件）に
  よるもので、staging 保持・dest 不在・source 不変の
  fail-closed 範囲が固定される。

## Non-goals

- variant diff adapter 本体（`ObjectKind` variant kind・
  `raw_kind_map` 列挙・`DIFF_POLICY_VERSION` bump）— #457。
- `restore_as_copy` の variant_sets defect 修復（nested
  identity 再採番 vs validate 規則見直し）— #458。
- apply surface の variant kind 追加（diff が emit しない
  ため対象不存在 — #457 の Deferred）。
- `vcs02_diff.rs`・`cb01_variant.rs` の既存 fixture の
  重複再実装（本 file は ledger 用の Case ID 粒度実測）。
- 実 device/plugin/consumer/GUI — 非永続・外部 gate。

## Risk

- `Equal`/`Complete` の観測値が「正しい仕様」と誤認され
  Pass に繰り上げられる — test コメント・design の Measured
  Contract・ledger の `Blocked`/`Fail` status で三重に
  「known gap」を明示する。
- restore の `Validation` が「正常な入力拒否」と誤認される
  — `fork_project` が `project_id` のみ再採番する実装事実を
  design/discovery に記録し、`dest` 不在・staging 保持を
  assert して fail-closed の範囲を固定する。
- `variant_sets` が空の project で restore が通ることを
  「variant restore が動く」と誤認 — fixture は必ず
  `variant_project()`（fork + adopt 済み set 保持）を使う。
