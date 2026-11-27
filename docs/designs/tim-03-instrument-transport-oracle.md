---
id: "DESIGN-m7Qw3Ln9Vx2Kr6TyPzD8"
type: "design"
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
  implements:
    - "docs/specs/tim-03-instrument-transport-oracle.md"
  implemented-by:
    - "docs/plans/tim-03-instrument-transport-oracle.md"
  depends-on:
    - "docs/designs/tim-03-l1-out-of-process-hosting.md"
    - "docs/designs/tim-03-transport-application.md"
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

# TIM-03 instrument direct L1 musical transport oracle — Design

## 責務境界

この slice は Synvelis が所有する test harness と evidence のみを変更し、production
transport implementation を変更しない。host-side transport application（PR #683）と
plugin-side fixture interpretation（本 slice）を分離する。production graph end-to-end
musical handoff は ruviform の責務であり、Synvelis 側で tempo map/PPQN conversion を
複製する恒久 workaround は作らない。

## endpoint oracle

`test_instrument` は generative instrument なので、入力は stereo silence、出力は stereo
dual mono とする。direct `PluginEndpoint::process` へ `TransportInfo` snapshot を渡し、
L1 child process が返す PCM を independent oracle で判定する。host-side production
composition や `control_apply_transport` は使わない。

fixture law は upstream source が定義するとおり、Playing かつ valid musical timing の
ときだけ PPQN boundary click を出す。click frame は

```text
ceil((boundary_tick - position_ticks) * sample_rate * 60000 / (bpm_milli * ppqn))
```

で計算する。120 BPM/PPQN 960/48 kHz では 1 tick 25 frame、150 BPM では 1 tick 20 frame。
選ぶ case は position 956→frame 100、position 940→frame 500、loop case position 950→
frame 250、150 BPM position 950→frame 200 とし、相互に異なる click frame で stale
pass-through replay を合格に使えないようにする。

## L1 latency と freshness

`OutOfProcessSpec::new` の lookahead は 2 blocks、activation max frames は 512 なので
latency は 1024 frames。matched block（`passthrough_submits` delta が 0）は call `c`
ではなく source call `c - 2` の rendering として扱う。call 0/1 に現れる initial
silence、`passthrough_submits > 0` の substituted/pass-through block、dropped submit が
絡んだ output は fresh click evidence に数えない。substituted block は以前の click を
replay し得るため content assertion を行わない。

各 case は複数 call で同じ snapshot を供給する。これは continuous playback model では
なく、per-block snapshot derivation を隔離する deterministic oracle である。seek/
position jump は click@100 phase の後に click@500 phase を供給することで表し、前
block state に引きずられないことを source-attributed matched output で判定する。
active loop range は `TransportInfo::with_loop_range` で供給し、fixture lawが per-block
snapshot derivation を乱さないことを click@250 で確認する。

## negative control

`Stopped` と `Paused` では click path が silent、`Playing` でも `musical=None` では
silent でなければならない。`musical=None` negative control は production graph の欠陥を
合格扱いするものではない。fixture が musical timing を捏造していないこと、および
upstream handoff 欠落時に audio oracle が false positive にならないことの証拠として
記録する。

## upstream gap

resolved ruviform revision `0479fa62e77e669a7bf84320d3d5418ffb949fc6` の
`crates/ruviform-audio-plugin-bridge/src/node.rs` では `call_transport_info(&span.ctx, rel, None, None)`
を呼び、tempo map/PPQN を渡さない。したがって production graph end-to-end の musical
transport は成立しない。この slice は ruviform Issue `https://github.com/scarecrow173/ruviform/issues/1040`
を upstream tracker として記録し、production end-to-end を NotRun/Blocked upstream とする。
