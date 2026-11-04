---
id: "TASK-034CTL01Sa3yBv9cE5fG6hIj"
type: "task"
status: "done"
title: "CTL-01: ControlOwner/AutomationTimeScope/ParameterRole 型・oracle・Schema v7 Migration・ops・検証"
created: "2026-11-05"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/121"
  changes:
    added:
      - "crates/synvelis-project/tests/ctl_01.rs"
      - "crates/synvelis-gateway/tests/ctl_01.rs"
    modified:
      - "crates/synvelis-project/src/control.rs"
      - "crates/synvelis-project/src/project.rs"
      - "crates/synvelis-project/src/ops.rs"
      - "crates/synvelis-project/src/migration.rs"
      - "crates/synvelis-project/src/store.rs"
      - "crates/synvelis-project/src/routing.rs"
      - "crates/synvelis-project/src/midi.rs"
      - "crates/synvelis-project/src/lib.rs"
      - "crates/synvelis-gateway/src/snapshot.rs"
      - "crates/synvelis-gateway/src/request.rs"
      - "crates/synvelis-gateway/src/prepare.rs"
      - "crates/synvelis-gateway/src/job.rs"
      - "crates/synvelis-gateway/tests/authorization.rs"
      - "crates/synvelis-workspace/src/control.rs"
      - "crates/synvelis-workspace/src/gateway_ui.rs"
      - "crates/synvelis-project/tests/controller_store.rs"
      - "crates/synvelis-project/tests/md_a.rs"
      - "crates/synvelis-project/tests/midi_source.rs"
      - "crates/synvelis-project/tests/pc_b_rt.rs"
      - "crates/synvelis-project/tests/routing_sf01.rs"
      - "crates/synvelis-workspace/tests/pc_a.rs"
      - "crates/synvelis-workspace/tests/pc_b.rs"
      - "crates/synvelis-workspace/tests/pc_c1.rs"
      - "crates/synvelis-workspace/tests/sv47_controls.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/ctl-01-control-owner-time-scope.md"
  implemented-by:
    - "docs/impl/ir/ctl-01-control-owner-time-scope.md"
  depends-on:
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
    - "docs/adr/ctl-01-control-owner-time-scope-schema-v7.md"
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
    - "docs/test-specs/ctl-01-e2e-control-owner-time-scope.md"
  references: []
  defers: []
  deferred-by: []
---

# CTL-01 Task 1: Owner/TimeScope 型・oracle・Schema v7 Migration・検証

## Goal

Issue #121 の型分離を `synvelis-project` に実装し、v6→v7 migration・
write-time/final oracle 検証・downstream 追随・test-spec の Rule を
検証する。

## Steps

1. `control.rs`: `ControlScope` を `ControlOwner`/`AutomationTimeScope`/
   `ParameterRole` に置換。`ControlOwner` の手書き Deserialize(legacy
   `scope` tag 受理)。`resolve_scope_pos(project, owner, time_scope,
   pos)`。`validate_control_binding`/`validate_control_owner` 集約。
   `ParameterTarget`・`ModulationSource`・`PreparedTarget` 更新。
2. `project.rs`: `validate_references` — owner 存在(routing maps
   含む)・owner×time_scope matrix・singleton (owner,role) 一意性・
   `gain_target`→`ControlOwner::Send(send.id)` owner mapping。
3. `ops.rs`: `AddParameterTarget`/`AddModulationSource` 新 field・
   write-time `validate_control_binding`。`RemoveTrack`/`RemoveBus`/
   `RemoveSend`/`remove_strip_owned` の cascade を監査し、routing
   entity 所有 target/source/lane/assignment の dangling を除去。
4. `migration.rs`: `migrate_v6_control` step 追加。`store.rs`:
   `SCHEMA_VERSION = 7`。
5. downstream: gateway `control_owner_json`/`ResourceRef`/`ResourceKind`
   (Strip/Send/Processor)・workspace `record_points`/`target_scope`・
   `midi.rs`/`lib.rs`・全既存テストを新 API へ。
6. `tests/ctl_01.rs`: test-spec Rule 群。golden eval migration
   equality・matrix・uniqueness・gain_target mapping・Track/Bus→
   Strip 一意性・ProjectAbsolute Unavailable・save/reload round-trip。
7. `mise run fmt` → `check` → `lint` → `test` → `test-rt` →
   `test-msrv`。`git fetch origin && git merge origin/main` 後に
   再検証。impl record 作成。

## Acceptance

- `ctl_01.rs` 全 test PASS、既存 `pc_*`/`routing_sf01`/`controller_store`
  test が新 API で PASS。
- `mise run` 全 gate PASS・warning なし。
- v6 snapshot → v7 への read-only migration が座標厳密保持・
  deterministic。
