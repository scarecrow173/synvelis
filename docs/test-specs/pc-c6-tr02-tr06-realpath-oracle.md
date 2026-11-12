---
id: "TSPEC-034WxmAJbjUwK6FBs7Xiy2"
type: "test-spec"
status: "approved"
title: "PC-C6 TR02/TR06 realpath oracle 検証仕様"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/designs/pc-c6-tr02-tr06-realpath-oracle-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/designs/pc-c6-tr02-tr06-realpath-oracle-design.md"
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
    - "docs/test-specs/pc-c6-tr-ledger-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/pc-c6-tr02-tr06-realpath-oracle-task.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/pc-c6-tr02-tr06-realpath-oracle-design.md"
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C6 TR02/TR06 realpath oracle 検証仕様

`crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs` の
`tr02_context_mismatch_dimensions_real_ledger` /
`tr06_silence_vs_unobserved_real_ledger` が担保する契約。全て
headless + in-process fake plugin — 実 GUI/実 plugin/OS/RT 観測は
対象外。trace は `ParamDispatch` + `DispatchTraceAdapter` +
`GraphRuntime` の実経路で生成し、期待 freshness/coherence は oracle
が fixture 宣言から独立に導出して `ControlTrace` の観測と field
単位で照合する。oracle と実装の差異が出た場合は、実装を oracle に
合わせて変更せず、契約解釈を spec/design/test-spec に立ち返って
確認する。

## Feature: TR02 — key↔ctx 全次元 mismatch の stale 分類（実 ledger 経路）

- Rule(TR02-r1): live evidence（plugin-reported + trusted carried
  probe）を持つ実経路 trace は、`project_revision` /
  `control_plan_generation` / `graph_generation` /
  `transport_generation` / `clock_epoch` / `loop_occurrence` /
  `evaluation_position` の 7 次元それぞれだけを変えた context で、
  対応する `StaleReason::<...>` の `Stale` に分類され `Current`
  にならない。期待値は独立 oracle と実装の両方で一致すること。
- Rule(TR02-r2): 全 7 次元を同時に変えた context は、実装の
  documented check order（`ProjectRevision` が先）に従い
  `StaleReason::ProjectRevision` に解決されること。独立 oracle も
  同じ順序で導出すること。
- Rule(TR02-r3): 上記いずれの stale 分類でも `is_coherent()` は
  true のまま — coherence は key/evidence 内部の整合であり、
  評価 context の鮮度とは独立であること。
- Rule(TR02-r4): 検証対象の `ControlTrace` の key フィールド
  （`ProjectRevision`・graph generation・plan generation 等）が
  実 dispatch/observe 経路の生成物であることを assert し、trace を
  手組みしない。

## Feature: TR06 — measured silence vs unobserved（実 ledger 経路）

- Rule(TR06-r1): target 801 に `MeasuredValue::Silence` +
  carried timestamp を記録すると、stored `ProbeObservation::
  Measuring` が `Silence`・timestamp・provenance を field 単位で
  保持し、observed/combined freshness が `Current` になること。
- Rule(TR06-r2): 同一実 plan で dispatch されたが未 touch の
  sibling target は `Unavailable { NoEvidence }` に分類され、
  silence 証拠が漏れないこと。
- Rule(TR06-r3): sibling を `ProbeObservation::Missing` /
  `Disabled` / `Lost { count }` で順に更新すると、それぞれ
  `Unavailable { ProbeMissing }` / `Unavailable { ProbeDisabled }` /
  `DataLoss { count }` に分類され、`Silence` と混同されないこと。
- Rule(TR06-r4): sibling の probe 状態遷移後も、silence を記録した
  target の trace は `Current` のまま変化しないこと — probe 状態は
  target 単位で独立であること。

## Examples

- TR02: target 701 を bind し実 plan を dispatch → `GraphRuntime`
  駆動 → `observe_cycle` で span 観測 → `record_reported` +
  `record_probe(Carried)`。`key.project_revision == ProjectRevision(5)`
  等を assert 後、7 次元 scenario table を oracle/実装で照合。
- TR06: target 801/802 を同一 plan で dispatch。801 に
  `record_probe(Measuring(Silence, ts, Carried))` → `Current`。
  802 は未 touch で `NoEvidence`、続けて `Missing`→`ProbeMissing`、
  `Disabled`→`ProbeDisabled`、`Lost{count:4}`→`DataLoss{4}`。
  各段階で 801 が `Current` のままであることを再 assert。

## Guarantees

- 全判定は決定論的・read-only 評価。`GraphRuntime` 駆動は
  headless で実時間に依存しない。
- 欠落・失敗・stale・lost・unavailable を成功・Live・観測済みと
  誤認しない。`MeasuredValue::Silence`（実測無音）を未観測
  （`Missing`/`NoEvidence`）と混同しない。
- headless + fake plugin の Pass を実 GUI/実 VST3/実 OS/実 RT の
  成功に繰り上げない。

## Non-goals

- TR01/03/04/05/07/08 の実経路再証拠 — 直前 slice
  （`docs/test-specs/pc-c6-tr-ledger-oracle.md`）で担保済み。
- adapter 単体契約（drop 写像・expiry window・receipt attribution
  内部・dedup）の再検証 — `trace_c3_05.rs` の責務。
- 実 GUI/plugin/OS/RT・consumer 照合（#114・#101・#102・Ruviform
  gate）。
- PC-C1-R01〜08・PC-C4-PL01〜08・PC-C5-UI01〜07・
  PC-C6-COMP01〜05（Issue の残件）。

## Risk

- TR02 の composite 順序は実装の check order 依存 — 実装が順序を
  変えた場合 test が fail で顕在化し、oracle/spec の再確認を要求
  する。順序を緩める変更は契約変更として扱う。
- TR06 の sibling 分離は probe channel が target 単位で独立である
  ことを前提 — adapter が target key で分離しなくなった場合 fail
  で顕在化する。
- oracle 自体の誤実装リスクは先行 slice と同じ — 規則の一次情報は
  `docs/test-specs/pc-c6-e2e-oracle-rel-tr.md` Rule(TR02)/Rule(TR06)
  と spec/design であり、差異時は契約解釈を再確認する。
