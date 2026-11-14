---
id: "DESIGN-034XEZZdGRhhT1iSrpYPas"
type: "design"
status: "approved"
title: "VCS-06 variant-domain matrix — fixture 設計"
created: "2026-11-14"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "docs/specs/vcs-06-variant-matrix.md"
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-06-variant-matrix.md"
  implemented-by: []
  depends-on:
    - "docs/specs/vcs-06-variant-matrix.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/vcs-06-monitoring-matrix-design.md"
    - "docs/designs/plugin-instances-vcs-diff-design.md"
    - "docs/designs/creative-branch-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-06-variant-matrix.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-06-variant-matrix.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 variant-domain matrix — fixture 設計

## Context

schema v14（sv#278 CB-01）で `Project::variant_sets` が永続化
domain として追加されたが、VCS-02 semantic diff の compare
surface（`ObjectKind`・`raw_kind_map`・domain adapter）は
variant state を一切列挙しない。`restore::fork_project` は
`project().clone()` + `project_id` 再採番のみで、nested
`VariantSet::origin`/`Branch::created_from` の
`SnapshotIdentity.project_id` を書き換えないため、
`validate_set_shape` が fork 後 project を拒否する。

本 slice は `src/` を変更せず、既存 pub API の外部観測契約のみを
oracle として、variant_sets domain の diff/restore 実測行を
ledger へ追加する「正直記録型」slice である。観測された gap
（silent `Equal`/silent drop/restore fail-closed）は Pass に
繰り上げず `Blocked`/`Fail` として記録し、修復は #457（diff
adapter）・#458（restore defect）の別 Issue に譲る。

## Scope

### In Scope

- `crates/synvelis-project/tests/vcs06_variant_matrix.rs`
  1 file・3 tests
- ledger `evidence/reports/vcs-06-fixture-matrix-ledger.md` の
  VCS-DIFF-11・VCS-APPLY-10・VCS-RESTORE-10 行追加と
  Environment ブロック更新
- `docs/discovery/variant-sets-vcs-diff-gap.md`（gap 記録）
- 本 spec/design/plan/task/test-spec/impl-ir チェーン

### Out of Scope

- `crates/*/src/` 変更全体（diff adapter・restore 修復・apply
  surface — #457/#458 の設計単位）
- 既存 `vcs06_*` matrix・`cb01_variant.rs` 等の既存 test file
- `ObjectKind` variant kind・`raw_kind_map` 列挙・
  `DIFF_POLICY_VERSION` bump
- `fork_project` nested identity 再採番・`validate_set_shape`
  規則見直し
- `*-REAL-*` 行・実 GUI・consumer gate・VCS Writer/3-way merge

## Case-ID 命名

親 spec（`SPEC-vcs06-fixture-fault-matrix`）の family 採番に
連番で追加する。既存 family section の末尾に追加し、family
単位の独立性を保つ。

| Case ID | 観測対象 | ledger status（実測） |
| --- | --- | --- |
| VCS-DIFF-11 | `variant_sets` のみ差分 → semantic change として観測されるべき | **Blocked**（compare surface gap — `Equal`/`Complete` を pin） |
| VCS-APPLY-10 | variant change selection の typed transplant | **Blocked**（diff が variant change を emit しないため apply 対象不存在） |
| VCS-RESTORE-10 | `variant_sets` verbatim restore-as-copy | **Fail**（`RestoreError::Validation` fail-closed を実測） |

## Components and Boundaries

新規 file は per-domain matrix file の先例
（`vcs06_monitoring_matrix.rs`・`vcs06_plugin_instance_matrix.rs`・
`vcs06_ui_matrix.rs`・`vcs06_git_matrix.rs`）に倣い、
variant-domain の diff/restore を 1 file に束ねる。helper は
vcs06 系 file と同型（`controller`・`apply_op`・
`AllPresentProbe`・`snap_live`・`spec_for`・`compare`・
`fields_of`・`MapBlobs`・`tmp_dir`・`request`）+ 本 slice 固有の
variant fixture helper（`arrangement_scope`・`fork_set`・
`adopt_branch_of`・`variant_project` — `cb01_variant.rs` の
構築 fixture を踏襲）。

### fixture 戦略

- diff 行: `ProjectController` + 実 variant engine
  （`fork_variant_set`/`adopt_branch` — `FakeDomainAdapter`・
  `InMemoryBranchContentStore`・`NoopRuntimePublish`）で
  authored state を構築し、`SnapshotResolver::capture_live` で
  before/after snapshot を生成して `compare_snapshots` を観測。
  実 engine 経由のため variant set の validate・undo 契約に沿った
  正規の状態遷移のみを打つ（手構築 `Project` は使わない）。
- mixed diff 行: `snap_manual` helper（vcs02/plugin-instance
  matrix と同型）で `variant_sets` を clear/保持した project を
  合成し、track rename（rename op が無いため手動）+ variant
  差分の混合 snapshot 対を構成する。
- restore 行: `MapBlobs`/`RestorePorts`/`request` で
  `restore_as_copy` を実 FS（temp dir）へ実行し、
  `RestoreError::Validation` の fail-closed を観測（staging
  保持・dest 不在を assert）。

## Data and Control Flow

1. **VCS-DIFF-11** `variant_set_presence_and_mutation_are_invisible_to_diff`:
   `AddTrack` → before capture（`variant_sets` 空を assert）→
   `fork_variant_set` で set 作成 → after capture（`variant_sets`
   len=1 を assert）→ `compare` で `overall=Equal`・
   `completeness=Complete`・0 changes・0 unresolved を assert。
   `identity.digest` 差と `variant_sets` map の実差を assert し
   AA fast path でないことを確認。続けて `adopt_branch` で
   adopted 付け替え → さらに `Equal`/`Complete` を assert
   （mutation も不可視）。
