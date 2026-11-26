---
id: "SPEC-SV114SVOSCONSUMER00000001"
type: "spec"
status: "approved"
title: "SV-OS consumer catalog — SV-OS-01〜05 catalog/headless/real lane slice (synvelis#114)"
created: "2026-11-26"
updated: "2026-11-26"
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
    - "docs/discovery/sv114-svos-consumer-catalog.md"
    - "docs/specs/sv-gui-05-real-os-e2e.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv114-svos-consumer-catalog.md"
    - "docs/test-specs/sv114-svos-consumer-catalog.md"
    - "evidence/sv114/sv-os-case-definitions.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv114-svos-consumer-catalog.md"
  references: []
  defers: []
---

# SV-OS consumer catalog spec

## 目的

frozen SV-OS-01〜05 を ruviform#464/#631 の `ScenarioEntry`/`ScenarioMatrix`/
`DriverBinding` consumer catalog として表し、headless lane と real-OS lane が
同じ case 定義を実行できるようにする。本 slice は Issue #114 全体の完了ではなく、
`Refs #114` の partial slice である。

## Scope

### 実装する

- `tests/common/sv_os_catalog.rs` に SV-OS-01〜05 の catalog declaration を実装する。
- `tests/sv_os_headless.rs` で Fake lane と `gpui-test` 時の `GpuiSimulated` laneを実行する。
- `tests/sv_os_real.rs` と `real-os-scenario` feature で `RealOsGuiTest` composition rootを作る。
- real lane の interactive desktop preflight を fail-closed にし、利用不可なら全件 NotRunとする。
- Required/Deferred/NotApplicable を case/capability 単位で記録する。

### 実装しない

- SV-OS-06〜09 executable case、SV-OS-09 Native Plugin Dock、PD-01〜07。
- physical OS pointer/keyboard input、IME composition、pixel golden、DPI/monitor 横断。
- ruviform 本体、pin、`Cargo.lock`、`mise.toml`、docs index、CI workflow の変更。
- `gpui-test` lane の pixel capture、headless pass の real GUI/pixel/native dock pass への昇格。

## 受入対応

| slice 受入 | 本 spec の要求 |
| --- | --- |
| driver-neutral catalog | case id/scenario id/required/spec extras/run function が `sv_os_entries` から追跡できること |
| 製品 oracle | catalog body が semantic adapter oracle（component/action/projection/receipt/stale generation）と、同一 case に対応する mounted `NativeShell`/`SynvelisApp` 製品 service oracle を評価し、catalog 登録や runner 起動だけを Pass にしないこと |
| headless lane | Fake/GpuiSimulated が同じ entries を実行し、driver/capability を正直に記録すること |
| real lane | `real-os-scenario` opt-in target が実 window composition を作り、preflight 不可なら NotRunとすること |
| lane verdict 分離 | Headless/Simulated、Real OS、Native Plugin、CI evidence を別列・別 verdictとすること |
| docs/evidence | 日本語 docs 4 点と `evidence/sv114/` 証拠を同期すること |

## Capability 分類

SV-OS-01〜04 は `SemanticAction`、SV-OS-05 は `SemanticAction + MultiWindow`を executable requirement とする。`RealOsPointerOrKeyboard`、`VisualCapture`、`Ime`は後続 GUI-L2 activation に Deferred、`NativePluginDock` は SV-OS-01〜05 ではNotApplicable とする。SV-OS-09 の native dock は本 slice に含めない。

## 証拠の意味づけ

catalog report の `driver` は実行 adapter、capabilities は adapter が提供する能力である。
Fake/GpuiSimulated の Pass は semantic lane verdict に限る。Real OS lane で window/focus/geometry上の semantic Command→Receipt→view-state が Passでも、physical OS input/pixel/service-backed create/open/close の全量を同時に主張しない。したがって Issue 全体の AC6 受入は引き続き後続である。
