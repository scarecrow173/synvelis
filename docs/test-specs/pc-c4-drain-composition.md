---
id: "TSPEC-a1d4f7c2e9b30586d1a4f7c2f0a3"
type: "test-spec"
status: "approved"
title: "PC-C4 drain composition test spec"
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
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/designs/pc-c4-drain-composition-design.md"
    - "docs/plans/pc-c4-drain-composition-plan.md"
    - "docs/tasks/pc-c4-drain-composition-task.md"
  implemented-by: []
  depends-on:
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
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/designs/pc-c4-drain-composition-design.md"
    - "docs/plans/pc-c4-drain-composition-plan.md"
  verified-by: []
  references:
    - "docs/test-specs/pc-c4-04-app-composition.md"
    - "docs/test-specs/pc-c4-06-control-op-surface.md"
  defers: []
  deferred-by: []
---

# PC-C4 drain composition test spec

Fixture: `crates/synvelis-app/tests/pc_c4_drain_composition.rs`。
pc_c4_04/06 と同じ fake plugin（announce + echo `ParamValue`）
と project 構築を使い、engine は **production composition root**
`compose_routing_engine` 経由で組み立てる（手組みの
`PluginHostProcessors` を fixture が直接作らないことが
composition root 検証を兼ねる）。

## Feature: drain 周期配線

### Rule: ui_pump が drain を周期駆動する

- **Given** composition 済み engine + RT-active surface
- **When** `app.ui_pump()` を 1 回呼ぶ
- **Then** `syn.control.status` の `drain` が直近 pass の typed
  `ReportDrainReport` を示し、plugin が announce した `ParamValue`
  が `param_reports`/`events` に計上される。generation は surface
  の generation、`generation_gated=false`、
  `inactive_gated=false`（RT 観測済み）。

### Rule: Reported→Observed 伝播

- **Given** fast-control write が `Delivered` され RT が echo を
  生成した状態
- **When** `app.drain_control_reports()`（決定的 1 pass）→
  `syn.control.observe`（正しい plan_generation）→
  `feed_control_cycle`
- **Then** ledger entry が `Reported` に進み、observe cycle の
  observation が plugin-reported 値を持ち、`fed >= 1`。
  host が値を捏造しない（reported 値は plugin の echo/announce）。

### Rule: inactive gate fail-closed

- **Given** surface attach 済みだが RT がまだ active generation を
  観測していない（`activate` 前）
- **When** `app.ui_pump()` / `drain_control_reports()` を呼ぶ
- **Then** `drain.inactive_gated == true`、`drained == 0`、
  host drain は呼ばれず report は plugin 側 queue に残る
  （成功・drained の誤報なし）。

### Rule: queue 飽和の typed 応答

- **Given** inactive surface（queue 経路）で fast-control queue
  capacity を超える書込みを連続投入
- **When** `syn.control.fast_control` op / `control_fast_control`
  を超過分まで呼ぶ
- **Then** 受理分は `Queued`、超過分は `Blocked` で
  `FastControlBlockKind::QueueFull` — `Delivered`/`Queued` の
  成功誤報は無い。

### Rule: project recreate が drain を fail-closed でクリア

- **Given** composition 済み engine + RT-active surface で drain
  evidence（`attached: true`）が記録済みの app
- **When** `syn.project.new`（session 再生成）を実行し、その後
  `drain_control_reports()` / `syn.control.status` を呼ぶ
- **Then** `last_control_drain` は honest default
  （`attached: false`）に戻り、明示 drain pass は no-op
  （engine/drain handle 共に `None`）、status は
  `engine: false` と `drain.attached: false` を一致して報告する
  — 死んだ session の surface を説明する stale evidence が
  `engine: false` の隣に残らない。

## Feature: production composition root

### Rule: op surface の実経路到達

- **Given** `compose_routing_engine` で組み立てた app
  （`AppService`/`UiRuntime` 経由）
- **When** `syn.control.status`/`syn.control.observe`/
  `syn.control.fast_control` op を dispatch する
- **Then** 全 op が typed response を返し、status が engine/dispatch
  の実 evidence（generation・surface・drain report）を含む。
  fake factory のみ使用し実機成功を主張しない。
