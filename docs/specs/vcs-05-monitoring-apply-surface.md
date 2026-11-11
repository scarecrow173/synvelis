---
id: "SPEC-vcs05-monitoring-apply-surface"
type: "spec"
status: "approved"
title: "VCS-05 monitoring kinds apply surface 拡張 slice — synvelis#96 Slice A"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "ISSUE-96"
    - "SPEC-vcs05-apply-restore-headless"
    - "SPEC-vcs06-monitoring-matrix"
  verified_by:
    - "TSPEC-vcs05-monitoring-apply-surface"
  implemented_by:
    - "TASK-vcs05-monitoring-apply-surface"
  supersedes: []
  superseded_by: []
---

# VCS-05 monitoring kinds apply surface 拡張 slice — synvelis#96 Slice A

## 目的

PR #318（`DIFF_POLICY_VERSION = 2`）で VCS semantic diff の分類
対象に追加された authored monitor 宣言（`ObjectKind::
TrackMonitorDecl`・`ObjectKind::PunchMarks`、
`ComparisonDomain::Recording`）を、VCS-05 Safe Apply Selected
Change の apply whitelist に追加する。現行は whitelist 外のため
monitoring kind を含む selection は fingerprint CAS 段で
`ApplyError::Unsupported` に fail-closed する（VCS-APPLY-07 が
`vcs06_monitoring_matrix.rs` で実測固定済み）。本 slice は
`Added`/`Modified` の typed entity transplant を可能にし、
VCS-APPLY-07 の判定を fail-closed から supported-apply の実測へ
更新する。

## Why Now

monitor 宣言は schema v12 で `Project::monitoring` に永続化される
authored state であり、restore-as-copy では verbatim round-trip
済み（VCS-RESTORE-07 Pass）。一方 apply surface だけが未対応で、
monitor 宣言のみが異なる snapshot 対を部分適用できない。
diff 分類・restore の先行 slice が両端を揃えたため、apply whitelist
拡張という残る 1 区間を本 slice で閉じる。Issue #96 コメントが
記録する残存 DoD の Slice A に相当する。

## スコープ

### In Scope

- `crates/synvelis-project/src/apply/entity.rs`:
  `supported_kind` への 2 kind 追加、`entity_value` の
  keyed-synthetic view（`TrackMonitorDecl`）と
  declaration-presence view（`PunchMarks`）、`write_entity` の
  独自 arm（`keyed_insert!` 非使用）。
- `crates/synvelis-project/src/apply/mod.rs`: module doc の
  scope-out 文言更新（monitoring 宣言が apply surface に乗る
  ことを反映）。`plan.rs` は変更なし（`Removed` 拒否は既存の
  `change_supported` が全 kind に適用済み）。
