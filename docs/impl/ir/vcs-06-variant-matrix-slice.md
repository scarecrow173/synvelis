---
id: "IMPL-034XF68snanLXncC17XApK"
type: "impl"
status: "completed"
title: "VCS-06 variant-domain matrix (synvelis#97 slice)"
created: "2026-11-14"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_variant_matrix.rs"
      - type: "file"
        path: "docs/discovery/variant-sets-vcs-diff-gap.md"
      - type: "file"
        path: "docs/specs/vcs-06-variant-matrix.md"
      - type: "file"
        path: "docs/designs/vcs-06-variant-matrix-design.md"
      - type: "file"
        path: "docs/plans/vcs-06-variant-matrix-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-06-task-variant-matrix.md"
      - type: "file"
        path: "docs/test-specs/vcs-06-variant-matrix.md"
      - type: "file"
        path: "docs/impl/ir/vcs-06-variant-matrix-slice.md"
      - type: "file"
        path: "evidence/sv97/vcs-06-variant-matrix-verification.md"
    modified:
      - type: "file"
        path: "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vcs-06-task-variant-matrix.md"
    - "docs/specs/vcs-06-variant-matrix.md"
    - "docs/designs/vcs-06-variant-matrix-design.md"
    - "docs/plans/vcs-06-variant-matrix-plan.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-06-variant-matrix-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-06-monitoring-matrix-slice.md"
    - "docs/specs/vcs-06-fixture-fault-matrix.md"
    - "docs/discovery/variant-sets-vcs-diff-gap.md"
    - "docs/discovery/plugin-instances-vcs-diff-gap.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-06-variant-matrix-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-06-variant-matrix.md"
  verified-by: []
  references: []
  defers:
    - "variant diff adapter 本体（ObjectKind variant kind・raw_kind_map・DIFF_POLICY_VERSION・apply surface — synvelis#457）"
    - "restore_as_copy の variant_sets defect 修復（fork_project nested identity 再採番 vs validate 規則見直し — synvelis#458）"
    - "実 GUI/実 OS/実 provider/実 plugin host/RT/consumer lane（*-REAL-*・COMPAT 行 — 外部 gate）"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-06 variant-domain matrix (synvelis#97 slice)

## Summary

`crates/synvelis-project/tests/vcs06_variant_matrix.rs` を新規
追加し、schema v14 `Project::variant_sets`（sv#278 CB-01）の
VCS-06 証拠行を headless fixture で固定した: variant-set の
fork/adopt のみ差分が `Equal`/`Complete`・0 changes（compare
surface gap — `ObjectKind` に variant kind 無し・`raw_kind_map`
未列挙）、mixed diff（variant + track rename）で variant 差分が
silent drop、`restore_as_copy` が nested origin identity 不整合
で `RestoreError::Validation` fail-closed。`src/` 変更なし・
正直記録型。ledger に VCS-DIFF-11（`Blocked`）・
VCS-APPLY-10（`Blocked`）・VCS-RESTORE-10（`Fail`）の 3 Case
行を実測値で追加した。gap 記録は
`docs/discovery/variant-sets-vcs-diff-gap.md`、follow-up Issue
#457（variant diff adapter）・#458（restore defect）を起票。

## Context

Implements `docs/tasks/vcs-06-task-variant-matrix.md` per
`docs/specs/vcs-06-variant-matrix.md` and
`docs/designs/vcs-06-variant-matrix-design.md`, verified by
`docs/test-specs/vcs-06-variant-matrix.md`（synvelis#97 の
variant-domain coverage 回収 slice — monitoring/plugin-
instances と同型の「新永続 domain への正直 fixture 拡張」。
Issue #97 は `*-REAL-*` 行・COMPAT 行・VCS Writer/3-way merge
の外部 gate 残存で Open のまま）。

## Implementation

### Changes

- `crates/synvelis-project/tests/vcs06_variant_matrix.rs`
  （新規・3 tests）— vcs06 系 matrix file と同型の helper
  （`controller`/`apply_op`/`AllPresentProbe`/`snap_live`/
  `spec_for`/`compare`/`fields_of`/`MapBlobs`/`tmp_dir`/
  `request`/`snap_manual`）+ 本 slice 固有の variant fixture
  helper（`arrangement_scope`/`fork_set`/`adopt_branch_of`/
  `variant_project` — `cb01_variant.rs` の構築 fixture を
  踏襲）。
  - `variant_set_presence_and_mutation_are_invisible_to_diff`
    （VCS-DIFF-11）: `variant_sets` 空 → `fork_variant_set`
    で set 作成 → `adopt_branch` で adopted 付け替え、の
    authored op 差分で `overall=Equal`・`completeness=Complete`・
    0 changes・0 unresolved を 2 段階で pin。`identity.digest`
    差と `variant_sets` map の実差を assert し AA fast path
    でないことを確認。
  - `mixed_diff_reports_track_change_but_drops_variant_set`
    （VCS-DIFF-11 mixed）: `snap_manual` で `variant_sets`
    clear/保持の snapshot 対を構成し、track rename + variant
    差分の mixed diff で `Different`・changes len=1・
    `ObjectKind::Track`・`Modified`・`name` field evidence
    のみを assert — variant 差分は emit されない。
  - `restore_as_copy_fails_closed_on_variant_set_origin_mismatch`
    （VCS-RESTORE-10）: `variant_project()`（fork + adopt 済み
    set 保持）→ `snap_live` → `restore_as_copy` を
    `new_project_id = ProjectId::new(777)` で実行 →
    `Err(RestoreError::Validation)` で detail が
    `origin belongs to a different project` を含むことを
    assert。`dest` 不在・staging dir 存在（recovery record）
    を assert — publish 前 fail-closed。
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` —
  VCS-DIFF-11（`Blocked` — compare surface gap）・
  VCS-APPLY-10（`Blocked` — diff が variant change を emit
  しないため apply 対象不存在）・VCS-RESTORE-10（`Fail` —
  `RestoreError::Validation` fail-closed）の 3 Case 行を
  既存行フォーマットで追加。silent `Equal`/silent drop/
  restore 拒否を Pass に繰り上げない。Environment ブロックを
  本 slice の計測環境（merged `origin/main` @ `3da82c5`・
  ruviform pin `fd1d4d5`・`SCHEMA_VERSION = 14`・rustc
  1.99.1）へ更新し、従来行の計測値は各行のまま残した。
  fixture 一覧に本 file を追加。"Explicitly out of scope"
  節に variant diff adapter（#457）と restore defect（#458）
  を `Blocked`/`Fail` として追記。
- `docs/discovery/variant-sets-vcs-diff-gap.md` — gap 記録
  （compare surface gap・silent drop・restore fail-closed の
  実測根拠・#457/#458 への接続）。
- docs チェーン 6 file（spec/design/plan/task/test-spec/ir）
  + `docs/*/README.md` index 再生成（`doc_maintenance.js
  index apply`）。
- `evidence/sv97/vcs-06-variant-matrix-verification.md` —
  canonical 検証結果・Case 判定の証拠記録（sv278 先例の
  `verification.md` 形式に準拠）。
- follow-up Issue 起票: #457（variant_sets VCS diff 分類
  追加）・#458（restore_as_copy variant_sets defect）。

### Decisions During Implementation

- `variant_sets` の diff 観測は「正しい `Equal`」ではなく
  compare-surface gap — `identity.digest` 差・`variant_sets`
  実差を assert して AA fast path でないことを確認し、
  adapter が走っても観測域に variant state が無いことを
  pin した（plugin_instances gap 修正前と同型の misreport
  形状）。
- mixed diff は `snap_manual` で `variant_sets.clear()` した
  before と track rename + `variant_sets` 保持の after を
  構成 — rename op が無いため手動 snapshot が必要
  （vcs02/plugin-instance matrix と同型の helper）。
- restore の fail-closed は `fork_project` が `project_id`
  のみ再採番し nested `SnapshotIdentity.project_id` が
  source のまま残るため `validate_set_shape` が拒否 —
  publish 前 validation で fail-closed（silent corruption
  なし）。修復は #458 の設計判断（nested identity 再採番 vs
  validate 規則見直し — `SnapshotIdentity` が「どの snapshot
  を指すか」の参照なら fork 後も source を指し続けるのが
  意味的に正しい可能性）。
- apply 行は fixture で apply を実行しない — diff が variant
  kind `SemanticChange` を emit しないため apply 対象の
  change object が存在せず、ledger には honest `Blocked`
  として記録（apply 行を Pass にしない）。
- 新規 file は per-domain matrix file 先例
  （`vcs06_monitoring_matrix.rs`・`vcs06_plugin_instance_matrix.rs`）
  に倣い variant-domain を 1 file に束ねた。

## Related Experiments

### Adopted

<!-- none（実契約の測定 fixture のみ — 探索的変更なし）。 -->

### Rejected

<!-- none -->

## Validation

`origin/main` @ `3da82c5`（post-#459）取り込み後の最終状態で実測
（Windows 11 x64・NTFS、rustc 1.99.1）:

- `mise run fmt-check` — Pass（`cargo fmt --all -- --check`、
  exit 0）
- `mise run check synvelis-project` — Pass（`cargo check
  --locked --all-targets -p synvelis-project`、exit 0）
- `mise run lint synvelis-project` — Pass（`cargo clippy
  --locked --all-targets -p synvelis-project -- -D warnings`、
  exit 0）
- `mise run test synvelis-project` — Pass（`cargo test
  --locked -p synvelis-project`、exit 0。全 suite 緑、
  `vcs06_variant_matrix.rs` 3 tests を含む — 既存 vcs06
  suite 非回帰）
- `mise run test-rt` — Pass（`cargo test --workspace --locked
  rt_`、exit 0）
- `mise run build synvelis-project` — Pass（`cargo build
  --locked -p synvelis-project`、exit 0）
- `mise exec -- prek run --all-files` — Pass（exit 0）

補助証拠（canonical = mise、直接 cargo は補助）:
`cargo test -p synvelis-project --test vcs06_variant_matrix` —
3 passed、`cargo clippy -p synvelis-project --all-targets --
-D warnings` — clean、`cargo fmt --all` — applied。

`mise run test-msrv` — **NotRun**（Daily CI 委譲 —
POLICIES.md §5・ユーザー指定「MSRV レーンは通常開発では行わず、
デイリーCI(daily-ci→ci.yml)で行う」に従い必須検証に含めない）。

NotRun / Unsupported（test-spec/spec 宣言どおり、headless
fixture は以下を証明しない）:

- VCS-DIFF-11 は `Blocked`（compare surface gap — variant
  diff adapter 未実装、#457）— silent `Equal`/drop を Pass
  に偽装しない。
- VCS-APPLY-10 は `Blocked`（diff が variant change を emit
  しないため apply 対象不存在）。
- VCS-RESTORE-10 は `Fail`（`RestoreError::Validation`
  fail-closed — #458 で修復追跡）。envelope readback による
  verbatim 確認は restore 成功が前提のため未実施。
- `*-REAL-*` 行・実 GUI・実 OS durability・実 provider/remote・
  plugin host・RT・consumer・他 OS・CI gate — NotRun（外部
  gate・後続 slice）。

## Risks

- `Equal`/`Complete`・silent drop・`Validation` 拒否は観測値を
  そのまま pin — 将来 #457/#458 で実装が入ると本 fixture の
  assert が fail し、ledger 行の更新が必要になる（意図的 —
  gap の解消を検出する tripwire として機能する）。
- `snap_manual` は `ProjectRevision(0)`・`SnapshotOrigin::Live`
  の合成 identity を生成する — 実 store round-trip とは異なる
  が、計測対象は variant 差分の silent drop であり snapshot
  生成経路は契約に影響しない。
- ledger Environment の更新は従来行の計測値を書き換えない
  （各行は計測時点の環境を保持。本 slice の新規行のみ現行値に
  紐付く）。

## Follow-ups

- synvelis#457: variant_sets を VCS semantic diff の分類対象に
  追加（`ObjectKind` variant kind・`raw_kind_map`・
  `DIFF_POLICY_VERSION`・apply surface 設計）。
- synvelis#458: `restore_as_copy` の variant_sets defect 修復
  （`fork_project` nested identity 再採番 vs validate 規則
  見直し）。修復後に VCS-RESTORE-10 の envelope readback
  verbatim 確認を follow-up。
- synvelis#97 残件（本 slice で Close しない）: `*-REAL-*` 行・
  COMPAT 行・実 GUI/実 OS/実 provider/consumer gate・VCS
  Writer/3-way merge の統合受入（Close は Coordinator 判断）。
