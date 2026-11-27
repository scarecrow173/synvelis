---
id: "DESIGN-dynTm03Qw8Lm4Vx7Kr"
type: "design"
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


# TIM-03 dynamic musical transport — Design

## 所有境界

`SessionRoutingEngine::rebuild` は committed project revision を処理した後、routing compile resultに関わらず control dispatchの musical syncを呼ぶ。tempo mapは routing fingerprintの一部ではないため、`Equivalent` は graph reuseとして正しく、musical source refreshは routing rebuildではなく control-plane reconciliationの責務である。`control_apply_transport` も transport command conversion前に syncをretryし、edit後最初の transport commandが stale planと組み合わさる経路を狭める。

`RoutingControlDispatch` は nodeごとの desired/current musical sourceを evidenceつきで管理する。compile不能・PPQN mismatch・範囲外は `MusicalTransportEvidence::Unavailable` と typed refresh errorで表し、fixed fallbackを返さない。initial install、source current、refresh必要を区別し、source currentなら再 installしない。

## refresh semantics

`ParamDispatch::refresh_musical_plan` は直前 planの eventsを順序どおり rebuildし、position/source sequence/payload/optional transport-generation bindingを保存する。`Play` requestは送らない。install成功時だけ last plan/ledgerを更新し、失敗時は desired sourceを保持して次回 syncでretryする。

MIDI reconcileは ownership boundaryを分ける。unchanged staged MIDI contentは再 installせず、clear/replaceは MIDI payloadだけを対象とし、non-MIDI param/automation eventsを保持する。これにより、 musical refresh で future control eventsが失われない。

## generation binding

`FastControlWrite::transport_generation` は optional bindingである。`None` は従来どおり unbound eventを構築し、`Some(generation)` は upstream `TransportPlanBuilder::bound_event` semanticsを使用する。既存 construction sitesには機械的に `None` を伝播し、production behaviorを変えない。test-only seeding APIや direct fake-plan installは設けない。

## real ABI oracle設計

project PPQN 480、adapter-projected fixture PPQN 960、48 kHz、256-frame block、L1 lookahead 512 framesを使う。baseline seek/loop/negative evidenceに加え、loopを `ClearLoop` で分離した後、(1) stop/lookahead分離後に tick 4800へ75.000 BPMを追加するcase、(2) 再生中に tick9600へ100.000 BPMを追加するcaseを測定する。independent lawは fixture visible tick deltaを adapter PPQNと milli-BPMでframesへ変換し、lookahead 512 framesを加える。
