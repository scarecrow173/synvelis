---
id: "IMPL-vcs05-monitoring-apply-surface"
type: "impl"
status: "completed"
title: "VCS-05 monitoring kinds apply surface 拡張 slice 実装記録"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/96"
  changes:
    added:
      - type: "file"
        path: "docs/specs/vcs-05-monitoring-apply-surface.md"
      - type: "file"
        path: "docs/designs/vcs-05-monitoring-apply-surface-design.md"
      - type: "file"
        path: "docs/plans/vcs-05-monitoring-apply-surface-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-05-monitoring-apply-surface-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-05-monitoring-apply-surface.md"
      - type: "file"
        path: "docs/impl/ir/vcs-05-monitoring-apply-surface.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/apply/entity.rs"
      - type: "file"
        path: "crates/synvelis-project/src/apply/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs05_apply.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_monitoring_matrix.rs"
      - type: "file"
        path: "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vcs-05-monitoring-apply-surface-task.md"
    - "docs/plans/vcs-05-monitoring-apply-surface-plan.md"
    - "docs/designs/vcs-05-monitoring-apply-surface-design.md"
    - "docs/specs/vcs-05-monitoring-apply-surface.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/vcs-05-monitoring-apply-surface-task.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-05-safe-apply-selected-change-restore-as-copy-headless.md"
    - "docs/impl/ir/rcd-03-vcs-diff-monitor-classification-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-05-monitoring-apply-surface.md"
    - "docs/designs/vcs-05-monitoring-apply-surface-design.md"
    - "docs/plans/vcs-05-monitoring-apply-surface-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-05-monitoring-apply-surface.md"
  verified-by:
    - "docs/test-specs/vcs-05-monitoring-apply-surface.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-05 monitoring kinds apply surface 拡張 slice 実装記録

## Summary

Issue #96（VCS-05 Safe Apply Selected Change / Restore As Copy）
の Slice A として、PR #318（`DIFF_POLICY_VERSION = 2`）で diff
分類済みの authored monitor 宣言（`ObjectKind::TrackMonitorDecl`/
`PunchMarks`）を VCS-05 apply の whitelist に追加した。
`Added`/`Modified` の typed entity transplant が可能になり、
VCS-APPLY-07 の証拠行は fail-closed（`Unsupported`）から
supported-apply の実測 verdict（Pass）へ更新された。
`Removed` は cascade design deferred のまま全 kind で
fail-closed を維持する（本 slice で確定）。

## Context

- spec `docs/specs/vcs-05-monitoring-apply-surface.md`
- design `docs/designs/vcs-05-monitoring-apply-surface-design.md`
- plan `docs/plans/vcs-05-monitoring-apply-surface-plan.md`
- test-spec `docs/test-specs/vcs-05-monitoring-apply-surface.md`
- task `docs/tasks/vcs-05-monitoring-apply-surface-task.md`

先行 slice 群が両端を揃えていた: monitor 宣言は schema v12 で
`Project::monitoring` に永続化（rcd-03-monitor-persistence）、
VCS semantic diff で typed change として分類
（rcd-03-vcs-diff-monitor-classification / PR #318）、
restore-as-copy で verbatim round-trip（VCS-RESTORE-07）。
残る apply surface だけが whitelist 外で、monitoring kind を
含む selection は fingerprint CAS 段で `ApplyError::Unsupported`
に fail-closed していた（`monitor_kind_selection_fails_apply_
closed` が実測固定）。本 slice はその 1 区間を閉じる。

## Implementation

### Changes

