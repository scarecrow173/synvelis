---
id: "DESIGN-lzcGjnJjPNFKiNNUpE2gE3"
type: "design"
status: "approved"
title: "VCS-06 monitoring-domain matrix — fixture 設計"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "docs/specs/vcs-06-monitoring-matrix.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-06-monitoring-matrix.md"
  implemented-by: []
  depends-on:
    - "docs/specs/vcs-06-monitoring-matrix.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-03-vcs-diff-monitor-classification-design.md"
    - "docs/designs/vcs-06-ui-headless-matrix-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-06-monitoring-matrix.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-06-monitoring-matrix.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 monitoring-domain matrix — fixture 設計

## Context

PR #318（`a218944b`）で `ObjectKind::TrackMonitorDecl` /
`ObjectKind::PunchMarks` が `ComparisonDomain::Recording` 配下に
分類追加され `DIFF_POLICY_VERSION = 2` になった。VCS-06 matrix
（`vcs06_diff_matrix.rs` / `vcs06_apply_matrix.rs` /
`vcs06_restore_matrix.rs`）は policy v1 時代に書かれ、
monitoring-domain の Case が ledger に無い。本 slice は
`src/` を変更せず、既存 pub API の外部観測契約のみを oracle
として、monitoring-domain の diff/apply/restore 実測行を
ledger へ追加する。

## Scope

### In Scope

- `crates/synvelis-project/tests/vcs06_monitoring_matrix.rs`
  1 file・4 tests
- ledger `evidence/reports/vcs-06-fixture-matrix-ledger.md` の
  VCS-DIFF-09/10・VCS-APPLY-07・VCS-RESTORE-07 行追加と
  Environment ブロック更新

### Out of Scope

- `crates/*/src/` 変更（apply whitelist 拡張等は別 slice）
- 既存 `vcs02_diff.rs` monitor 分類 fixture の重複再実装
- `*-REAL-*` 行・実 GUI・consumer gate・VCS Writer/3-way merge

## Case-ID 命名

親 spec（`SPEC-vcs06-fixture-fault-matrix`）の family 採番に
連番で追加する。新しい sub-family は作らない — 既存 family
section の末尾に追加し、family 単位の独立性を保つ。

| Case ID | 観測対象 |
| --- | --- |
| VCS-DIFF-09 | `TrackMonitorDecl` add/modify/clear → typed change |
| VCS-DIFF-10 | `PunchMarks` declaration-presence → typed change |
| VCS-APPLY-07 | monitoring kind を含む selection の apply → `Unsupported` fail-closed |
| VCS-RESTORE-07 | restore-as-copy の `project.monitoring` verbatim round-trip |

## Components and Boundaries

新規 file は per-domain matrix file の先例
（`vcs06_ui_matrix.rs`・`vcs06_git_matrix.rs`）に倣い、
monitoring-domain の diff/apply/restore を 1 file に束ねる
（既存 3 file への分散追加より Case family が追跡しやすい）。
helper は vcs06 系 file と同型（`controller`・`apply`・
`AllPresentProbe`・`snap_live`・`spec_for`・`compare`・
`changes_of`・`fields_of`）。

### fixture 戦略

- diff 行: `ProjectController` + 実 `ProjectOp`
  （`AddTrack`/`SetTrackMonitor`/`ClearTrackMonitor`/
  `SetPunchMarks`）→ `SnapshotResolver::capture_live` で
  before/after snapshot を生成し `compare_snapshots` を観測。
  実 ops 経由のため宣言の validate・undo 契約に沿った正規の
  状態遷移のみを打つ（手構築 `Project` は使わない）。
- apply 行: `vcs06_apply_matrix.rs` と同型の `Src`/
  `intent_for`/`run_apply` 構造。monitoring kind は
  `target_fingerprint`（pub API）自体が `Unsupported` を返す
  ため、intent の `expected_target_fingerprint` には到達不能な
  placeholder digest を置き、`apply_selected` までの fail-closed
  経路を実測する。
- restore 行: `vcs06_restore_matrix.rs` と同型の
  `MapBlobs`/`RestorePorts`/`request` で `restore_as_copy` を
  実 FS（temp dir）へ実行し、`dest/snapshots/<HEAD>` の
  `SnapshotEnvelope` を parse して `project.monitoring` を
  source と照合する。

## Data and Control Flow

1. **VCS-DIFF-09** `track_monitor_decl_add_modify_clear_are_typed_changes`:
   `AddTrack` → before capture → `SetTrackMonitor`（Added）→
   宣言 field 変更の `SetTrackMonitor`（Modified）→
   `ClearTrackMonitor`（Removed）の 3 遷移を 3 compare で観測。
   `ObjectKind::TrackMonitorDecl`・`stable_id == track.to_tagged()`
   ・owner `Track` への重複 change 無し・`domain() ==
   ComparisonDomain::Recording` を assert。併せて diff honesty の
   3 系統を assert — AA（同一 monitoring 状態）= `Equal`・0
   changes、Recording 除外 spec では monitor-only 差分が `Equal`・
   Recording のみ spec では `Different`、異 `project_id` の
   monitoring 保持 snapshot は `Unknown` +
   `AmbiguousIdentity{CrossProject}`（`snap_manual` helper で
   foreign identity を生成）。
