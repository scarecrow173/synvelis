---
id: "TSPEC-PCc4030bs3vN1pQ8wXyZ2aF"
type: "test-spec"
status: "approved"
title: "PC-C4-03 param observation — drain→ledger・gate・typed 終端 test 保証"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c4-03-param-observation.md"
    - "docs/designs/pc-c4-03-param-observation-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-03-param-observation.md"
    - "docs/designs/pc-c4-03-param-observation-design.md"
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-03-param-observation.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c4-03-param-observation.md"
  defers: []
  deferred-by: []
---

# PC-C4-03 test spec — param observation

対象 test: `crates/synvelis-player/tests/pc_c4_03_observation.rs`（8 件、
headless）。

## Feature: drain → ledger 前進

- **Rule PC-C4-03-R1**: `Drained` の `EventKind::ParamValue` は session
  binding 経由で `record_reported` され、ledger entry が `Reported`・
  value が plugin-reported 値になる。dispatched 値と異なる値で区別する。
- **Example**: `pc_c4_03_01` — write 0.8 に対し plugin announce 0.5 が
  `Reported` value として観測される。counters: `drained=1, events=4,
  param_reports=3, reported=1, unbound_param=1, no_pending=1`。

## Feature: 正直終端

- **Rule PC-C4-03-R2**: `ReportDrain::Unavailable`・`InvalidHandle`・
  `Failed` は pending entry を `Unavailable`（value なし）に終端する。
  success 偽装なし。
- **Example**: `pc_c4_03_02`（Unavailable 注入）、`pc_c4_03_06` 後半
  （InvalidHandle）。
- **Rule PC-C4-03-R3**: 非終端 `HostError` は typed `errors` 証拠のみで
  ledger 不変（`Dispatched` を維持）。
- **Example**: `pc_c4_03_06` 前半（`HostError::Ipc`）。

## Feature: generation gate

- **Rule PC-C4-03-R4**: `generation`/`active_generation` 不一致は全
  endpoint の drain 前に gate し、report は plugin-side queue に残る。
  `observe_cycle` も同じ generation で fail-closed。
- **Example**: `pc_c4_03_03`（誤 generation 拒否→正 drain で回収）、
  `pc_c4_03_04`（inactive gate→queued reports 温存）、
  `pc_c4_03_08`（gen swap 後の旧 generation endpoint 拒否）。

## Feature: ledger 非対象 report

- **Rule PC-C4-03-R5**: unbound `ParamId`・`Midi` 等の非 ParamValue
  kind・eligible entry のない bound param は counter 証拠のみで
  observation を生まない。
- **Example**: `pc_c4_03_05`（write 前 drain: `no_pending=1,
  unbound_param=1, events=3, observations 空`）。

## Feature: drain-once

- **Rule PC-C4-03-R6**: 繰り返し drain は pending buffer を一度だけ消費
  し、report の重複適用をしない。scheduler channel（`observe_cycle`）
  とは disjoint。
- **Example**: `pc_c4_03_07`。

## NotRun

- **L1 slot 実値**: pin `b02942c2` に wire report verb なし
  （RF#253 PR #564 未マージ）。`Unavailable` 分岐は注入 seam で検証済み。
  pin bump 後に実 L1 slot での drain を再検証する。
