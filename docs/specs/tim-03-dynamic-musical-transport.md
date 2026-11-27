---
id: "SPEC-d7Tm03DynQw8Ln4Vx2Kr"
type: "spec"
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


# TIM-03 dynamic musical transport — Spec

## 目的

production composition が所有する `TransportPlan` を、committed project の tempo map / tempo revision / project PPQN の変更へ動的に追随させる。baseline PR #686 の production musical transport は initial/current source供給を確立済みであり、この slice は routing compile が `Equivalent` であっても committed revision 後の musical source が live planへ反映されることを検証する。

## 要求

1. tempo map・tempo revision・project PPQN は complete な plan-owned musical sourceとして扱い、片方だけの供給や fixed 960/default tempo fallback を作らない。
2. project revision 処理後に `sync_musical_transport` を実行し、routing publish の有無や `Equivalent` 判定に依存せず current project sourceを再評価する。
3. musical refresh は `Play` re-anchor を行わず、event position・source sequence・payload・optional transport-generation binding・staged MIDIを保持する。
4. unchanged MIDI content を再 install せず、MIDI clear/replace が unrelated param/automation eventを消さない。
5. `FastControlWrite::transport_generation` は optional production semantic とし、`None` の既存 unbound behaviorを保ち、`Some` は upstream bound-event semanticsを使う。test-only seeding APIや fake plan direct installは設けない。
6. real ABI evidenceは、少なくとも測定 transport 前の valid tempo editと再生中の valid tempo editの2 caseを VST3/CLAP双方で独立 click lawにより検証する。
7. 各 dynamic caseで revision/PPQN/current-source identity、typed refresh errors、RT install receipt、lookahead/pass-through分離を evidenceへ記録する。

## 非スコープ

full dynamic tempo edit matrix、full frame/boundary matrix、fault/state/crash isolation、GUI/Native Dock/EXT-L3、macOS/Linux 実走、3OS CI、AudioLens cross-consumer、fixture lawや adapter projectionの変更。`test-rt` は本 slice specの canonical commandではなく、実行する場合も補助証拠として区別する。
