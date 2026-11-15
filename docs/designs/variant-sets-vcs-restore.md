---
id: "DESIGN-034XI4wmwNPA3krWXzz5bA"
type: "design"
status: "approved"
title: "variant_sets restore-as-copy origin identity 修復設計 — persisted provenance 規則の境界確定 (synvelis#458)"
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
    - "docs/specs/variant-sets-vcs-restore.md"
  implemented-by:
    - "docs/impl/ir/variant-sets-vcs-restore.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
    - "docs/designs/creative-branch-design.md"
    - "docs/designs/variant-sets-vcs-diff.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/variant-sets-vcs-restore.md"
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
  derived-by:
    - "docs/plans/variant-sets-vcs-restore.md"
  verifies: []
  verified-by: []
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/457"
  deferred-by: []
metadata: {}
---

# variant_sets restore-as-copy origin identity 修復設計 — persisted provenance 規則の境界確定 (synvelis#458)

## Context

`docs/specs/variant-sets-vcs-restore.md` が固定した defect:
`restore::fork_project`（`restore/mod.rs`）は clone した `Project`
の top-level `project_id` のみを再採番し、`variant_sets` 以下の
nested `SnapshotIdentity`（`VariantSet::origin`・全
`Branch::created_from`）を verbatim 保持する。ところが
`validate_set_shape`（`variant/engine.rs`）は
`origin.project_id == project.project_id`・
`created_from.project_id == project.project_id` を要求するため、
restore 産 project は publish 前の staged-envelope
`validate_references` → `validate_persisted` で
`RestoreError::Validation` となり、variant set を持つ project の
restore-as-copy が全て失敗する（VCS-RESTORE-10 `Fail` pin、
synvelis#97 記録済み）。

本 design は discovery
（`docs/discovery/variant-sets-vcs-diff-gap.md` #458 節）の二択 —
(a) nested identity 再帰再採番 vs (b) validate 規則見直し — に
結論を出す。

## Decisions

### D1: 修復方針は (b) — persisted 境界で same-project 規則を緩め、provenance は verbatim 維持（(a) は却下）

`SnapshotIdentity` は「どの snapshot bytes を指すか」の authored
provenance であり、`digest` は source snapshot の exact bytes、
`locator` は variant により `project_id` を内包する
（`FixedSnapshotLocator::Live/Saved/Recovery`）。`fork_project`
が `SnapshotIdentity.project_id` だけを書き換えると、digest・
locator が依然 source project の bytes を指すまま「別 project の
identity」を名乗る虚偽参照を捏造し、CB-R-01-2
（`docs/specs/creative-branch.md`）の「Revision 単独で同一性を
断定しない」固定参照契約にも反する。したがって nested
`SnapshotIdentity` は **一切書き換えず**、same-project 要件を
要求する側（validate 規則）を provenance の意味論に合わせて
見直す。(a) は provenance の改竄であり却下。

### D2: 規則見直しの境界は「live op 境界では SameProject、persisted/load oracle では verbatim 許容」

`validate_set_shape` は 2 経路で共有される:

- `VariantOp::Fork::validate`（`engine.rs`）— live op 境界。
  fork する set の `origin`/`created_from` は
  `fork_variant_set` が `resolver.capture_live` で刻む live
  identity であり、same-project は **by-construction に真**。
  規則は冗長だが、防御として明示的に残す — 将来 fork-from-
  snapshot 等の op が増えても「live project に挿入される set の
  provenance はその project の capture である」不変条件を op
  境界で維持できる。`VariantOp` は `pub(crate)` で外部からの
  偽造経路はないが、in-crate の op 構築が増えた場合の guard と
  して残す価値がある。
- `validate_persisted`（`Project::validate_references` 経由）—
  load oracle。`controller.load`・restore の `validate_staged`・
  `decode_candidate`・`resolve_render_project` 等、persisted
  bytes を信頼境界として検査する全経路がここを通る。restore-
  as-copy 産 project は provenance が source project を指すのが
  **正しい**ため、same-project 要件はこの境界では誤り。

実装: `validate_set_shape` に provenance 規則を表す
`pub(crate)` enum（`ProvenanceRule::{SameProject, Persisted}`）
を追加し、2 箇所の `project_id` 一致検査を
`SameProject` 時のみ適用する。`Persisted` では provenance の
`project_id` を一切検査しない — persisted 層では「どの project
の snapshot から派生したか」は検証不能な歴史的事実であり、
verbatim 受理が唯一の正直な規則（捏造参照を拒否する術がない）。

### D3: `fork_project` は変更しない — provenance verbatim が設計意図

`fork_project` の clone + `project_id` 再採番のみの実装は
D1/D2 の結論と整合しており、変更不要。`variant_sets` map 全体
（`VariantSetId`/`BranchId` key・`revision`・`BranchContentRef`・
`last_adopt`・`origin`/`created_from` 全 `SnapshotIdentity`
field）が source と verbatim 一致する — 再採番されるのは
top-level `project_id` のみ、という VCS-RESTORE-10 の期待形
そのもの。nested identity を走査して書き換える処理は追加しない
（provenance 改竄を避けるためにも、追加コードを持たないのが
最も単純で正しい）。

### D4: `stable_id_counts` に `variant-set`/`branch` を追加

`StableIdReport`（policy `"preserve"`）の `counts` は entity
census であり、`variant_sets`/`branches` を欠落していると
receipt が「何が verbatim 保持されたか」を報告しない。
`project.variant_sets.len()` → `"variant-set"`、
`set.branches.len()` 合計 → `"branch"` を既存 key 規約
（kebab-case）で末尾に追加する。ID の再採番は行わない
（policy `"preserve"` どおり count のみ）。

