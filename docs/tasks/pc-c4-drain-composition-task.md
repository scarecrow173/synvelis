---
id: "TASK-a1d4f7c2e9b30586d1a4f7c2f0a2"
type: "task"
status: "done"
title: "PC-C4 drain composition task1 — drain 周期配線・production composition root"
created: "2026-11-13"
updated: "2026-11-13"
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
    - "docs/plans/pc-c4-drain-composition-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-drain-composition-plan.md"
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/designs/pc-c4-drain-composition-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-drain-composition-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-drain-composition.md"
  verified-by:
    - "docs/test-specs/pc-c4-drain-composition.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C4 drain composition task1 — drain 周期配線・production composition root

## 作業項目

- [x] player: `PluginReportDrain` + `PluginHostProcessors::product`/
  `report_drain`（host cell を app が名指さない seam）
- [x] app engine: `control_drain_reports`（surface generation +
  RT 観測 active generation で fail-closed に drain）
- [x] app: drain 登録 field/setter + project 再生成時クリア
- [x] app: `drain_control_reports`（pub 1 pass）+ `ui_pump` 周期配線
- [x] app: `compose_routing_engine` production composition root
- [x] native bin: boot project 時に composition を呼ぶ
- [x] fixture `pc_c4_drain_composition.rs`: 周期伝播・
  inactive gate fail-closed・queue 飽和 typed・composition op 経路
- [x] canonical 9 lane 全 pass
