---
id: "IMPL-034VNP5tgdXZr0d9jU3FD0"
type: "impl"
status: "completed"
title: "plugin_instances VCS semantic diff 分類 slice 実装記録"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/324"
  changes:
    added:
      - type: "file"
        path: "docs/plans/plugin-instances-vcs-diff-plan.md"
      - type: "file"
        path: "docs/tasks/plugin-instances-vcs-diff-task.md"
      - type: "file"
        path: "docs/test-specs/plugin-instances-vcs-diff.md"
      - type: "file"
        path: "docs/impl/ir/plugin-instances-vcs-diff.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/diff/types.rs"
      - type: "file"
        path: "crates/synvelis-project/src/diff/adapters.rs"
      - type: "file"
        path: "crates/synvelis-project/src/diff/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs02_diff.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/plugin_instance_persist.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_apply_matrix.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/plugin-instances-vcs-diff-task.md"
    - "docs/plans/plugin-instances-vcs-diff-plan.md"
    - "docs/designs/plugin-instances-vcs-diff-design.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/plugin-instances-vcs-diff-task.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/plugin-instances-vcs-diff-gap.md"
    - "docs/impl/ir/rcd-03-vcs-diff-monitor-classification-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/plugin-instances-vcs-diff-design.md"
    - "docs/plans/plugin-instances-vcs-diff-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/plugin-instances-vcs-diff.md"
  verified-by:
    - "docs/test-specs/plugin-instances-vcs-diff.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# plugin_instances VCS semantic diff 分類 slice 実装記録

## Summary

Issue #324（VCS-05 umbrella #96 由来の discovery
`plugin-instances-vcs-diff-gap`）の実装 slice として、schema v13 の
`Project::plugin_instances`（persisted `StablePluginInstanceId`
store）を VCS-02 semantic diff の比較対象 entity 集合へ分類追加
した。`ObjectKind::PluginInstance` を keyed-synthetic collection
（`ProcessorId` 空間借用・`raw_kind_map` 未登録）として enum 末尾に
追加し `ComparisonDomain::Routing` に写像、`RoutingAdapter::diff`
で `instances` map を `diff_map` 照合する。`id`/`plugin` field は
`Replaced`、pre-v13 側の `id` は migration 採番 artifact として
`Suppressed` → `PartialProjection`、`watermark` は allocator
bookkeeping として比較対象外。instance-only 差分が v2 の `Equal`
誤報から `Different` に変わるため `DIFF_POLICY_VERSION` を 2 → 3
へ bump した。apply surface（`src/apply/`）は `supported_kind`
非追加の fail-closed を維持する（Deferred Design Concern）。

## Context

- design `docs/designs/plugin-instances-vcs-diff-design.md`
  （approved・Option A 採用）
- plan `docs/plans/plugin-instances-vcs-diff-plan.md`
- task `docs/tasks/plugin-instances-vcs-diff-task.md`
- test-spec `docs/test-specs/plugin-instances-vcs-diff.md`
- discovery `docs/discovery/plugin-instances-vcs-diff-gap.md`
  （resolved — route 判断の正本）
- spec `docs/specs/vcs-02-semantic-diff-changeset.md`（分類規約の
  正本）

spec-doc は作成していない。discovery の Promotion Candidates に
「spec-doc / adr-doc: 不要（VCS-02 spec の既存分類規約内に収まる
拡張で、新規 product intent / architecture decision ではない —
design-doc で十分）」と route 判断が記録済みであり、本 slice は
その判断に従う。`src/apply/`・VCS-06 ledger
（`evidence/reports/vcs-06-fixture-matrix-ledger.md`）・
`docs/*/README.md` index・`tests/vcs05_apply.rs`・
`tests/vcs06_monitoring_matrix.rs` は並行 worker 占有のため
無変更。

## Implementation

### Changes

- `crates/synvelis-project/src/diff/types.rs`:
  `ObjectKind::PluginInstance` を enum 末尾（`PunchMarks` の後）
  に追加 — 既存 variant の `as u8` discriminant を保持
  （change ID・issue dedup key が discriminant を材料にする）。
  `domain()` で `ComparisonDomain::Routing` へ写像。
  `DIFF_POLICY_VERSION` を 2 → 3 へ bump（比較対象 entity 集合が
  変わり instance-only 差分が v2 の `Equal` から `Different` に
  なる。version は `ComparisonKey` 内にあり新旧 policy の cache
  key は衝突しない — RCD-03 v1→v2 と同根拠）。
