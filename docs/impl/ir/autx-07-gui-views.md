---
id: "IMPL-AUTX07GuiViews"
type: "impl"
status: "completed"
title: "AUTX-07 GPUI Action Palette・Macro/Workflow 製品 View・承認/実行状態 UI（headless-first slice、synvelis#266）"
created: "2026-11-16"
updated: "2026-11-16"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/266"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/automation_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx07_automation_ui.rs"
      - type: "file"
        path: "crates/synvelis-app/src/automation.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/autx07_automation.rs"
      - type: "file"
        path: "crates/synvelis-theme/src/status.rs"
      - type: "file"
        path: "docs/test-specs/autx-07-gui-views.md"
      - type: "file"
        path: "docs/impl/exp/autx-07-gui-views.md"
      - type: "file"
        path: "evidence/sv266/README.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/state.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/view.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-theme/src/lib.rs"
      - type: "file"
        path: "docs/impl/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/test-specs/autx-07-gui-views.md"
  implemented-by: []
  depends-on:
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-atomic-macro.md"
    - "docs/automation/design-workflow-run.md"
    - "docs/automation/design-trust-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/automation/spec.md"
    - "docs/automation/design-owner-dag.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-atomic-macro.md"
    - "docs/automation/design-workflow-run.md"
  derived-by: []
  verifies:
    - "docs/test-specs/autx-07-gui-views.md"
  verified-by: []
  references: []
  defers:
    - "実 GPUI/実 OS 入力・Pixel・Native Plugin Dock・IME/DPI/2Window/WindowClose/Undo/Save/Reload の実機受入（synvelis#114/#267 別 Gate）"
    - "Script Editor/Diagnostics/Completion との同一 Capability/Schema/Availability/Target source 照合（synvelis#274 Owner）"
    - "assets/themes/*.dtcg.json への status-* token 値の追加（assets は本スライスの編集範囲外 — vocabulary は synvelis-theme::status が正本）"
    - "synvelis-automation crate 統合後の live adapter 接続（#264 別 Worker リース）"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUTX-07 GPUI automation views（synvelis#266 headless slice）

## 概要

Issue #266 の headless-first スライス。Action Palette・Macro Browser・
Workflow Editor v1・Run Monitor の 4 surface を、driver-neutral な
`ruviform_ui::view::ViewSpec` + state projection + intent decode として
実装した。GUI は Project/Grant/Job/認可の第二正本を持たず、唯一の変換で
ある `ViewAction → WireRequest`（[`automation_requests`]）だけを所有する。

## 構造

- `crates/synvelis-workspace/src/automation_ui.rs` — 権威→表示の純粋
  projection 層。`project_action_catalog`/`project_macro_preset`/
  `project_workflow_plan`/`project_run_monitor`/`project_job_status`、
  明示 target の `capture_target` 検証、`automation_requests` /
  `macro_step_requests` による `WireRequest`  lowering、view-local
  `AutomationViewState`/`AutomationViewAction`。
- `crates/synvelis-workspace/src/view.rs` — `ViewType` に
  `ActionPalette`/`MacroBrowser`/`WorkflowEditor`/`RunMonitor` を追加
  （registry/factory は kind 非依存のため追加コード不要）。
- `crates/synvelis-workspace/src/state.rs` — `ViewState.automation:
  Option<AutomationViewState>`。tolerant deserialize で malformed /
  newer payload は `None` に degrade し snapshot 全体を拒否しない。
- `crates/synvelis-app/src/automation.rs` — 4 surface の `ViewSpec`
  builder（`*_screen_spec`）、screen state、`ViewHost` mount helper、
  `decode_automation_command`（唯一の intent→action decode 点）。
  `synvelis-gateway` 非依存 — gateway 語彙は表示文字列として到達する。
- `crates/synvelis-theme/src/status.rs` — `StatusTone` + DTCG token
  path（`status-*`）の正本 vocabulary と label→tone 写像。

## 不変条件（fail-closed）

- `executable` は `Availability::Available` ∧ `GrantState::Satisfied`
  のときのみ true。Disabled/Planned/Retired・Missing/Unknown grant は
  常に blocked 表示で、Preview/Submit/Apply button を spec が生成しない。
- `capture_target` は `TargetContract` の shape/kind を厳密照合し、
  mismatch は error — GUI は target を推測しない（GTC-002）。
- `project_job_status` は Pending→Pending、provider 未確認の
  cancel/失敗を Indeterminate に写像し、requested≠confirmed を維持する。
  Pending phase も cancel axis を適用する（Requested→CancelRequested、
  Unconfirmed/Confirmed on pending→Indeterminate）。
- `automation_requests` の view-local action は空 Vec、domain action の
  み `WireRequest` を mint する。`synvelis.job.cancel` は catalog 内で
  一意に解決できなければ `UnknownCapability`/`InvalidRequest`、
  target contract・availability の共有 gate を通り、`args.operation`
  は tagged `operation-<32hex>` 形式を境界で検証する — 推測しない。
- `capture_target` は `TargetContract` の shape/kind に加え、空の
  resource id / resource-set / create parent を `InvalidRequest` で
  拒否する（wire で lower できない target を凍結しない）。
- macro apply は preset の `availability`/`grant` から executable を
  再導出し（public field の手組み立てを信用しない）、step 毎の
  availability/target contract を再検査し、idempotency key を
  `<key>/<index>` で suffix する。

## Deferred / 範囲外

- 実 OS GPUI lane（#114/#267）、#274 Script Editor 統合、
  `synvelis-automation` live adapter 接続、`assets/themes` への token 値
  追加は別 Gate/別スコープ（`evidence/sv266/README.md` の対応表参照）。
