---
id: "IMPL-x9Qw4Ln7Vx3Kr8TyPmD2"
type: "impl"
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

# TIM-03 production musical transport — Implementation

## 変更点

- `crates/synvelis-player/src/musical_transport.rs`: project snapshot compile、typed unavailable、change basis、plan builder application。
- `crates/synvelis-player/src/dispatch.rs`: `ParamDispatch`へcurrent musical source保持、全mintのbuilder一本化、payload-preserving refreshを追加。
- `crates/synvelis-player/src/routing_control.rs`: surface sync/evidence、`sync_midi_plans` boundaryでの initial empty musical plan、retry evidence、observed span tempo revisionを追加。
- `crates/synvelis-player/tests/tim03_musical_transport_plan.rs`: Synvelis consumer供給責務のplayer-level oracle。
- `crates/synvelis-app/tests/tim03_l1_production_musical_transport.rs`: VST3/CLAP production composition→PluginNode→real ABI instrument PCM oracle。
- `mise.toml`: `test-tim03-production-transport`追加。
- `Cargo.lock`: ruviform scoped pin `511e1a7944c35797942ba065c092b9263067c48a`。

## Evidence semantics

host plan PPQNは project fact 480。fixture visible PPQN 960は VST3/CLAP adapter projectionであって fixed fallbackではない。`missing/unknown`は player-level testで tempo map/PPQN両方が空であることを確認し、production surfaceでも `MusicalTransportEvidence::Unavailable` として残る設計である。

## Limitations

dynamic tempo edit matrix/full matrix/fault/state/GUI/EXT-L3/3OS/cross-consumerは本sliceのPassに含めない。`TransportMirror`本体は upstream API の暫定 revision 0 semanticを残すが、production routing-control observationは live plan revisionを使う。docs indexは編集していない。

## Dynamic refresh implementation note

実装は project revision reconciliation、musical plan refresh、MIDI ownership preservation、optional transport-generation bindingの4点に限定した。`engine.rs` は compile resultが `Equivalent`でも `sync_musical_transport(project)` を実行し、transport conversion前にも retryする。`routing_control.rs` は unchanged MIDI contentの再 installを避け、clear/replaceでは non-MIDI eventsを保持する。`dispatch.rs` は refresh/restage/clearの全builder pathで event position/source sequence/payload/optional generation bindingを再構成し、musical refreshでは `Play`を送らない。

`FastControlWrite::transport_generation` は optional production semanticとして追加した。既存 construction sitesには機械的に `None`を伝播し、`Some`は upstream `bound_event` semanticsに乗る。real ABI evidenceでは plan installがRT receiptで `Applied` になり、2つの representative tempo edit後に fixture click lawの frameが VST3/CLAP双方で一致した。
