---
id: "PLAN-dynTm03Pw8Ln5Vx9Kr"
type: "plan"
status: "approved"
title: "TIM-03 dynamic musical transport"
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
  implements: []
  implemented-by: []
  depends-on: []
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


# TIM-03 dynamic musical transport — Plan

1. baseline PR #686 `82095ea5c9545345ff193156525243bd8ca0fcc4` の production musical transportを前提に、routing `Equivalent` revisionで musical syncがskipされる原因を切り分ける。
2. `engine.rs` / `routing_control.rs` / `dispatch.rs` に限定して dynamic refresh reconciliationを実装する。
3. `FastControlWrite::transport_generation` を optional production semanticとして追加し、compile propagationが必要な既存 construction sitesには `None` のみ機械的に伝播する。
4. player/control representative mixed-plan testで staged MIDI、unbound event、generation-bound eventの保存と invalid project stateのhonest absenceを検証する。
5. production real ABI oracleに representative dynamic tempo edit 2 caseを追加し、VST3/CLAPで independent click lawを検証する。
6. `test-tim03-dynamic-musical-transport` canonical mise entrypointを追加し、dynamic evidenceを再現可能にする。
7. canonical commandsを実行して raw log/exit codeを `evidence/sv127/dynamic-musical-transport/` へ保存する。
8. dynamic docs chain・JSONL・verification.md・PR本文を整備し、docs indexはCoordinator統合時整備に委譲する。

## 判断記録

- dynamic refreshはrouting rebuildではなくcontrol-plane reconciliationで行う。tempo mapをrouting fingerprintへ追加すると不要なgraph rebuild/control generation mintを招き、TIM-03の責務境界を超える。
- musical refreshで `Play` を再送しない。position re-anchorはtransport command層の責務であり、tempo map更新に純粋なplan refreshを混ぜない。
- `transport_generation` は upstream contractを production write pathへ写す optional fieldであり、test-only seeding APIではない。
