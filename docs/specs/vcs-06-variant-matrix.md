---
id: "SPEC-034XEYEHJTGgHuKiIeqiXd"
type: "spec"
status: "approved"
title: "VCS-06 variant-domain matrix — variant_sets の diff/restore 正直記録証拠行"
created: "2026-11-14"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "SPEC-vcs06-fixture-fault-matrix"
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
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
    - "docs/specs/vcs-06-monitoring-matrix.md"
    - "docs/specs/creative-branch.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 variant-domain matrix — variant_sets の diff/restore 正直記録証拠行

## What Should Be Built

`crates/synvelis-project/tests/vcs06_variant_matrix.rs`（新規・
test のみ）を追加し、schema v14 で永続化 domain として導入された
`Project::variant_sets`（sv#278 CB-01）について、VCS-06 の固定
Case ID 付き証拠行を
`evidence/reports/vcs-06-fixture-matrix-ledger.md` へ追加する。
本 slice は「正直記録型」— 現行の compare surface が
`variant_sets` を観測不能であること（silent `Equal` /
silent drop）と、`restore_as_copy` が nested origin identity
不整合で fail-closed 拒否することを実測 pin し、ledger では
`Blocked`/`Fail` として記録する。silent `Equal` を Pass に
繰り上げない。

- variant-set の作成・mutation のみ差分が semantic diff で
  観測されないこと（`Equal`/`Complete`・0 changes）を pin。
- variant 差分 + 通常 entity 差分の mixed diff で variant 差分が
  silent drop されることを pin。
- `restore_as_copy` が `variant_sets` を含む project を
  `RestoreError::Validation` で fail-closed 拒否することを
  実測記録（publish 前・staging 保持・dest 不在・source 不変）。

## Why Now

VCS-06 fixture matrix は monitoring domain（schema v12・
`vcs06_monitoring_matrix.rs`）と plugin_instances domain
（schema v13・`vcs06_plugin_instance_matrix.rs`）を同型の
「新永続 domain への正直 fixture 拡張」でカバー済みだが、
schema v14 の `variant_sets` は未カバー。`Project::variant_sets`
は `#[serde(default)]` で snapshot envelope に永続化され digest
を変えるが、`ObjectKind` に variant kind が無く
`raw_kind_map` も列挙しないため compare surface が観測不能。
Issue #97 は統合受入 ledger Issue であり、coverage gap を実測
証拠で埋める必要がある。発見の記録は
`docs/discovery/variant-sets-vcs-diff-gap.md`、adapter 実装は
VCS-02 系の別 Issue（#457）・restore defect 修復は別 Issue
（#458）に譲る。

## Users and Value

- Issue #97 の受入担当者: variant_sets domain の diff/restore
  行を個別 Case ID（VCS-DIFF-11・VCS-RESTORE-10・APPLY-10）で
  追跡できる。
- 将来の variant diff adapter 担当者（#457）: silent `Equal`/
  silent drop の観測値 pin が、compare surface 拡張の先行条件
  と境界を明示する。
- 将来の restore 修復担当者（#458）: `RestoreError::Validation`
  の fail-closed 実測が、`fork_project` の nested identity
  再採番 vs validate 規則見直しの設計判断の根拠になる。

## Scope

### In Scope