- `crates/synvelis-project/tests/vcs05_apply.rs`・
  `tests/vcs06_monitoring_matrix.rs`: VCS-APPLY-07 の
  fail-closed assert を supported-apply assert へ書換え、
  Added/Modified apply の実測を追加。
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`:
  VCS-APPLY-07 行を実測 verdict へ更新、Environment ブロックを
  実走環境で更新。
- 本 slice の doc chain（spec/design/plan/task/test-spec/impl-ir）。

### Out of Scope

- `Removed` change の apply — cascade design deferred のまま
  全 kind で fail-closed 維持（monitoring の `Removed` も許可
  しない。本 Spec で確定）。
- `ObjectKind::PluginInstance` の apply surface 追加 —
  keyed-synthetic diff 分類は #324 で merge 済みだが、apply
  側の設計判断は別 Issue/別 slice。
- `src/diff/` の変更、`ObjectKind` の追加、apply surface への
  他 kind 追加。
- `tests/vcs06_apply_matrix.rs`・`tests/plugin_instance_persist.rs`
  等の別 Worker 予定領域、`docs/*/README.md` index。
- 実 OS FS durability・実 Provider/plugin E2E・GUI/RT・
  #97 REAL lanes・ruviform#294 — NotRun 維持。

## Requirements

- `TrackMonitorDecl` の entity identity は `stable_id` が借用する
  `TrackId` 空間（diff の keyed-synthetic 規約と同一）。宣言自体は
  id を持たないため `keyed!`/`keyed_insert!` は使わず、
  `stable_id` から `TrackId` を parse して `project.monitoring.
  tracks` を直接参照・挿入する。`max_new_id` には寄与しない
  （track 自体が同 apply で Added なら `Track` write が reseed
  を担う）。
- `PunchMarks` は project singleton（stable_id = project tag、
  diff classifier と同じ規約）で、`punch_in`/`punch_out` いずれか
  `Some` の時のみ entity 存在とみなす。全 `None` は「宣言なし」
  = absent-entity encoding（fingerprint 上 `null`）で、diff の
  declaration-presence 分類と一致させる。
- `Added`/`Modified` のみ apply 可能。`Removed` は
  `ApplyError::Unsupported{change: Removed, "removal changes are
  deferred (cascade design)"}` で fail-closed。`Reordered`/
  `Reconnected`/`Replaced`/`Moved` は monitoring kind に該当せず
  既存の kind 別 gate がそのまま拒否する。
- apply は typed entity transplant の既存契約を維持する:
  whole-entity payload、fingerprint CAS、preflight、
  `ProjectController` 単一 writer 経由 1 undo step、journal
  receipt。JSON path patch は導入しない。
- 失敗経路（fingerprint mismatch・plan reject・journal Failed）
  で live `project.monitoring`・revision・undo 履歴が不変である
  ことを fixture で固定する。

## Acceptance Criteria

- [ ] `TrackMonitorDecl`/`PunchMarks` の `Added` selection が
  typed entity transplant として適用され、live
  `project.monitoring` が after snapshot の宣言と一致する
  （1 apply = 1 undo step、undo/redo で monitoring 状態が
  往復する）。
- [ ] `Modified` selection が宣言 field 編集・punch mark 編集を
  適用する。
- [ ] `Removed` selection は全 monitoring kind で
  `Unsupported` fail-closed、live/journal が honest
  （VCS-APPLY-07 の Removed 行として実測固定）。
- [ ] `target_fingerprint` が monitoring kind で解決する
  （旧来の `Unsupported` を返さない）。
- [ ] `vcs05_apply.rs`・`vcs06_monitoring_matrix.rs` 既存 fixture
  が非回帰で pass。`mise run fmt-check`/`check`/`lint`/`test`/
  `test-rt`/`test-msrv`/`build` が全 pass。
- [ ] ledger VCS-APPLY-07 行が実測 verdict（Pass）と環境情報へ
  更新される。

## Deferred Design Concerns

- `Removed` change の cascade apply — 全 kind 共通の別設計
  （monitoring の decl clear・punch clear を含む）。
- `PluginInstance` の apply surface — #324 の diff 分類は merge
  済み。transplant/preflight（plugin-state blob 依存）の設計が
  必要で別 slice。
- recording/comping/SMF/clock-epoch 領域の apply — monitoring
  宣言以外は scope-out のまま。
- engine/runtime への実適用、GUI approval UX — app 層・#97。

## Sources and Relations

- `docs/specs/vcs-05-apply-restore-headless.md`（apply/restore
  headless slice の正本 spec — 本 slice はその surface 拡張）。
- `docs/specs/rcd-03-vcs-diff-monitor-classification.md`（diff
  分類の正本 — keyed-synthetic・declaration-presence 規約の
  出典）。
- `docs/specs/vcs-06-monitoring-matrix.md`（VCS-APPLY-07 証拠行の
  matrix spec）。
- `docs/designs/rcd-03-vcs-diff-monitor-classification-design.md`
  （diff 側 keyed-synthetic/singleton 設計）。
- GitHub issue `scarecrow173/synvelis#96`（Refs。本 spec は
  Issue を Close しない）。
