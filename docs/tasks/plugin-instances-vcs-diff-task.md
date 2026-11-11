---
id: "TASK-034VNs3KBar3pyoUwECyIh"
type: "task"
status: "in-progress"
title: "plugin_instances VCS semantic diff 分類"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/324"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/plugin-instances-vcs-diff-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/plugin-instances-vcs-diff-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/plugin-instances-vcs-diff.md"
  references: []
  defers: []
  deferred-by: []
---

# plugin_instances VCS semantic diff 分類

## Work

`crates/synvelis-project/src/diff/` に persisted plugin-instance
identity の分類を実装する。`ObjectKind::PluginInstance` を enum
末尾に追加し `domain()` = `Routing` に写像、
`RoutingAdapter::diff` で `plugin_instances.instances` の
`diff_map` 照合を行い、`classify_routing` に `PluginInstance` arm
（`id`/`plugin` → `Replaced`、pre-v13 `id` → `Suppressed`）を
追加する。`DIFF_POLICY_VERSION` を 3 に進める。
`tests/vcs02_diff.rs` に分類セクションを追加し、select-all apply
fixture（`plugin_instance_persist.rs`・`vcs06_apply_matrix.rs`）を
apply-surface kind に絞る。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/plugin-instances-vcs-diff-plan.md`
- `docs/designs/plugin-instances-vcs-diff-design.md`
- `docs/discovery/plugin-instances-vcs-diff-gap.md`
- `docs/specs/vcs-02-semantic-diff-changeset.md`（分類規約の正本）
- `docs/test-specs/plugin-instances-vcs-diff.md`

## Dependencies

- schema v13 の `Project::plugin_instances`（PC-C4-05・PR #313）
  は merge 済み — `PluginInstanceIdentities{instances, watermark}`、
  `EntityKey for ProcessorId`、`PluginInstanceIdentity` の
  `PartialEq`+`Serialize` は利用可能。
- VCS-02 diff engine の既存契約（`diff_map`・`classify_field`・
  `raw_is_ambiguous`・`emit_change`）。
- keyed-synthetic 先例: `SmfDocument`/`AppliedCommit`/
  `TrackMonitorDecl`（`raw_kind_map` 未登録）。

## Affected Files

- `crates/synvelis-project/src/diff/types.rs`
- `crates/synvelis-project/src/diff/adapters.rs`
- `crates/synvelis-project/src/diff/engine.rs`（コメントのみ）
- `crates/synvelis-project/tests/vcs02_diff.rs`
- `crates/synvelis-project/tests/plugin_instance_persist.rs`
- `crates/synvelis-project/tests/vcs06_apply_matrix.rs`
- `docs/`（本 chain + impl-ir。README index は別 worker 占有）

## Done When

- [ ] `ObjectKind::PluginInstance` が `Routing` domain で分類
  され、entry の Added/Removed・`id`/`plugin` 差の `Replaced`・
  その他 field 差の `Modified` が diff に現れる。
- [ ] `plugin_instances.instances` が `raw_kind_map` 未登録の
  まま keyed-synthetic として照合される。
- [ ] pre-v13 側を含む比較で `id` 差が `Suppressed` →
  `PartialProjection` になり、v13 同士では `Replaced` になる。
- [ ] `watermark` のみの drift が `Equal` を維持する。
- [ ] instance-only 差分が policy v3 で `Different` になり、
  `DIFF_POLICY_VERSION == 3`。
- [ ] `PluginInstance` change が apply surface 外
  （`target_fingerprint` で `Unsupported`）であることを pin。
- [ ] `tests/vcs02_diff.rs` の新規セクションが test-spec R1–R5
  を検証して pass。既存 fixture 全 pass（非回帰）。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`build` が pass。
- [ ] review-protocol で blocker なし。

## Verification

- [ ] `mise run test`（canonical）+ fixture 実行。
- [ ] `git fetch origin main && git merge origin/main` 後の再検証。
