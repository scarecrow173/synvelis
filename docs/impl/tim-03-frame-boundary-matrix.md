---
id: "IMPL-fbI127kQw1Ln4KrTyVzF"
type: "impl"
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

# TIM-03 frame/boundary matrix — Implementation

## 変更点

- `tim03_process_boundary.rs`: policy/scheduler/node exact frame-boundary matrix、param frame-offset/order matrixを追加。
- `tim03_request_plan.rs`: wired session matrixを caseごとの fresh instanceへ修正し exact process lengthsを固定。
- `tim03_loop_boundary.rs` / `tim03_driver_loop_boundary.rs`: physical window 100/128/256の loop-wrap derivation matrixを追加。
- `common/tim03_l1.rs`: explicit variable-block session/run/activation helperを追加。既存 256-frame wrapper semanticsは変更しない。
- `tim03_l1_frame_boundary_matrix.rs`: production real ABI matrix evidenceを追加。
- `mise.toml`: `test-tim03-frame-boundary-matrix` laneのみ追加。

## Evidence semantics

headless exact process-length proofと production real ABI PCM proofは分離する。production PCMは click lawによる primary evidenceだが、全physical call lengthsの authoritative proofではない。project PPQN 480はhost/project fact、fixture visible PPQN 960はadapter projectionであり fixed fallbackではない。

## Limitations

public seamの arbitrary interior scheduling unsupported cellsは NotRun/unsupportedのまま。fake-onlyを production ABI Passに昇格しない。docs indexは編集していない。
