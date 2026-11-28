---
id: "TSPEC-w2Ln8Qx4Vm7Kr9TyPzD5"
type: "test-spec"
status: "approved"
title: "tim-03-instrument-transport-oracle"
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
  depends-on:
    - "docs/designs/tim-03-instrument-transport-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-03-instrument-transport-oracle.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-03-instrument-transport-oracle.md"
  verified-by: []
  references:
    - "docs/designs/tim-03-instrument-transport-oracle.md"
  defers: []
  deferred-by: []
---

# Test Spec — TIM-03 instrument direct L1 musical transport oracle

Fixture: `crates/synvelis-app/tests/tim03_l1_instrument_transport.rs`、
`crates/synvelis-app/tests/common/tim03_l1.rs`、mise task
`test-tim03-instrument-transport`。

## Rules

- **IT-01 real ABI route**: VST3/CLAP `test_instrument` は scan で見え、Synvelis 所有
  test-only L1 runner 経由で instantiate → initialize → activate → start-processing →
  endpoint prepare → endpoint drop → stop/deactivate/destroy が完了する。fake plugin や
  in-process 偽装で Pass にしない。
- **IT-02 artifact contract**: fixture/runner path、size、SHA-256、staged fixture SHA-256、
  runner resolution env、ruviform resolved revision を evidence に記録する。hash mismatch は
  fail-closed。
- **IT-03 independent click law**: expected click frame は fixture law
  `ceil((boundary_tick - position_ticks) * sample_rate * 60000 / (bpm_milli * ppqn))`
  から独立計算する。観測値の後付けで期待値を決めない。
- **IT-04 120 BPM oracle**: 120 BPM/PPQN 960/48 kHz で position 956 は frame 100、
  position 940 は frame 500 に amplitude 0.5 の click を出す。
- **IT-05 non-120 oracle**: 150 BPM/PPQN 960/48 kHz で position 950 は frame 200 に click を出し、
  click frame が tempo に応じて変わることを示す。
- **IT-06 seek/position jump**: click@100 phase の後に click@500 phase を供給し、
  source-attributed matched output が前 block state を引きずらず各 block-start snapshot の
  click frame を示す。
- **IT-07 loop range**: active loop range を `TransportInfo::with_loop_range` で供給した
  position 950/120 BPM case は frame 250 に click を出し、per-block derivation が乱れない。
- **IT-08 negative controls**: `Stopped`、`Paused`、`Playing + musical=None` は click path が
  silent。`-0.0` は IEEE equality では silent として扱う。
- **IT-09 PCM shape**: click block は stereo identical で、click frame 以外は silent。
  amplitude は bit-exact 0.5。voice は triggered しないため click 以外の残響を許容しない。
- **IT-10 latency/freshness**: L1 latency は lookahead 2 × 512 = 1024 frames。matched
  output at call `c` は source call `c - 2` と attribution し、initial silence、
  pass-through/substituted block、dropped submit に関わる output を fresh evidence に数えない。
- **IT-11 honesty boundary**: production composition → `PluginNode` → plugin end-to-end
  musical transport は upstream gap により NotRun/Blocked upstream と記録し、Pass にしない。
- **IT-12 determinism/ordering**: runner build → integration test は dedicated opt-in lane
  `mise run test-tim03-instrument-transport` に束ね、test は `--ignored` と `--test-threads=1`
  で実行する。この artifact-dependent test は generic workspace-wide / MSRV / RT lane の
  対象外とする。

## Evidence

両 format の JSONL record に descriptor、scan/lifecycle/endpoint verdict、artifact/hash、
latency/lookahead、case ごとの input transport、independent expected click frame、observed
click frame/amplitude、stereo identical、non-click silence、negative controls、excluded
blocks、upstream issue、NotRun/Blocked upstream を記録する。
