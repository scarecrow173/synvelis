---
id: "TESTSPEC-r6Qw3Ln8Vx2Kr5TyPmD7"
type: "test-spec"
status: "approved"
title: "TIM-03 production musical transport"
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

# TIM-03 production musical transport — Test Spec

## Player-level plan oracle

`crates/synvelis-player/tests/tim03_musical_transport_plan.rs`

- non-default PPQN 480、tempo revision 9/10、non-default tempo/signatureを compiled sourceとして param/MIDI/clear planが carryする。
- PPQN mismatchを typed errorとし、無供給planに tempo map/PPQN/fixed 960が出ない。
- musical refreshが event position、source sequence、payload、transport generation bindingを保持し、staged MIDIを失わない。

## Production real ABI oracle

`crates/synvelis-app/tests/tim03_l1_production_musical_transport.rs`

- production seam `compose_routing_engine_with_out_of_process_and_boundaries`、L1 runner、sibling staging、hash fail-closedを使う。
- direct endpoint process API/caller supplied `TransportInfo`は使わない。
- project PPQN 480、tempo revision 2、93.75 BPM。adapter projection PPQN 960を host PPQNと区別する。
- seek/position jump: delta 3/2/1 ticksのclick frameを independent law `ceil((boundary_tick-position_ticks)*sample_rate*60000/(bpm_milli*ppqn))`＋L1 lookaheadで判定する。amplitude 0.5、stereo bit-identical、非click frame zero。
- loop: 12 ticks=3 blocksのrangeで各iterationのboundary clickがlookahead後のblock 4/7 frame 0に現れる。
- stopped/paused: lookahead flush後のfresh outputはsilent。
- initial silence、lookahead、pass-through/duplicate疑いのblockはpositive evidenceに数えない。

## Canonical lane

`mise run test-tim03-production-transport` は runner build後に `#[ignore]` の artifact-dependent integration testを `--ignored` と `--test-threads=1` で実行する。generic workspace-wide / MSRV / RT lane はこの test を実行しない。

## Dynamic refresh test oracle

player/control oracle は representative mixed planを使い、staged MIDI 2件、unbound fast-control event、transport-generation-bound eventを1 planに共存させる。tempo revisionを進めた refresh後に、event position/source sequence/payload/bindingと staged MIDIが一致し、plan revision/PPQN/current-source identityが新しい project snapshotを指すことを要求する。PPQN mismatchを注入した cloned projectでは typed unavailableとなり、両 musical halfが空で fixed 960が出ないことを要求する。

production real ABI oracle は VST3/CLAPの両方で、(1) stop/lookahead分離後に tick 4800へ75.000 BPMを追加し quarter16 delta2の clickが672 frame、(2) 再生中に tick9600へ100.000 BPMを追加し quarter24 delta2の clickが632 frame、を independent lawで検証する。各 edit後の control evidenceは revision/PPQN/current-source identity/refresh error emptyを示し、RT receiptは対応 plan generationの `Applied` を示す。loop evidenceの後は `ClearLoop`で測定窓を分離し、旧loopがdynamic positionに混入しないようにする。
