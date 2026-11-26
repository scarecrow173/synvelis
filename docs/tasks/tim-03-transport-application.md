---
id: "TASK-w5Vn8Qx2Lm4Kr7TyZpD3"
type: "task"
status: "approved"
title: "TIM-03 transport application minimal vertical"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/tim-03-transport-application.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# TIM-03 transport application minimal vertical — Tasks

## 実施済み

- [x] merged TIM-03 infrastructure と L1 runner/fixture contract の調査。
- [x] `TransportApplicationCommand` / conversion evidence / report / typed error を追加。
- [x] `SessionRoutingEngine::control_apply_transport` を追加。
- [x] boundary declaration を受け取る production composition variant を追加。
- [x] shared L1 helper `tests/common/tim03_l1.rs` を追加。
- [x] `tim03_l1_transport_application.rs` で VST3/CLAP deterministic scenario を実装。
- [x] `mise run test-tim03-transport` を追加。
- [x] `evidence/sv127/transport-application.jsonl` を追加。

## 残り（この slice の完了処理）

- [x] 正規検証 lane と prek を実行し、`fmt-check` の os error 206 を infrastructure failure として記録。
- [ ] commit、push、PR 作成。
- [ ] docs index は Coordinator が統合時に整備する。
