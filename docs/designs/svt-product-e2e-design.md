---
id: "DSGN-SvtE2e02aB2cD3eF4gH5iJ7"
type: "design"
status: "approved"
title: "SV-GUI-05: SVT-01〜09 Headless UI Scenario 設計"
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
    - "docs/specs/svt-product-e2e.md"
  implemented-by: []
  depends-on:
    - "docs/specs/svt-product-e2e.md"
    - "docs/designs/sv-gui-03-05-gpui-scenario-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
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

# SV-GUI-05 設計: SVT-01〜09 Headless UI Scenario

## Overview

SV-GUI-03 slice5 が確立した driver-neutral scenario lane を、
Issue #114 の SVT-01〜09 製品 State/Receipt へ拡張する。2 つの
証拠水準を明確に分離する:

1. **製品 State/Receipt lane（headless, OS-L1）** —
   `tests/common/mounted.rs` の `MountedFixture` が
   `SynvelisApp` → `ViewHost` → `NativeShell` → `FakeDriver` を
   `synvelis-native` と同じ composition で立て、`syn.*` op /
   `rvf.ui.command.v1` intent → `UiRuntime`/`AppService` →
   receipt/projection → host display を end-to-end で検証する。
   ここだけが「製品の State/Receipt」の権威経路。
2. **GPUI Test seam lane（`gpui-test`, OS-L1 semantic）** —
   `GpuiGuiTest` は製品 service を持たないため、SVT-02/03/05/07
   の view-layer 契約（intent 形状・gesture parity・stale pin）
   のみを `tests/common/scenario.rs` の driver-neutral oracle で
   検証する。`ScenarioReport.driver = GpuiSimulated` を維持し、
   semantic pass を実機証拠へ昇格させない。

## SVT case → 検証経路 map

| SVT | 製品 State/Receipt（headless） | view-layer（GPUI seam） | 外部 gate |
| --- | --- | --- | --- |
| SVT-01 Project Create/Open→View | `svt01_*`: `syn.project.new` → `project.info` → view.list/gesture 生存 | — | 実 window 再描画は GUI-L2 |
| SVT-02 Selection→Inspector Focus | `svt02_*`: `syn.view.select` → view.list + inspector state、範囲外 reject | `select_row_scenario` | 実 focus/IME は GUI-L2 |
| SVT-03 Fader gesture→receipt→UI | `svt03_*`: Applied receipt + Observed projection + committed display | `fader_parity_scenario` | 実 pointer は GUI-L2 |
| SVT-04 Undo/Redo・reject 真実保持 | `svt04_*`: `set_ppqn` → undo/redo Button → `undone`/`redone`、unknown op reject | — | revision conflict CAS は project 層既存 test |
| SVT-05 Transport Req/Applied/Observed | `svt05_*`: phase 進行 + `syn.control.receipt` broadcast + projection | `seek_parity`/`invoke_scenario` | 実再生 audio は EXT-L3 |
| SVT-06 Waveform/Meter bounded/stale | `svt06_*`: `syn.viz.*` bounded query・over-budget reject・source_changed 解消 | — | NaN gap/実 canvas pixel は NotRun |
| SVT-07 Dock/2Window/Close/Job | `svt07_*`: 2 window adopt・Destroyed で project/view 真実保持 | secondary seek parity | 実 dock/OS focus は EXT-L3、Job op は Unsupported |
| SVT-08 Save/Reload oracle | `svt08_*`: bundle `HEAD`/`snapshots/rN.json` を直接読み照合 | — | 実 FS fault は別 lane |
| SVT-09 IPC dup/delay/reorder/reattach | `svt09_*`: detach で command reject・reattach で復帰 | stale generation pin | duplicate/reorder delivery は binding 層 test |

## 実 Service / Mock Service の区別

- **実 Service（権威）**: `AppService` dispatch の `Result`/
  `Notify`、`syn.view.list`/`syn.project.info`/`syn.viz.meta` の
  read model、bundle ファイルの直接 parse。これらだけが truth
  を証明する。
- **Mock/View-local（投影）**: `ViewHost` の display/phase/
  optimistic selection。receipt/projection が authoritative に
  land したことの *観測* であり、truth の証拠にはしない。
- view-local `select`/`tab` action や host 側 gesture state は
  mock 相当 — 製品 op への変換と reject が契約。

## Evidence honesty

`evidence/sv114/` に (a) SVT-01〜09 headless L1 被覆 map、
(b) Ruviform #243/#313/#388/#389 Fake/Fixture との L1 契約比較
（2 Window Ownership/Focus/Close/Undo/Save・stale generation・
Plugin Editor Session/NativeSurfaceSlot）を記録する。
GUI-L2/EXT-L3 は `NotRun`/`Unsupported` のまま — semantic pass
を昇格させない。
