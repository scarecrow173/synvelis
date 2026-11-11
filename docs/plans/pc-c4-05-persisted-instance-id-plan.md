---
id: "PLAN-034VvHwf0OT5qLdTopdnmC"
type: "plan"
status: "approved"
title: "PC-C4-05 persisted StablePluginInstanceId plan"
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
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
    - "docs/designs/pc-c4-05-persisted-instance-id-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-05-persisted-instance-id.md"
  verified-by:
    - "docs/test-specs/pc-c4-05-persisted-instance-id.md"
  references:
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
    - "docs/designs/pc-c4-05-persisted-instance-id-design.md"
  defers: []
  deferred-by: []
---

# PC-C4-05 persisted StablePluginInstanceId plan Implementation Plan

## Goal

`StablePluginInstanceId` の採番・永続化・復元を `synvelis-project` の
schema v13 identity store に移し、`RoutingControlDispatch` が persisted
identity を hydration する。save→reload・restart 横断で同一 id が
復元され、unknown/stale identity は全境界で fail-closed。

## Scope

単一 implementation stream — project 側の store/migration/oracle と
player 側の hydration は同一不変条件の表裏であり、分割すると
中間状態（persisted store はあるが hydration が ephemeral のまま）
が検証不能になる。worker task は 1 件（task1）に集約する。

## Upstream Documents

- `docs/specs/pc-c4-05-persisted-instance-id.md`

## Design Inputs

- `docs/designs/pc-c4-05-persisted-instance-id-design.md`（approved）

## File Map

| Path | Responsibility |
| --- | --- |
| `crates/synvelis-project/src/plugin_instance.rs` | 新規: `PluginInstanceId`・`PluginInstanceIdentity`・`PluginInstanceIdentities`（allocate/ensure/remove/sync_processor/resolve/validate_against） |
| `crates/synvelis-project/src/project.rs` | `Project::plugin_instances` field・`Project::new` init・`validate_references` へ oracle 呼出 |
| `crates/synvelis-project/src/store.rs` | `SCHEMA_VERSION = 13` + 履歴 doc |
| `crates/synvelis-project/src/migration.rs` | `MigrationStep{from:12}` + `migrate_v12_plugin_instances`（ProcessorId 昇順 backfill） |
| `crates/synvelis-project/src/ops.rs` | `InsertProcessor` で `allocate`、`RemoveProcessor` で `remove` |
| `crates/synvelis-project/src/apply/entity.rs` | `write_entity` Processor arm で `sync_processor`（Coordinator 承認済み拡張） |
| `crates/synvelis-project/src/lib.rs` | `pub mod plugin_instance` + 型 export |
| `crates/synvelis-project/tests/plugin_instance_persist.rs` | 新規: round-trip・watermark 単調・oracle reject・migration・apply 回帰 |
| `crates/synvelis-project/tests/md_a.rs`, `tim_02_smf.rs` | `SCHEMA_VERSION` literal pin があれば 13 へ更新 |
| `crates/synvelis-player/src/routing_control.rs` | `vend_nodes` を `resolve` hydration に置換・`next_instance` 撤去 |
| `crates/synvelis-player/src/binding.rs` | `StablePluginInstanceId` doc 更新のみ |
| `crates/synvelis-player/tests/pc_c4_05_persisted_instance.rs` | 新規: hydration・reload 同一 id・missing/stale fail-closed |

## Implementation Sequence

- [ ] T1 — project tests を先に書く（red）: `plugin_instance_persist.rs`
  に save→load round-trip・採番単調・oracle reject・v12→v13 migration・
  VCS apply 回帰の各 test を実装。
- [ ] T2 — `plugin_instance.rs` 新設 + `lib.rs`/`project.rs`/`store.rs`/
  `migration.rs`/`ops.rs`/`apply/entity.rs` を実装し T1 を green に
  （green）。
- [ ] T3 — player tests を先に書く（red）: `pc_c4_05_persisted_instance.rs`
  に hydration・reload 同一 id・entry 欠落/plugin 不一致の
  `without_control` を実装。
- [ ] T4 — `routing_control.rs` hydration + `binding.rs` doc を実装し
  T3 を green に。
- [ ] T5 — `mise run fmt` → `check` → `lint` → `test` → `test-rt` →
  `test-msrv` → `check-headless-deps` を順に実行し全て green。

## Dependencies and Risks

- T1→T2→T3→T4→T5 は直列（TDD red→green）。
- Risk: apply 回帰 — `write_entity` の sync 漏れは post-apply oracle
  が必ず reject するため test で検出可能（T1 の apply 回帰 test が
  その gate）。
- Risk: `SCHEMA_VERSION` pin を持つ既存 test の literal 更新漏れ —
  `mise run test` で検出。
- Risk: v12 body の `plugin_instances` 欠落 — `#[serde(default)]` +
  migration backfill で担保（T1 migration test が検証）。

## Task Breakdown

- task1（`docs/tasks/pc-c4-05-task1-persisted-instance-id.md`）:
  project store/schema/migration/ops/apply + player hydration + tests
  の全量。独立 stream なし。

## Verification

- [ ] `mise run test` — 新規 project/player test + 既存全 test green
- [ ] `mise run check` / `mise run lint` — warning ゼロ
- [ ] `mise run fmt-check` — 差分なし
- [ ] `mise run test-rt` / `mise run test-msrv` / `mise run check-headless-deps` — 全て green
- [ ] save→load で `plugin_instances` が byte 同一の project state を
  復元（test が `Project` PartialEq で検証）

## Open Questions / Gaps

- `ProcessorBinding` の正式 rebind op は deferred（design 参照）。
- app 側接続は pc-c4-04 別 slice — 本 plan では `NotRun` 明記。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker が T1→T5 を直列実行）
- Independent work streams: none
- User approval required before dispatch: no（dispatch 済み worker task）

## Review Handoff

- `apply/entity.rs` の範囲拡張は Coordinator 承認済み — reviewer は
  承認条件（Added/Resolved 新規・plugin 不変 Modified 維持・
  Unresolved 除去・ensure 意味論）との一致を確認すること。
- `driver.rs` を変更しない判断（clip-insert FX は別 domain）の妥当性。
