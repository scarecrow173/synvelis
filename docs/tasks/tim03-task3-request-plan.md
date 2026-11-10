---
id: "TASK-034gOTXfiNaCeDRTpeP2x"
type: "task"
status: "done"
title: "TIM-03 task3 — per-request plan wiring + precision provenance（ProcessBoundarySession）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/tim03-process-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/plans/tim03-process-boundary.md"
    - "docs/tasks/tim03-task1-process-boundary.md"
    - "docs/tasks/tim03-task2-attach-wiring.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/designs/tim03-process-boundary-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim03-process-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim03-process-boundary.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim03-process-boundary.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-03 task3 — per-request plan wiring + precision provenance

## Work

- `crates/synvelis-player/src/process_boundary.rs`:
  - `PrecisionProvenance`: `StablePluginInstanceId` + `NodeId` +
    capability generation + graph generation の 4 要素 pin
    （`PrecisionProvenance::new` + accessor）。transport/plan
    generation は upstream scheduler 所有の event-scoped 軸のため
    複製しない設計判断を doc comment に明記。
  - `BoundarySessionError`: `Capability`/`Evaluate`（upstream
    `ProcessBoundaryError` 保持）・`InstanceMismatch`/
    `NodeMismatch`/`CapabilityGeneration`/`GraphGeneration`/
    `FrameBaseMismatch`・`QueueFull`/`InstallPlan`/`Dispatch`
    （upstream typed error を潰さず伝播）。
  - `ProcessBoundarySession`: `open(control, capability, mode,
    consent, budget, provenance)` が `AttachClockContext::of` で
    verified clock context を取得し `attach_delivery` を再評価
    （`Unsupported`/`OfflineUncertified` は open 前に fail-closed）。
    request API は全て provenance 照合を staging/install の前に
    実行:
    - `plan_window` — session 固定 mode/consent/budget で
      `evaluate`（純粋・staging なし）。
    - `stage_window` — evaluate 成功後に `(TransportCommand,
      due_frames)` を `control.request` へ push。window 内 due は
      `Seek`/`Other` 境界として legality 検査済み。
    - `stage_plan` — window 内 due の `install_plan` を
      `PlanInstall` 境界として検査後に install。
    - `dispatch_params`/`dispatch_midi` — provenance +
      `dispatch.node()` pin 一致 + `check_frame_base` を通してから
      `ParamDispatch` へ委譲（foreign node/stale clock mapping は
      write 解決前に typed refusal）。
- `crates/synvelis-player/src/lib.rs`: 3 型を re-export。
- `crates/synvelis-player/tests/tim03_request_plan.rs` 新設:
  実 `PluginNode`/`GraphRuntime`/`ParamDispatch` fixture で
  10 test（window/boundary 行列・同時境界・min/max legality・
  capability 3 分岐・stale provenance 全軸・foreign dispatch・
  誤配送 0・valid provenance 配送）。
- docs chain 更新（spec/design/plan/test-spec の deferred 項目を
  本 slice で充足済みに更新）+ 本 task/impl record。

## Classification

- consumer policy/provenance layer の per-request orchestration
  slice。RT 配送機構・scheduler 状態・generation 照合の最終防衛は
  Ruviform 既存実装を消費のみ（重複実装なし）。session は非 RT
  orchestration API であり RT callback 内では評価しない。

## Upstream Documents

- `docs/specs/tim03-process-boundary.md`
- `docs/designs/tim03-process-boundary-design.md`
- `docs/plans/tim03-process-boundary.md`
- `docs/test-specs/tim03-process-boundary.md`
- `docs/tasks/tim03-task1-process-boundary.md`（task1・done）
- `docs/tasks/tim03-task2-attach-wiring.md`（task2・done）

## Dependencies

- task1 の `ProcessCapability::evaluate`/`ProcessBoundaryError`
  oracle と `attach_delivery` 投影（task2）。
- Ruviform pin `0f5b573`: `PluginNodeControl`（`request`/
  `install_plan`/`plan_builder`/`check_frame_base`）、
  `AttachClockContext`（#307 verified frame base）、
  `TransportPlan`/`TransportCommand`、`InstallPlanError`/
  `FrameBaseMismatch`。
- PC-C4（#221 merged）の `ParamDispatch`/`PluginBindingSession`
  generation 管理パターン。
- `synvelis_project::port_capability_generation`（capability
  fingerprint）の provenance 軸。

## Affected Files

- `crates/synvelis-player/src/process_boundary.rs`（変更）
- `crates/synvelis-player/src/lib.rs`（変更）
- `crates/synvelis-player/tests/tim03_request_plan.rs`（新規）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/tim03-*`
  および各 `README.md` index（変更）

## Done When

- `ProcessBoundarySession` が request ごとに provenance 照合 →
  `evaluate` → stage/install/dispatch の順で fail-closed に動作
  し、stale/mismatch は typed `BoundarySessionError` で何も
  stage しない（誤 Param/MIDI 配送 0）。
- 100/128/256f・boundary=0/1/100/last・min/max・同時
  Loop/Seek/Tempo+Param・capability 3 分岐を fixture で固定。
- upstream typed error（`ProcessBoundaryError`/`InstallPlanError`/
  `FrameBaseMismatch`/`DispatchError`）を variant として保持。
- `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv` が pass。

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check` — exit 0
- [x] `mise run lint` — exit 0
- [x] `mise run test` — exit 0（`tim03_request_plan` 10/10 を
  含む全 suite 0 failed）
- [x] `mise run test-rt` — exit 0
- [x] `mise run test-msrv` — exit 0（rust 1.89）
- 補助証拠（canonical = mise）:
  `cargo test -p synvelis-player --test tim03_request_plan` →
  10 passed。
