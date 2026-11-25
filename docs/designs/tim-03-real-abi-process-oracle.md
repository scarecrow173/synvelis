---
id: "DESIGN-NhzhoY6wYQRrrjxtkUsnO"
type: "design"
status: "approved"
title: "TIM-03 real ABI process oracle boundary slice"
created: "2026-11-25"
updated: "2026-11-25"
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
  implements:
    - "docs/specs/tim-03-real-abi-process-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-03-session-driver-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-03-driver-loop-boundary.md"
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

# TIM-03 real ABI process oracle boundary slice — Design

## Overview

設計の中心は、real ABI fixture を 3 つの層に分けて測定することである。

1. **artifact/discovery layer**: binary path・SHA-256・descriptor scan。
2. **host lifecycle layer**: in-process `PluginHost` 上で `test_gain` を
   `Processing` まで進め、同じ instance の `prepare_endpoint` rejection を直接測る。
3. **production consumer layer**: `PluginHostProcessors::product` と
   `compose_routing_engine` が最初の defined rejection で fail-closed になることを
   実走する。

この分離が必要なのは、production adapter の最初の rejection ordering と endpoint
attach の architecture gap が同一ではないためである。混ぜると evidence が嘘をつく。

## Artifact resolution

test file は `ruviform_plugin_dock_test::artifact::resolve_artifact_named` を第一
経路とし、explicit env override（`SYNVELIS_TIM03_REAL_ABI_VST3` /
`SYNVELIS_TIM03_REAL_ABI_CLAP`）を優先する。upstream helper が解決できず、かつ
explicit override が無い場合だけ、test executable と同じ `target/debug/deps` を
`ruviform_plugin_test_<format>-<hash>.<dll>` 命名規則で探索する小さな fallback を
test file 内に持つ。これは `plugin_dock_probe.rs` の先例に倣うが、共通 module は
作らず、編集もしない。unresolved artifact は `Unsupported` や skip ではなく panic
による test failure とする（対象 OS は Windows で、upstream artifact は supported）。

`sha256_hex` で artifact bytes を hash し、descriptor scan result と一緒に JSON line
として出力する。hash は bit-for-bit reproducible build を意味しないので expected hash
には pin せず、実測値の記録に使う。descriptor id は ABI identity なので expected
value に pin する。

## Lifecycle and endpoint measurement

`measure_endpoint_boundary` は `PluginHost::new` に `test_gain` factory を登録し、
production host command surface で次を順に実行する。

```text
Scan → Instantiate(plugin_id) → Initialize → Activate(48 kHz, 1..=256)
→ StartProcessing → prepare_endpoint(bounds 256 frames / 2 channels)
```

Activate config と endpoint bounds を同じ 256 frames に揃えるのは、bounds mismatch
ではなく endpoint capability 自体を測るためである。upstream ruviform pin の
`PluginFactory::create_sendable` は default `None` で、doc は `!Send` instance を
endpoint-style consumer が `EndpointUnavailable` で reject すると定める
（`ruviform-plugin-core/src/plugin.rs:398-417`）。upstream real fixture integration
test も「in-process real plugins cannot attach」「`prepare_endpoint` only succeeds on
the L1 route」と明記する（`ruviform-plugin-test-vst3/tests/l1_real_test_plugin_e2e.rs:1318-1326`）。
この slice の direct measurement は、VST3/CLAP 両方で `Err(EndpointUnavailable)` を
返すことを assert する。成功した場合は architecture が変わったか誤った経路を通った
ので test failure とする。

teardown は endpoint rejection 後に StopProcessing → Deactivate → Destroy を
best-effort で発行し、real module instance を test process 内で長生きさせない。

## Why adapter connect reports Incompatible first

`PluginHostProcessors::connect` は consumer declared bus table を typed endpoint attach
config へ投影する。本 test の semantic processor は stereo Main input/output を持つため、
declaration は stereo input + stereo output である。しかし pinned ruviform VST3/CLAP
in-process adapter は `Plugin::buses` を override せず、default empty table を返す。
`PluginHost::buses` は in-process Plain slot ではこの slice を `Reported([])` として
返し、bridge `attach()` は endpoint preparation より先に declared/reported table を
cross-check する。empty report には declared input/output が存在しないので、最初の
defined rejection は `AttachError::Bus(MissingPort)` であり、Synvelis adapter では
`ProcessorConnectError::Incompatible` に写像される。

