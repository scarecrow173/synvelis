---
id: "TASK-dynTm03Tw8Ln6Vx3Kr"
type: "task"
status: "approved"
title: "TIM-03 dynamic musical transport"
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


# TIM-03 dynamic musical transport — Tasks

## Done

- [x] `Equivalent` revision reconciliationでmusical syncがskipされる原因を切り分ける
- [x] rebuild後および transport conversion前の authoritative sync/retryを実装する
- [x] musical refreshで event position/source sequence/payload/transport-generation binding/staged MIDIを保持する
- [x] unchanged MIDI再install回避と MIDI/non-MIDI ownership preservationを実装する
- [x] `FastControlWrite::transport_generation` を追加し、expanded compile-propagation filesへ `None` のみ機械的に伝播する
- [x] player/control representative mixed-plan oracleを追加する
- [x] VST3/CLAP production real ABI oracleに representative dynamic tempo edit 2 caseを追加する
- [x] dynamic docs chainを追加する
- [x] `test-tim03-dynamic-musical-transport` mise laneを追加する
- [x] canonical verification raw evidenceとPR Verification記録を整備する

## Follow-up / NotRun

- [ ] full dynamic tempo edit matrix
- [ ] automation parameter matrix expansion
- [ ] 100/128/256 frame × boundary full matrix
- [ ] fault/state/crash isolation、RT/Crash最終判定
- [ ] GUI/Native Dock/EXT-L3、macOS/Linux実走、3OS CI、AudioLens cross-consumer
- [ ] docs index同期（Coordinatorが統合時に実施）
