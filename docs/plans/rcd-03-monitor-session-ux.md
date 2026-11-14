---
id: "PLAN-034XCVbR5MMdkD8Zb98FyN"
type: "plan"
status: "approved"
title: "RCD-03 monitor session UX plan"
created: "2026-11-14"
updated: "2026-11-15"
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
    - "docs/specs/rcd-03-monitor-session-ux.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-monitor-session-ux.md"
    - "docs/designs/rcd-03-monitor-session-ux.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-03-monitor-session-ux.md"
  references: []
  defers: []
  deferred-by: []
---
## Goal

`syn.monitoring.session` の start params に session-scope の loop/punch
マークと count-in/pre-roll 宣言を追加し、`GateSpec::Loop`/
`GateSpec::PunchInLoop` の plan 構築と `TransportPhase::CountIn`/`PreRoll`
の位相投影を `synvelis-app` で実現する。

## Scope

単一 implementation stream。`synvelis-app` の monitor session UX 宣言
拡張に閉じ、recording/player/project の意味論・API は変更しない。

## Upstream Documents

- `docs/specs/rcd-03-monitor-session-ux.md`
- `docs/designs/rcd-03-monitor-session-ux.md`

## Design Inputs

- `docs/designs/rcd-03-monitor-session-ux.md`（D1–D5）

## File Map

- `crates/synvelis-app/src/monitoring.rs` - params/state 拡張、gate 構築、位相導出、session_json/probe_segments/live_routes 更新
- `crates/synvelis-app/src/ui_ops.rs` - `op_monitoring_session` start params パース拡張
- `crates/synvelis-app/tests/monitor_arm.rs` - Loop/PunchInLoop/count-in/pre-roll fixture pin（既存拡張）
- `docs/impl/ir/rcd-03-monitor-session-ux.md` - Implementation Record
- `docs/impl/exp/rcd-03-monitor-session-ux.md` - Experiment Log
- `evidence/sv117/` - 検証記録

## Implementation Sequence

- [ ] `MonitorSessionParams`/`MonitorSessionState` に loop/punch/count-in/pre-roll 宣言を追加
- [ ] `start_session` の gate 構築を D2 の優先順位で拡張、境界検証を追加
- [ ] 位相導出を D3 規則で共通化し session_json/live_routes/probe_segments に適用
- [ ] `session_json` の gate 分岐に loop/punch マーク値を投影
- [ ] `op_monitoring_session` の start params パースを拡張
- [ ] `monitor_arm.rs` に Loop/PunchInLoop/count-in/pre-roll fixture を追加
- [ ] 検証（fmt/check/lint/test/build/test-rt/check-app-native/lint-app-native）
- [ ] impl/ir・impl/exp・evidence・index 再生成・PR

## Dependencies and Risks

- `loop_start < transport_start` は負の SessionFrame で scheduler fault → 境界で早期拒否
- count-in/pre-roll 区間の gate 閉鎖は `GateScheduler` 由来であることを fixture で pin（app で再実装しない）
- 既存 Punch/Normal 投影の非回帰は既存テストで担保

## Task Breakdown

- T1: params/state + gate 構築 + 位相導出（monitoring.rs）
- T2: op params パース（ui_ops.rs）
- T3: fixture pin（tests/monitor_arm.rs）
- T4: docs/evidence/検証/PR

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check synvelis-app`
- [ ] `mise run lint synvelis-app`
- [ ] `mise run test synvelis-app`
- [ ] `mise run build synvelis-app`
- [ ] `mise run test-rt`
- [ ] `mise run check-app-native`
- [ ] `mise run lint-app-native`
- [ ] 既存 `monitor_gate`（synvelis-recording）・`monitor_engine`（synvelis-player）非回帰

## Open Questions / Gaps

- なし。session-scope 宣言経路は design D1 で確定済み。
