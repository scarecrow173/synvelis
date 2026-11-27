---
id: "PLAN-fbP127zQw2Ln6KrTyVzC"
type: "plan"
status: "approved"
title: "TIM-03 frame/boundary matrix"
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

# TIM-03 frame/boundary matrix — Plan

## Plan

1. 既存 TIM-03 boundary/session/loop/driver/production musical transport architectureを read-only診断する。
2. `tim03_process_boundary.rs` に policy/scheduler/node exact matrixと param frame-offset ordering matrixを追加する。
3. `tim03_request_plan.rs` の wired session matrixを fresh-instance exact sequenceへ修正する。
4. `tim03_loop_boundary.rs` / `tim03_driver_loop_boundary.rs` に 100/128/256 geometry derivation matrixを追加する。
5. `common/tim03_l1.rs` に explicit variable-block config/run/activation helperを追加し、既存 256-frame wrapper semanticsを保つ。
6. `tim03_l1_frame_boundary_matrix.rs` を追加し、VST3/CLAP test_instrumentで seek/natural loop/tempo revision positiveを独立 PCM oracleで検証する。
7. `test-tim03-frame-boundary-matrix` lane、docs chain、JSONL/human evidence、canonical verification、PRを作る。

## 判断記録

- public seamが arbitrary interior command schedulingを持たない cellは unsupportedとし、lease拡張や direct injectionを行わない。
- production PCMから全 physical process lengthを直接再構成できないため、exact length proofはheadless recording plugin layerに留める。
