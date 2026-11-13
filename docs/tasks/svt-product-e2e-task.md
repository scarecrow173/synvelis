---
id: "TASK-SvtE2e04aB2cD3eF4gH5iJ9"
type: "task"
status: "done"
title: "SV-GUI-05: SVT-01〜09 Headless UI Scenario タスク"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/svt-product-e2e-plan.md"
  implemented-by:
    - "docs/impl/ir/svt-product-e2e.md"
  depends-on:
    - "docs/plans/svt-product-e2e-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/svt-product-e2e.md"
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

# SV-GUI-05 タスク: SVT-01〜09 Headless UI Scenario

## Tasks

- [x] `tests/common/mounted.rs` に mounted fixture + clip bundle
  helper を集約し `common/mod.rs` へ登録。
- [x] `tests/svt_product_e2e.rs` に SVT-01〜09 の製品
  State/Receipt test を実装（10 test）。
- [x] `tests/svt_product_e2e_gpui.rs` に SVT-mapped GPUI
  scenario を実装（`gpui-test` feature）。
- [x] `evidence/sv114/` に L1 被覆 map と Ruviform L1 契約比較を
  記録。
- [x] docs index 再生成・検証レーン実行・review-protocol。

## Done when

- headless lane の全 SVT test が pass し、gpui-test lane の
  SVT scenario が `GpuiSimulated` 証拠として pass する。
- `NotRun`/`Unsupported` 項目が evidence に正直に列挙される。
