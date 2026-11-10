---
id: "DESIGN-OIDrCGhRjzwr0IzN1tzVkg"
type: "design"
status: "approved"
title: "SF-XR-01 Design — CTL-01/CTL-02 cross-gate oracle fixture"
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
  implements:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/adr/sfxr01-ctl-crossgate-oracle-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sf-06-cross-gate-oracle-domain-evidence.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/adr/sfxr01-ctl-crossgate-oracle-fixture.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sfxr01-ctl-crossgate-oracle-fixture.md"
  references:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/adr/sfxr01-ctl-crossgate-oracle-fixture.md"
  defers: []
  deferred-by: []
---

# SF-XR-01 Design — CTL-01/CTL-02 cross-gate oracle fixture

## Overview

`crates/synvelis-player/tests/sf_xr_01.rs` に 2 つの test を置く。
どちらも `ProjectController` で構築した persisted project を
`lower_semantic`（player の semantic lowering）へ通し、
persisted 契約と generation-scoped `RoutingSourceMap`/typed
`GraphSpec` の一致を独立 oracle として検証する。実装内部 helper
は呼ばず、公開 API（`routing.sends`/`connections`/`control.targets`/
`send_level_target_id`/`RoutingSourceMap`/`GraphSpec::connections`）
だけを根拠にする。

## sfxr01_01 — persisted authority → serialization/undo → lowering

1 project に対し 1 つの `RoutingEditIntent`（5 edits・1 CAS）で
Group bus・bus feed・bus→master・PreFader send（approved swap
channel map）・PostFader send（map 無し）を atomic commit する。

- persisted: strip 数 = track 2 + bus 1 + master 1、各 track/bus
  が一意 strip を持ち `StripOwner` back-ref が一致、master-kind
  bus が exactly one。各 send は `kind = Send(id)` connection と
  1:1、`gain_target` が `ControlOwner::Send`/`ParameterRole::
  SendLevel`/`ProjectMusical` の target を指し、serialized
  `SendConfig` の key が `{id, connection, tap, gain_target}` のみ
  （gain 値を持たない）。
- serialization: `InMemoryStore` save→load で `live()` JSON と
  revision が一致。undo→pre-gesture project 完全一致、redo→
  committed project 完全一致。
- lowering: `semantic_nodes` で各 strip が一意 node、bus がその
  strip の node に解決、4 strip の node が distinct。`send_taps`
  で PostFader = main-out port 解決と一致、PreFader = 同 node の
  別 port。send connection は gain→dst edge に `Resolved` し、
  persisted map が spec edge の `(src,dst)` entries に一致
  （`[1,0]` dst→src → `[(0,1),(1,0)]`）。`node_roles` の
  `SendGain.gain` が persisted base（0.25/0.75）と一致。
- mute: `SetStripMute` 後の再 lowering で strip B に触れる全
  connection（send 2 + auto main edge）が `NotLowered(MutedStrip)`、
  `send_taps` から両 send が消え、無関係 edge は `Resolved` のまま。
  mute が `ControlOwner::Strip`/`ParameterRole::Mute` target を
  生むことも確認。

## sfxr01_02 — CreateSend channel-map consent + atomicity

- mutated staging clone（strip out / master in を mono 化し既存
  edge を 1→1 に維持、bus in は stereo のまま）に対し
  `routing_preflight` で matrix を実測: mismatch+map 無し→
  `routing-requires-decision`（Approved でも）、mismatch+map+
  未承認→`routing-requires-decision`、mismatch+map+Approved→
  staged（send・Send-kind connection・Send-owned SendLevel target・
  persisted map が candidate に出る）、不正 map→
  `routing-unsupported-map`。
- commit 経路: equal layout で map 宣言のみ（未承認）→ typed
  reject、revision・serialized project・`routing.sends` が不変。
  成功 op の後に reject しても undo が pre-op 状態へ正確に戻る
  ことを確認（reject が undo stack を消費しない）。Approved なら
  commit され map が connection に永続化される。

## Lane 境界

headless のみ。実 channel 変換の DSP 結果・実 OS GUI・native
plugin・consumer は本 fixture の対象外で、ledger では `NotRun`
のまま記録する。