- `crates/synvelis-project/tests/vcs06_variant_matrix.rs`
  （新規・test のみ）
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` の新規 Case
  行追加（VCS-DIFF-11・VCS-APPLY-10・VCS-RESTORE-10）と
  Environment ブロックの現在値への更新
- `docs/discovery/variant-sets-vcs-diff-gap.md`（gap 記録）
- 本 spec / design / plan / task / test-spec / impl-ir チェーン
- `docs/*/README.md` 管理 index の再生成

### Out of Scope

- `crates/*/src/` の変更全体（variant diff adapter・restore
  defect 修復・apply surface 追加はいずれも別 Issue — #457/#458）
- 既存 `vcs06_*` matrix・`cb01_variant.rs` 等の既存 test file
  （非回帰対象・触らない）
- `ObjectKind` variant kind 追加・`raw_kind_map` 列挙・
  `DIFF_POLICY_VERSION` bump（#457 の設計単位）
- `fork_project` の nested identity 再採番・`validate_set_shape`
  規則見直し（#458 の設計単位）
- apply surface への variant kind 追加（diff が variant change
  を emit しないため apply 対象が存在しない — ledger には
  honest `Blocked` として記録）
- `*-REAL-*` 行・実 GUI・consumer gate・VCS Writer / 3-way
  merge・他 OS・Issue #97 の Close

## Requirements

- `variant_sets` が schema v14 の authored project state として
  永続化され、snapshot envelope の `identity.digest` を変える
  ことを fixture で確認する。
- variant-set の fork（`fork_variant_set` による set 作成）と
  mutation（`adopt_branch` による adopted 付け替え）のみ差分の
  snapshot 対を `compare_snapshots` で比較し、観測値が
  `overall = Equal`・`completeness = Complete`・0 changes・
  0 unresolved であることを pin する（`identity.digest` 差・
  `variant_sets` 実差を assert し AA fast path でないことを
  確認）。
- variant set + track rename の mixed diff で、`Different` の
  changes が Track `Modified` のみであり variant 差分が emit
  されないことを pin する。
- `restore_as_copy` を variant set を含む project に対して実行し、
  `RestoreError::Validation`（`origin belongs to a different
  project`）で fail-closed すること、staging が保持され
  destination が publish されず source が不変であることを
  実測記録する。
- ledger の各行は実測値のみ記録し、`Blocked`/`Fail` を Pass に
  繰り上げない。ledger の case row は `| Case | Expected |
  Observed | Status | Recovery/Rollback |` の 5 列形式で、
  Issue #97 が要求する記録項目（Synvelis commit・Cargo.lock
  Ruviform SHA・OS/FS/Git/Plugin・Fixture/Capability・
  Expected/Observed・Status・Recovery/Rollback・CI/PR URL）は
  Environment ブロックと各行の注記で充足する既存形式に従う。

## Acceptance Criteria

- [ ] variant-set の作成・mutation のみ差分が VCS-06 matrix で
  Case-ID 付きに検証され、観測値 `Equal`/`Complete`・0 changes
  が正直に pin されることを確認。
- [ ] mixed diff（variant + 通常 entity）で variant 差分が
  silent drop されることを確認。
- [ ] `restore_as_copy` の variant_sets 拒否が
  `RestoreError::Validation`・publish 前 fail-closed・staging
  保持・dest 不在・source 不変として実測記録されることを確認。
- [ ] ledger に新規 Case 行（VCS-DIFF-11 `Blocked`・
  VCS-APPLY-10 `Blocked`・VCS-RESTORE-10 `Fail`）を追加し、
  既存行フォーマットに従う。
- [ ] 既存の VCS-06 matrix test・全 canonical lane が回帰しない。
- [ ] `*-REAL-*` 行・実 GUI・consumer gate・VCS Writer/3-way
  merge は本 slice 対象外 — Pass 扱いしない。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `build` を canonical 検証として実行し結果を
  impl-ir に記録する（`test-msrv` は Daily CI 委譲のため NotRun
  記録可 — POLICIES.md §5・ユーザー指定）。

## Deferred Design Concerns

- variant diff adapter 本体（`ObjectKind` variant kind・
  `raw_kind_map` 列挙・`DIFF_POLICY_VERSION` bump・apply
  surface）— VCS-02 系の別 Issue（#457）。
- `restore_as_copy` の variant_sets defect 修復（`fork_project`
  の nested identity 再採番 vs validate 規則見直し）— 別 Issue
  （#458）。
- `BranchContentStore` の fork 先への bytes 複製・apply surface
  の variant transplant 設計 — #457 の Deferred。
- `*-REAL-*`・COMPAT・VCS Writer/3-way merge — 外部 gate 残存。

## Sources and Relations

- `docs/discovery/variant-sets-vcs-diff-gap.md`（gap 記録 —
  本 spec の根拠 discovery）。
- `docs/specs/vcs-06-fixture-fault-matrix.md`（親 matrix spec。
  本 spec はその variant-domain coverage gap を回収する）。
- `docs/specs/vcs-06-monitoring-matrix.md` /
  plugin-instances 同型 slice（先行の正直 fixture 拡張の先例）。
- `docs/specs/creative-branch.md`（Creative Branch 親 spec —
  `variant_sets` 永続化 domain の正本、synvelis#277 CB-00）。
- `docs/impl/ir/cb-01-variant-set.md`（CB-01 variant model の
  implementation record — sv#278）。
- `docs/specs/vcs-02-semantic-diff-changeset.md`（semantic 対象/
  対象外・suppression 規約の正本）。
- synvelis#97（統合受入 Issue、本 slice は Open のまま）。
- synvelis#457（variant diff adapter — 本 slice が pin した gap
  の実装 Issue）。
- synvelis#458（restore_as_copy variant_sets defect — 本 slice
  が実測 Fail として記録した defect の修復 Issue）。
- synvelis#278（CB-01 variant model）。
