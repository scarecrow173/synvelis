---
id: "PLAN-SvtE2e03aB2cD3eF4gH5iJ8"
type: "plan"
status: "approved"
title: "SV-GUI-05: SVT-01〜09 Headless UI Scenario 実装計画"
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
    - "docs/designs/svt-product-e2e-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/svt-product-e2e-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/svt-product-e2e-task.md"
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

# SV-GUI-05 実装計画: SVT-01〜09 Headless UI Scenario

## Scope

- `crates/synvelis-app/tests/common/mounted.rs`（新規）— mounted
  fixture + clip bundle helper。
- `crates/synvelis-app/tests/common/mod.rs` — `pub mod mounted;` 追加。
- `crates/synvelis-app/tests/svt_product_e2e.rs`（新規）— SVT-01〜09。
- `crates/synvelis-app/tests/svt_product_e2e_gpui.rs`（新規,
  `gpui-test` feature）— SVT view-layer scenario。
- `evidence/sv114/`（新規）— L1 被覆 map + Ruviform 契約比較。
- 本 doc chain 7 file + 生成 index。

プロダクトコード（`src/`）の変更は行わない — 既存の op/service/
shell 面で被覆可能な不変条件のみを対象とし、未対応は
`NotRun`/`Unsupported` として記録する。

## File map / dependency graph

```
common/mounted.rs ──┬──> svt_product_e2e.rs   (headless lane)
                    └──> (view_mount.rs は無変更で既存 fixture を維持)
common/scenario.rs ──> svt_product_e2e_gpui.rs (gpui-test lane)
evidence/sv114/*.md ← 検証結果の記録（コード非依存）
```

## Task breakdown

1. `common/mounted.rs` fixture 集約（view_mount.rs と同一
   composition、`pub` 公開）。
2. `svt_product_e2e.rs` — SVT-01〜09 の 10 test。
3. `svt_product_e2e_gpui.rs` — SVT-mapped GPUI scenario。
4. `evidence/sv114/` 記録。
5. docs index 再生成 + 検証レーン実行。

## Verification matrix

| 変更 | 検証 |
| --- | --- |
| `synvelis-app` tests のみ | `mise run fmt-check`、`mise run check synvelis-app`、`mise run lint synvelis-app`、`mise run test synvelis-app` |
| `gpui-test` feature test 追加 | `mise run check-app-gpui`、`mise run lint-app-gpui`、`mise run test-app-gpui` |

`crates/synvelis-workspace`/`synvelis-native` のコード変更なし —
影響範囲は `synvelis-app` test target のみ。

## Risk / rollback

- 新規 test file のみで既存挙動を変えない。失敗時は test を
  修正するか、被覆不能なら `NotRun`/`Unsupported` 記録へ落とす。
  プロダクトコードへの workaround 追加は行わない。