2. **VCS-DIFF-11 mixed** `mixed_diff_reports_track_change_but_drops_variant_set`:
   `variant_project()` で track + forked variant set + adopted
   candidate を構築。`snap_manual` で `variant_sets.clear()` した
   before と、track rename + `variant_sets` 保持の after を構成
   → `compare` で `overall=Different`・changes len=1・
   `ObjectKind::Track`・`ChangeKind::Modified`・`name` field
   evidence を assert — variant 差分は emit されない。
3. **VCS-RESTORE-10** `restore_as_copy_fails_closed_on_variant_set_origin_mismatch`:
   `variant_project()` → `snap_live` → `restore_as_copy` を
   `new_project_id = ProjectId::new(777)` で実行 →
   `Err(RestoreError::Validation)` で `detail` が
   `origin belongs to a different project` を含むことを assert。
   `dest` 不在・staging dir（`.copy-var.restore-r-var`）存在を
   assert — publish 前 fail-closed・recovery record 保持。

## Measured Contract — compare-surface gap

`compare_snapshots` は `before.identity.digest !=
after.identity.digest` のときだけ curated domain adapter を走らせ、
`ObjectKind` enum（末尾 `PluginInstance`）に variant kind が無く
`raw_kind_map` も `variant_sets` を列挙しないため、variant-only
差分は全 adapter で change 0・issue 0 → `Equal`/`Complete`。
digest は envelope 全体の SHA-256 で `variant_sets` をカバーする
ため AA fast path ではない — adapter が走っても観測域に variant
state が無い。これは plugin_instances gap（synvelis#324 で
`ObjectKind::PluginInstance` 追加により解消）の修正前と同型の
misreport 形状。

## Measured Contract — restore fail-closed

`restore::fork_project`（`restore/mod.rs:707-711`）は
`snapshot.projection.project().clone()` に `project_id` 再採番
のみを行う。`variant_sets` 以下の nested identity
（`VariantSet::origin`・全 `Branch::created_from`）は source
project の `project_id` を保持したまま clone される。
`validate_set_shape`（`variant/engine.rs:405-474`）が
`origin.project_id != project.project_id` で
`Error::Validation("origin belongs to a different project")` を
返し、`restore_as_copy` の publish 前 staged-envelope
`validate_references`（`restore/mod.rs:1237`・`1397`）で
`RestoreError::Validation` に変換される。fail-closed のため
silent corruption/publish はないが、variant set を持つ project
の restore-as-copy は一切成功しない capability gap。

## Non-Functional Constraints

- deterministic: 全 fixture は in-memory controller + temp dir
  のみ・時刻非依存・test 間で ID 空間を共有しない。
- fail-closed のみ: restore 行は「失敗すること」が Expected で
  あり、staging 保持・dest 不在・source 不変の不変条件を assert
  する。
- src 非変更: 全 assert は既存 pub API（`synvelis_project::diff`/
  `restore`/`variant`/`snapshot`/`store`）経由。
- 正直記録: `Equal`/`Complete`・silent drop・`Validation` 拒否は
  観測値をそのまま pin し、Pass/Blocked/Fail の ledger status を
  実測に一致させる。

## Risks and Trade-offs

| risk | 対応 |
| --- | --- |
| `Equal`/`Complete` の観測値を「仕様」と誤認して Pass にする | test コメントと ledger で「known compare-surface gap・`Blocked`」を明示。`identity.digest` 差・`variant_sets` 実差を assert して AA fast path でないことを確認 |
| mixed diff で variant 差分が「変更なし」と誤読される | changes len=1・`ObjectKind::Track`・`Modified`・`name` field を個別 assert — variant change が emit されないことを pin |
| restore の `Validation` を「正常な拒否」と誤認 | `fork_project` が `project_id` のみ再採番する実装事実と `validate_set_shape` の不変条件を design/discovery に記録。`dest` 不在・staging 保持を assert して fail-closed の範囲を固定 |
| `variant_sets` が空の project で restore が通ることを「variant restore が動く」と誤認 | fixture は必ず `variant_project()`（fork + adopt 済み set を持つ）を使い、空 map の影響を排除 |
| ledger Environment ブロックの schema/commit が stale | 本 slice の計測環境（`SCHEMA_VERSION = 14`・merged `origin/main`・現行 ruviform pin）へ更新し、従来行の計測値は各行のまま残す |

## Deferred Design Concerns

- variant diff adapter 本体（`ObjectKind` variant kind・
  `raw_kind_map` 列挙・`DIFF_POLICY_VERSION` bump・apply
  surface）— #457 の設計単位。
- `restore_as_copy` の variant_sets defect 修復（`fork_project`
  の nested identity 再採番 vs `validate_set_shape` 規則見直し —
  `SnapshotIdentity` が「どの snapshot を指すか」の参照なら
  fork 後も source を指し続けるのが意味的に正しい可能性）—
  #458 の設計単位。
- `BranchContentStore` の fork 先への bytes 複製・apply surface
  の variant transplant — #457 の Deferred。
- `*-REAL-*`・COMPAT・VCS Writer/3-way merge — 外部 gate 残存。

## Verification Notes

- `cargo test -p synvelis-project --test vcs06_variant_matrix`
  （補助）で 3 tests pass、fmt/clippy clean（canonical `mise`
  結果は impl-ir）。
