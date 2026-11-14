---
id: "IMPL-034XAVfgfhd8qdJqSX5m0c"
type: "impl"
status: "completed"
title: "VCS-05 PluginInstance apply surface 拡張 slice 実装記録"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/96"
  changes:
    added:
      - type: "file"
        path: "docs/specs/vcs-05-plugin-instance-apply.md"
      - type: "file"
        path: "docs/designs/vcs-05-plugin-instance-apply.md"
      - type: "file"
        path: "docs/plans/vcs-05-plugin-instance-apply.md"
      - type: "file"
        path: "docs/tasks/vcs-05-plugin-instance-apply-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-05-plugin-instance-apply.md"
      - type: "file"
        path: "docs/impl/ir/vcs-05-plugin-instance-apply.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/apply/entity.rs"
      - type: "file"
        path: "crates/synvelis-project/src/apply/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/src/apply/plan.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs02_diff.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/plugin_instance_persist.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_apply_matrix.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs"
      - type: "file"
        path: "docs/designs/plugin-instances-vcs-diff-design.md"
      - type: "file"
        path: "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vcs-05-plugin-instance-apply-task.md"
    - "docs/plans/vcs-05-plugin-instance-apply.md"
    - "docs/designs/vcs-05-plugin-instance-apply.md"
    - "docs/specs/vcs-05-plugin-instance-apply.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/vcs-05-plugin-instance-apply-task.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-05-monitoring-apply-surface.md"
    - "docs/impl/ir/plugin-instances-vcs-diff.md"
    - "docs/impl/ir/pc-c4-05-persisted-stableplugininstanceid-schema-v13-player-hydration.md"
    - "docs/impl/ir/vcs-05-safe-apply-selected-change-restore-as-copy-headless.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-05-plugin-instance-apply.md"
    - "docs/designs/vcs-05-plugin-instance-apply.md"
    - "docs/plans/vcs-05-plugin-instance-apply.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-05-plugin-instance-apply.md"
  verified-by:
    - "docs/test-specs/vcs-05-plugin-instance-apply.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-05 PluginInstance apply surface 拡張 slice 実装記録

## Summary

Issue #96（VCS-05 Safe Apply Selected Change / Restore As Copy）
の続 slice として、PR #324（`DIFF_POLICY_VERSION = 3`）で diff
分類済みの keyed-synthetic object `ObjectKind::PluginInstance`
を VCS-05 apply の whitelist に追加した。`Added`/`Replaced`
selection が typed entity transplant で適用可能になり、persisted
`PluginInstanceIdentity`（issued `PluginInstanceId` + pinned
`PluginRef`）は after snapshot から byte-faithful に移植され、
instance `watermark` は `max(live, after)` で単調 reseed される
（後退しない・再発行しない）。`Removed` は cascade design
deferred のまま全 kind で fail-closed を維持する。
`docs/designs/plugin-instances-vcs-diff-design.md` の Deferred
Design Concern（apply 対応の切り出し条件）を本 slice で解消した。

## Context

- spec `docs/specs/vcs-05-plugin-instance-apply.md`
- design `docs/designs/vcs-05-plugin-instance-apply.md`
- plan `docs/plans/vcs-05-plugin-instance-apply.md`
- test-spec `docs/test-specs/vcs-05-plugin-instance-apply.md`
- task `docs/tasks/vcs-05-plugin-instance-apply-task.md`

先行 slice が両端を揃えていた: `Project::plugin_instances` は
schema v13 で永続化（pc-c4-05）、`PluginInstanceIdentities::
validate_against` が entry↔binding 整合の authoritative oracle、
VCS semantic diff で `Added`/`Removed`/`Replaced` に分類
（plugin-instances-vcs-diff / PR #324）。残る apply surface だけが
whitelist 外で fail-closed していた（`vcs02_diff.rs` の pin
fixture）。本 slice はその 1 区間を閉じ、VCS-06 ledger に
VCS-APPLY-09 行と VCS-RESTORE-08（plugin_instances verbatim
restore-as-copy）行を追加した。

