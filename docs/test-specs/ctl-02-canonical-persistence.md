---
id: "TSPEC-LxmoRkKfW8lDUiHntJVgGO"
type: "test-spec"
status: "approved"
title: "CTL-02 canonical persistence 検証仕様"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/122"
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
    - "docs/specs/ctl-02-canonical-persistence.md"
    - "docs/designs/ctl-02-canonical-persistence.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/ctl-02-adopt-sendlevel.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ctl-02-canonical-persistence.md"
    - "docs/designs/ctl-02-canonical-persistence.md"
  derived-by: []
  verifies:
    - "docs/specs/ctl-02-canonical-persistence.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# CTL-02 canonical persistence 検証仕様

`crates/synvelis-project/tests/ctl_02.rs` が担保する persistence 契約。
全て headless project 層 fixture であり、実 Plugin/OS/GUI/RT 観測は
対象外。canonical 実行入口は mise task とする。

## Feature: v15→v16 deterministic normalization

- Rule(CTL02P-R1-r1): schema v15 snapshot に非 canonical な master fader、
  通常 strip の bool role（Mute/Solo）、SendLevel を置いて load すると、
  v15→v16 migration 後にそれぞれ canonical `kind`/`mapping`/`time_scope`/
  `capability: None` を持つ。
- Rule(CTL02P-R1-r2): 同じ fixture で `name`/`base`/target ID/owner/role
  が保持される。`base` は authored mixer/send level の値正本なので、
  seeded default へ戻さない。
- Rule(CTL02P-R1-r3): automation lane の target 参照、send の
  `gain_target` binding、`id_watermark` が変わらない。migration は ID を
  採番しない。
- Rule(CTL02P-R1-r4): free-form targetと plugin/processor capability target
  は canonical internal contract の対象外であり、migration 前後で
  `ParameterTarget` が等しい。
- Rule(CTL02P-R1-r5): 同じ v15 bytes を 2 回 load した projection が等しい
  — deterministic migration。

## Feature: v16 current snapshot fail-closed oracle

- Rule(CTL02P-R2-r1): schema v16 snapshot で internal target の `kind` を
  canonical から外すと load は `Error::Validation` で失敗する。
- Rule(CTL02P-R2-r2): `mapping` を外す場合も同じく fail-closed。
- Rule(CTL02P-R2-r3): `time_scope` を外す場合も同じく fail-closed。
- Rule(CTL02P-R2-r4): `capability: Some` を持つ場合も同じく fail-closed。
- Rule(CTL02P-R2-r5): validation error message は target id、owner、role、
  外れた field 名を含む。
- Rule(CTL02P-R2-r6): v16 body は migration されず修復もされない。reject
  だけが観測される。

## Feature: current-state pass と非回帰

- Rule(CTL02P-R3-r1): fresh project は canonical internal spec oracle を
  pass する。
- Rule(CTL02P-R3-r2): v9 snapshotは ordered ladder で v16 まで migration
  され、strip seeding と canonicalization の両方を通って oracle を pass
  する。
- Rule(CTL02P-R3-r3): 既存 strip seeding、first-use `Pan`、send create
  atomicity、canonical adoption/rebind fixture は非回帰である。
- Rule(CTL02P-R3-r4): `SCHEMA_VERSION` fixed assertions は 16 を期待する。
  v15 は ladder で受け入れられ、v16 より新しい version は既存 test の
  `UnsupportedSchema` 挙動どおり拒否される。

## Guarantees

- persisted current state の internal target は 1 つの canonical spec を
  持つ。free-form/plugin target はこの invariant の対象外。
- migration は authored `name`/`base` と identity/reference graph を保持
  する。
- current schema の異常は silent repair されず、人間が追跡できる validation
  error になる。

## Non-goals

- GUI / Inspector / Mixer の表示分離。
- 実 plugin binding、SourceMap/ControlTrace 統合、Graph generation 照合、
  誤配送ゼロ実測。
- Channel mismatch approval flow、Realtime/Offline/2Window、AudioLens。
- orphan/unreferenced target の削除や cascade。
- EXT-L3 / GUI-L2 / 3OS / provisioned runner。

## Canonical verification lanes

| Lane | Command |
| --- | --- |
| format | `mise run fmt-check` |
| check | `mise run check synvelis-project` |
| lint | `mise run lint synvelis-project` |
| CTL-02 fixture | `mise run test synvelis-project --test ctl_02` |
| package regression | `mise run test synvelis-project` |
| RT non-regression | `mise run test-rt` |
| headless dependency boundary | `mise run check-headless-deps` |

`fmt-check` が Windows `os error 206` で起動不能な場合は Pass に見せ
かけない。environmental failure として記録し、package scoped fmt を補助
証拠として区別する。
