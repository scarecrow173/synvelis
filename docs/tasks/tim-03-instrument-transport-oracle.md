---
id: "TASK-q9Lm4Vx2Kw7Rn5TyPzD6"
type: "task"
status: "approved"
title: "TIM-03 instrument direct L1 musical transport oracle"
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
    - "docs/plans/tim-03-instrument-transport-oracle.md"
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

# TIM-03 instrument direct L1 musical transport oracle — Tasks

## 実施項目

- [x] upstream fixture law、direct endpoint transport API、`PluginNode` handoff gap を確認。
- [x] ruviform Issue `https://github.com/scarecrow173/ruviform/issues/1040` を起票。
- [ ] `tests/common/tim03_l1.rs` に必要な helper を追加。
- [ ] `tim03_l1_instrument_transport.rs` を実装。
- [ ] `mise run test-tim03-instrument-transport` を追加。
- [ ] VST3/CLAP で click oracle、negative control、loop/position jump、150 BPM を実走。
- [ ] JSONL/human-readable evidence を追加。
- [ ] 正規検証 lane と prek を実行。
- [ ] review、commit、push、PR 作成。

## 残り（この slice の完了処理）

- [ ] docs index は Coordinator が統合時に整備する。
