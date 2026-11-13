---
id: "TASK-6bPp4B6e9YSSWgScL0msLl"
type: "task"
status: "done"
title: "CTL-02: adoption 経路の非 canonical SendLevel 抑止タスク"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/122"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/ctl-02-adopt-sendlevel-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/ctl-02-adopt-sendlevel-plan.md"
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/designs/ctl-02-adopt-sendlevel-design.md"
    - "docs/adr/ctl-02-adopt-sendlevel.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/ctl-02-adopt-sendlevel-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/ctl-02-adopt-sendlevel.md"
  verified-by:
    - "docs/test-specs/ctl-02-adopt-sendlevel.md"
  references: []
  defers: []
  deferred-by: []
---

# CTL-02: adoption 経路の非 canonical SendLevel 抑止タスク

## Work

- T1 — `crates/synvelis-project/src/control.rs`:
  `send_level_spec()`/`has_canonical_send_level_spec()` 追加、
  `canonical_internal_spec` の Send 分岐を共有化
- T2 — `crates/synvelis-project/src/ops.rs`: `AddSend`/
  `SetSendGainTarget` validate に canonical spec 検査追加
- T3 — `crates/synvelis-project/tests/ctl_02.rs`: 非 canonical
  adopt reject・canonical adopt 受理 fixture 追加
- T4 — 最終検証（canonical 9 コマンド）+ impl record + index 再生成

## Classification

- bugfix slice / synvelis-project 層 headless write-time 契約
  （Issue #122 CTL-02 残存観測: adoption 経路の非 canonical
  `SendLevel` persisted 化）

## Upstream Documents

- plan: `docs/plans/ctl-02-adopt-sendlevel-plan.md`
- spec: `docs/specs/ctl-02-adopt-sendlevel.md`
- design: `docs/designs/ctl-02-adopt-sendlevel-design.md`
- adr: `docs/adr/ctl-02-adopt-sendlevel.md`
- test-spec: `docs/test-specs/ctl-02-adopt-sendlevel.md`

## Dependencies

- 前提: `origin/main`（`1cd9d58`）に PR#350 の
  `AddParameterTarget` canonical spec 強制が merge 済みである
  こと。
- 参照のみ: `crates/synvelis-project/src/routing_tx.rs`
  （`SendGainSpec` staging — 変更しない）、
  `docs/impl/ir/ctl-02-strip-send-host-parameter-plugin-binding-routing.md`
  （先行 slice の契約正本）。

## Affected Files

- `crates/synvelis-project/src/control.rs`（変更）
- `crates/synvelis-project/src/ops.rs`（変更）
- `crates/synvelis-project/tests/ctl_02.rs`（変更）
- `docs/` チェーン（specs/adr/designs/plans/tasks/test-specs/
  impl/ir `ctl-02-adopt-sendlevel*`）
- `docs/{specs,adr,designs,plans,tasks,test-specs}/README.md`
  （管理 index 再生成）

## Done When

- [x] 非 canonical spec を持つ target の `AddSend`/
  `SetSendGainTarget`/`RoutingEdit` adopt が `Error::Validation`
  で reject され persisted されない
- [x] canonical spec を持つ target の adopt が引き続き受理され
  `Send`+`SendLevel` に seal される
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build` が
  全て exit 0
- [x] impl record + 管理 index 再生成済み、残件が NotRun/Deferred
  記録

## Verification

- canonical: `mise run fmt-check` / `mise run check` /
  `mise run lint` / `mise run test` / `mise run test-rt` /
  `mise run check-app-native` / `mise run lint-app-native` /
  `mise run test-msrv` / `mise run build` — 全 exit status を
  impl record に記録。

## NotRun / Deferred（正直な記録）

- 実 Plugin/OS・実機受入・#103/#109 Domain Oracle・AudioLens・
  実 Plugin verified binding・Device Oracle — Issue #122 の外部
  gate であり本 slice 対象外。
- snapshot load 時の既存 persisted 非 canonical `SendLevel` への
  対処（load-time oracle/ migration）— 別 slice の判断事項。
