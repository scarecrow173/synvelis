---
id: "TASK-5FrDNAlmiEt3QTiDj5a82C"
type: "task"
status: "in-progress"
title: "SF-04 slice4 T1 — app/session wiring（probe port・syn.probe.*・drain hook）"
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
    - "docs/plans/sf-04-slice4-app-wiring.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-04-slice4-app-wiring.md"
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
    - "docs/test-specs/sf-04-slice4-app-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice4 T1 — app/session wiring（probe port・syn.probe.*・drain hook）

## Work

1. `synvelis-player`: `ProbeTraceBridge::insert_adapter` 追加。
2. `synvelis-app/engine.rs`: `Inner.engine` → `Inner.bridge`
   （`ProbeTraceBridge`）再構成 + `with_probe_bridge`/
   `register_probe_adapter`/`route_probe_target`/
   `unroute_probe_target`。
3. `synvelis-app/ui_ops.rs`: `last_probe_drain` field、
   `drain_probe`（`ui_pump` tail）、`op_probe_control`/
   `op_probe_state`、param parse + `ProbeTraceError`/`ProbePortError`
   → `ErrorBody` 写像。
4. `synvelis-app/service.rs`: `syn.probe.control`/`syn.probe.state`
   登録 + dispatch。
5. `synvelis-app/tests/sf04_probe_wiring.rs`: headless fixture。
6. docs index 再生成・mise 検証・review-protocol・commit/push/PR。

## Done When

- `syn.probe.control`/`syn.probe.state` が wire 経由で動作し、
  engine 未接続時に誠実なエラー/空投影を返す。
- `ui_pump` drain hook が `ControlSession::probe_drain` を唯一経路
  として呼ぶ。
- probe op が project revision/undo/graph_builds/master PCM を
  変えないことがテストで確認される。
- `mise run check`/`lint`/`test`（対象範囲）が green。
