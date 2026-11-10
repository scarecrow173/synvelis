---
id: "IMPL-034V1rsGSFTsDZgYwBlfaz"
type: "impl"
status: "completed"
title: "TIM-03 attach wiring — capability→AttachConfig composition root"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/tim03_attach_wiring.rs"
      - type: "file"
        path: "docs/tasks/tim03-task2-attach-wiring.md"
      - type: "file"
        path: "docs/impl/ir/tim-03-attach-wiring-capability-attachconfig-composition-root.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/process_boundary.rs"
      - type: "file"
        path: "crates/synvelis-player/src/host.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "docs/specs/tim03-process-boundary.md"
      - type: "file"
        path: "docs/designs/tim03-process-boundary-design.md"
      - type: "file"
        path: "docs/plans/tim03-process-boundary.md"
      - type: "file"
        path: "docs/test-specs/tim03-process-boundary.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/tim03-task2-attach-wiring.md"
    - "docs/plans/tim03-process-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim03-task2-attach-wiring.md"
    - "docs/impl/ir/tim-03-process-boundary-implementation-record.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim03-process-boundary.md"
    - "docs/impl/ir/tim-03-process-boundary-implementation-record.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/tim03-task2-attach-wiring.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-03 attach wiring — capability→AttachConfig composition root

## Summary

Issue #127 の follow-up slice: task1 で未配線だった
`ProcessCapability` oracle を `PluginHostProcessors::connect`
（composition root）へ配線し、capability/precision/consent/budget
から Ruviform `AttachConfig::{policy,budget}` を決定的に投影する。
unsupported/uncertified は instantiate 前に typed connect error
で fail-closed。headless fake `PluginHost` で 10 test を追加。

## Context

`docs/tasks/tim03-task2-attach-wiring.md` /
`docs/plans/tim03-process-boundary.md` /
`docs/specs/tim03-process-boundary.md` /
`docs/designs/tim03-process-boundary-design.md` に従う。
task1 IR（`tim-03-process-boundary-implementation-record.md`）の
Follow-ups 第 1 項「plan → `DeliveryPolicy`/`ScheduleBudget`/
consented degraded wiring」を本 slice で充足。

## Implementation

### Changes

- `crates/synvelis-player/src/process_boundary.rs`:
  - `AttachDelivery`（`policy: DeliveryPolicy` +
    `budget: ScheduleBudget` の投影結果）追加。
  - `ProcessCapability::attach_delivery(mode, consent, budget)`:
    `ExecutionMode`/`DegradedConsent`/`ScheduleBudget` を
    `DeliveryPolicy`/`ScheduleBudget` へ写像する non-RT
    preflight。Realtime は無同意 `Exact`、同意ありは
    `Degraded` で `min(consent.max_deferral,
    budget.max_deferral)` clamp。`OfflineExact` は常に `Exact`
    要求で certified exact-class（`ExactWithinVerifiedCapability`/
    `FrameAccurateEvents`）以外を `OfflineUncertified` で拒否
    （consent は offline を救済しない）。`Unsupported` capability
    と degenerate contract（`from_process_config` 由来の
    `max_frames==0` 等）は `Unsupported` で事前拒否。
- `crates/synvelis-player/src/host.rs`:
  - `BoundaryDeclaration`（`precision`/`in_block`/`consent`/
    `budget`）+ `PluginHostProcessors.boundaries:
    BTreeMap<ProcessorId, BoundaryDeclaration>` registry +
    `register_boundary`。宣言は次の connect のみ・processor 単位
    で scoping される。
  - `connect` は `Instantiate` 前に
    `ProcessCapability::from_process_config`（activated
    `ProcessConfig` から min/max contract 導出）→
    `attach_delivery` を評価。失敗は plugin を instantiate
    しないため lifecycle/teardown 不要。
  - `connect_attached` は投影結果を `AttachConfig` へ渡し、
    宣言なし processor は従来どおり `Exact` +
    `ScheduleBudget::default()`（後方互換）。
  - `ProcessorConnectError::{CapabilityUnsupported,
    OfflineUncertified}` 追加。`fault()` 写像:
    `CapabilityUnsupported` → `PluginUnavailable`、
    `OfflineUncertified` → `OfflineCapabilityMismatch`、将来の
    未対応 variant は `Invalid` に fail-closed。
