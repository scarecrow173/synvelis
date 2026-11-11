---
id: "TSPEC-034VvKHc3n0JmiEzTuFtD2"
type: "test-spec"
status: "approved"
title: "PC-C4-05 persisted StablePluginInstanceId test spec"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
    - "docs/designs/pc-c4-05-persisted-instance-id-design.md"
    - "docs/plans/pc-c4-05-persisted-instance-id-plan.md"
    - "docs/tasks/pc-c4-05-task1-persisted-instance-id.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
    - "docs/designs/pc-c4-05-persisted-instance-id-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-05-persisted-instance-id-plan.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
    - "docs/designs/pc-c4-05-persisted-instance-id-design.md"
  verified-by: []
  references:
    - "docs/test-specs/pc-c4-02-e2e-plugin-instance-binding.md"
  defers: []
  deferred-by: []
---

# PC-C4-05 persisted StablePluginInstanceId test spec

## Purpose

Issue #101 の第三 slice が、`StablePluginInstanceId` の採番・永続化・
復元を project schema v13 の identity store に移し、player 側
hydration が persisted identity を fail-closed で消費することを
Fixture+Expected/Observed で固定する。save→reload・restart 横断で
同一 id が復元されることを最重要保証とする。実 VST3/OS・app 側接続
（pc-c4-04）は対象外（NotRun）。

## Feature

Persisted plugin instance identity — project schema v13・identity
store・ops/apply 同期・player hydration（headless）

## Rules

### PC-C4-05-01 — schema v13 永続化と round-trip

- **Given**: `Resolved` processor を持つ project を v13 store に
  `save`。
- **When**: `load` で再構築。
- **Then**: `plugin_instances`（entry 集合・`next` watermark）が
  save 前と完全一致し、各 entry の `PluginRef` が processor の
  authored binding と一致。新規 project の初期状態は
  `watermark=1`・entry 空。

### PC-C4-05-02 — 採番単調性と undo/redo

- **Given**: Resolved processor A を insert（id=1 採番）。
- **When**: A を remove → 新規 Resolved processor B を insert →
  undo（A 復元）→ redo（B 復元）。
- **Then**: B の id は A の id を再利用しない（watermark 単調増分）。
  undo 後の snapshot は A の id=1 をそのまま復元し、redo 後は B の
  採番済み id を復元する（snapshot 復元であり再採番ではない）。

### PC-C4-05-03 — oracle reject（load/apply 双方）

- **Given**: identity store に以下のいずれかの不正を持つ project
  state を構成: (a) entry の processor が存在しない、(b) entry の
  `PluginRef` が processor binding と不一致（descriptor/version/
  `capability_revision` いずれか差分）、(c) 2 entry が同一 id、
  (d) id が watermark 範囲外（`id >= next` または `id == 0`）、
  (e) `Resolved` processor に entry が無い、(f) `Unresolved`
  processor に entry がある。
- **When**: `load`（または apply 後の `validate_references`）。
- **Then**: typed `Error::Validation`/`Error::NotFound` で fail-closed
  reject。silent repair・default 補完はしない。

### PC-C4-05-04 — v12→v13 migration backfill

- **Given**: schema v12 の snapshot bytes（`plugin_instances` field
  なし）を `load`。
- **When**: v12→v13 migration step が走る。
- **Then**: 全 `Resolved` processor が `ProcessorId` 昇順に id
  1..=N で backfill され、`next = N+1`。`Unresolved` processor は
  entry を持たない。元 bytes は書き換えない（load 時の projection
  のみ）。

### PC-C4-05-05 — VCS apply 同期

- **Given**: plugin host が利用可能な project に対する apply。
- **When**: (a) `Resolved` processor の Added change、
  (b) plugin 不変の Modified change、(c) plugin 変更の Modified
  change、(d) `Unresolved` processor の Added change を apply。