- `crates/synvelis-project/src/diff/adapters.rs`:
  `RoutingAdapter::diff` の SendConfig `diff_map` 末尾に
  `diff_map(ctx, ObjectKind::PluginInstance,
  &before.plugin_instances.instances,
  &after.plugin_instances.instances, no_range, false)` を追加。
  `classify_field` で `pre_v13 = before_id.schema_version < 13 ||
  after_id.schema_version < 13` を算出し `classify_routing` の
  signature を拡張（`pre_v7` と同型の either-side 規則）。
  `classify_routing` に `PluginInstance` arm を追加:
  `"id" if pre_v13` → `Suppressed("pre-v13-plugin-instance-id")`、
  `"id" | "plugin"` → `Replaced`、その他 → `Modified` 既定。
- `crates/synvelis-project/src/diff/engine.rs`:
  `raw_kind_map` の keyed-synthetic 除外コメントに
  `plugin_instances.instances` を追記（`ProcessorId` 空間を
  借用するため登録すると全 Resolved processor が
  `AmbiguousIdentity` を捏造する — 挙動不変・明記のみ）。
- `crates/synvelis-project/tests/vcs02_diff.rs`:
  plugin-instance 分類セクション 8 test 追加 —
  entry Added/Removed（Processor cascade 併発・`unresolved = 0`
  pin）、`id` swap → `Replaced`（instance-only・Processor 無差分）、
  `plugin` rebind → `Replaced`（`id`/`plugin.*` evidence）、
  `watermark`-only drift → `Equal`/`Complete`、pre-v13 `id` →
  `Suppressed` → `PartialProjection`（`Partial`/`Unknown`）、
  policy v3 で instance-only 差分が `Different`、processor raw
  の cross-kind 衝突 → `AmbiguousIdentity(CrossKindId)`、
  `target_fingerprint` が `PluginInstance` で `Unsupported`。
- `crates/synvelis-project/tests/plugin_instance_persist.rs` /
  `tests/vcs06_apply_matrix.rs`: select-all apply fixture
  （`cs.changes.iter().collect()`）が新 `PluginInstance(Added)`
  change を拾うと `target_fingerprint` で `Unsupported` panic
  するため、`apply_surface_kind`/`apply_surface_changes` helper を
  追加して選択を apply-surface kind に絞る（`apply::entity::
  supported_kind` のミラー — crate-private で integration test
  から到達不能なため複写）。fail-closed 契約の fixture への反映
  であり apply 挙動の変更ではない。
- `docs/` chain: plan/task/test-spec/impl-ir の 4 文書（spec は
  route 判断により不作成）。README index は別 worker 占有のため
  未更新。

### Decisions During Implementation

- `plugin_instances.instances` は `diff_map` で通常 entity map
  として処理するが `raw_kind_map` には登録しない。key が
  `ProcessorId` を借用する keyed-synthetic view（`SmfDocument`/
  `AppliedCommit`/`TrackMonitorDecl` と同型）なので、登録すると
  全 Resolved processor が `{Processor, PluginInstance}` の内部
  衝突で `AmbiguousIdentity` 化する。逆に entry key の raw が
  `Track` 等の別真 kind と衝突した場合は既存
  `raw_is_ambiguous` gate がそのまま entry を未解決化する —
  regression test で確認済み。
- `watermark` は `instances` map の外にある store 直上の値で
  `diff_map` の比較対象に入らない — `id_watermark`・`revision`
  と同じ allocator bookkeeping として semantic 対象外（design
  の Option D 棄却根拠どおり）。entry 同一で watermark drift
  のみの差は `Equal` を維持することを fixture で固定。
- `id`/`plugin` を `Replaced`（`Identity` ではなく）に分類:
  entry の同一性は `ProcessorId` key で確定しており
  `AmbiguousIdentity` を捏造しない。persisted
  `StablePluginInstanceId` の差し替えは `Clip.source_id`/
  `Asset.digest` と同じ content-identity replacement の意味。