### D5: readback 検証は `vcs06_variant_matrix.rs` の pin flip で行う

`restore_as_copy_fails_closed_on_variant_set_origin_mismatch` を
成功系 `restore_as_copy_preserves_variant_sets_verbatim` に
rename・flip し、(1) `RestoreOutcome::Published`、(2) published
destination の envelope readback で `variant_sets` が source と
verbatim 一致・`project_id` のみ `new_project_id`、(3)
`origin`/`created_from` の `project_id` が **source project を
指し続ける**こと（= provenance 保存・rewrite でないことの pin）、
(4) `stable_ids.counts` の `variant-set`/`branch` entry、を assert
する。`vcs05_restore.rs` への追加 pin は不要 — VCS-RESTORE-10 の
期待形は本 test が envelope readback で直接検証する。

### D6: 既知の capability 限界は正直記録（修復しない）

`BranchContentRef::Captured.payload_digest` の payload bytes は
`BranchContentStore`（project bundle 外）にあり、restore-as-copy
はこれを複製しない（#457 Deferred・別設計）。したがって restore
産 project の非採用 candidate branch は `decode_candidate` で
`CandidateUnavailable` になりうる — live-mirror adopted branch
（`BranchContentRef::Live`）と set 構造自体は有効で、candidate
編集/adopt は payload bytes の store 移送が別途必要。ledger
`VCS-RESTORE-10` の Recovery/Rollback 欄と `evidence/sv458/` に
この限界を明記する。`src/diff/`・`src/apply/`・`BranchContentStore`
経路は一切変更しない。

## Components and Boundaries

- `crates/synvelis-project/src/variant/engine.rs` —
  `ProvenanceRule` enum 追加、`validate_set_shape` に規則引数、
  `VariantOp::Fork::validate` → `SameProject`、
  `validate_persisted` → `Persisted`。
- `crates/synvelis-project/src/restore/mod.rs` —
  `stable_id_counts` に `variant-set`/`branch` 追加。
  `fork_project`・`validate_staged`・publish 経路は不変。
- `crates/synvelis-project/tests/vcs06_variant_matrix.rs` —
  restore pin flip + envelope readback assert。
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` —
  VCS-RESTORE-10 `Fail`→`Pass` flip + env 記録 + capability
  限界の正直記録。
- `evidence/sv458/` — 本 slice の verification evidence。

## Data and Control Flow

1. `restore_as_copy` → `fork_project`（不変）: clone +
   `project_id` 再採番のみ。`variant_sets` は nested provenance
   ごと verbatim clone。
2. `materialize` → staged envelope 書き出し（不変）。
3. `validate_staged` → `envelope.project.validate_references()` →
   `validate_persisted` → `validate_set_shape(.., Persisted)`:
   構造不変条件（branches 非空・adopted 存在・content kind
   mirror・revision 非零等）は全て維持し、provenance の
   `project_id` 一致のみ要求しない → publish へ進む。
4. publish → receipt の `stable_ids.counts` が
   `variant-set`/`branch` を含む。
5. 復元 project の load（`controller.load` /
   `SnapshotResolver` envelope 読み）も同じ `Persisted` 規則で
   受理される — open/load で拒否されない。

## Non-Functional Constraints

- fail-closed 性質の維持: 構造不変条件・digest 検証・watermark・
  destination safety・staging/resume 契約は全て不変。緩めるのは
  provenance `project_id` の同一性要求のみで、偽造 bytes・破損
  envelope・dangling branch 参照は従来どおり拒否される。
- cross-crate `validate_references` consumer
  （`synvelis-render`/`synvelis-gateway`/`synvelis-workspace`）は
  規則緩和で「受理が増える」方向のみ — 既存 Pass が Fail に
  なる経路はなく、`check-full` で非回帰を確認する。
- `VariantOp` は `pub(crate)` — 規則 enum も `pub(crate)` に留め、
  public API 表面を増やさない。

## Risks and Trade-offs

- persisted 層が「foreign provenance を持つ variant set」を
  受理するようになる — これは restore-as-copy の正しい出力で
  あり、捏造ファイルと restore 産を persisted 層で区別する術は
  存在しない。provenance は write-only の歴史的記録（解決経路
  なし — `origin`/`created_from` の読み取りは validate と diff
  field class のみ）であり、受理しても実行系に影響しない。
- live op 境界の SameProject 維持を忘れると live fork の
  provenance 不変条件が無防備になる — D2 で明示的に残す。
- `BranchContentStore` bytes 非複製の限界により、restore 産
  project の candidate branch は `CandidateUnavailable` —
  capability gap として ledger/証拠に記録し、修復は別設計
  （scope 外）。

## Deferred Design Concerns

- `BranchContentStore` の fork 先への payload bytes 複製・
  apply surface の variant transplant — #457 Deferred・別 Issue。
  restore 産 project の candidate branch を実際に adopt/edit
  可能にするには store 移送設計が必要。
- 実 OS durability・GUI 受入 — synvelis#97 統合 lane。

## Verification Notes

- [x] `mise run fmt-check` / `check synvelis-project` /
  `lint synvelis-project` / `test synvelis-project` /
  `test-rt` / `build synvelis-project` / `check-full` pass。
- [x] `restore_as_copy_preserves_variant_sets_verbatim` が
  Published + envelope readback verbatim + provenance source
  指し継続を assert して pass。
- [x] `variant_sets` 空 project の既存 restore test
  （`vcs05_restore.rs`・`vcs06_restore_matrix.rs`）が非回帰。
- [x] `variant_changes_stay_fail_closed_on_apply_surface`
  （vcs07）が不変で pass。