2. **VCS-DIFF-10** `punch_marks_presence_transitions_are_typed_changes`:
   `{None,None}`→`{in,out}` = Added + `SemanticTimeRange::Musical`、
   `{in,out}`→`{in',out}` = Modified + `punch_in` field evidence、
   `{in,out}`→`{None,None}` = Removed + 消えた宣言の covered span
   を assert。併せて partial 宣言（`{in,None}`）= `Added` +
   `affected_range == None`、inverted 宣言（`{out<in}`）=
   `Modified` + `affected_range == None` を assert — 無効な
   range を捏造しないことを固定。`domain() == Recording` も
   assert。
3. **VCS-APPLY-07** `monitor_kind_selection_fails_apply_closed`:
   monitor decl + punch marks を両方持つ after snapshot の
   changeset を作り、`TrackMonitorDecl` change 選択と
   `PunchMarks` change 選択の 2 系統で (a) `target_fingerprint` →
   `Err(ApplyError::Unsupported{kind, change: Modified,
   "object kind outside the apply surface"})`、(b) `apply_selected`
   → 同 `Unsupported`（fingerprint CAS 段で fail-closed —
   `plan_changes` 未到達のため `change` field は常に `Modified`
   を報告する実測挙動も記録）、(c) `live.monitoring` 不変・
   journal `Failed` 記録を assert。
4. **VCS-RESTORE-07** `restore_as_copy_preserves_monitoring_verbatim`:
   decl（`lane`/`arm`/`monitor`/`channel_map` 全非 default）+
   punch marks を持つ project を capture → `restore_as_copy` →
   `SnapshotEnvelope` 読み戻しで `project.monitoring` が source
   と `PartialEq` 一致（fork `project_id` のみ差分）。

## Measured Contract — apply fail-closed

`apply_inner` の fingerprint CAS 段
（`entity::fingerprint(live, &objects)`）が `supported_kind`
whitelist で fail し、`ApplyError::Unsupported{kind,
change: Modified, detail: "object kind outside the apply
surface"}` を返す。`plan_changes` の `change_supported` にも
同じ whitelist があるが、fingerprint が先に fail するため
実測される `change` field は選択した change kind ではなく常に
`Modified`。本 slice はこの実測挙動をそのまま記録する（
修正しない・偽装しない）。

## Non-Functional Constraints

- deterministic: 全 fixture は in-memory controller + temp dir
  のみ・時刻非依存・test 間で ID 空間を共有しない。
- fail-closed のみ: apply 行は「失敗すること」が Expected であり、
  live project / undo / journal の不変条件を assert する。
- src 非変更: 全 assert は既存 pub API（`synvelis_project::diff`/
  `apply`/`restore`/`monitoring_data`/`store`）経由。

## Risks and Trade-offs

| risk | 対応 |
| --- | --- |
| `target_fingerprint` が `Unsupported` を返すため intent の fingerprint を正当に構成できない | placeholder digest を置くこと自体を test で明示（`apply_inner` は fingerprint 計算段で先に fail するため `expected` 値は比較されない）。両経路を別 assert で実測する |
| monitor-only changeset の `completeness != Complete` で `NotApplicable` になり `Unsupported` 経路に届かない | fixture は asset/processor 無しで `Complete` + `Different` を満たす（`resolve_verified` を通過して fingerprint 段へ到達する構成） |
| vcs02_diff.rs と fixture が重複する | vcs02 は分類規約の一次証拠。本 matrix は ledger Case ID 粒度の独立実測行として、apply/restore を含む monitoring-domain 全体を 1 file に束ねる役割分担 |
| ledger Environment ブロックの schema/commit が stale | 本 slice の計測環境（`SCHEMA_VERSION = 13`・merged `origin/main`・現行 ruviform pin）へ更新し、従来行の計測値は各行のまま残す |

## Deferred Design Concerns

- monitoring kind の apply surface 追加（transplant payload・
  `protected_fields`・reconcile・preflight の設計が必要 —
  別 slice）。
- `Unsupported` error の `change` field が fingerprint 段で常に
  `Modified` を報告する点（選択 change kind を伝えない）—
  実測記録のみ。修正は apply surface 拡張 slice の設計判断。

## Verification Notes

- `cargo test -p synvelis-project --test vcs06_monitoring_matrix`
  （補助）で 4 tests pass、fmt/clippy clean（canonical `mise`
  結果は impl-ir）。