- `crates/synvelis-project/src/apply/entity.rs`:
  - `supported_kind` の `matches!` に `TrackMonitorDecl`/
    `PunchMarks` を追加。
  - `entity_value` に 2 arm 追加。`TrackMonitorDecl` は
    `stable_id` を `TrackId::from_tagged` で parse し
    `project.monitoring.tracks` を参照する keyed-synthetic
    view（宣言は id を持たず `keyed!` macro の型パラメータで
    表現できないため専用 arm。parse 失敗は `invalid_id` で
    fail-closed）。`PunchMarks` は `punch_in`/`punch_out`
    いずれか `Some` の時のみ `Some(to_value(marks))`、全
    `None` は `Ok(None)` — diff classifier（`diff_punch_marks`）
    と同じ declaration-presence 規則で absent-entity encoding
    （fingerprint `null`）に揃える。
  - `write_entity` に `keyed_insert!` 非使用の 2 arm 追加。
    `TrackMonitorDecl` は `stable_id` から `TrackId` を parse
    して `monitoring.tracks.insert(track, decl)`（宣言は id
    非保持のため `max_new_id` に寄与しない — 対象 track 自体が
    同 apply で `Added` なら `Track` write の `keyed_insert!`
    が reseed を担う。comment で明記）。`PunchMarks` は
    `monitoring.punch = marks` を代入。
  - `monitoring_data::{PunchMarks, TrackMonitorDecl}` を import。
- `crates/synvelis-project/src/apply/mod.rs`: module doc の
  scope-out 文言を更新 — `Removed` は全 kind（monitoring 宣言
  を含む）で cascade design deferred のまま、recording/
  comping/SMF/clock-epoch 領域のうち authored monitor 宣言
  `TrackMonitorDecl`/`PunchMarks` のみが `Added`/`Modified` で
  apply surface に乗ることを明記。
- `crates/synvelis-project/tests/vcs05_apply.rs`:
  `monitor_decl_and_punch_apply_as_one_undo_step` 追加 —
  monitor-only `Complete`/`Different` changeset の `Added`
  2-change selection が `Applied` で commit され、live
  `monitoring.tracks`/`punch` が after と一致、1 undo step で
  monitoring-empty へ戻り redo で宣言が復帰することを固定
  （14 tests）。
