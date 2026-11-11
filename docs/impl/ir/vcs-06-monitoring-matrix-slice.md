---
id: "IMPL-bgZP66dLupCITdABVLTgJp"
type: "impl"
status: "completed"
title: "VCS-06 monitoring-domain matrix (synvelis#97 slice)"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_monitoring_matrix.rs"
      - type: "file"
        path: "docs/specs/vcs-06-monitoring-matrix.md"
      - type: "file"
        path: "docs/designs/vcs-06-monitoring-matrix-design.md"
      - type: "file"
        path: "docs/plans/vcs-06-monitoring-matrix-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-06-task-monitoring-matrix.md"
      - type: "file"
        path: "docs/test-specs/vcs-06-monitoring-matrix.md"
      - type: "file"
        path: "docs/impl/ir/vcs-06-monitoring-matrix-slice.md"
    modified:
      - type: "file"
        path: "evidence/reports/vcs-06-fixture-matrix-ledger.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vcs-06-task-monitoring-matrix.md"
    - "docs/specs/vcs-06-monitoring-matrix.md"
    - "docs/designs/vcs-06-monitoring-matrix-design.md"
    - "docs/plans/vcs-06-monitoring-matrix-plan.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-06-monitoring-matrix-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-06-ui-headless-matrix-slice.md"
    - "docs/specs/vcs-06-fixture-fault-matrix.md"
    - "docs/specs/rcd-03-vcs-diff-monitor-classification.md"
    - "docs/impl/ir/rcd-03-monitor-persistence-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-06-monitoring-matrix-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-06-monitoring-matrix.md"
  verified-by: []
  references: []
  defers:
    - "monitoring kind の apply surface 追加（transplant payload・preflight 設計を要する別 slice）"
    - "実 GUI/実 OS/実 provider/実 plugin host/RT/consumer lane（`*-REAL-*`・COMPAT 行 — 外部 gate）"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-06 monitoring-domain matrix (synvelis#97 slice)

## Summary

`crates/synvelis-project/tests/vcs06_monitoring_matrix.rs` を新規
追加し、PR #318（`a218944b`・`DIFF_POLICY_VERSION = 2`）で
`ComparisonDomain::Recording` 配下に分類追加された monitor 宣言
（`ObjectKind::TrackMonitorDecl`/`ObjectKind::PunchMarks`）の
VCS-06 証拠行を headless fixture で固定した: decl の
add/modify/clear → typed change、punch marks の
declaration-presence 遷移（partial/inverted 宣言に range を捏造
しないこと含む）、`apply_selected`/`target_fingerprint` の
`Unsupported` fail-closed 実測、`restore_as_copy` の
`project.monitoring` verbatim round-trip。`src/` 変更なし。
ledger に VCS-DIFF-09/10・VCS-APPLY-07・VCS-RESTORE-07 の
4 Case 行を実測値で追加した。

## Context

Implements `docs/tasks/vcs-06-task-monitoring-matrix.md` per
`docs/specs/vcs-06-monitoring-matrix.md` and
`docs/designs/vcs-06-monitoring-matrix-design.md`, verified by
`docs/test-specs/vcs-06-monitoring-matrix.md`（synvelis#97 の
monitoring-domain coverage 回収 slice。RCD-03 spec/impl が明示的に
defer した VCS-06 fixture 行。Issue #97 は `*-REAL-*` 行・COMPAT
行・VCS Writer/3-way merge の外部 gate 残存で Open のまま）。

## Implementation

### Changes

- `crates/synvelis-project/tests/vcs06_monitoring_matrix.rs`
  （新規・4 tests）— vcs06 系 matrix file と同型の helper
  （`controller`/`apply_op`/`AllPresentProbe`/`snap_live`/
  `compare`/`changes_of`/`fields_of`/`Src`/`MapBlobs`/`request`）
  + 本 slice 固有の `compare_with`（domain 絞込 spec）と
  `snap_manual`（vcs02 と同型、foreign `project_id` 生成用）。
  - `track_monitor_decl_add_modify_clear_are_typed_changes`
    （VCS-DIFF-09）: `SetTrackMonitor` upsert → `Added`
    （`stable_id == track.to_tagged()`・owner `Track` に重複
    change 無し）、宣言 field 変更 → `Modified`（`arm`/`monitor`/
    `channel_map` field evidence）、`ClearTrackMonitor` →
    `Removed`。`domain() == Recording`。併せて AA = `Equal`・0
    changes、Recording 除外 spec で monitor-only 差分 `Equal` /
    Recording のみ spec で `Different`、異 `project_id` →
    `Unknown` + `AmbiguousIdentity{CrossProject}` を固定。
  - `punch_marks_presence_transitions_are_typed_changes`
    （VCS-DIFF-10）: `{None,None}`→宣言 = `Added` +
    `SemanticTimeRange::Musical [in,out)`、mark 変更 =
    `Modified` + `punch_in` evidence、宣言→`{None,None}` =
    `Removed` + covered span。partial `{in,None}` = `Added` +
    `affected_range == None`、inverted `{out<in}` = `Modified` +
    `affected_range == None`（無効 range を捏造しない）。
  - `monitor_kind_selection_fails_apply_closed`（VCS-APPLY-07）:
    monitor decl + punch marks を持つ `Complete`/`Different`
    changeset を構築し、`TrackMonitorDecl`/`PunchMarks` 選択の
    2 系統で `target_fingerprint` と `apply_selected` の両経路が
    `ApplyError::Unsupported{kind, change: Modified,
    "object kind outside the apply surface"}` を返すことを実測。
    live `monitoring`・`live_revision` 不変・journal `Failed`
    記録を assert。
  - `restore_as_copy_preserves_monitoring_verbatim`
    （VCS-RESTORE-07）: lane binding 付き全非 default decl +
    punch marks の project を `restore_as_copy` → `Published`
    bundle の `SnapshotEnvelope` を readback。`monitoring ==
    source`（`PartialEq` 全 field）+ per-field spot check、fork
    `project_id` のみ差分、`receipt.effects.source/git` の
    untouched invariant を assert。
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` —
  VCS-DIFF-09/10・VCS-APPLY-07・VCS-RESTORE-07 行を既存行
  フォーマットで追加（APPLY-07 は `Unsupported` 実測として記録、
  Pass 偽装なし）。Environment ブロックを本 slice の計測環境
  （merged `origin/main` @ `7822857`・ruviform pin `9865cd57`・
  `SCHEMA_VERSION = 13`）へ更新し、従来行の計測値は各行のまま
  残した。fixture 一覧に本 file を追加（28 → 32 tests）。
- docs チェーン 6 file + `docs/*/README.md` index 再生成
  （`doc_maintenance.js apply --force-index`）。

### Decisions During Implementation

- monitoring kind は `target_fingerprint`（pub API）自体が
  `Unsupported` を返すため、intent の
  `expected_target_fingerprint` には到達不能な placeholder
  digest を置く。この設計意図を test コメントと design の
  Measured Contract 節に明記（`apply_inner` は fingerprint 計算
  段で先に fail し `expected` 値は比較されない）。
- `apply_selected` の実測 fail 点は fingerprint CAS 段で、
  `plan_changes` の `change_supported` に届かない。そのため
  error の `change` field は選択した change kind（`Added`）では
  なく常に `Modified` を報告する。assert はこの実測値に合わせ、
  variant・kind・detail を個別に固定して偽装を防いだ（修正は
  apply surface 拡張 slice の設計判断として deferred 記録）。
- monitor-only changeset の apply fixture は asset/processor 無し
  で構成し `completeness == Complete` + `overall == Different`
  を assert — `resolve_verified` の `NotApplicable` で早期失敗し
  fingerprint 段に届かない経路を避けるため。
- partial/inverted punch 宣言は `SetPunchMarks` が verbatim 受理
  する（validate なし）ため実 op 経由で生成 — `punch_range` が
  `MusicalRange::try_new` 失敗/片側 `None` で `None` を返し、
  typed presence change に無効 range を添えない実契約を固定。
- 新規 file は per-domain matrix file 先例（`vcs06_ui_matrix.rs`
  ・`vcs06_git_matrix.rs`）に倣い monitoring-domain を 1 file に
  束ねた（既存 3 file への分散追加より Case family が追跡
  しやすい）。

## Related Experiments

### Adopted

<!-- none（実契約の測定 fixture のみ — 探索的変更なし）。 -->

### Rejected

<!-- none -->

## Validation

`origin/main` @ `7822857`（post-#321、doc-only merge）取り込み後の
最終状態で実測（Windows 11 x64・NTFS、pinned rustc 1.99.0、MSRV
lane は rust 1.89.0）:

- `mise run fmt-check` — Pass（`cargo fmt --all -- --check`、
  exit 0）
- `mise run check` — Pass（`cargo check --workspace
  --all-targets`、exit 0）
- `mise run lint` — Pass（`cargo clippy --workspace
  --all-targets -- -D warnings`、exit 0）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  exit 0。200 suites 全緑、`vcs06_monitoring_matrix.rs` 4 tests
  を含む）
- `mise run test-rt` — Pass（`cargo test --workspace --locked
  rt_`、exit 0、191 suites）
- `mise run test-msrv` — Pass（`mise exec rust@1.89.0 -- cargo
  test --workspace --locked`、exit 0、200 suites 全緑）

補助証拠（canonical = mise、直接 cargo は補助）:
`cargo test -p synvelis-project --test vcs06_monitoring_matrix` —
4 passed、`cargo clippy -p synvelis-project --all-targets --
-D warnings` — clean、`cargo fmt --all` — applied。

検証中に観測した非 scope 事象: なし（初回 canonical 全 lane 緑。

clippy の `cloned_ref_to_slice_refs`/`assert_is_empty` 指摘は
fixture コード内のもので修正済み）。

review-protocol（worker 契約に基づく承認代替）: 実施 1 回。
Reviewer A（spec 適合）/ Reviewer B（敵対的正当性）の分離 pass →
Validator → Judge の順で実施し、確認済み指摘を修正した:

- CONFIRMED P1: partial/inverted punch 宣言・Recording domain
  除外・AA-equal・異 `project_id` の coverage 欠損（受入条件の
  必須 evidence）→ assert 追加で解消。
- CONFIRMED P1: clippy `cloned_ref_to_slice_refs`×2・
  `assert_is_empty`×2 → `std::slice::from_ref`・
  `assert_eq!(x.len(), 0)` へ修正（既存慣行と同一）。
- CONFIRMED P3: `stable_id` が `Added` change のみ assert
  → `Modified`/`Removed` にも追加。
- CONFIRMED P3: front-matter 関係の不整合（test-spec `source:
  []`・`derives-from` へ spec 欠落、design `source`/`depends-on`
  へ spec 欠落）→ UI slice 慣行へ揃えて修正。
- REJECTED: placeholder fingerprint が `FingerprintMismatch` で
  fail して原因を偽装する疑義 → 実測は `Unsupported` exact
  match（fingerprint 計算自体が先に fail）で非該当。
- REJECTED: `snap_manual` の digest 算出式が vcs02 と異なる疑義
  → digest は一意・決定的であればよく、identity 衝突の計測目的を
  阻害しない。

残 blocker なし — verdict ACCEPT。

NotRun / Unsupported（test-spec/spec 宣言どおり、headless
fixture は以下を証明しない）:

- `VCS-APPLY-07` は `Unsupported` 実測（monitoring kind は現行
  apply surface 外）— Pass 偽装なし。apply surface 追加は別
  slice。
- `*-REAL-*` 行・実 GUI・実 OS durability・実 provider/remote・
  plugin host・RT・consumer・他 OS・CI gate — NotRun（外部
  gate・後続 slice）。

## Risks

- `apply_selected` 系 fixture は `expected_target_fingerprint`
  に placeholder を置く構成依存 — 将来 fingerprint 計算順序が
  変わると意図が変わり得るが、その場合 test が明示的に fail
  する（黙って通る経路はない）。
- `snap_manual` は `ProjectRevision(0)`・`SnapshotOrigin::Live`
  の合成 identity を生成する — 実 store round-trip とは異なる
  が、計測対象は identity 衝突時の `Unknown` honesty であり
  snapshot 生成経路は契約に影響しない。
- ledger Environment の更新は従来行の計測値を書き換えない
  （各行は計測時点の環境を保持。本 slice の新規行のみ現行値に
  紐付く）。

## Follow-ups

- monitoring kind の apply surface 追加（transplant payload・
  `protected_fields`・reconcile・preflight 設計）— 別 slice。
  `Unsupported{change: Modified}` 報告 quirk の扱いも同 slice
  の設計判断。
- synvelis#97 残件（本 slice で Close しない）: `*-REAL-*` 行・
  COMPAT 行・実 GUI/実 OS/実 provider/consumer gate・VCS
  Writer/3-way merge の統合受入（Close は Coordinator 判断）。