- pre-v13 `id` suppression は either-side 規則:
  `plugin_instances` は `#[serde(default)]` で pre-v13 snapshot
  は `migrate_v12_plugin_instances` が `ProcessorId` 昇順に
  backfill するが、mint される `id` 値は authored 履歴ではなく
  投影 artifact（pre-v6 routing の projection ID と同類）。
  `classify_control` の `pre_v7` と同じ構造で `classify_field`
  が schema_version を見て `classify_routing` へ渡す。
- select-all fixture の絞り込みは `apply_surface_kind` の
  `matches!` 複写で実装 — `supported_kind` が crate-private
  （`pub(crate)`）で integration test から参照不能なため。
  許可 kind の一覧は現行 `supported_kind` と同一で、`PluginInstance`
  を含まないことが fail-closed pin の実体。
- `DIFF_POLICY_VERSION` bump は必須と判断した根拠:
  instance-only 差分が policy v2 では `Equal`・v3 では
  `Different` と observable 差が生じ、version が
  `ComparisonKey` の材料に含まれるため bump しないと旧 policy で
  計算・保存された結果と key 衝突する。

## Related Experiments

### Adopted

<!-- 実験 log なし（既存 adapter/diff_map/classify 規約への整合的
拡張。route/gate 設計は approved design で確定済み）。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク、Windows 11・rust 1.99.0
pinned）:

- `mise run fmt-check` — Pass（`cargo fmt --all -- --check`、
  exit 0。初回は新規 test の整形差分で fail → `mise run fmt`
  で整形後 pass）
- `mise run check` — Pass（`cargo check --workspace
  --all-targets`、exit 0）
- `mise run lint` — Pass（`cargo clippy --workspace
  --all-targets -- -D warnings`、exit 0。初回は
  `assert!(cs.unresolved.is_empty())` の `assert_is_empty`
  lint で fail → `assert_eq!(cs.unresolved.len(), 0)` に修正後
  pass）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  exit 0）
- `mise run test-rt` — Pass（`cargo test --workspace --locked
  rt_`、exit 0）
- `mise run test-msrv` — Pass（rust 1.89 toolchain で
  `cargo test --workspace --locked`、exit 0）
- `mise run build` — Pass（`cargo build --workspace --locked`、
  exit 0）

補助証拠（canonical = mise、直接 cargo は補助）:
`mise exec -- cargo test -p synvelis-project --test vcs02_diff`
— 36 passed / 0 failed（新規 8 test 含む）。
`mise exec -- cargo test -p synvelis-project --test
plugin_instance_persist --test vcs06_apply_matrix` — 13 + 8
test 全 pass。

検証中に観測した事象:

- 前セッションの未コミット変更を引き継ぎ、本セッションで lint
  （`assert_is_empty`）と fmt の残件を解消してから全 lane を
  実走した。

## Risks

- rebind round-trip（Resolved→Unresolved→Resolved 等）で
  `PluginInstance(Replaced)`/`Removed`+`Added` が出るのは
  semantic truth（persisted identity が実際に差し替わった）で
  あり noise ではない — changelog 圧縮は view 層の責務。
- processor 削除/binding 遷移で `Processor` change と
  `PluginInstance` change が併発する keyed-synthetic cascade は
  `TrackMonitorDecl`/`SmfDocument` と同じ許容済み挙動。
- pre-v13 比較で `id` 差があれば `PartialProjection` —
  instance-only の差では `Unknown`/`Partial` に倒れる。投影
  artifact を authored change と誤記しないための意図的な保守性。

## Follow-ups

- `PluginInstance` change の apply/restore 対応（byte-faithful
  な `id` 値復元 + instance watermark reseed）: monotonic
  never-reissued 契約と live 採番履歴の統合設計が必要 —
  design の Deferred Design Concern、別 Issue 候補。
- VCS-06（#97）ledger への VCS-DIFF 行追記: 別 worker 占有 —
  PR 本文「未対応の作業」に列挙。
- `docs/*/README.md` index 更新: 別 worker 占有。
- Issue #324 は本 slice 完了でも Close 判断は coordinator/人間
  に委ねる（ledger 組込み等の残件あり）。