## Implementation

### Changes

- `crates/synvelis-project/src/apply/entity.rs`:
  - `supported_kind` の `matches!` に `PluginInstance` を追加。
  - `entity_value` に `PluginInstance` arm 追加。`stable_id` を
    `ProcessorId::from_tagged` で parse し
    `project.plugin_instances.instances` を keyed-synthetic
    view として参照（entry は owning processor を key に持つ）。
    entry 不在は `Ok(None)` — diff classifier と同じ不在
    encoding（fingerprint `null`）に揃える。parse 失敗は
    `invalid_id` で fail-closed。
  - `write_entity` の `PluginInstance` arm は clippy
    `too_many_lines` 対策で `write_plugin_instance` helper へ
    分離。`stable_id` から `ProcessorId` を parse、payload を
    `PluginInstanceIdentity` として deserialize、
    `instances.insert(processor, identity)`、`watermark` を
    `max(live, after.plugin_instances.watermark)` で reseed。
    `PluginInstanceId` は entity `id_watermark`（u128）とは別の
    u64 domain のため `max_new_id` に寄与しない（comment で
    明記）。entry↔binding 整合・id range・一意性は書込後の
    `validate_references` → `validate_against` が oracle。
  - `plugin_instance::{PluginInstanceIdentities,
    PluginInstanceIdentity}` 等を import。
- `crates/synvelis-project/src/apply/plan.rs`:
  `ChangeKind::Replaced` の supported-kind whitelist に
  `PluginInstance` を追加（`Added` は全 supported kind で既に
  受理）。`Removed` は `change_supported` が全 kind で
  `Unsupported` を返す既存拒否のまま（変更不要）。
- `crates/synvelis-project/src/apply/mod.rs`: module doc の
  scope-out 文言を更新 — `PluginInstance` が `Added`/`Replaced`
  で apply surface に乗ること、`Removed` は引き続き deferred
  であることを明記。
- `crates/synvelis-project/tests/vcs02_diff.rs`:
  `plugin_instance_change_is_on_apply_surface` 追加 —
  `supported_kind` 解決と `target_fingerprint` が persisted
  `PluginInstanceId` を含むこと（id 差し替えで fingerprint が
  変わる = byte-faithful transplant 契約）、pre-v13 suppression
  規約の維持を固定。旧来の fail-closed pin を supported 期待へ
  更新。
- `crates/synvelis-project/tests/plugin_instance_persist.rs`・
  `crates/synvelis-project/tests/vcs06_apply_matrix.rs`:
  apply whitelist mirror helper に `PluginInstance` を追加
  （diff 側 `ObjectKind` 一覧との drift 防止 pin）。
- `crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs`:
  apply fixture helper（`snap_manual`/`intent_for`/
  `sources_for`/`run_apply` 等の VCS-06 matrix 規約）を追加し、
  plugin-instance apply 8 test を実装:
  - `plugin_instance_added_selection_transplants_identity_
    verbatim` — resolved processor 追加に伴う `Added`
    selection が `Applied`、live entry が after の
    `{id=7, plugin}`/`watermark=8` と byte-faithful 一致
    （live mint の id=1 ではない）、1 apply = 1 undo step。
  - `plugin_instance_replaced_selection_transplants_new_
    identity` — capability bump rebind（`PluginRef` 差し替え +
    `SemanticPort` `PluginDeclared` stamp を同梱 selection）で
    `{id=9, plugin_b}`/`watermark=10` verbatim、undo で
    `{id=1, plugin_a}`/wm2 復帰。
  - `plugin_instance_watermark_never_moves_backward` —
    after watermark < live の id-only `Replaced` で entry id は
    after 値・watermark は live 保持（`max(live, after)`）、
    retired range 再発行なしを undo 往復込みで固定。
  - `plugin_instance_removed_selection_stays_fail_closed` —
    `Removed` が `Unsupported{PluginInstance, Removed,
    "removal changes are deferred (cascade design)"}`、
    journal `Failed`、live store 不変。
  - `plugin_instance_rebind_without_processor_change_fails_
    closed` — Processor change を伴わない `plugin` drift は
    write 後 `validate_against` が `ApplyError::Rejected`、
    live/journal 不変（entry↔binding oracle が最終権威）。
  - `plugin_instance_target_missing_and_conflict_fail_closed`
    — `Replaced` on absent → `TargetMissing`、`Added` on
    present → `TargetConflict`、live 不変（silent remap なし）。
  - `plugin_instance_added_selection_rejects_unavailable_
    plugin` — plugin unavailable で `MissingPlugin` preflight
    reject、processor/entry/undo 不変。
  - `plugin_instance_ack_gap_replays_without_double_mutation`
    — Applied receipt 喪失（ACK gap）後 retry が `Replayed`、
    store 再採番・二重 mutation・二重 undo なし。
  processor-bearing endpoint は `snap_manual`（Complete）で
  capture する — live capture は opaque plugin state を
  `Unknown` として記録し `Partial` → `NotApplicable` になるため
  （`plugin_instance_persist.rs` の確立規約に一致）。
