---
id: "PLAN-vcs05-monitoring-apply-surface"
type: "plan"
status: "approved"
title: "VCS-05 monitoring kinds apply surface 拡張 — 実装 plan"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "SPEC-vcs05-monitoring-apply-surface"
    - "DSGN-vcs05-monitoring-apply-surface"
  implemented_by:
    - "TASK-vcs05-monitoring-apply-surface"
  derives_from:
    - "SPEC-vcs05-monitoring-apply-surface"
    - "DSGN-vcs05-monitoring-apply-surface"
  verified_by:
    - "TSPEC-vcs05-monitoring-apply-surface"
  supersedes: []
  superseded_by: []
---

# VCS-05 monitoring kinds apply surface 拡張 — 実装 plan

## Goal

PR #318 で diff 分類済みの `ObjectKind::TrackMonitorDecl`/
`PunchMarks` を VCS-05 apply whitelist に追加し、`Added`/
`Modified` の typed entity transplant を可能にする。
`Removed` は全 kind で fail-closed 維持。VCS-APPLY-07 の証拠行を
実測 verdict へ更新する。

## Scope

`crates/synvelis-project/src/apply/entity.rs`・`src/apply/mod.rs`
（doc のみ）と `tests/vcs05_apply.rs`・`tests/vcs06_monitoring_
matrix.rs`、`evidence/reports/vcs-06-fixture-matrix-ledger.md`、
本 doc chain に閉じる。`src/diff/`・`src/apply/plan.rs`・
他 crate・README index は変更しない。

## Upstream Documents

- `docs/specs/vcs-05-monitoring-apply-surface.md`
- `docs/designs/vcs-05-monitoring-apply-surface-design.md`
- `docs/specs/vcs-05-apply-restore-headless.md`（apply 契約の正本）
- `docs/designs/rcd-03-vcs-diff-monitor-classification-design.md`
  （diff 側 identity 規約の正本）

## File Map

- `crates/synvelis-project/src/apply/entity.rs` —
  `supported_kind` に 2 kind 追加。`entity_value` に
  `TrackMonitorDecl`（`TrackId::from_tagged` →
  `monitoring.tracks` 参照）と `PunchMarks`
  （declaration-presence）の専用 arm。`write_entity` に
  `keyed_insert!` 非使用の 2 arm（`monitoring.tracks.insert` /
  `monitoring.punch` 代入）。`monitoring_data` import 追加。
- `crates/synvelis-project/src/apply/mod.rs` — module doc の
  scope-out 文言を更新（monitoring 宣言が Added/Modified で
  apply surface に乗ること、Removed は全 kind deferred のまま
  を明記）。
- `crates/synvelis-project/tests/vcs05_apply.rs` —
  `monitor_decl_and_punch_apply_as_one_undo_step` 追加
  （monitoring import・`MusicalTick` import 追加）。
- `crates/synvelis-project/tests/vcs06_monitoring_matrix.rs` —
  `unsupported_intent` を `intent_for`（実 fingerprint）へ改名・
  汎化。`monitor_kind_selection_fails_apply_closed` を
  `monitor_kind_added_selection_applies_typed_entities`・
  `monitor_kind_removed_selection_stays_fail_closed`・
  `monitor_kind_modified_selection_applies_typed_entities` の
  3 test へ再構成（`MonitorFixture`/`apply_added_selection`
  helper 追加）。module doc 更新。
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` —
  VCS-APPLY-07 行を supported-apply 実測へ更新、Environment
  ブロックを実走環境（commit・ruviform rev・schema・toolchain）
  で更新、fixture test 数を実数へ修正。
- `docs/` — 本 chain 6 文書（README index は対象外）。

## Implementation Sequence

- [x] spec/design/plan/task/test-spec の doc chain 作成
- [x] `entity.rs`（supported_kind/entity_value/write_entity）
- [x] `mod.rs` module doc 更新
- [x] `tests/vcs05_apply.rs`・`tests/vcs06_monitoring_matrix.rs`
- [x] `origin/main` merge（#324 PluginInstance diff 取り込み、
  conflict なし）
- [x] ledger VCS-APPLY-07 行 + Environment 更新
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`build` で検証
- [ ] Conventional Commits で commit → push → PR（Refs #96）
  → Issue #96 コメント

## Dependencies and Risks

- `plan.rs` の `change_supported` が `Removed` を全 kind で
  `Unsupported` にする既存契約に依存 — monitoring の `Removed`
  拒否は plan 層のまま（entity 層で Removed 用 write を
  実装しない）。
- `PunchMarks` の declaration-presence 規則が diff classifier
  （`diff_punch_marks`）と apply entity view の 2 箇所に存在 —
  drift リスクは test-spec の presence 遷移 fixture で固定。
- `TrackMonitorDecl` は `stable_id` が `Track` と同一 ID 空間を
  借用 — fingerprint material は kind を含むため衝突しないが、
  `entity_value` の parse 失敗時は `InvalidIntent` で
  fail-closed（通常経路では発生しない防御）。
- 並行 Worker が `src/diff/`・`vcs06_apply_matrix.rs`・
  `plugin_instance_persist.rs` を変更中 — 本 slice はその領域に
  触れない。merge 時の conflict は merge-base が近いため
  最小限の見込み。

## Task Breakdown

- `docs/tasks/vcs-05-monitoring-apply-surface-task.md` —
  単一 task（entity.rs + mod.rs + fixtures + ledger + 検証）。

## Verification

- [ ] `mise run fmt-check` `check` `lint` `test` `test-rt`
  `test-msrv` `build` 全 pass。
- [ ] `cargo test -p synvelis-project --test vcs05_apply
  --test vcs06_monitoring_matrix`（補助証拠、canonical =
  mise run test）。
- [ ] fixture が test-spec T1–T4 を網羅。

## Open Questions / Gaps

- `Removed` cascade apply・`PluginInstance` apply・
  recording/comping/SMF 領域 apply は別設計（spec 参照）。

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

- reviewer は keyed-synthetic view（`stable_id` → `TrackId`
  parse → `monitoring.tracks`）、PunchMarks の
  declaration-presence が diff classifier と一致すること、
  `max_new_id` 非寄与の妥当性、`Removed` fail-closed 維持、
  既存 kind 非回帰を重点確認。
