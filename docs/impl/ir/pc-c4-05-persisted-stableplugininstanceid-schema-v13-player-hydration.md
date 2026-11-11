---
id: "IMPL-034VvMRwKN7mUk0Wv50AKg"
type: "impl"
status: "in-progress"
title: "PC-C4-05 persisted StablePluginInstanceId — schema v13 + player hydration"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added:
      - type: "file"
        path: "docs/specs/pc-c4-05-persisted-instance-id.md"
      - type: "file"
        path: "docs/designs/pc-c4-05-persisted-instance-id-design.md"
      - type: "file"
        path: "docs/plans/pc-c4-05-persisted-instance-id-plan.md"
      - type: "file"
        path: "docs/tasks/pc-c4-05-task1-persisted-instance-id.md"
      - type: "file"
        path: "docs/test-specs/pc-c4-05-persisted-instance-id.md"
      - type: "file"
        path: "docs/impl/ir/pc-c4-05-persisted-stableplugininstanceid-schema-v13-player-hydration.md"
      - type: "file"
        path: "crates/synvelis-project/src/plugin_instance.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/plugin_instance_persist.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c4_05_persisted_instance.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/apply/entity.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_control.rs"
      - type: "file"
        path: "crates/synvelis-player/src/binding.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/sf03_l6.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/tim03_session_wiring.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tim_02_smf.rs"
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
    - "docs/tasks/pc-c4-05-task1-persisted-instance-id.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/pc-c4-05-task1-persisted-instance-id.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-05-persisted-instance-id-plan.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/pc-c4-05-persisted-instance-id.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C4-05 persisted StablePluginInstanceId — schema v13 + player hydration

## Summary

`StablePluginInstanceId` の採番・永続化・復元を `synvelis-project` の
schema v13 identity store（`Project::plugin_instances`）に移し、
`RoutingControlDispatch::vend_nodes` が persisted identity を
hydration して `PluginBindingSession::attach` /
`ProcessBoundarySession::open` provenance に渡す。unknown/stale
identity は project oracle と player `without_control` の両境界で
fail-closed。

## Context

Issue #101 の第三 slice。PC-C4-02 が導入した runtime-only
`StablePluginInstanceId`（`RoutingControlDispatch::next_instance`
採番）は save→reload・engine restart で不安定だった。本 slice は
spec `docs/specs/pc-c4-05-persisted-instance-id.md`・design
`docs/designs/pc-c4-05-persisted-instance-id-design.md`・plan・task1・
test-spec に従う。

## Implementation

### Changes

- `crates/synvelis-project/src/plugin_instance.rs`（新規）:
  `PluginInstanceId`（transparent u64）・`PluginInstanceIdentity`
  （id + pin 先 `PluginRef`）・`PluginInstanceIdentities`
  （`watermark` + `ProcessorId` keyed entries）。`allocate`/`ensure`/
  `remove`/`sync_processor`/`resolve`/`validate_against` を実装。
  `Default` は手実装で `watermark = 1`（derive だと 0 になるため）。
- `project.rs`: `Project::plugin_instances`（`#[serde(default)]`）
  追加、`Project::new` で default 初期化、`validate_references` 末尾で
  `plugin_instances.validate_against(self)` を呼ぶ oracle 化。
- `store.rs`: `SCHEMA_VERSION` 12 → 13、v13 field を文書化。
- `migration.rs`: `from: 12` step 追加。`routing.processors` を
  `ProcessorId` 昇順で `sync_processor` し、Resolved に 1 始まりの
  決定的 id を backfill。`Unresolved` は entry なし。entity
  `id_watermark` は不変。
- `ops.rs`: `InsertProcessor` が `Processor` 構築後に
  `sync_processor`、`RemoveProcessor` が `remove`（watermark は
  単調で戻さない → id 再利用なし）。
- `apply/entity.rs`: VCS apply の Processor 直接 write 経路で
  `sync_processor`（Coordinator 承認済み scope 拡張）。
- `routing_control.rs`: `RoutingControlDispatch::next_instance`
  撤去。`vend_nodes` が `project.plugin_instances.resolve` で
  persisted id を hydrate し、失敗時は `without_control` に
  fail-closed（runtime 採番への fallback なし）。`vend_nodes` は
  self 不要になったため associated fn 化。
- `binding.rs`: `StablePluginInstanceId` / `PluginBindingSession::
  attach` の doc を persisted authored identity の意味論に更新。
- テスト: 新規 `plugin_instance_persist.rs`（13 件）・
  `pc_c4_05_persisted_instance.rs`（3 件）。既存の世代跨ぎ
  `assert_ne!(instance_id)` 2 件（`sf03_l6.rs`・
  `tim03_session_wiring.rs`）を persisted identity の新契約
  （generation pin が freshness を担う）に更新。schema pin
  2 件（`md_a.rs`・`tim_02_smf.rs`）を v13 に更新。

### Decisions During Implementation

- `apply/entity.rs` の Processor 直接 write 経路への identity sync
  追加は Coordinator 承認済み（`write_all`/`write_entity` の唯一の
  caller は `ApplySelectedOp::apply`）。
- `driver.rs` の clip-insert FX ephemeral id は別 domain として
  変更しない。
- runtime は invalid persisted state を「修復」しない —
  `resolve` が `None` を返した時点で `without_control` に落とす
  （project oracle が load/apply 境界で既に reject 済みのため、
  player 側は観測用 fail-closed のみ）。
- `ensure` の semantics: 同一 `PluginRef`（`capability_revision`
  含む完全一致）なら id 保持、差分があれば新規採番 — apply の
  冪等性と rebind の新 instance 化を両立。

## Related Experiments

### Adopted

### Rejected

## Validation

merge `origin/main`（daabeda）取り込み後の canonical 結果:

- `mise run fmt` / `mise run fmt-check`: pass。
- `mise run check`: pass（workspace all-targets）。
- `mise run lint`: pass（`-D warnings`）。
- `mise run test`: pass — workspace 全 test binary 0 failure
  （新規 `plugin_instance_persist` 13 件・`pc_c4_05_persisted_instance`
  3 件を含む）。
- `mise run test-rt`: pass（0 FAILED）。
- `mise run check-headless-deps`: pass。
- `mise run check-ruviform-rev`: pass（merge で ruviform rev が
  `9865cd57` へ更新されたため `mise run sync-ui-deps` を実行済み —
  gitignore された生成物のみ、追跡ファイルの変更なし）。
- `mise run test-msrv`: 初回実行で `synvelis-app` test
  `monitor_arm` の rlib 形式エラー（cargo 1.89 artifact 系、
  本 slice のコードとは無関係の領域）。再実行で確認中。
- 補助証拠（canonical = mise）: `cargo test -p
  synvelis-project --test plugin_instance_persist` 13/13、
  `cargo test -p synvelis-player --test pc_c4_05_persisted_instance`
  3/3。

## Risks

- apply 経路の sync 漏れ → post-apply `validate_references` が検出
  （回帰 test で gate）。
- v12 body の field 欠落 → `#[serde(default)]` + migration backfill。

## Follow-ups

- app/workspace 側接続配線は pc-c4-04 別 slice（本 slice NotRun）。
- `ProcessorBinding` rebind の正式 op 化は deferred。