- `docs/designs/plugin-instances-vcs-diff-design.md`:
  Deferred Design Concern の `PluginInstance` apply 項を
  「解決済み — 本 slice の spec/design で切り出し済み」へ更新。
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`:
  VCS-APPLY-09 行（plugin-instance apply verdict と全 fixture
  実測）と VCS-RESTORE-08 行（plugin_instances verbatim
  restore-as-copy）を追加、fixture test 数・Environment を
  実測値へ更新。
- `docs/` — 本 chain 5 文書 + 本 IR（README index は
  doc-maintenance の `index apply` で再生成、手編集なし）。

### Decisions During Implementation

- persisted `PluginInstanceId` は **byte-faithful transplant**
  とした（remap/再発行しない）。理由: (a) `StablePluginInstanceId`
  は authored state であり、schema v13 が「issued id の保存」を
  不変条件としている。(b) apply 後 fingerprint が after snapshot
  と一致することで、CAS 検証と ACK-gap replay の reconcile が
  機能する — remap すると post-apply fingerprint が after と
  乖離し replay 判定を壊す。(c) `PluginInstanceIdentities::
  allocate` の never-reissued 保証は watermark が単調である
  限り保たれるため、write 側は id を発行せず watermark のみ
  `max(live, after)` で reseed すれば十分。
- `PluginInstanceId` を entity `id_watermark`（u128）へ畳み
  込まない判断を明記した。plugin-instance id は別 u64 domain
  であり、`max_new_id` へ混入すると entity id 空間を誤って
  前進させる。owning `Processor` が同 apply で `Added` の場合の
  entity watermark reseed は `Processor` write 自身が担う。
- `PluginInstance` 自体への plan 層 plugin preflight は追加
  しなかった。payload は `PluginRef` を内包するが、entry↔
  binding の verbatim 一致・id range・一意性・processor 存在は
  write 後の `validate_references` → `validate_against` が
  authoritative oracle として検査する。`preflight_plugins` は
  `Processor` write の plugin availability を既に gate しており、
  `PluginInstance` の `Added`/`Replaced` は必ず対応する
  `Processor` change を伴う（binding が変わらなければ entry
  も変わらない）ため、追加 gate は redundant。
- `entity_value`/`write_entity` の `stable_id` parse 失敗は
  `ApplyError::InvalidIntent`/`Error::InvalidId` — diff が
  生成しない異常 `stable_id` への防御的 fail-closed。
- `Removed` を plugin-instance でも許可しない判断を spec で
  確定。entry の不在は processor unbind/removal に連動する
  cascade 削除を意味し、他 kind の `Removed` と同じ deferred
  設計問題を共有する。`plan.rs` の `change_supported` が既に
  全 kind の `Removed` を `Unsupported` にするため entity 層に
  Removed write は実装しなかった。
- `write_entity` が clippy `too_many_lines` (104/100) を超えた
  ため `write_plugin_instance` helper へ抽出（`#[allow]` による
  抑制は行わない — 契約の禁止事項）。test 側は
  `assert_is_empty`/`similar_names`/`cloned_ref_to_slice_refs`
  の指摘を既存規約（`assert_eq!(changes_of(...).len(), 0)`、
  `std::slice::from_ref`、binding 改名）へ合わせて解消。

