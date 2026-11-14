---
id: "TSPEC-AUTX07GuiViewsHeadless"
type: "test-spec"
status: "approved"
title: "AUTX-07 GUI views headless test spec（synvelis#266）"
created: "2026-11-16"
updated: "2026-11-16"
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
        path: "crates/synvelis-theme/src/status.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx07_automation_ui.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/autx07_automation.rs"
      - type: "file"
        path: "docs/impl/ir/autx-07-gui-views.md"
      - type: "file"
        path: "docs/impl/exp/autx-07-gui-views.md"
      - type: "file"
        path: "docs/test-specs/autx-07-gui-views.md"
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
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
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
    - "docs/impl/ir/autx-07-gui-views.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
  derived-by: []
  verifies:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/267"
    - "https://github.com/scarecrow173/synvelis/issues/274"
  defers: []
  deferred-by: []
---

# AUTX-07 GUI views tests

## Purpose

`crates/synvelis-workspace/tests/autx07_automation_ui.rs` と
`crates/synvelis-app/tests/autx07_automation.rs` が固定する
AUTX-07 headless 契約を記録する。実機 GPUI/OS lane（#114/#267）での
再検証が来ても、headless で担保した行と残る `NotRun` 行の境界を
失わないための仕様。

## Feature

AUTX-07 automation surfaces — fail-closed projection・explicit target・
request lowering・run status honesty

## Rules

- **Rule AUTX-PALETTE-FAILCLOSED** catalog projection は
  `Available` ∧ `Satisfied` のみ `executable=true`。Disabled/Planned/
  Retired・grant Missing/Unknown は blocked 表示のまま catalog に残り、
  実行可能に見せない（`fail_closed_matrix_never_renders_blocked_rows_executable`、
  `blocked_rows_stay_visible_in_catalog`）。
- **Rule AUTX-TARGET-EXPLICIT** `capture_target` は `TargetContract`
  の shape/kind を厳密照合し、missing/wrong-kind/wrong-shape/None 契約
  への explicit target を `TargetMismatch` で拒否する。GUI は target
  を推測しない（GTC-002）。CapturedTarget は capability id+version と
  project を固定保持する。
- **Rule AUTX-REQ-VIEWLOCAL** view-local action（select/category/
  capture/macro-preview/macro-select/workflow-select/run-select）は
  `automation_requests` で空 Vec — GUI 側で request を捏造しない。
  domain action（preview/submit/macro-apply/job-cancel）のみ
  `WireRequest` を mint する。
- **Rule AUTX-REQ-FAILCLOSED** unknown capability・target 不足・
  non-Available capability・catalog 未登録の `synvelis.job.cancel`
  は全て error — request を生成しない。
- **Rule AUTX-MACRO-ATOMIC** macro apply は preset 順の step request
  を同一 project・caller-supplied consistency で生成し、idempotency
  key を `<key>/<index>` で suffix する。step 毎に availability と
  target contract を再検査する。
- **Rule AUTX-MONITOR-HONEST** `project_job_status` は Pending を
  Pending のまま、provider 未確認の cancel/失敗を Indeterminate に、
  cancel requested/confirmed/unconfirmed を別 status に写像する。
  Job Pending を Completed と表示しない。
- **Rule AUTX-STATE-TOLERANT** `ViewState.automation` は malformed /
  newer payload を `None` に degrade し snapshot 全体を拒否しない。
  view-local state のみを保持し project/grant/job truth を持たない。
- **Rule AUTX-DECODE-STRICT** `decode_automation_command` は未知
  target/action・malformed params・malformed explicit target を `None`
  で拒否し、bad target を "no target" にすり替えない。

## Examples

- **Example AUTX-PALETTE-FAILCLOSED-01** `Availability::Disabled` +
  `GrantState::Satisfied` の行は `executable=false`・
  `blocked` に理由文字列を持ち、catalog から消えない。
- **Example AUTX-TARGET-EXPLICIT-01** `Resource{Source}` 契約に
  `Resource{kind:"track"}` を渡すと `TargetMismatch`。
- **Example AUTX-MONITOR-HONEST-01** `(Running, CancelLink::Requested)`
  → `CancelRequested`、`(Cancelled, CancelLink::NotRequested)` →
  `CancelUnconfirmed`（cancelled と表示しない）、
  `(Completed, CancelLink::Unconfirmed)` → `Indeterminate`。
- **Example AUTX-STATE-TOLERANT-01** `ViewState` JSON の `automation`
  に `"not-an-object"` や未知 field 混入 object を入れても parse が
  成功し `automation == None` になる。

## NotRun / Deferred

- 実 GPUI Windows/macOS/Linux の Pointer/Keyboard/IME/Focus/DPI/
  2Window/WindowClose・Alert・Undo/Save/Reload 受入 — Case ID は
  `evidence/sv266/README.md` に記録、#114/#267 別 Gate。
- Script Editor (#274) との同一 Capability/Schema/Availability/
  Target source 照合 — 契約レベルでは同一の Gateway capability
  descriptor を参照するが、Editor 実装は #274 Owner。
- `assets/themes/*.dtcg.json` への `status-*` token 値追加 —
  編集範囲外。vocabulary（`synvelis_theme::status::StatusTone`）は
  本スライスで確定済み。
- `synvelis-automation` crate（#264）の live adapter 接続 — main 未
  統合のため fixture/mock catalog で駆動。
