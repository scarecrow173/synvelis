---
id: "PLAN-034VNrIZteNN2bsGVGokZF"
type: "plan"
status: "approved"
title: "plugin_instances VCS semantic diff 分類 slice"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/plugin-instances-vcs-diff-gap.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/plugin-instances-vcs-diff-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/plugin-instances-vcs-diff.md"
  references: []
  defers: []
  deferred-by: []
---

# plugin_instances VCS semantic diff 分類 slice Implementation Plan

## Goal

schema v13 の `Project::plugin_instances`（persisted
`StablePluginInstanceId` store）を VCS-02 semantic diff の分類対象に
追加し、instance identity のみが異なる snapshot 対が `Equal` と
誤報しないようにする。`ObjectKind::PluginInstance`（keyed-synthetic
collection）を追加し、apply surface は fail-closed のまま変更
しない。

## Scope

`crates/synvelis-project/src/diff/`（types/engine/adapters）と
`crates/synvelis-project/tests/`（`vcs02_diff.rs` 新規セクション +
select-all fixture の apply-surface 絞り込み）に閉じる。
`src/apply/`・`synvelis-app`/`player`/`recording`/`library`/
`workspace`/`gateway`/`ui/` は変更しない（新 kind は apply
surface 外として既存の fail-closed に委ねる — Deferred Design
Concern）。

spec-doc は作成しない: discovery の route 判断どおり、本変更は
VCS-02 spec の既存分類規約内に収まる拡張であり、新規 product
intent / architecture decision ではない（design-doc で十分）。

## Upstream Documents

- `docs/designs/plugin-instances-vcs-diff-design.md`（approved・
  Option A 採用の正本）
- `docs/specs/vcs-02-semantic-diff-changeset.md`（分類規約の正本）
- `docs/discovery/plugin-instances-vcs-diff-gap.md`（resolved —
  route 判断と証拠の正本）

## Design Inputs

- `docs/designs/plugin-instances-vcs-diff-design.md`
- `docs/designs/rcd-03-vcs-diff-monitor-classification-design.md`
  （keyed-synthetic 追加の直接先例）
- `docs/designs/pc-c4-05-persisted-instance-id-design.md`
  （`PluginInstanceIdentities` の不変条件の正本）

## File Map

- `crates/synvelis-project/src/diff/types.rs` —
  `ObjectKind::PluginInstance` 追加（enum 末尾、`domain()` =
  `ComparisonDomain::Routing`）、`DIFF_POLICY_VERSION` 2 → 3。
- `crates/synvelis-project/src/diff/adapters.rs` —
  `RoutingAdapter::diff` に `plugin_instances.instances` の
  `diff_map` 照合追加、`classify_routing` に `PluginInstance` arm
  （`id`/`plugin` → `Replaced`、pre-v13 `id` → `Suppressed`）、
  `classify_field` で `pre_v13` を算出して signature 拡張。
- `crates/synvelis-project/src/diff/engine.rs` —
  `raw_kind_map` 除外コメントに `plugin_instances.instances` を
  追記（挙動不変 — 登録しない旨の明記）。
- `crates/synvelis-project/tests/vcs02_diff.rs` —
  plugin-instance 分類の新規セクション。
- `crates/synvelis-project/tests/plugin_instance_persist.rs` /
  `tests/vcs06_apply_matrix.rs` — select-all apply fixture を
  apply-surface kind に絞る helper 追加（`PluginInstance` change
  が `target_fingerprint` で `Unsupported` になる fail-closed を
  反映）。
- `docs/` — 本 chain（plan/task/test-spec/ir）。README index は
  別 worker 占有のため触れない。

## Implementation Sequence

- [x] spec 不要の route 判断を確認（discovery Promotion
  Candidates）+ design/plan/task/test-spec の doc chain 作成
- [x] `types.rs`（ObjectKind + domain + policy version）
- [x] `adapters.rs`（RoutingAdapter diff + classify_routing arm +
  pre_v13 引き回し）+ `engine.rs`（除外コメント）
- [x] `tests/vcs02_diff.rs` plugin-instance 分類セクション
- [x] select-all apply fixture の apply-surface 絞り込み
  （`plugin_instance_persist.rs`・`vcs06_apply_matrix.rs`）
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`build` で検証
- [x] review-protocol レビュー → blocker 解消 → impl-ir 記録
- [x] Conventional Commits で commit → push → PR（Refs #324）

## Dependencies and Risks

- `EntityKey for ProcessorId`・`PluginInstanceIdentity` の
  `PartialEq`+`Serialize` は実装済み — `diff_map` は既存 helper
  でそのまま照合できる。
- `plugin_instances.instances` を `raw_kind_map` に登録すると
  全 Resolved processor が `{Processor, PluginInstance}` の内部
  衝突で `AmbiguousIdentity` を捏造する — keyed-synthetic 除外の
  既存規約どおり登録しない（fixture が cross-kind 衝突ケースを
  カバーする）。
- pre-v13 側の persisted `id` は migration 採番の投影 artifact —
  `Suppressed("pre-v13-plugin-instance-id")` → `PartialProjection`
  で authored change と誤記しない（`pre_v7` と同型の either-side
  規則）。
- `DIFF_POLICY_VERSION` 2 → 3: instance-only 差分が v2 の `Equal`
  から v3 の `Different` に変わるため bump 必須。`ComparisonKey`
  が `policy_version` を束縛するため旧 key と衝突しない
  （RCD-03 v1→v2 と同根拠）。
- 並行 worker が `src/apply/`・ledger・`docs/*/README.md`・
  `tests/vcs05_apply.rs`・`tests/vcs06_monitoring_matrix.rs` を
  占有 — 本 slice はその範囲に触れない。

## Task Breakdown

- `docs/tasks/plugin-instances-vcs-diff-task.md` —
  単一 task（types/adapters 実装 + fixture + 検証）。

## Verification

- [x] `mise run fmt-check` `check` `lint` `test` `test-rt`
  `test-msrv` `build` 全 pass。
- [x] `cargo test -p synvelis-project --test vcs02_diff`
  （補助証拠、canonical = mise run test）— 36 passed。
- [x] fixture が test-spec R1–R5 を網羅。

## Open Questions / Gaps

- `PluginInstance` change の apply/restore 対応（byte-faithful な
  `id` 値復元 + instance watermark reseed）は monotonic
  never-reissued 契約との統合設計が必要で別 Issue 候補 —
  本 slice は fail-closed を維持する。
- VCS-06 ledger への VCS-DIFF 行追記は別 worker 占有 — PR 本文の
  「未対応の作業」に列挙する。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker dispatch 内で実装）。
- Independent work streams: none。
- User approval required before dispatch: yes（worker contract に
  より review-protocol の blocker なしを承認として扱う）。

## Review Handoff

- reviewer は keyed-synthetic 除外（raw_kind_map）、pre-v13 `id`
  suppression の either-side 規則、`Replaced` 分類の根拠、
  `DIFF_POLICY_VERSION` bump の必要性、select-all fixture 絞り込み
  の妥当性、既存 kind 非回帰を重点確認。
