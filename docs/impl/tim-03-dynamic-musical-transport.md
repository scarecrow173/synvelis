---
id: "IMPL-dynTm03Iw8Ln8Vx6Kr"
type: "impl"
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


# TIM-03 dynamic musical transport — Implementation

## 変更点

- `crates/synvelis-app/src/engine.rs`: committed revision処理後に routing compile resultに関わらず `sync_musical_transport(project)` を実行し、transport conversion前にもretryする。
- `crates/synvelis-player/src/routing_control.rs`: current musical source evidence、refresh retry、unchanged MIDI no-op、MIDI/non-MIDI ownership-preserving reconcile、read-only evidence seamsを実装する。
- `crates/synvelis-player/src/dispatch.rs`: refresh/restage/clearのbuilder pathで event position/source sequence/payload/optional generation bindingを保持し、musical refreshでは `Play`を送らない。
- `crates/synvelis-player/src/musical_transport.rs` / `lib.rs`: plan current-source identityを production test/control evidenceから検査できるよう公開する。
- `FastControlWrite::transport_generation`: optional binding。`None`は従来unbound、`Some`はupstream bound-event semantics。expanded compile-propagation filesには `None`のみ機械的に伝播した。
- `crates/synvelis-player/tests/tim03_musical_transport_plan.rs`: representative mixed-planとinvalid-state oracle。
- `crates/synvelis-app/tests/tim03_l1_production_musical_transport.rs`: baseline production oracleへ representative dynamic tempo edit 2 caseを追加。
- `mise.toml`: `test-tim03-dynamic-musical-transport` canonical entrypoint。

## Evidence semantics

host/project PPQN 480はproject factであり、fixture visible PPQN 960はVST3/CLAP adapter projectionである。fixed 960 fallbackではない。dynamic evidenceは machine rows `evidence/sv127/dynamic-musical-transport.jsonl` と raw logs `evidence/sv127/dynamic-musical-transport/` に記録する。

## Limitations

full dynamic matrix、automation parameter matrix expansion、full frame/boundary matrix、fault/state/crash isolation、GUI/EXT-L3、macOS/Linux runtime、3OS CI、AudioLens cross-consumerは本 sliceのPassに含めない。docs indexは編集していない。`test-rt` は本 slice specのcanonical commandではない。実行済みでも補助証拠としてのみ記録し、required lanesの代替にしない。
