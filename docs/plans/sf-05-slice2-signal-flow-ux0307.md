---
id: "PLAN-6xRqunIew52aqbQt4yZ3qy"
type: "plan"
status: "approved"
title: "SF-05 slice2 実施計画 — Signal Flow gesture/inspection/persist"
created: "2026-11-10"
updated: "2026-11-10"
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
  implements:
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-05-slice2-routing-gesture-inspection-persist-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-slice2-signal-flow-ux0307.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice2 実施計画 — Signal Flow gesture/inspection/persist

## Goal

SF-UX-03（Port Drag routing gesture）・SF-UX-04（Signal Flow
inspection focus と link 追従）・SF-UX-07（`SignalFlowViewState`
の `ViewInstanceId` 単位 persist + schema v2→v3）を
`synvelis-workspace` + `synvelis-app` に実装し、headless
fixture で受入を検証する。

## Scope

単一実装 stream。workspace gesture/inspection/persist + app
op + test の一括 slice。`synvelis-project` 以下の domain
crate・Ruviform・probe request/capture・実 GUI は含まない。

## Upstream Documents

- `docs/specs/sf-05-slice2-signal-flow-ux0307.md`
- `docs/adr/sf-05-slice2-routing-gesture-inspection-persist-boundary.md`
- `docs/designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md`

## Design Inputs

- `docs/designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md`

## File Map

- `crates/synvelis-workspace/src/routing_gesture.rs` - 新規
  gesture model/preflight
- `crates/synvelis-workspace/src/signal_flow_ui.rs` - serde・
  `SignalFlowViewState`/`SignalFlowInspectionFocus`/変換 helper
- `crates/synvelis-workspace/src/state.rs` - `ViewState` field
  + tolerant deserializer
- `crates/synvelis-workspace/src/workspace.rs` - gesture +
  inspection API
- `crates/synvelis-workspace/src/link.rs` - link→Signal Flow
  追従
- `crates/synvelis-workspace/src/persist.rs` - schema v3
- `crates/synvelis-workspace/src/lib.rs` - export
- `crates/synvelis-workspace/tests/sf_05_s2.rs` - 新規
- `crates/synvelis-app/src/ui_ops.rs` - inspection/routing ops・
  per-view state 移管
- `crates/synvelis-app/src/app.rs` - view options 読み替え
- `crates/synvelis-app/src/service.rs` - op 登録
- `crates/synvelis-app/tests/sf05_signal_flow.rs` - 追加 test
- `docs/` チェーン一式

## Implementation Sequence

- [x] 1. `signal_flow_ui.rs` に serde + `SignalFlowViewState`/
  `SignalFlowInspectionFocus`/`signal_flow_entity_from_semantic`
- [x] 2. `state.rs`/`persist.rs` に `signal_flow` field・
  tolerant deserialize・schema v3
- [x] 3. `routing_gesture.rs` + `workspace.rs` gesture/inspection
  API + `link.rs` 追従
- [x] 4. `lib.rs` export → workspace `cargo check`
- [x] 5. app `ui_ops.rs`/`app.rs`/`service.rs` ops +
  `UiOpsState.signal_flow` 廃止 → app `cargo check`
- [x] 6. `tests/sf_05_s2.rs` + `tests/sf05_signal_flow.rs`
  追加・pass 確認
- [ ] 7. `mise run fmt-check`/`check`/`lint`/`test`（+ app-native
  存在範囲）
- [ ] 8. `git fetch origin main && git merge origin/main` →
  docs index/lockfile 再生成 → 最終検証
- [ ] 9. commit/push・PR（`Refs #108`）

## Dependencies and Risks

- `ProjectOp::Routing`/`ProjectSession::apply`/`routing_tx`
  preflight は既存 public API を consume のみ —
  `synvelis-project` は編集しない。
- `ViewState` additive field + tolerant deserializer で v2/v3
  payload を両立 — `deny_unknown_fields` は維持。
- gesture commit の stale 3 条件（revision/capability/
  window）は fixation 時点の値との比較のみ — project の
  現在値で retarget しない。
- `SemanticTarget` 変換は Track のみ — Lane/Clip/Source は
  Signal Flow focus を更新しない（暗黙写像なし）。

## Task Breakdown

- `docs/tasks/sf-05-slice2-signal-flow-ux0307.md` 参照。

## Verification

- [ ] `mise run fmt-check` exit 0
- [ ] `mise run check` exit 0
- [ ] `mise run lint` exit 0
- [ ] `mise run test` exit 0（`sf_05_s2.rs`/
  `sf05_signal_flow.rs` 含む）
- [ ] `mise run test-rt`/`check-app-native`/`lint-app-native`/
  `build-app-native`/`test-msrv` — 存在範囲で実行、不能は
  NotRun 記録
- [ ] 実 OS/GPUI は NotRun（#114 gate）

## Open Questions / Gaps

- `SemanticTarget` → `SignalFlowEntityRef` の変換は Track のみ
  — Bus/Strip/Processor/Port/Connection/Send を指す link
  event の Signal Flow 追従は後続 slice の判断。
- gesture は snapshot 復元で復活しない（session-only）—
  crash/再起動を跨ぐ drag は再開しない意図的設計。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
  consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一 worker dispatch）。
- Independent work streams: none。
- User approval required before dispatch: no（worker contract
  により review-protocol で代替）。

## Review Handoff

- reviewer focus: fixation の 3 stale 条件・drop 後 retarget
  不可・`NeedsMapping` consent・link 追従が Follow のみ・
  schema v3 additive migration・malformed degrade・
  `UiOpsState.signal_flow` 廃止後の view-1/view-2 整合。
