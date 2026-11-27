---
id: "DESIGN-q4Nx8Lm2Vw6Kr9TyPzD3"
type: "design"
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

# TIM-03 production musical transport — Design

## 所有境界

`musical_transport.rs` は project snapshot を compiled plan-owned source へ変換する唯一の Synvelis consumer boundary とする。ここで `Project::ppqn` と `TempoMapData::ppqn` の一致、selectable PPQN range、`TempoMapData::compile()` を検証し、`PlanMusicalTransport` を返す。error は `MusicalTransportError` で evidence に残し、fallback を返さない。

`ParamDispatch` は current `Option<PlanMusicalTransport>` を保持し、すべての mint (`dispatch`, `dispatch_midi`, `clear_midi_plan`, refresh) を `next_plan_builder()` に集約する。builder は node の verified `plan_builder` を使い、source があるときだけ `with_musical_transport` を適用する。

## refresh semantics

`refresh_musical_plan()` は直前 plan の events を順序どおり rebuild する。position/source sequence/payload/optional transport-generation bindingをコピーし、`Play` request は送らない。install 成功時だけ ledger/last plan を更新する。失敗時は desired source を保持し、次回 sync で retry する。

`RoutingControlDispatch` は attach/sync と `sync_midi_plans` で current project を compile し、全 node の desired source を更新する。`sync_midi_plans` は SessionRoutingEngine の every-revision prepare から呼ばれるため、routing publish が Equivalent でも tempo/PPQN snapshot は reconciled される。content-free node 向け initial empty musical plan はこの production MIDI-plan reconciliation boundary でだけ明示的に install し、raw attach/manual sync では既存の plan generation 意味論を変えない。invalid source では plan-less の honest negative を維持する。

## observation honesty

`TransportMirror::span_context` は upstream API 上 revision 0 を返すため、routing-control observation で span を作る場所だけ live `last_plan.tempo_revision` を上書きする。これは mirror semantic を変えるものではなく、control ledger が fabricated default revisionを evidence として出さないための補正である。

## adapter projection

host/project PPQN は plan evidenceとして 480 などの project fact を使う。VST3/CLAP adapter は ABI の beat floating-point表現を経て fixture visible PPQN 960へ reproject する。これは format adaptation であり fixed host fallbackではない。production PCM oracleは projected PPQNを明示して click lawを計算する。

## Dynamic refresh design

`SessionRoutingEngine::rebuild` は committed revision を処理した後、compile result に関わらず `sync_musical_transport` を呼ぶ。tempo map は routing fingerprint に含めないため `Equivalent` が正しい graph reuse であり、musical refresh は control-plane reconciliationとして独立に行う。`control_apply_transport` も conversion 前に同じ sync を retry し、project edit と次の transport command の間で rebuild sync が失敗したまま残る経路を狭める。

`RoutingControlDispatch` は node ごとに current project source を保持し、plan 無しの initial install・source current・refresh必要を区別する。refresh は `ParamDispatch::refresh_musical_plan` で既存 plan の各 eventを同じ position/source sequence/payload/transport-generation bindingで rebuild し、`Play` を送らない。MIDI staging は content equalityを照合し、unchanged contentの再 installを避ける。clear/replace は MIDI payloadだけを対象とし、non-MIDI eventsを preserve する。

`FastControlWrite::transport_generation` は production semantic の optional bindingである。`None` は従来の unbound eventと同じ builder path、`Some(generation)` は upstream `bound_event` pathを使う。test-only seeding API や fake plan direct install は設けない。