- **Then**: (a) 新規 id 採番・apply 成功（reject されない）、
  (b) 既存 id 維持、(c) 新規 id 採番、(d) entry 無しで成功。
  apply 後の `validate_references` が全 invariant を満たす。

### PC-C4-05-06 — player hydration と fail-closed

- **Given**: persisted identity を持つ project から構築した
  `RoutingEngine` + `RoutingControlDispatch`。
- **When**: `vend_nodes` が semantic processor を vend する。
- **Then**: `ProcessBoundarySession::open` provenance と
  `PluginBindingSession::attach` に persisted `StablePluginInstanceId`
  が渡る。project を save→load して engine を再構築しても同一 id が
  復元される。identity entry 欠落・`PluginRef` 不一致・watermark
  範囲外の processor は `without_control` evidence に記録され、
  session/binding を生成せず delivery 0。

### PC-C4-05-07 — NotRun 宣言

- 実 VST3/OS backend・実 GUI・app/workspace 側の接続配線
  （pc-c4-04 別 slice）・`driver.rs` の clip-insert FX ephemeral id
  （別 domain）は NotRun。`ProcessorBinding` の正式 rebind op は
  deferred。

## Fixtures

- `crates/synvelis-project/tests/plugin_instance_persist.rs`（新規）:
  - `fresh_project_has_empty_identity_store` — 初期 watermark=1・空。
  - `resolved_insert_allocates_and_roundtrips` — insert→save→load で
    同一 entry・同一 id。
  - `unresolved_insert_has_no_entry` — Unresolved は entry 無し。
  - `remove_then_insert_never_reuses_id` — remove→insert で id が
    watermark を超えて単調増分。
  - `undo_redo_restores_identity_snapshot` — undo/redo で entry 集合が
    snapshot 復元される。
  - `v12_snapshot_migrates_with_deterministic_backfill` — v12 bytes
    → v13 で ProcessorId 昇順 backfill・`next=N+1`。
  - `oracle_rejects_*` — dangling entry・plugin 不一致
    （capability_revision 差分含む）・id 重複・watermark 範囲外・
    Resolved entry 欠落・Unresolved entry の各 reject。
  - `vcs_apply_*` — Added/Resolved 採番・plugin 不変 Modified 維持・
    plugin 変更 Modified 再採番・Unresolved Added 成功。
- `crates/synvelis-player/tests/pc_c4_05_persisted_instance.rs`（新規）:
  - `hydration_uses_persisted_instance_id` — session/binding の
    provenance が persisted id と一致。
  - `reload_restores_same_instance_id` — save→load→engine 再構築で
    同一 id。
  - `missing_or_mismatched_identity_fails_closed` — entry 欠落・
    plugin 不一致で `without_control`・delivery 0。

## Evidence

- `mise run test`（`cargo test --workspace --locked`、canonical）。
- `mise run check` / `mise run lint` / `mise run fmt-check`。
- `mise run test-rt` / `mise run test-msrv` / `mise run
  check-headless-deps`。
- 実 plugin backend・app 配線: NotRun（follow-up gate）。

## Guarantees

- save→reload・engine restart 横断で authored processor の
  `StablePluginInstanceId` が不変。
- 発行済み id は processor 削除後も再利用されない（watermark 単調）。
- persisted state の identity 不整合は load/apply の両境界で typed
  reject され、runtime は不正 state を補完しない。
- `PluginRef.capability_revision` 差分は別 authored identity として
  新規 id を採番する。

## Non-goals

- 実 plugin backend・GUI・app/workspace 配線の検証。
- `driver.rs` clip-insert FX の ephemeral id 採番（別 domain）。
- `ProcessorBinding` rebind の正式 op 化（deferred）。
- binding snapshot の永続化（pc-c4-02 deferred のまま）。

## Risk

- oracle が弱いと不正 persisted state が runtime まで到達し、
  stale binding が静かに通る。`validate_references` での全単射
  検査がこの保証の根拠。
- apply 経路の sync 漏れは post-apply oracle が必ず検出するが、
  回帰 test で事前に gate する。
