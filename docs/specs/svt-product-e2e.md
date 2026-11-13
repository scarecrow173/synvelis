---
id: "SPEC-SvtE2e01aB2cD3eF4gH5iJ6"
type: "spec"
status: "approved"
title: "SV-GUI-05: SVT-01〜09 製品 State/Receipt Headless UI Scenario"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
    - "docs/specs/sv-gui-04-02-close-veto.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/svt-product-e2e-design.md"
    - "docs/plans/svt-product-e2e-plan.md"
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

# SV-GUI-05 仕様: SVT-01〜09 製品 State/Receipt Headless UI Scenario

## What Should Be Built

Issue #114 の残件スライスとして、SVT-01〜09 が対象とする
Project / Transport / Undo / Save / Job / Window / Plugin Panel の
**製品 State/Receipt** を、Fake Driver + GPUI Test seam の
Headless UI Scenario で検証する。実 Service（`AppService` over sealed
`SynvelisApp`）と Mock/View-local 状態の不変条件を区別して記録し、
実 OS Window/Input/Pixel・native dock・実 plugin editor・実
Audio/MIDI（GUI-L2/EXT-L3）は `NotRun`/`Unsupported` のまま残す。

- `crates/synvelis-app/tests/common/mounted.rs` に、app → mounted
  `ViewHost` → `NativeShell` → `FakeDriver` の composition fixture を
  集約する（`view_mount.rs` の composition と同一配線。
  `view_mount.rs` 自体は無変更で維持する）。
- `crates/synvelis-app/tests/svt_product_e2e.rs` に SVT-01〜09 の
  製品 State/Receipt scenario を追加する。各 case は op → receipt →
  projection/observed UI state を実 Service 経路で検証し、独立した
  製品 Oracle（`syn.view.list`/`syn.project.info`/bundle ファイル
  直接読み）と照合する。
- `crates/synvelis-app/tests/svt_product_e2e_gpui.rs`
  （`gpui-test` feature）に、SVT-02/03/05/07 の view-layer 契約を
  `GpuiGuiTest` で実行する SVT-mapped scenario を追加する。
  製品 service を持たない adapter なので semantic/view-layer 証拠に
  限定する。
- `evidence/sv114/` に SVT-01〜09 の被覆 map・実 Service/Mock
  Service 不変条件の区別・Ruviform #243/#313/#388/#389 Fake/Fixture
  との L1 契約比較を記録する。

## Acceptance Criteria

- [ ] SVT-01〜09 の headless 可能な製品 State/Receipt 不変条件が
  `svt_product_e2e.rs` で検証される。
- [ ] SVT-02/03/05/07 の view-layer 契約が `GpuiGuiTest` で
  `GpuiSimulated` 証拠として検証される。
- [ ] 実 Service と Mock/View-local の不変条件が evidence で区別
  される。`Pass`/`Fail`/`NotRun`/`Unsupported` が正直に記録される。
- [ ] 2 Window Ownership/Focus/Close/Undo/Save・stale generation・
  Plugin Editor Session/NativeSurfaceSlot の L1 契約が、Ruviform 側
  Fake/Fixture との比較として `evidence/sv114/` に記録される。
- [ ] 実 OS GUI・実 plugin・実 device を要する項目は `NotRun`/
  `Unsupported` のまま。semantic pass を昇格させない。

## Non-Goals

- 実 Window/OS Input/Focus/IME/DPI/Pixel（GUI-L2、Ruviform #313/
  #314 系）。
- 実 native dock・実 plugin editor session・実 Audio/MIDI device
  （EXT-L3、synvelis#176 系）。
- `CallOutcome::Job` を返す `syn.*` op の新設 — 現行 AppService に
  job producer がないため job-lifetime は `Unsupported` 記録。
- Plugin Panel の実 editor — `syn.app.info` は plugin 面を
  unavailable と正直に報告しており、その状態を fake success に
  置き換えない。
- Ruviform 共通 Scenario Runner / Driver / NativeSurface /
  PluginEditorSession 実装の変更（外部 owner の責務）。
