---
id: "IMPL-SvtE2e01aB2cD3eF4gH5iK4"
type: "impl"
status: "completed"
title: "SV-GUI-05: SVT-01〜09 Headless UI Scenario 実装記録"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  changes:
    added:
      - type: "file"
        path: "docs/specs/svt-product-e2e.md"
      - type: "file"
        path: "docs/designs/svt-product-e2e-design.md"
      - type: "file"
        path: "docs/plans/svt-product-e2e-plan.md"
      - type: "file"
        path: "docs/tasks/svt-product-e2e-task.md"
      - type: "file"
        path: "docs/test-specs/svt-product-e2e.md"
      - type: "file"
        path: "docs/impl/ir/svt-product-e2e.md"
      - type: "file"
        path: "docs/impl/exp/svt-product-e2e.md"
      - type: "file"
        path: "crates/synvelis-app/tests/common/mounted.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/svt_product_e2e.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/svt_product_e2e_gpui.rs"
      - type: "dir"
        path: "evidence/sv114"
    modified:
      - type: "file"
        path: "crates/synvelis-app/tests/common/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/svt-product-e2e-task.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/svt-product-e2e.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-GUI-05 実装記録: SVT-01〜09 Headless UI Scenario

## 実装内容

- `tests/common/mounted.rs`: `MountedFixture`（app → `ViewHost` →
  `NativeShell` → `FakeDriver`、`view_mount.rs` と同一
  composition）+ `write_clip_bundle`/`open_clip_project`/op helper を
  `pub` 集約。`view_mount.rs` は無変更。
- `tests/svt_product_e2e.rs`: SVT-01〜09 の 10 test。製品
  State/Receipt を `syn.*` op / `rvf.ui.command.v1` → `AppService`
  → receipt/projection → host の実経路で検証し、`syn.view.list`/
  `syn.project.info`/bundle ファイル直接 parse を独立 Oracle とする。
- `tests/svt_product_e2e_gpui.rs`（`gpui-test`）: SVT-02/03/05/07
  相当の driver-neutral scenario を `GpuiGuiTest` で実行。
- `evidence/sv114/`: L1 被覆 map + Ruviform #243/#313/#388/#389
  L1 契約比較。

## 検証結果（実測）

- `cargo test -p synvelis-app --test svt_product_e2e`:
  10 passed / 0 failed（Windows, mise toolchain）。
- GPUI lane・fmt/lint/check は PR 本文の検証表を正本とする。

## Deferred scope（canonical doc 参照を持たない外部 gate）

- 実 Window/OS Input/Focus/IME/DPI/Pixel・native dock・実 plugin
  editor・実 Audio/MIDI（GUI-L2/EXT-L3 — synvelis#176 系の別 gate、
  Ruviform #313/#314/#388/#389 acceptance）。
- `CallOutcome::Job` を返す `syn.*` op（現行 AppService に job
  producer なし — Unsupported）。
- Plugin Panel 実 editor session（`syn.app.info` が unavailable を
  正直に報告 — fake success にしない）。

## 設計上の判断

- fader gesture commit は undoable project op を生成しないため、
  SVT-04 の mutation は `syn.project.set_ppqn`（`ProjectOp` として
  history に乗る）を使用。
- transport play Button の terminal phase は `Applied`（receipt
  権威）。`Observed` は correlated projection が land した場合のみ
  — 両者を分離して assert。
- `syn.viz.*` の `view_stale=true` は `ui_pump` 内で解消される
  transient — app op 境界では resync 済み `status=ok` を assert し、
  transient 自体は `synvelis-workspace/tests/sv48_viz.rs` の
  `VizUi` 層 test が保証することを evidence に記録。
