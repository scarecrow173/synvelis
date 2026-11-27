---
id: "TASK-m2Qw8Ln5Vx9Kr6TyPzD4"
type: "task"
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

# TIM-03 production musical transport — Tasks

## Done

- [x] scoped ruviform pin更新（`0479fa62e77e669a7bf84320d3d5418ffb949fc6` → `511e1a7944c35797942ba065c092b9263067c48a`）
- [x] vendor resyncとpin consistency check
- [x] plan-owned project musical source compile
- [x] production ParamDispatch全mintへの供給
- [x] refresh payload preservationとhonest unavailable
- [x] content-free instrument initial musical plan
- [x] control observation tempo revision補正
- [x] player-level integration tests
- [x] VST3/CLAP production real ABI testとmise lane
- [x] JSONL evidence

## Follow-up / NotRun

- [ ] dynamic tempo edit matrix
- [ ] 100/128/256 frame × boundary full matrix
- [ ] fault/state/crash isolation、RT/Crash最終判定
- [ ] GUI/Native Dock/EXT-L3、3OS CI、AudioLens cross-consumer
- [ ] docs index同期（Coordinatorが統合時に実施）

## Dynamic refresh extension tasks

- [x] `Equivalent` revision reconciliation で musical sync が skip される原因を切り分ける。
- [x] `ParamDispatch::refresh_musical_plan` の payload/order/binding preservation を実装する。
- [x] MIDI reconcile が unchanged content を restage せず non-MIDI events を落とさないよう修正する。
- [x] `FastControlWrite::transport_generation` を production control path に通し、`None` compatibilityと`Some` preservationを検証する。
- [x] VST3/CLAP production oracle に representative dynamic tempo edit 2 caseを追加する。
- [x] machine/human evidence を `evidence/sv127/` に保存する。
