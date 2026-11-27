---
id: "IMPL-z5Vw8Lm2Qx4Kr7TyPnD9"
type: "impl"
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
    - "docs/plans/tim-03-instrument-transport-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-03-instrument-transport-oracle.md"
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
  references:
    - "evidence/sv127/instrument-transport-oracle.jsonl"
    - "evidence/sv127/instrument-transport-oracle-verification.md"
  defers: []
  deferred-by: []
---

# TIM-03 instrument direct L1 musical transport oracle — Implementation Record

## 実装内容

- `crates/synvelis-app/tests/common/tim03_l1.rs`
  - `test_instrument` descriptor selection と artifact/revision evidence helper を追加した。
- `crates/synvelis-app/tests/tim03_l1_instrument_transport.rs`
  - direct L1 endpoint に caller supplied `TransportInfo` を渡す VST3/CLAP oracle を追加した。
  - stopped/paused/missing musical negative control、120 BPM click@100/click@500、
    loop click@250、150 BPM click@200、source attribution/freshness 分類を実装した。
- `mise.toml`
  - runner build と dedicated integration test を束ねる `test-tim03-instrument-transport`
    を追加した。
- `evidence/sv127/instrument-transport-oracle.jsonl`
  - format ごとの machine-readable evidence を追加した。

## upstream boundary

production graph end-to-end musical transport は ruviform Issue
`https://github.com/scarecrow173/ruviform/issues/1040` で追跡する upstream handoff gap
により、この slice の Pass 範囲に含めない。Synvelis 側 production code、ruviform pin、
`Cargo.toml`/`Cargo.lock` は変更しない。

## 観測結果の要点

VST3/CLAP 両 format で stopped/paused/missing musical negative control、
120 BPM click@100/click@500、loop click@250、150 BPM click@200 が pass した。
`mise run test-tim03-instrument-transport` は exit 0、その他の canonical lane は
`evidence/sv127/instrument-transport-oracle-verification.md` の表どおり。workspace-wide
`fmt-check` は Orca worktree パス長依存の os error 206 で infrastructure failure、
package-scoped rustfmt check は pass。実測詳細は
`evidence/sv127/instrument-transport-oracle.jsonl` と
`evidence/sv127/instrument-transport-oracle-verification.md` を正本とする。