## Related Experiments

### Adopted

<!-- 実験 log なし（既存 keyed-synthetic transplant 規約と
     plugin-instance 永続化モデルへの整合的拡張）。transplant
     vs remap の判断は design doc で確定済み。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク、Windows 11 x64・
rust 1.99.0 pinned、`git merge origin/main` で `f12aa66` 取り込み
済み — conflict なし）:

- `mise run fmt-check` — Pass（`cargo fmt --all -- --check`、
  exit 0）
- `mise run check synvelis-project` — Pass（`cargo check
  --locked --all-targets -p synvelis-project`、exit 0）
- `mise run lint synvelis-project` — Pass（`cargo clippy
  --locked --all-targets -p synvelis-project -- -D warnings`、
  exit 0。初回実行は `too_many_lines` + test 3 件の指摘で Fail、
  helper 分離と規約整合で修正後に再実行 Pass）
- `mise run test synvelis-project` — Pass（`cargo test
  --locked -p synvelis-project`、exit 0。`vcs02_diff`・
  `plugin_instance_persist`・`vcs06_apply_matrix`・
  `vcs06_plugin_instance_matrix` 含む全 test binary 緑）
- `mise run test-rt` — Pass（`cargo test --workspace --locked
  rt_`、exit 0）
- `mise run build synvelis-project` — Pass（`cargo build
  --locked -p synvelis-project`、exit 0）
- `mise run test-msrv` — NotRun（POLICIES.md §5 により MSRV
  別再コンパイルは Daily Full CI へ委譲。タスク指定の canonical
  検証は package 引数付き `check`/`lint`/`test`/`build` +
  `fmt-check` であり `*-msrv` は対象外）
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index plan --json` — Pass（管理 index 10 dir 全て
  `verdict: in-sync`）

補助証拠（canonical = mise、直接 cargo は補助）:
`mise exec -- cargo test -p synvelis-project --test
vcs06_plugin_instance_matrix` — 9 tests all Pass（apply 8 +
restore-as-copy verbatim 1 を含む）。

## Risks

- `Removed` change は全 kind で `Unsupported` のまま —
  plugin-instance entry の除去（processor unbind/removal 連動）
  は apply できない（cascade design deferred、`plan.rs` の
  `change_supported` が拒否）。
- byte-faithful transplant は after snapshot の `PluginInstanceId`
  をそのまま採用する — divergent lineage 間で retired id が衝突
  するケースの追跡は store 履歴化を要する別設計（ledger
  VCS-APPLY-09 の残存リスク欄に記録）。
- `entity_value`/`write_entity` の `stable_id` parse 失敗は
  `InvalidIntent`/`InvalidId` で fail-closed — 通常経路
  （diff → intent → apply）では発生しない防御的チェック。
- 実 OS FS durability・実 Provider/plugin E2E・GUI/RT・
  #97 REAL lanes・ruviform#294 は headless fixture の範囲外で
  未検証（NotRun 委譲先、下記 Follow-ups 参照）。

## Follow-ups

- `Removed` change の cascade apply（全 kind — plugin-instance
  entry 除去を含む）→ 別設計。
- divergent lineage の retired-id 衝突追跡（store 履歴化）→
  別設計。
- recording/comping/SMF/clock-epoch 領域の apply → scope-out
  のまま。
- 実 OS FS durability・実 Provider/plugin E2E・GUI/RT・
  #97 REAL lanes・ruviform#294 → NotRun 委譲先。
- Issue #96 は本 slice 完了でも残存 DoD（Removed cascade・
  Restore Offline Playback 等）があるため Close しない。
