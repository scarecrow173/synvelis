---
id: "SPEC-p7Mx2Qw9Ln4Kr8TyVzD6"
type: "spec"
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

# TIM-03 production musical transport — Spec

## 目的

Synvelis production composition から mint されるすべての `TransportPlan` が、current project snapshot の tempo map・tempo revision・project PPQN を complete な plan-owned musical source として carry する。これにより production `PluginNode` 経路で ruviform#1043 の per-span `MusicalTimeSource` が real VST3/CLAP instrument に届く。

## 要求

1. tempo map と PPQN を片方だけ載せない。compile 不能・PPQN mismatch・範囲外は typed unavailable とし、plan の musical half は空のままにする。
2. missing/unknown を fixed 960 PPQN、default tempo map、caller supplied direct `TransportInfo` で補わない。project PPQN が 960 の場合だけ 960 は project fact として使う。
3. param batch、MIDI plan、clear/empty plan、musical refresh plan は同じ authoritative builder pathを通る。
4. plan refresh は event position、source sequence、payload、transport generation binding、staged MIDI、transport anchoring を落とさない。`Play` re-anchor は行わない。
5. content-free instrument には initial composition で intentional empty musical plan を供給する。既存 plan がある限り同一 source を再 install しない。
6. control observation span の `tempo_revision` は mirror default `0` ではなく live plan の revision を反映する。
7. production real ABI evidence は non-default project PPQN、non-120 tempo、seek/position jump、loop derivation、stopped/paused negative、amplitude/stereo/non-click silence、L1 freshness exclusionを含む。

## 非スコープ

dynamic tempo edit matrix、full frame/boundary matrix、fault/state/crash isolation、GUI/Native Dock/EXT-L3、macOS/Linux 実走、3OS CI、AudioLens cross-consumer、fixture law や adapter projection の変更。
