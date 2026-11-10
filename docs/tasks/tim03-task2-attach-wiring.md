---
id: "TASK-034V1qQsacdOwJ7UiGFCAW"
type: "task"
status: "done"
title: "TIM-03 task2 — capability→AttachConfig attach 配線（composition root）"
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

# TIM-03 task2 — capability→AttachConfig attach 配線（composition root）

## Work

- `crates/synvelis-player/src/process_boundary.rs`:
  `AttachDelivery` 追加 + `ProcessCapability::attach_delivery`
  （`ExecutionMode` + optional `DegradedConsent` + `ScheduleBudget`
  → `DeliveryPolicy`/`ScheduleBudget` 投影。Realtime 無同意は
  `Exact`、同意ありは `Degraded` で
  `min(consent.max_deferral, budget.max_deferral)` clamp、
  `OfflineExact` は certified exact-class 必須で fail-closed）。
- `crates/synvelis-player/src/host.rs`:
  `BoundaryDeclaration`（precision/in-block/consent/budget）+
  `PluginHostProcessors::register_boundary` で processor 単位の
  宣言 registry。`connect` は instantiate 前に
  `ProcessCapability::from_process_config` → `attach_delivery`
  を評価し、`AttachConfig::{policy,budget}` へ投影。
  `ProcessorConnectError::{CapabilityUnsupported,
  OfflineUncertified}` 追加 + `RoutingFault` 決定的写像
  （`PluginUnavailable`/`OfflineCapabilityMismatch`）。
  宣言なし processor は従来どおり `Exact` + default budget。
- `crates/synvelis-player/src/lib.rs`: `AttachDelivery`/
  `BoundaryDeclaration` re-export。
- `crates/synvelis-player/tests/tim03_attach_wiring.rs` 新設:
  `tim03_aw_delivery_*`（投影 unit）+ `tim03_aw_connect_*`
  （fake `PluginHost` + atomic instance counter で preflight
  拒否時に plugin 未 instantiate を証明）の 10 test。
- docs chain 更新（plan/spec/design/test-spec の deferred 項目を
  本 slice で充足済みに更新）+ 本 task/impl record。

## Classification

- consumer policy layer の composition-root 配線 slice。
  RT 配送機構は Ruviform 既存実装を消費のみ。oracle 自体は
  non-RT preflight のまま（RT 経路からは呼ばない）。

## Upstream Documents

- `docs/specs/tim03-process-boundary.md`
- `docs/designs/tim03-process-boundary-design.md`
- `docs/plans/tim03-process-boundary.md`
- `docs/test-specs/tim03-process-boundary.md`
- `docs/tasks/tim03-task1-process-boundary.md`（task1・done）

## Dependencies

- task1 の `ProcessCapability::evaluate`/`ProcessBoundaryError`
  oracle（`process_boundary.rs`）。
- Ruviform pin `0f5b573`: `AttachConfig`/`DeliveryPolicy`/
  `ScheduleBudget`/`BlockContract`/`InBlockTransportSupport`。
- PC-C4（#221 merged）の consumer 基盤。

## Affected Files

- `crates/synvelis-player/src/process_boundary.rs`（変更）
- `crates/synvelis-player/src/host.rs`（変更）
- `crates/synvelis-player/src/lib.rs`（変更）
- `crates/synvelis-player/tests/tim03_attach_wiring.rs`（新規）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/tim03-*`
  および各 `README.md` index（変更）

## Done When

- `BoundaryDeclaration` → `AttachConfig::{policy,budget}` 投影が
  `PluginHostProcessors::connect` で instantiate 前に評価され、
  typed `CapabilityUnsupported`/`OfflineUncertified` で fail
  する（plugin lifecycle/teardown 不要）。
- Realtime Exact default・consent 付き Degraded clamp・
  OfflineExact certified gate・宣言 scoping・undeclared
  processor 後方互換を headless test で固定。
- `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`check-headless-deps` が pass。

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check` — exit 0
- [x] `mise run lint` — exit 0
- [x] `mise run test` — exit 0（`tim03_attach_wiring` 10/10、
  `tim03_process_boundary` 17/17、`tim03_process_boundary_rt`
  1/1 を含む全 suite 0 failed）
- [x] `mise run test-rt` — exit 0
- [x] `mise run test-msrv` — exit 0（rust 1.89）
- [x] `mise run check-headless-deps` — exit 0
- 補助証拠（canonical = mise）:
  `cargo test -p synvelis-player --test tim03_attach_wiring` →
  10 passed。
