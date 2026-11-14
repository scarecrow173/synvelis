---
id: "PLAN-vcs05-plugin-instance-apply"
type: "plan"
status: "approved"
title: "VCS-05 PluginInstance apply surface 拡張 — 実装 plan"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "SPEC-vcs05-plugin-instance-apply"
    - "DSGN-vcs05-plugin-instance-apply"
  implemented_by:
    - "TASK-vcs05-plugin-instance-apply"
  derives_from:
    - "SPEC-vcs05-plugin-instance-apply"
    - "DSGN-vcs05-plugin-instance-apply"
  verified_by:
    - "TSPEC-vcs05-plugin-instance-apply"
  supersedes: []
  superseded_by: []
---

# VCS-05 PluginInstance apply surface 拡張 — 実装 plan

## Goal

`DIFF_POLICY_VERSION = 3` で diff 分類済みの
`ObjectKind::PluginInstance` を VCS-05 apply whitelist に追加し、
`Added`/`Replaced` の typed entity transplant（byte-faithful
`PluginInstanceIdentity` + monotone watermark reseed）を可能に
する。`Removed` は全 kind で fail-closed 維持。
`plugin-instances-vcs-diff-design.md` の Deferred Design
Concern を本 slice で解消する。

## Scope

`crates/synvelis-project/src/apply/entity.rs`・`src/apply/plan.rs`
（`change_supported` の `Replaced` arm）・`src/apply/mod.rs`
（doc のみ）と `tests/vcs02_diff.rs`・`tests/vcs06_apply_matrix.rs`・
`tests/plugin_instance_persist.rs`・`tests/vcs06_plugin_instance_
matrix.rs`、`evidence/reports/vcs-06-fixture-matrix-ledger.md`、
`docs/designs/plugin-instances-vcs-diff-design.md`（Deferred
Concern 更新）、本 doc chain に閉じる。`src/diff/`・
`src/plugin_instance.rs`・`src/project.rs`・`src/ops.rs`・他
crate・README index は変更しない。

## Upstream Documents

- `docs/specs/vcs-05-plugin-instance-apply.md`
- `docs/designs/vcs-05-plugin-instance-apply.md`
- `docs/specs/vcs-05-apply-restore-headless.md`（apply 契約の正本）
- `docs/designs/plugin-instances-vcs-diff-design.md`
  （diff 側 keyed-synthetic 規約・Deferred Concern の正本）
- `crates/synvelis-project/src/plugin_instance.rs`
  （identity store 不変条件の正本）

## File Map

- `crates/synvelis-project/src/apply/entity.rs` —
  `supported_kind` に `PluginInstance` 追加。
  `entity_value` に keyed-synthetic arm（`ProcessorId::
  from_tagged` → `plugin_instances.instances` 参照）。
  `write_entity` に専用 arm（`PluginInstanceIdentity` 挿入 +
  `watermark = max(live, after.plugin_instances.watermark)`
  reseed、`max_new_id` 非寄与を comment 明記）。
  `plugin_instance::PluginInstanceIdentity` import 追加。
- `crates/synvelis-project/src/apply/plan.rs` —
  `change_supported` の `Replaced` arm 許可 kind に
  `PluginInstance` 追加、detail message 更新。
- `crates/synvelis-project/src/apply/mod.rs` — module doc の
  scope-out 文言更新（`PluginInstance` が `Added`/`Replaced`
  で apply surface に乗ること、`Removed` は全 kind deferred
  のままを明記）。
- `crates/synvelis-project/tests/vcs02_diff.rs` —
  `plugin_instance_change_is_outside_apply_surface` を
  `target_fingerprint` 解決の新契約 assert へ書換え
  （rename: `plugin_instance_change_is_on_apply_surface`）。
- `crates/synvelis-project/tests/vcs06_apply_matrix.rs` —
  `apply_surface_kind` に `TrackMonitorDecl`/`PunchMarks`/
  `PluginInstance` 追加（`supported_kind` mirror の完全同期）、
  stale comment 2 箇所更新。
- `crates/synvelis-project/tests/plugin_instance_persist.rs` —
  `apply_surface_kind` 同上、comment 更新。
- `crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs` —
  apply fixture helper（`snap_manual`/`compare`/`Src`/
  `sources_for`/`intent_for`/`run_apply`/`AllPlugins`/
  `journal_key`/`LossyJournal`）追加 + apply test 7 本
  （module doc 更新）。
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` —
  VCS-APPLY-07 行へ plugin-instance apply 実測を追記、
  Environment ブロック・fixture test 数を実測で更新。
- `docs/designs/plugin-instances-vcs-diff-design.md` —
  Deferred Design Concern を本 slice design への解決済み
  参照へ更新。
- `docs/` — 本 chain 6+1 文書（README index は対象外）。

## Implementation Sequence

- [ ] spec/design/plan/task/test-spec の doc chain 作成
- [ ] `entity.rs`（supported_kind/entity_value/write_entity）
- [ ] `plan.rs`（`change_supported` Replaced arm）
- [ ] `mod.rs` module doc 更新
- [ ] `tests/vcs02_diff.rs`・`tests/vcs06_apply_matrix.rs`・
  `tests/plugin_instance_persist.rs` 更新
- [ ] `tests/vcs06_plugin_instance_matrix.rs` apply fixture 追加
- [ ] `origin/main` merge → 衝突確認
- [ ] ledger 更新 + `docs/designs/plugin-instances-vcs-diff-
  design.md` Deferred Concern 更新
- [ ] `mise run fmt-check`/`check`/`lint`/`test` で検証
- [ ] impl-ir/impl-exp 記録 + docs index 再生成
- [ ] Conventional Commits で commit → push → PR（Refs #96）

## Dependencies and Risks

- `plan.rs` の `change_supported` が `Removed` を全 kind で
  `Unsupported` にする既存契約に依存 — `PluginInstance` の
  `Removed` 拒否は plan 層のまま（entity 層で Removed 用
  write を実装しない）。
- `validate_against` oracle が entry↔binding bijection・id
  range・一意性を最終検査 — apply 層は事前検査を追加せず
  oracle に委ねる（既存契約、YAGNI）。
- byte-faithful transplant の retired-id 衝突受容は design
  で文書化済み — restore-as-copy と同じ semantics。
- `Processor` write との write 順で watermark 中間値は変わり
  うるが最終 entry は after と一致（fingerprint/replay は
  watermark 非対象）。
- 並行 Worker が `src/diff/`・他 test file を変更中の可能性
  — 本 slice は apply/ と指定 test file のみに触れる。
  merge 時 conflict は最小限の見込み。

## Task Breakdown

- `docs/tasks/vcs-05-plugin-instance-apply-task.md` —
  単一 task（entity.rs + plan.rs + mod.rs + fixtures +
  ledger + docs + 検証）。

## Verification

- [ ] `mise run fmt-check` `check` `lint` `test` 全 pass。
- [ ] `cargo test -p synvelis-project --test vcs02_diff
  --test vcs06_apply_matrix --test plugin_instance_persist
  --test vcs06_plugin_instance_matrix`（補助証拠、
  canonical = mise run test）。
- [ ] fixture が test-spec T1–T5 を網羅。

## Open Questions / Gaps

- `Removed` cascade・recording/comping/SMF 領域 apply・
  retired-id 履歴の store 化は別設計（spec 参照）。

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

- reviewer は byte-faithful transplant + `max` watermark reseed
  の never-reissued 保証、`max_new_id` 非寄与、Processor write
  との順序非依存、oracle 委譲の妥当性、`Removed` fail-closed
  維持、既存 kind 非回帰を重点確認。
