---
id: "PLAN-4fJZbNlcZy6NsCvF40RggB"
type: "plan"
status: "approved"
title: "SF-04 slice4 実装計画 — app/session wiring（probe port・syn.probe.*・drain hook）"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-04-slice4-app-wiring.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice4-app-wiring.md"
    - "docs/designs/sf-04-slice4-app-wiring-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice4 実装計画 — app/session wiring（probe port・syn.probe.*・drain hook）

## Goal

slice 2 が defer した app 層 port 供給を実装し、`syn.probe.*` ops と
`ui_pump` drain hook で Probe ON/OFF・観測 drain を wire 面へ出す。
probe 意味論は変更しない — 配線と wire 契約のみ。

## File Map

| File | Change |
| --- | --- |
| `crates/synvelis-player/src/probe_trace.rs` | `ProbeTraceBridge::insert_adapter` 追加（additive） |
| `crates/synvelis-app/src/engine.rs` | `Inner.engine` → `Inner.bridge: ProbeTraceBridge` 再構成 + `with_probe_bridge`/`register_probe_adapter`/`route_probe_target`/`unroute_probe_target` |
| `crates/synvelis-app/src/ui_ops.rs` | `UiOpsState::last_probe_drain`、`drain_probe`（ui_pump tail）、`op_probe_control`、`op_probe_state`、param parse/error 写像 |
| `crates/synvelis-app/src/service.rs` | `syn.probe.control`/`syn.probe.state` を OPS 登録 + dispatch arm |
| `crates/synvelis-app/tests/sf04_probe_wiring.rs` | headless fixture（新規） |
| `docs/` | spec/design/plan/task/test-spec + index 再生成 |

`crates/synvelis-workspace`・`crates/synvelis-project` は変更なし
（既存 API をそのまま使う）。

## Steps

1. `ProbeTraceBridge::insert_adapter` — adapter 事後登録経路を開く。
2. `SessionRoutingEngine` を bridge 所有へ再構成し port 供給 API を
   追加。既存公開 API（`run`/`receipt`/`probe_bindings`/…）の
   シグネチャは維持。
3. `syn.probe.control`/`syn.probe.state` op + drain hook を実装。
4. headless fixture で wire 契約・drain・不変条件を検証。
5. `mise` 検証タスク + `origin/main` merge + review-protocol。

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-app-native`（存在する範囲）。
- `crates/synvelis-app/tests/sf04_probe_wiring.rs`:
  probe request→state→drain→remove の wire 経路、engine 未接続の
  fail-closed、revision/graph_builds 不変、同一 point 2 target の
  相関分離。

## Risks

- `RefCell` borrow の再入: `with_probe_bridge` の `f` 内で
  `engine.run`/`on_project_revision` を呼ぶと二重 borrow panic。
  op 実装は `control_with`/`control_mut` のみ呼び engine 公開
  メソッドは呼ばないことで回避（drain も `probe_drain` のみ）。
- `ProbeSubscriptionKey` の serde round-trip: request 戻り値の
  `key` をそのまま後続 op に渡す契約で検証する。
