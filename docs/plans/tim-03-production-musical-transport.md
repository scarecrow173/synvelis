---
id: "PLAN-z8Qw5Ln2Vx7Kr4TyPmD9"
type: "plan"
status: "approved"
title: "TIM-03 production musical transport"
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
  implements: []
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

# TIM-03 production musical transport — Plan

1. ruviform pinを `84abc76f` descendant `511e1a7944c35797942ba065c092b9263067c48a` へ scoped updateし、`sync-ui-deps`/`check-ruviform-rev` を通す。
2. `musical_transport.rs` に compile/error/source typeを追加する。
3. `ParamDispatch` の全 mintを一本化し、refresh rebuild APIを追加する。
4. `RoutingControlDispatch` で initial/dynamic sync、production MIDI-plan reconciliation boundaryでの content-free initial empty plan、typed surface evidence、observed span revisionを実装する。
5. player-level `tim03_musical_transport_plan` で PPQN 480/non-default tempo/revision、invalid negative、payload preservationを検証する。
6. production real ABI `tim03_l1_production_musical_transport` を VST3/CLAPで実装し、`test-tim03-production-transport` laneを追加する。
7. canonical lanesを実行し、失敗は scope内外と infrastructure を分類する。
8. docs/evidence/PRを作成し、docs index はCoordinator統合時整備に委譲する。

## 判断記録

- engine層へ新しい所有点を増やさない。既存の every-revision `stage_midi_delivery` が `sync_midi_plans` を呼ぶため、routing-control を authoritative apply boundary にできる。
- 93.75 BPM/PPQN480/256-frame block は 1 host tick=64 frames、1 block=4 ticksになり、adapter PPQN 960 projectionでも fractional tick roundingを oracleへ混入しない。
- loop caseは12 ticks=3 blocksのactive loopで、quarter boundaryが各iterationのblock 2 frame 0に来るよう設計する。
