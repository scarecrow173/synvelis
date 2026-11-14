---
id: "TSPEC-034XHDIP0PrKJMfqaVowAi"
type: "test-spec"
status: "approved"
title: "PC-C4 webview composition test spec"
created: "2026-11-15"
updated: "2026-11-15"
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
    - "docs/specs/pc-c4-webview-composition.md"
    - "docs/designs/pc-c4-webview-composition.md"
    - "docs/plans/pc-c4-webview-composition.md"
    - "docs/tasks/pc-c4-webview-composition.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-webview-composition.md"
    - "docs/designs/pc-c4-webview-composition.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-webview-composition.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c4-webview-composition.md"
    - "docs/designs/pc-c4-webview-composition.md"
    - "docs/plans/pc-c4-webview-composition.md"
  verified-by: []
  references:
    - "docs/test-specs/pc-c4-drain-composition.md"
    - "docs/test-specs/pc-c4-observe-engine-wiring.md"
  defers: []
  deferred-by: []
---

# PC-C4 webview composition test spec

## Purpose

webview product bin の composition 経路
（`compose_product_routing`）が spec の gate・attach・非 stub
契約を満たすことを、headless fixture で pin する。実 WebView
backend・実 plugin factory を必要としない範囲で「op surface が
stub でなく実 evidence を返す」ことを保証し、実機受入は
外部 gate に委ねる。

## Feature: webview engine composition gate と op surface

### Rule: gate 閉は engine stub を維持する

`compose_product_routing(app, None)`（`SYNVELIS_PROJECT_DIR`
未設定に相当）は no-op であり、`syn.control.status` は
`engine:false`・`generation`/`surface` null・`drain.attached:
false` を返す。session revision や `ui_pump` を経ても stub の
まま — fail ではなく意図された degrade。

### Rule: gate 開は composition 時点で engine を attach する

`compose_product_routing(app, Some(dir))` は呼出時点で
`attach_routing_engine` + `set_control_report_drain` を行い、
publish 前でも `syn.control.status` が `engine:true` を返す。
最初の session revision で publish → control surface が
generation 1 で attach され、`surface` が実 report を持つ。
empty plugin registry のため `frame_base` は null のまま
（捏造しない）。

### Rule: drain は composition 登録の handle で駆動される

`ui_pump` 1 回が `drain_control_reports` を 1 pass 駆動し、
`syn.control.status.drain` が `attached:true`・
`generation:1`・RT 未観測のため `inactive_gated:true`・
`drained:0`（endpoint 無し・gated で host を呼ばない）を
verbatim 返す。

### Rule: recreate は engine を session と共に detach する

`project.new` 後は `engine:false`・`generation`/`surface` null・
`drain.attached:false` に戻り、dead session の evidence を
残さない。再 attach は deferred（synvelis#106）。

## Examples

- Given fresh `SynvelisApp`+`AppService`+`UiRuntime`、
  When `compose_product_routing(app, None)` を呼び
  `syn.control.status` を dispatch、
  Then `engine:false`・`generation` null・`surface` null・
  `drain.attached:false`。
- Given 同 fixture、When `compose_product_routing(app,
  Some(dir))` → `SetProjectName` revision → `ui_pump`、
  Then `engine:true`・`generation:1`・`surface` object・
  `drain{attached:true,generation:1,inactive_gated:true,
  drained:0}`。
- Given attach 済み fixture、When `project.new`、
  Then `engine:false`・全 evidence が honest default に reset。

## Guarantees

- `synvelis.rs` が呼ぶのと同一の lib helper
  （`compose_product_routing`）を経路として使い、fixture が
  `PluginHost`/`PluginHostProcessors` を名指さない —
  production wiring 自体を検証する。
- `synvelis-native.rs`・`service.rs`・他 crate は無変更で、
  webview 経路の追加のみを検証する。

## Non-goals

- 実 VST3/AU/CLAP plugin factory・実 OS backend・実 GUI・
  wire-schema L1・実 WebView2 boot — 外部 gate（NotRun）。
- `synvelis-native` bin の同等経路 — 既存
  `pc_c4_drain_composition`/`pc_c4_observe_engine_wiring`
  fixture が担保。
- project recreate 後の再 attach — synvelis#106 の責務。
- engine `run`/RT activation・observe cycle の詳細 —
  observe-engine-wiring/drain fixture が担保。

## Risk

- bin の `std::env::var_os` 読み自体は fixture 対象外
  （process boundary）— compile（`check-app-webview`）と
  code review で担保し、helper の `Option` 契約が gate 意味を
  testable にしている。