- `crates/synvelis-player/src/lib.rs`: `AttachDelivery`/
  `BoundaryDeclaration` re-export。
- `crates/synvelis-player/tests/tim03_attach_wiring.rs`（新規・
  10 test）: `tim03_aw_delivery_*` 5（Exact default・consented
  Degraded clamp・offline certified gate・consent 無視・
  unsupported/degenerate）+ `tim03_aw_connect_*` 5（typed
  connect error・offline uncertified・processor scoping・
  undeclared legacy・consented realtime connect）。fake
  `PluginHost` + atomic instance counter で preflight 拒否時の
  未 instantiate を証明。
- docs chain: spec/design/plan/test-spec の deferred 記述を
  本 slice 充足に更新、task2 + 本 record 追加、index 再生成。

### Decisions During Implementation

- **attach 時点の policy/budget のみ投影**: boundary request は
  runtime event であり `evaluate`（offset 列を伴う plan oracle）
  は引き続き non-RT preflight 専用。attach では「その processor
  が取り得る delivery 精度の上限」を `AttachConfig` に固定し、
  per-request の legality/deferral 判定は upstream
  `TransportScheduler`/`PluginNode` が担う（責務分離維持）。
- **consent は attach 宣言の一部**: `BoundaryDeclaration` が
  `DegradedConsent` を保持し、Realtime connect 時にのみ効く。
  Offline build（`BuildMode::Offline` → `OfflineExact`）は consent
  を無視して certified exact-class を要求 — consent による
  offline 品質低下を構造的に排除。
- **失敗は instantiate 前**: `attach_delivery` 評価を
  `Instantiate` 呼出しより前に置き、unsupported/uncertified
  plugin は一切 instantiate されない（test の atomic counter で
  固定）。これにより plugin 側 lifecycle/teardown の失敗経路を
  新設せず既存 `ProcessorConnectError` のみで表現できる。
- **`CapabilityUnsupported` → `PluginUnavailable` 写像**:
  `RoutingFault` に capability 専用 variant を追加せず、既存
  fault 語彙へ決定的に写像（`OfflineUncertified` のみ既存
  `OfflineCapabilityMismatch` を再利用）。consumer 側の新規
  project-layer 型を増やさない。

## Related Experiments

### Adopted

- なし（experiment log なし）。

### Rejected

- なし。

## Validation

- 補助証拠（canonical = mise）:
  - `cargo test -p synvelis-player --test tim03_attach_wiring`
    → 10 passed, 0 failed。
- canonical（`mise run`、全て exit 0）:
  - `fmt-check` / `check` / `lint`（clippy `-D warnings`）。
  - `test`（`cargo test --workspace --locked`、全 suite
    0 failed。`tim03_attach_wiring` 10・`tim03_process_boundary`
    17・`tim03_process_boundary_rt` 1 を含む）。
  - `test-rt`（`rt_` filter、RT hot path alloc-free 系を含む）。
  - `test-msrv`（rust 1.89 toolchain、全 suite 0 failed）。
  - `check-headless-deps`（headless 10 crate に GUI 依存なし）。
- `git fetch origin main && git merge origin/main` 後の再検証:
  PR 本文に結果を記録。

## Risks

- `BoundaryDeclaration` の登録は現状 `PluginHostProcessors` を
  直接触る caller（composition root/app 統合層）が行う想定で、
  実 app からの自動導出（precision provenance・boundary offset
  の Tempo/Loop 由来）は未実装 — 後続 slice。
- 実 CLAP/VST3/OS matrix・AudioLens f64/native PCM 実測は
  NotRun（別 gate: Ruviform #196/#253、synvelis #294）。
- RT hot path の no-alloc は attach 時に `AttachConfig` へ
  固定値を渡すのみで新規 RT 経路を追加しないため、既存
  `stats_alloc` 証拠（task1）を維持。

## Follow-ups

- `TimingPrecision` certified class の認定 provenance
  （`ExactWithinVerifiedCapability`/`FrameAccurateEvents` を
  正当化する計測・宣言経路）。
- boundary offset の Tempo map/Loop occurrence 自動導出
  （現状 caller が `BoundaryDeclaration`/request を与える）。
- 実 plugin CLAP/VST3・OS backend×rate×block matrix — 別 gate
  （NotRun）。
- AudioLens f64 & native PCM 非回帰の実測 — #294 別 Gate
  （NotRun）。
