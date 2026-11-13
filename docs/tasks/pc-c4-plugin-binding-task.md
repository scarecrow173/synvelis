---
id: "TASK-heuaegDgtyVrPGdnyMJko1"
type: "task"
status: "in-progress"
title: "PC-C4 plugin binding task1 — persisted instance authority の bind 照合"
created: "2026-11-14"
updated: "2026-11-14"
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
    - "docs/plans/pc-c4-plugin-binding-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-plugin-binding-plan.md"
    - "docs/specs/pc-c4-plugin-binding.md"
    - "docs/designs/pc-c4-plugin-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-plugin-binding-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-plugin-binding.md"
  verified-by:
    - "docs/test-specs/pc-c4-plugin-binding.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C4 plugin binding task1 — persisted instance authority の bind 照合

## 作業項目

- [ ] project: `PluginParameterInstanceBinding` +
  `plugin_parameter_instance_binding` + lib export
- [ ] project: ctl_02 に canonical mapping テスト追加
- [ ] player: `bind` で plugin 照合後に instance authority 照合
- [ ] player: pc_c4_binding / pc_c6_oracle_pl fixture を persisted
  id へ整合（2-instance は別 ProcessorId で model 化）
- [ ] player: instance authority typed reject テスト追加
  （store 欠落・id 再発行・A→B→A drift・照合順序）
- [ ] player: `pc_c4_binding_rt.rs` session-bound dispatch の RT
  fixture
- [ ] canonical lane: fmt-check / check / lint / test（両
  package）/ test-rt
- [ ] impl/ir・impl/exp・evidence/sv101・docs index 再生成
