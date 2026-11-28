---
id: "TESTSPEC-dynTm03Ew8Ln7Vx5Kr"
type: "test-spec"
status: "approved"
title: "TIM-03 dynamic musical transport"
created: "2026-11-27"
updated: "2026-11-28"
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


# TIM-03 dynamic musical transport — Test Spec

## Player/control oracle

`crates/synvelis-player/tests/tim03_musical_transport_plan.rs`

- production mint pathsが PPQN 480、non-default tempo、current revisionを carryする。
- invalid/mismatched project stateは typed unavailableで、fixed 960/partial pairを作らない。
- musical refreshは event position、source sequence、payload、transport-generation bindingを保持する。
- representative mixed-plan testは staged MIDI 2件、unbound fast-control event、`Some(TransportGeneration(1))` eventを同じplanに共存させ、refresh後もstaged MIDIと全event identityが保たれることを要求する。
- PPQN mismatchを注入した cloned projectでは typed unavailableとなり、両 musical halfが空であることを要求する。

## Production real ABI dynamic oracle

`crates/synvelis-app/tests/tim03_l1_production_musical_transport.rs`

- production seam `compose_routing_engine_with_out_of_process_and_boundaries`、test-only L1 runner、sibling staging、hash fail-closedを使う。direct endpoint process API/caller supplied `TransportInfo`は使わない。
- host/project PPQN 480と adapter projection PPQN 960を区別する。
- baseline positive seek casesは delta 3/2/1 ticksで click frame 704/640/576、loop positivesは 1024/1792、stopped/paused negativesはsilent。
- dynamic case 1: loopを `ClearLoop`で分離し、測定transport前に tick4800へ75.000 BPMを追加する。quarter16 delta2のexpected click frameは672。
- dynamic case 2: 再生中に tick9600へ100.000 BPMを追加する。quarter24 delta2のexpected click frameは632。
- 各 dynamic caseで control evidenceが project/plan revision、PPQN、current-source identity、empty refresh errorsを示し、RT install receiptが対応plan generationの `Applied` を示す。
- click impulseはamplitude 0.5、stereo identical、non-click frames silent。lookahead/pass-through/substituted blockはpositive fresh evidenceに数えない。

## Canonical lane

`mise run test-tim03-dynamic-musical-transport` は test-only L1 runnerをbuildし、`#[ignore]` の production musical transport integration testを `--ignored` と `--test-threads=1` で実行する。同名test binaryがbaseline production evidenceとdynamic evidenceを1回のVST3/CLAP runで出力するため、このentrypointは両 formatのdynamic evidenceを再現する。generic workspace-wide / MSRV / RT lane はこの test を実行しない。