- `crates/synvelis-project/tests/vcs06_monitoring_matrix.rs`:
  `unsupported_intent` を `intent_for`（実 `target_fingerprint`
  を計算する caller-side intent）へ改名・汎化し、
  `MonitorFixture`（S0=plain track / S1=+decl_v1+punch /
  S2=decl_v2+punch 編集）と `apply_added_selection` helper を
  追加。`monitor_kind_selection_fails_apply_closed` を 3 test
  へ再構成:
  - `monitor_kind_added_selection_applies_typed_entities` —
    `target_fingerprint` が両 kind で解決（旧来 `Unsupported`
    を返さないことを実測）、`Added` 適用 + live 一致。
  - `monitor_kind_modified_selection_applies_typed_entities` —
    `Modified` 適用（decl_v2・punch_in=480）+ 連続 undo が
    S1→S0 monitoring-empty へ 1 step ずつ復帰。
  - `monitor_kind_removed_selection_stays_fail_closed` —
    S1 live に対する S1→S0 `Removed` selection が両 kind で
    `Unsupported{change: Removed, "removal changes are deferred
    (cascade design)"}`、journal `Failed`、live 不変。
  （6 tests）
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`:
  VCS-APPLY-07 行を supported-apply 実測（Pass）へ更新、
  Environment ブロックを実走環境（origin/main @ `1093530`
  post-#332、ruviform `fd1d4d5f`、schema v13、rustc 1.99.0 /
  git 2.54.0.windows.1）で更新、fixture test 数を実数
  （vcs06_monitoring_matrix 6・合計 34）へ修正。
- `docs/` — 本 chain 6 文書（README index は対象外）。

### Decisions During Implementation

- `TrackMonitorDecl` は `keyed!`/`keyed_insert!` macro を使わず
  専用 arm とした。理由: (a) decl は id を持たず `stable_id`
  が `TrackId` 空間を借用する keyed-synthetic view で、lookup
  先 map（`monitoring.tracks`）も値型（`TrackMonitorDecl`）も
  `Track` と異なる。(b) `keyed_insert!` は entity の `id` field
  を `max_new_id` に畳み込むが、decl には `id` が存在しない —
  無理に macro へ合わせると reseed 規則を誤る。`stable_id`
  parse → map 直接操作が diff 側 `diff_map` の keyed-synthetic
  規約と対称で最も明示的。
- `PunchMarks` の entity 存在を declaration-presence
  （いずれか `Some`）で定義した。serialized 形状は常に field
  を持つ `PunchMarks` struct だが、authored 意味では
  `{None,None}` = 宣言なし。diff classifier（`diff_punch_marks`
  の `punch_declared`）と同一規則を entity view にも適用し、
  fingerprint の absent=`null` encoding と `Added`/`Removed`
  分類の presence を一致させた。規則の正本は diff 側であり、
  code comment にその旨を明記（2 箇所化の drift リスクは
  test-spec T3/T4 の presence 遷移 fixture で固定）。
- `Removed` を monitoring kind でも許可しない判断を spec で
  確定した。decl clear/punch clear は値戻しに見えるが、
  selection が entity の不在を意味する以上他 kind の `Removed`
  と同じ cascade 設計問題を共有する。`plan.rs` の
  `change_supported` が既に全 kind の `Removed` を
  `Unsupported` にするため、本 slice では entity 層に Removed
  write を実装せず plan 層の既存拒否に委ねた（変更不要）。
- `entity_value`/`write_entity` の parse 失敗はそれぞれ
  `ApplyError::InvalidIntent`/`Error::InvalidId` — diff が
  生成しない異常 `stable_id` への防御的 fail-closed であり、
  通常経路（diff → intent → apply）では発生しない。
- `plan.rs`・preflight・journal・engine は無変更。monitoring
  payload は digest/plugin/port 参照を含まず `_` arm で
  素通し、`ApplySelectedOp` の scratch-clone 単一 `Operation`
  経由で `project.monitoring` は undo/redo・
  `validate_references` oracle に自動的に乗る。

## Related Experiments

### Adopted

<!-- 実験 log なし（既存 keyed-synthetic/singleton 規約への整合的拡張）。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク、Windows 11 x64・rust 1.99.0
pinned、`git merge origin/main` で `1093530`（#324
PluginInstance diff）取り込み済み — conflict なし）:

- `mise run fmt-check` — Pass（`cargo fmt --all -- --check`、
  exit 0）
- `mise run check` — Pass（`cargo check --workspace
  --all-targets`、exit 0）
- `mise run lint` — Pass（`cargo clippy --workspace
  --all-targets -- -D warnings`、exit 0）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  exit 0。全 workspace test binary 緑 — `vcs05_apply.rs`
  14 tests・`vcs06_monitoring_matrix.rs` 6 tests を含む）
- `mise run test-rt` — Pass（`cargo test --workspace --locked
  rt_`、exit 0）
- `mise run test-msrv` — Pass（`mise exec rust@1.89.0 -- cargo
  test --workspace --locked`、exit 0）
- `mise run build` — Pass（`cargo build --workspace --locked`、
  exit 0）

補助証拠（canonical = mise、直接 cargo は補助）:
`mise exec -- cargo test -p synvelis-project --test vcs05_apply
--test vcs06_monitoring_matrix` — vcs05_apply 14 passed /
vcs06_monitoring_matrix 6 passed / 0 failed。

検証中に観測した事象:

- 前セッションの未コミット変更（entity.rs・mod.rs・両 test
  file）を引き継ぎ、compile・test で実測確認後に commit・
  merge・doc/ledger 更新を行った。merge は origin/main の
  #324（`ObjectKind::PluginInstance` 追加・
  `DIFF_POLICY_VERSION` 2→3）を conflict なしで取り込み、
  apply surface は `PluginInstance` 非対応のまま（#324 の
  fail-closed 設計判断と整合）。

## Open Items

- `Removed` change の cascade apply（全 kind — monitoring の
  decl clear/punch clear を含む）→ 別設計。
- `PluginInstance` の apply surface（transplant/preflight —
  plugin-state blob 依存の設計が必要）→ #324 系の別 slice。
- recording/comping/SMF/clock-epoch 領域の apply → scope-out
  のまま。
- 実 OS FS durability・実 Provider/plugin E2E・GUI/RT・
  #97 REAL lanes・ruviform#294 → NotRun 委譲先。
- Issue #96 は Slice A 完了でも残存 DoD（Removed cascade・
  PluginInstance apply 等）があるため Close しない。
