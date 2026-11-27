---
id: "DESIGN-fbD127mQw4Ln8KrTyVzB"
type: "design"
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

# TIM-03 frame/boundary matrix — Design

## 設計

### Headless matrix

`ProcessCapability::evaluate`、実 `TransportScheduler`、実 `PluginNode`/`GraphRuntime`、wired `ProcessBoundarySession` を別々に検証する。各 cell で fresh scheduler/node/session を使い、queue residue を次 cell へ持ち込まない。contract `min=1/max=512` では exact partitionが segment lengthと一致するため、exact physical process length sequence を直接固定する。

### Event order

same-position plan events は upstream の `(position_ticks, source_sequence)` ordering を consumer evidence として固定する。same `ParamId` same offset は stable source orderで配送され、最終値は last-wins として読む。frame offset は媒体 frame の絶対位置ではなく physical process call start relative である。

### Routing/driver derivation

routing surface と HeadlessDriver の両方で scheduler-faithful wrap simulation から upcoming physical window の loop-wrap boundaryを導く。W=100では loop endが window endに一致する case、W=128では interior wrap、W=256では same-head cascade wrapを使い、head wrapと interior cutを区別する。

### Production real ABI

variable block geometry は explicit helperでのみ構成し、既存 256-frame helper semanticsを変えない。L1 latencyは `lookahead_blocks × activated max_frames` であり、この production seamでは activated max_framesが engine block geometryなので W=100/128/256に応じて 200/256/512 framesとなる。click lawは fixture lawを独立計算し、production codeへ複製しない。
 Natural loop positiveは first in-loop clickと post-wrap clickの両方を要求し、W=100ではblock-grid latticeを満たす loop tailを選んで quantizationをごまかさない。
