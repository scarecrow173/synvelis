---
id: "SPEC-a1d4f7c2e9b30586d1a4f7c2e"
type: "spec"
status: "approved"
title: "PC-C4 drain composition — drain_output_reports 周期配線と production composition root"
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
    - "docs/specs/pc-c4-04-app-composition.md"
    - "docs/specs/pc-c4-06-control-op-surface.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-04-app-composition.md"
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/specs/pc-c4-07-transport-observe.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-plugin-parameter-binding-frame-base-capability-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-04-app-composition.md"
    - "docs/specs/pc-c4-06-control-op-surface.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-drain-composition.md"
  verified-by:
    - "docs/test-specs/pc-c4-drain-composition.md"
  references:
    - "docs/specs/pc-c4-04-app-composition.md"
    - "docs/specs/pc-c4-06-control-op-surface.md"
  defers:
    - "L1 wire 検証・実 VST3/OS backend・実 GUI・AudioLens/Ruviform consumer 照合（synvelis#101 残件・後続 slice）"
  deferred-by: []
---

# PC-C4 drain composition — drain_output_reports 周期配線と production composition root

## 背景

synvelis#101（長寿 epic）の 2026-11-12 統合コメントが残件として名指しした
slice のうち、本 spec は **`drain_output_reports` の周期配線** と
**production composition root** の 2 件のみを対象とする。

- `RoutingControlDispatch::drain_output_reports`（synvelis-player）は
  PC-C4-03/04/06 で typed drain・generation gate・ledger `Reported`
  進行まで実装済みだが、呼び出しは headless fixture が
  `with_control_dispatch` 経由で手動駆動するだけで、app の周期経路
  （`ui_pump`）にも製品 composition root（`synvelis-native`）にも
  配線されていない。
- 製品入口は `SessionRoutingEngine`/`PluginHostProcessors` を一度も
  組み立てていないため、`syn.control.*` op surface と
  `observe_cycle` は実経路で到達不能だった（engine stub のまま）。

Issue は本 slice merge 後も Open 継続（L1 wire 検証・実 backend・
実 GUI・consumer 照合が残件）。PR は `Refs #101`。

## 目的

1. app 層が plugin→host output report の drain を**決定的・周期的**に
   駆動し、`Reported → Observed` の伝播・generation gate の
   fail-closed・queue 飽和時の typed 応答を headless fixture で pin する。
2. 製品入口（native bin / app shell）が `observe_cycle`/control op
   surface を**実経路**で組み立てられる production composition root
   を定義し、headless composition テストで検証する。

## スコープ

- `crates/synvelis-player`: composition root が host cell を名指さずに
  drain を駆動するための seam（`PluginReportDrain`）と
  `PluginHostProcessors` の product 構築口。
- `crates/synvelis-app`: engine の generation-scoped drain 駆動、
  `ui_pump` 周期配線、composition root 関数、native bin の配線、
  fixture テスト。
- doc-chain: `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/pc-c4-drain-composition*`。

## 非目標（本 slice の範囲外）

- 実 RT thread / OS timer への drain 配線 — caller/deterministic
  driver が周期を供給する。`ui_pump` は 16 ms UI tick と op tail で
  駆動される既存の非 RT 周期であり、本 slice はそれを使う。
- 実 VST3/AU/CLAP plugin backend・OS device backend
  （ruviform#253 系の外部 gate）。
- L1 wire schema 検証、GUI 実機受入、AudioLens/Ruviform consumer
  照合、oracle（synvelis#103）作業。
- project 切替（`project.new`/`project.open`）時の engine 再 attach
  — 既存契約どおり session 再生成で detach され、再 attach は
  後続 slice（SF-03 slice 5 の deferred 項目）。
- `synvelis`（webview）bin への composition — 同様に後続。
- `synvelis-project` 等、app/player 以外の crate、`mise.toml`、
  `.github/`、Ruviform 側の変更。

## 受入条件

- [ ] `ui_pump` が control tick と同じ周期で `drain_output_reports`
  を駆動し、typed `ReportDrainReport` を `syn.control.status` の
  `drain` に既存どおり verbatim 露出する。
- [ ] Reported→Observed 伝播・active-generation gate の
  fail-closed（drain せず・帰属しない）・queue 飽和時の typed
  `Blocked(QueueFull)`（成功誤報なし）を fixture で pin する。
- [ ] production composition root（`compose_routing_engine`）を
  native bin が呼び、headless composition テストが op surface
  （`syn.control.status`/`observe`/`fast_control`）の実経路到達を
  検証する。実 plugin は fake/in-process factory で代替し、
  実機成功に繰り上げない。
- [ ] canonical 9 lane（fmt-check/check/lint/test/test-rt/
  check-app-native/lint-app-native/test-msrv/build）が exit 0。
- [ ] #101 残件は PR 本文「未対応の作業」に記載し `Refs #101`。
