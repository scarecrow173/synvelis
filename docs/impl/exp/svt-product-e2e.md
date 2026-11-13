---
id: "EXP-SvtE2e01aB2cD3eF4gH5iK3"
title: "SV-GUI-05: SVT-01〜09 Headless UI Scenario 実験記録"
created: "2026-11-14"
updated: "2026-11-14"
---

# SV-GUI-05 実験記録: SVT-01〜09

対象: synvelis#114 / SV-GUI-05。実装記録は
`docs/impl/ir/svt-product-e2e.md` を参照。

## 試行と結果

1. **fader commit → project.undo**: `undone:false`。
   mixer fader gesture は `AutomationMode::Off` の
   `SetParameterBase` で session control 層に留まり、project
   history の undoable `ProjectOp` にはならない。→ `set_ppqn`
   （`ProjectOp::SetProjectPpqn`）へ mutation を変更して解決。
2. **transport play phase**: delivery 後も `Applied` で `Observed`
   に到達しない case あり — play Button の terminal は receipt
   権威の `Applied`。`Observed` は correlated projection が
   必要で、transport state 投影が button の observe 対象と
   一致しない場合がある。→ phase を `Applied|Observed` の
   terminal として assert し、receipt broadcast を別途検証。
3. **viz `view_stale`**: `syn.viz.source_changed` op 内の
   `ui_pump` が resync するため、後続 `syn.viz.meta`/`query` では
   `view_stale=false`・`status=ok` のみ観測可能。transient
   `view_stale=true` は `VizUi` 層（`sv48_viz.rs`）で保証済み。
   → app 境界では resolved-state 契約を assert。
4. **graph router**: `route_graph.js` は repo 全体の既存
   broken `defers` relation（本タスクと無関係）で fail-closed
   `blocked`。→ 宣言済み delegate 順（spec→design→plan→task→
   test-spec→impl）を手動で踏襲し、本記録に残す。
