---
id: "TASK-034XCSySHeHzz9kpi5K0V8"
type: "task"
status: "done"
title: "RCD-03 monitor session UX task"
created: "2026-11-14"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-03-monitor-session-ux.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-03-monitor-session-ux.md"
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
    - "docs/test-specs/rcd-03-monitor-session-ux.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 monitor session UX task

## Work

`crates/synvelis-app` の `syn.monitoring.session` start params に
session-scope の loop 宣言（`loop_start`/`loop_end`）と
count-in/pre-roll 宣言（`count_in_start`/`count_in_end`・
`pre_roll_start`/`pre_roll_end`）を追加する。
`MonitorUxState::start_session` が宣言を検証し、
`GateSpec::Loop`/`GateSpec::PunchInLoop`（punch は authored
`project.monitoring.punch`）を `preflight` 経由で構築する。
`session_json`/`live_routes` は共有の `transport_phase` 導出で
`CountIn`/`PreRoll`/`PunchWait`/`PunchOpen`/`PostRoll` を投影し、
宣言区間では session frame が負のため `gate_open == false` を
構造的に保証する。`tests/monitor_arm.rs` に E8 fixture 6 test
を追加し、test-spec E8 を検証する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-03-monitor-session-ux.md`
- `docs/designs/rcd-03-monitor-session-ux.md`
- `docs/specs/rcd-03-monitor-session-ux.md`
- `docs/test-specs/rcd-03-monitor-session-ux.md`

## Dependencies

- crate slice（PR #214）: `synvelis_recording::GateSpec`（
  `Loop`/`PunchInLoop` 含む）・`preflight`・`MonitorProbe`・
  `decide`・`TransportPhase`。
- app Arm/Monitoring UX slice（PR #269）: `syn.monitoring.*` op・
  `MonitorUxState`・`session_json` 投影。
- MonitorEngine slice（`docs/impl/ir/rcd-03-monitor-engine.md`）:
  recording 側 `monitor_gate.rs` の Loop/PunchInLoop 6 test。
- persistence slice（PR #307）: `project.monitoring.punch` が
  punch マークの正本。

## Affected Files

- `crates/synvelis-app/src/monitoring.rs`
- `crates/synvelis-app/src/ui_ops.rs`
- `crates/synvelis-app/src/service.rs`
- `crates/synvelis-app/tests/monitor_arm.rs`
- `docs/`（本 chain + index 再生成）
- `evidence/sv117/rcd-03-monitor-session-ux.md`（新規）

## Done When

- [x] `syn.monitoring.session` start が loop/count-in/pre-roll
  宣言を受理し、不正宣言（負 session frame・roll 区間の
  `transport_start` 跨ぎ・空区間・重複・対の片方のみ）を拒否する。
- [x] `session.gate` が `"loop"`/`"punch_in_loop"` を投影し、
  loop/punch マーク値と pass 構造の `gate_open` 区間が正直に
  投影される。
- [x] count-in/pre-roll 宣言区間で `phase` が
  `"count_in"`/`"pre_roll"` になり、`gate_open == false`・
  route は Playback（隠れた PCM を録音しない）。
- [x] authored punch が loop 外の場合 `preflight` が拒否し
  session は開始されない（fail-closed）。
- [x] 宣言は session-scope — project revision・
  `project.monitoring` を消費せず session stop で消える。
- [x] `tests/monitor_arm.rs` E8 6 test + 既存 E1–E7 が pass、
  `monitor_gate`/`monitor_engine` 非回帰。
- [x] `mise run fmt-check`/`check synvelis-app`/`lint
  synvelis-app`/`test synvelis-app`/`build synvelis-app`/
  `test-rt`/`check-app-native`/`lint-app-native` が pass。
- [ ] review-protocol で blocker なし。
- [ ] `git fetch origin main && git merge origin/main` 後の
  再検証。

## Verification

- [x] `mise run test synvelis-app`（canonical）— 42 test binary
  全緑、`monitor_arm.rs` 20 test。
- [x] 補助証拠（cargo 直接実行）: `cargo test --locked
  -p synvelis-recording --test monitor_gate --test
  monitor_punch_session`（24 test）、`-p synvelis-player
  --test monitor_engine`（10 test）— 全緑。
- [x] `mise run test-msrv` は NotRun（Daily CI 委譲、ユーザー
  指定）。
- [ ] `git fetch origin main && git merge origin/main` 後の
  再検証。
