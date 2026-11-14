---
id: "SPEC-uivmQvR2wTbLpNcJyH4sDf"
type: "spec"
status: "approved"
title: "SF-05 ui/ slice — Signal Flow・Probe headless view-model consumer path（synvelis#108）"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
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
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/specs/sf-05-slice3-probe-subscriber-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-ui-viewmodel.md"
  references: []
  deferred-by: []
---

# SF-05 ui/ slice: Signal Flow・Probe headless view-model consumer path

## What Should Be Built

Issue #108（Signal Flow Editor）の **ui/ consumer slice**。service 側の
`syn.signal_flow.*`（view/visual/inspection/routing）と
`syn.probe.*`（control/state/compare）は全て main 統合済み
（`crates/synvelis-app/src/ui_ops.rs`）。欠落しているのは ui/ 側の
consumer のみであり、本 slice はその **typed IPC surface + DOM-free
view-model + headless vitest** を実装する。

- **`ui/src/ipc.ts`**: `OpName` union に `syn.signal_flow.view`・
  `syn.signal_flow.visual`・`syn.signal_flow.inspection`・
  `syn.signal_flow.routing` を追加する（`syn.probe.*` は既存のまま）。
- **`ui/src/viewmodel/signal-flow-view-model.ts`（新規）**:
  `ConsoleViewModel` の先例に従う DOM-free owner。`call` 経由で
  `syn.signal_flow.view` の projection を verbatim 保持し、
  coverage/status の Debug 語彙を再命名・縮退しない。
  `published_generation` と `observed_active_generation` を区別し
  （Publish≠Active）、probe 未取得をゼロ観測と混同しない。
  visual/inspection/routing の各 action を typed method として提供し、
  visual ack の `view_generation` を staleness token として保持する。
  routing outcome `applied|cancelled|stale` を typed に保持し、
  stale で retarget しない。
- **`ui/src/viewmodel/probe-view-model.ts`（新規）**: `syn.probe.control`
  の session-scoped action（request/set_mode/capture/stop_capture/
  remove — `view_id` を送らない）と view-scoped lease action
  （subscribe/unsubscribe/capture_for/abort_capture — `view_id`
  `"view-{n}"` を VM API で必須化）を分離する。`syn.probe.state` の
  projection（`engine_attached`、bindings、subscribers、segments、
  drain、traces）と `syn.probe.compare` の `AcousticObservation`
  verbatim（`engine_attached:false` → `observation:null`、
  `conditional`/`incomparable` を `aligned` へ昇格しない）を提供する。
- **`ui/test/` vitest spec（新規）**: `installFakeHost` harness で
  intent の op+params 完全一致、wire→VM 写像、エラー経路
  （wire error reject、malformed `view_id` の surface）、
  wire に無い状態を捏造しないことを検証する。

## Why

SF-05 の service 側 op は完成しているが、ui/ に consumer が無いと
Signal Flow Editor の headless 表示・操作経路が成立しない。view-model
層を先に typed に固定することで、後続の GUI mount slice が wire
contract を再解釈するリスクを排除し、headless で契約を証明できる。

## Acceptance（本 slice 分）

- [ ] `OpName` union に 4 つの `syn.signal_flow.*` op が追加され、
  `syn.probe.*` は変更されない。
- [ ] `SignalFlowViewModel` が `syn.signal_flow.view` response を
  verbatim 保持し、coverage/status/Debug 語彙・publish/active
  generation 分離・probe 未取得表現を維持する。
- [ ] visual/inspection/routing の全 action が正確な params で
  intent を送り、ack の `view_generation`・inspection focus・
  routing outcome を typed に返す。stale outcome を retarget しない。
- [ ] `ProbeViewModel` が session-scoped/view-scoped action を
  `view_id` 規約どおり分離し、`syn.probe.state`/`compare` の
  projection を verbatim 保持する（engine 未接続・incomparable を
  正直に表現）。
- [ ] vitest spec が fake host 経由で intent 完全一致・wire→VM
  写像・エラー経路・非捏造を検証し、既存テストは全て green。
- [ ] 実 GUI mount・実 WebView/GPUI window・2-window・SF-UX-08/
  Windows L2・#109/#103 oracle join は **NotRun 継続**で Pass
  扱いしない。

## Out of Scope

- view-model の mount/画面接続（`ui/src/store.ts`・`app.tsx`・
  `views/`・`components/` — 製品決定で後続 slice）。
- Rust 変更全般（service 側は完成済み）。
- 実 OS/GPUI・実 2-window・HiDPI/a11y の GUI 受入（#114 gate）。
- #109 independent oracle・#103 との Case ID 結合、
  `evidence/reports/sf-06-cross-gate-ledger.md`。
- Workspace schema v2→view state 移行・visual layout の semantic
  diff 除外の運用受入（SF-UX-07 の残部）。
