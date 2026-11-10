---
id: "ADR-sBklxwfqib4N0abc6I92n9"
type: "adr"
status: "accepted"
title: "SF-XR-01 — CTL-01/CTL-02 cross-gate oracle は既存契約の test fixture のみで構成する"
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
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-06-cross-gate-domain-oracle-case-id-evidence-ledger-headless.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
  defers: []
  deferred-by: []
---

# SF-XR-01 — CTL-01/CTL-02 cross-gate oracle は既存契約の test fixture のみで構成する

## Status

accepted（2026-11-10）

## Context

Issue #109 の SF-XR-01 行は CTL-01（#121）と CTL-02（#122）への
依存で `NotRun` のまま残っていた。両 Issue の契約は main に
実装済み（`tests/ctl_01.rs`・`tests/ctl_02.rs` project+player・
`sf_02.rs` consent/atomicity・`sf06_oracle.rs` invariant oracle）。
ledger が要求するのは「cross-gate での照合」— persisted model・
serialization/undo・player semantic lowering の 3 gate を 1 つの
fixture で突き合わせること — であり、新規契約ではない。

## Decision

- fixture は `crates/synvelis-player/tests/sf_xr_01.rs` に置く。
  project 側の persisted 不変条件と `lower_semantic` の
  `RoutingSourceMap`/`GraphSpec` を同一 committed project 上で
  照合する必要があり、player test が両 crate を参照できる唯一の
  層である。
- `crates/*/src/` は変更しない。consent matrix の project 側は
  既存 `routing_preflight` を直接呼び、commit 経路は
  `ProjectOp::Routing` の `apply` で検証する。
- lane 判定は正直に分ける: persisted/lowering の headless 実測は
  Pass、実 OS GUI・native plugin・consumer・実 channel 変換の
  DSP 結果は `NotRun` のままとする。

## Consequences

- SF-XR-01 の T0/T2 headless 部分が実測 Pass になり、残りは
  external gate（#114/#176/#219/#294 系）に依存する `NotRun` と
  して ledger に明示される。
- fixture が契約不整合を検出した場合は src を変更せず、
  orchestration で報告する運用とする。
