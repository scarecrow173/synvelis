---
id: "SPEC-fbM127xQw8Ln2KrTyVzA"
type: "spec"
status: "approved"
title: "TIM-03 frame/boundary matrix"
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

# TIM-03 frame/boundary matrix — Spec

## 目的

Issue #127 TIM-03 のうち frame/boundary/event-order matrix slice を層別に固定する。対象は 100/128/256 frame window、window-relative boundary offset 0/1/100/last、Loop/Seek/tempo-map revision/Param/process-start-relative frame offset、exact process length sequence である。

## 要求

1. headless fake/mock layer で valid `W×B` cell を exhaustive に検証し、window=100/B=100 を clamp せず typed invalid/out-of-window negative とする。
2. routing/driver loop-wrap derivation を 100/128/256 physical window geometry で検証し、head wrap offset 0 と interior cut、window-end wrap の次 window 所属を区別する。
3. production real ABI evidence は production composition → PluginNode → out-of-process L1 → ruviform test_instrument のみで行う。direct endpoint process API、caller supplied `TransportInfo`、test-only seeding、fake plan direct install、direct endpoint injection は禁止する。
4. public production seam が arbitrary interior seek/tempo-plan/param scheduling を表現できない cell は fake pass で埋めず `unsupported-by-current-production-seam` とする。
5. project PPQN 480 と VST3/CLAP adapter projection PPQN 960を区別し、fixed 960 fallback を使わない。

## 非スコープ

full dynamic tempo edit matrix、広い automation matrix、fault/state/crash isolation、GUI/EXT-L3、macOS/Linux runtime、3OS CI、AudioLens cross-consumer、ruviform pin/vendor/Cargo.lock変更。
