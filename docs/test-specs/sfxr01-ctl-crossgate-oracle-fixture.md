---
id: "TSPEC-58bMtNin4rGl51xvg20P4n"
type: "test-spec"
status: "approved"
title: "SF-XR-01 検証仕様 — CTL-01/CTL-02 cross-gate oracle fixture"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
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
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/designs/sfxr01-ctl-crossgate-oracle-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
  verified-by: []
  references:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/designs/sfxr01-ctl-crossgate-oracle-fixture.md"
  defers: []
  deferred-by: []
---

# SF-XR-01 検証仕様 — CTL-01/CTL-02 cross-gate oracle fixture

## Feature: SF-XR-01 cross-gate oracle

`crates/synvelis-player/tests/sf_xr_01.rs` が保証すること。

## Rules & Examples

### R1 — strip/send persisted authority（sfxr01_01）

- 5 edits の `RoutingEditIntent` が 1 revision で commit される。
- track/bus/master が一意 strip を持ち `StripOwner` back-ref が
  一致、master-kind bus が exactly one。
- 各 send の `gain_target` が `ControlOwner::Send`+
  `ParameterRole::SendLevel`+`ProjectMusical` の target を指し、
  `SendConfig` の serialized key に gain 値が存在しない。
- save→load で project JSON/revision が一致、undo/redo が
  pre/post gesture project を完全復元する。

### R2 — source-map/graph consistency（sfxr01_01）

- `semantic_nodes`: 各 strip が一意 fader node、bus がその
  strip の node に解決、4 strip で distinct。
- `send_taps`: PostFader = strip main-out 解決と一致、
  PreFader = 同 node の別 port。
- send connection の `Resolved` edge が spec 上に存在し、
  persisted `ExplicitChannelMap`（dst→src `[1,0]`）が spec edge
  の `(src,dst)` `[(0,1),(1,0)]` に一致。map 無し send の edge
  は `channel_map() == None`。
- `node_roles` の `SendGain.gain` が persisted base と一致。

### R3 — mute audibility（sfxr01_01）

- `SetStripMute` 後、mute した strip に触れる全 connection
  （send 含む）が `NotLowered(MutedStrip)`、`send_taps` から
  消え、無関係 edge は `Resolved` のまま。mute が
  `ControlOwner::Strip`/`Mute` target を生む。

### R4 — channel-map consent + atomicity（sfxr01_02）

- mismatch+map 無し→`routing-requires-decision`（Approved でも）、
  mismatch+map+未承認→`routing-requires-decision`、
  mismatch+map+Approved→staged（send/conn/map/gain target が
  candidate に出る）、不正 map→`routing-unsupported-map`。
- commit 経路で未承認 map は typed reject し revision・
  serialized project・undo stack を不変に保つ。Approved は
  commit され map が永続化される。

## Out of scope（NotRun のまま）

実 channel 変換 DSP・実 OS GUI・native plugin host・consumer
lane・RT/probe 実測。これらは ledger で `NotRun` を維持する。
