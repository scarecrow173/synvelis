---
id: "TSPEC-AUTX07MountWiring"
type: "test-spec"
status: "approved"
title: "AUTX-07 composition-mount wiring headless test spec（synvelis#266 SV266-NR-05）"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/266"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/automation_ui.rs"
      - type: "file"
        path: "crates/synvelis-app/src/automation.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/binding.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis-native.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/sv266_wiring.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx07_catalog_seam.rs"
      - type: "file"
        path: "docs/impl/ir/autx-07-mount-wiring.md"
      - type: "file"
        path: "docs/impl/exp/autx-07-mount-wiring.md"
      - type: "file"
        path: "docs/test-specs/autx-07-mount-wiring.md"
      - type: "file"
        path: "evidence/sv266-wiring/README.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/test-specs/autx-07-gui-views.md"
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-atomic-macro.md"
    - "docs/automation/design-workflow-run.md"
    - "docs/automation/design-trust-boundary.md"
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-07-gui-views.md"
    - "docs/impl/ir/autx-07-mount-wiring.md"
    - "docs/impl/exp/autx-07-mount-wiring.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/test-specs/autx-07-gui-views.md"
  derived-by: []
  verifies:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-trust-boundary.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/267"
    - "https://github.com/scarecrow173/synvelis/issues/264"
  defers: []
  deferred-by: []
---

# AUTX-07 mount wiring tests

## Purpose

`crates/synvelis-app/tests/sv266_wiring.rs` が固定する
SV266-NR-05（composition-root mount + `autx.*` action dispatch）の
headless 契約を記録する。`docs/test-specs/autx-07-gui-views.md` が
固定した projection/decode 契約を composition root へ接続した後も、
mount・dispatch・fail-closed 劣化の境界を失わないための仕様。

## Feature

AUTX-07 automation surfaces — composition mount・`autx.*` decode/dispatch・
catalog seam・fail-closed 劣化

## Rules

- **Rule AUTX-MOUNT-REGISTER** 4 surface（`ViewType::{ActionPalette,
  MacroBrowser, WorkflowEditor, RunMonitor}`）は `syn.autx.visual
  {action:"open"}` で detached view として adopt され、wire `view-{n}`
  を返す。`NativeShell::attach_view` が attach に成功し、
  `automation_screen_cell` が publish した screen state から各 surface
  の `ViewSpec` が再構成できる。
- **Rule AUTX-DECODE-PARAMS** `autx.*` target 宛の `params` 付き
  `action` intent は `decode_view_command` を通過し
  `decode_automation_command` の `(target, action)` 分類表へ届く。
  "params not representable"・"unknown target" で落ちない。
- **Rule AUTX-DISPATCH-OWN** `NativeShell` は `autx.*` target を
  `syn.autx.action` へ dispatch し、`syn.control.dispatch` /
  `parse_target` へ到達させない。command の `Result` は到達 channel
  （`driver.post_event`）へ返る。
- **Rule AUTX-LOCAL-ONLY** view-local action（select/category/capture/
  workflow-select）は `ViewState.automation` のみを変更し、Project
  正本・他 view の state に第二コピーを作らない。
- **Rule AUTX-NOCATALOG-REJECT** catalog 未 push で request 系 action
  （capture/preview/submit/apply/cancel）は fake data・no-op success
  ではなく、種別の判別できる typed rejection を返す。view-local
  action は catalog 無しでも `ViewState.automation` を mutate する。
- **Rule AUTX-CATALOG-LOWER** catalog push 済みで request 系 action は
  `automation_requests` を通じて `WireRequest` へ lowering され、
  結果が op `Result` の `requests` として観測できる。macro apply は
  `<key>/<index>` idempotency suffix・step 毎の availability/target
  再検査を維持する。
- **Rule AUTX-STRICT-REJECT** 未知 `autx.*` target/action・malformed
  params・malformed explicit target は decode/dispatch のどちらの層
  でも reject され、"no target" へのすり替えや silent drop をしない。

## Examples

- **Example AUTX-MOUNT-REGISTER-01** `syn.autx.visual{action:"open",
  surface:"action-palette"}` → `{"view_id":"view-3","opened":true}`、
  `attach_view` が `generation >= 1` を返し、automation cell に
  `ActionPalette` variant の screen state が現れる。
- **Example AUTX-DECODE-PARAMS-01** palette table select が mint する
  `{"kind":"action","target":"autx.palette","action":"select",
  "params":{"row":0}}` は `rvf.ui.command.v1` envelope 経由で shell
  へ届き、`ok` の `Result` で settle する。
- **Example AUTX-NOCATALOG-REJECT-01** catalog 未 push で
  `autx.action/submit` → `ok:false`、error が catalog 未供給を
  区別できる message/code を持つ。
- **Example AUTX-CATALOG-LOWER-01** catalog push 後の
  `autx.action/submit{capability:"synvelis.transport.set-position"}`
  → `requests[0].capability.id == "synvelis.transport.set-position"`、
  `consistency.kind == "expected-revision"`。

## NotRun / Deferred

- 実 GPUI/実 OS の mount・Pointer/Keyboard/IME/Focus/DPI/2Window/
  WindowClose 受入 — Case ID は `evidence/sv266/README.md` §3
  （SV266-NR-01）に記録、#114/#267 別 Gate。本 slice の headless
  mount/dispatch 成功を実機 Pass に昇格しない。
- `synvelis-automation` live adapter からの catalog 実供給
  （SV266-NR-04）— #264/sv#265。`syn.autx.catalog` は JSON 境界で
  注入可能だが、本 slice では test fixture のみが push する。
- Workflow plan / Run monitor projection の push 経路 —
  `WorkflowPlanView`/`RunMonitorView` の producer は adapter slice
  の責務。両 view は empty-state render と view-local select
  （workflow）/ typed reject（run select・cancel）で正直に劣化する。
- `status-*` token の assert — 並列 sv#266 theme-parity Worker の
  リース（SV266-NR-03）。
- gpui-test GPUI Semantic scenario（SV266-NR-06）— #267/別 lane。
- `*-msrv`・`*-full` 系検証 — Daily Full CI 委譲。
