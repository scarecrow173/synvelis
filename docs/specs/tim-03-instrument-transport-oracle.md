---
id: "SPEC-i3Tn9Qx7Lm4Vw8KzRpD2"
type: "spec"
status: "approved"
title: "TIM-03 instrument direct L1 musical transport oracle"
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
  implemented-by:
    - "docs/designs/tim-03-instrument-transport-oracle.md"
  depends-on:
    - "docs/specs/tim-03-l1-out-of-process-hosting.md"
    - "docs/specs/tim-03-transport-application.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-03-instrument-transport-oracle.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-03 instrument direct L1 musical transport oracle

## 目的

ruviform#630 の正規 ABI `test_instrument` VST3/CLAP fixture を、Synvelis 所有の
test-only L1 runner 経由で scan/instantiate/activate/start-processing し、direct L1
endpoint API に caller supplied `TransportInfo` を与えて、plugin-side musical
transport click law を独立 PCM oracle で検証する。

PR #683 の host-side transport application evidence は `test_gain` を使うため、
plugin が musical timing を解釈した証拠を持たない。この slice は direct endpoint に
限定してその欠測を埋める。production composition → `PluginNode` → plugin の
end-to-end musical transport pass は、ruviform 側 handoff gap が解消されるまで
Blocked upstream / NotRun とし、Pass と偽らない。

## スコープ

- `crates/synvelis-app/tests/tim03_l1_instrument_transport.rs` を追加し、VST3/CLAP を
  `--test-threads=1` で実走する。
- fixture/runner 解決、sibling staging、hash mismatch fail-closed は既存 TIM-03 L1
  contract を再利用する。`test_instrument` descriptor は name で fail-closed に選択する。
- direct endpoint で scan/lifecycle/endpoint prepare/drop/teardown を確認する。
- `Stopped`/`Paused`/`musical=None` negative control、120 BPM の position 956→frame 100、
  position 940→frame 500、150 BPM case、seek/position jump、active loop range を検証する。
- L1 latency/lookahead を考慮し、initial silence/pass-through/substituted block を
  fresh click evidence に数えない。
- upstream gap を evidence に記録し、ruviform Issue `https://github.com/scarecrow173/ruviform/issues/1040`
  を参照する。ruviform 側コードや pin は変更しない。
- `mise run test-tim03-instrument-transport` を canonical 入口として追加する。
- docs chain と machine-readable/human-readable evidence を残す。

## 非スコープ

- production composition → `PluginNode` → plugin の end-to-end musical transport pass。
- dynamic tempo edit matrix、全 frame/block boundary matrix、automation matrix 拡張。
- `test_fault`/`test_stateful`、crash/hang/timeout isolation、State/Unload、RT/Crash 最終判定。
- GUI、Native Dock、実 Window、editor、native slot、Windows EXT-L3 最終受入、
  provisioned endpoint、macOS/Linux 実走、3OS CI、AudioLens cross-consumer。
- ruviform 側変更、ruviform pin 更新、`Cargo.toml`/`Cargo.lock` 更新、docs index 更新。
- Synvelis production code の修正。

## 受入証拠

- VST3/CLAP 両 format で `mise run test-tim03-instrument-transport` が exit 0。
- click frame 期待値は fixture law `ceil((boundary_tick - position_ticks) * sample_rate * 60000 / (bpm_milli * ppqn))`
  から独立計算し、観測値の後付けで決めない。
- click block は stereo identical、click frame のみ amplitude 0.5、非 click frame は
  IEEE 意味で silent（`-0.0` を silent に含める）。
- production end-to-end musical transport は upstream gap により NotRun/Blocked upstream と明記する。