したがって production connect の `Incompatible` は endpoint attach が可能だったという
意味でも、endpoint gap が無いという意味でもない。direct lifecycle measurement が示す
`EndpointUnavailable` は independent evidence であり、両方を併記する。bus mismatch を
test-specific wrapper で隠して endpoint まで辿り着いたように見せることは、production
忠実性を壊すので採用しない。

## Production composition measurement

`production_failure` は `SynvelisApp` の session に track/strip を作り、`test_gain` の
`ProcessorBinding::Resolved` を strip chain に挿入する。`compose_routing_engine` は
`ProductPlugin { factory, params }` を `PluginHostProcessors::product` に登録する
production composition root であり、test が desktop binary 用の空 registry path
（`compose_product_routing`）を変更することはない。

lowering は chain member ごとに `SemanticNodeRole::ChainProcessor` node を emit し、
compile stage はそれを `ProcessorHost::connect` へ渡す。したがって上記 adapter
rejection は build 全体を fail-closed にする。`Incompatible` の deterministic fault
mapping は `RoutingFault::UnsupportedFormatOrTimeMap` であり、test は activation
receipt が `Failed` または `Blocked` の typed fault を持ち、active/published に
到達しないことを確認する。fault を `PluginUnavailable` と書き換えないのは、
`Unavailable` endpoint rejection ではなく `Incompatible` bus rejection が production
path の first rejection だからである。

## Independent process oracle boundary

`test_gain` の public law `out[c][n] = in[c][n] * gain(n)` と normalized→plain
mapping は upstream fixture contract として既知だが、本 slice は process endpoint に
attach できないため independent PCM oracle を実行しない。expected PCM を計算しても
観測 PCM が存在せず、比較は成立しない。したがって次は全て `NotRun` とする。

- deterministic input clip に対する PCM/crc 照合。
- automation lane → plugin parameter event の sample-accurate offset。
- zero-frame block flush、same-offset last-wins、frame offset exactness。
- loop/seek/tempo revision 跨ぎの process boundary 照合。
- real ABI path での illegal process length 0 件、誤配送 0 件、stale generation 0 件。
- `test_instrument` の MIDI/transport click oracle。

これは verification の成功条件を弱めたものではなく、現在の production architecture
で public path が到達できる上限を fail-closed evidence として固定したものである。
解除条件は、Synvelis production composition path に upstream L1 out-of-process
hosting が配線され、real ABI fixture が typed endpoint に attach できるようになる
ことである。

## Isolation and repeatability

- VST3/CLAP test は同じ手順を独立した factory instance で実行する。`library.factories()`
  は scan cache から複数の factory handle を供給できるため、lifecycle measurement、
  adapter measurement、composition measurement を同じ loaded module の別 instance に
  分ける。
- session media root は `std::env::temp_dir()` 配下に process id と atomic sequence を
  含む一意 directory として作り、test 間で共有しない。
- test output は machine-readable JSON line（prefix `tim03-real-abi-oracle`）で、
  artifact path/hash、descriptor list、scan/lifecycle verdict、bus report、
  `create_sendable` capability、endpoint error、adapter error、production fault、
  process oracle `NotRun` rationale を含む。
- `factory_class_id` は format 固有の意味を持つ。VST3 では descriptor id が raw
  component TUID bytes の lowercase hex、CLAP では descriptor id 自体が factory
  instantiation id である。別の UUID 表記へ変換して upstream constant と見せかける
  ことはしない。

## Non-goals

- production source に evidence accessor や test support を追加しない。
- upstream bus enumeration、L1 out-of-process hosting、crash isolation、GUI、OS matrix
  を実装しない。
- Fake Plugin の pass を real ABI pass と混同しない。canonical `synvelis-player` /
  `test-rt` lane は非回帰 evidence であり、real ABI process evidence ではない。
