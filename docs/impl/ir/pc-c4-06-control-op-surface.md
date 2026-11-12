---
id: "IMPL-7c4e9a1b2d3f4e5a6b7c8f0a4b5"
type: "impl"
status: "completed"
title: "PC-C4-06 control op surface — syn.control.* typed-evidence ops"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added:
      - type: "file"
        path: "docs/specs/pc-c4-06-control-op-surface.md"
      - type: "file"
        path: "docs/designs/pc-c4-06-control-op-surface-design.md"
      - type: "file"
        path: "docs/plans/pc-c4-06-control-op-surface-plan.md"
      - type: "file"
        path: "docs/tasks/pc-c4-06-control-op-surface-task.md"
      - type: "file"
        path: "docs/test-specs/pc-c4-06-control-op-surface.md"
      - type: "file"
        path: "docs/impl/ir/pc-c4-06-control-op-surface.md"
      - type: "file"
        path: "crates/synvelis-app/tests/pc_c4_06_control_op_surface.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-app/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-app/src/engine.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
  implements:
    - "docs/plans/pc-c4-06-control-op-surface-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-06-control-op-surface-plan.md"
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C4-06 control op surface — implementation record

## Summary

Issue #101 PC-C4 残件 slice — pc-c4-04 が app composition root に
実装した control surface の deferred API を、`synvelis-app` の
`AppService` 上の `syn.control.*` op（`fast_control`/`status`/
`observe`）として公開した。

## Context

PC-C4-04
（`docs/specs/pc-c4-04-app-composition.md`）が app composition
root に実装した control surface の全機能（catalog vend・
attach/sync・`control_tick`・`control_fast_control`・
`control_observe_cycle`・`feed_control_cycle`・
`with_control_dispatch` seam）に対し、`service.rs` の
`OPS`/`dispatch` には `syn.control.dispatch` / `subscribe` /
`unsubscribe` / `targets` のみが登録され、deferred API は
service 境界に露出していなかった（pc-c4-04 IR の NotRun 明記
どおり「`syn.*` op 登録は別 Worker」）。GUI・headless driver・
将来の CLI adapter が typed evidence を取得する経路が無かった
ため、本 slice はその deferred API を `syn.control.*` op として
登録し、wire schema・所有境界・honest-evidence 規約を定めた
（design Context — pc-c4-04 の「app は typed evidence の
pass-through surface であり、物理 call partition・host borrow・
plugin 値を invent しない」境界規約の延長）。

上流文書: spec `docs/specs/pc-c4-06-control-op-surface.md`、
design `docs/designs/pc-c4-06-control-op-surface-design.md`、
plan `docs/plans/pc-c4-06-control-op-surface-plan.md`、task
`docs/tasks/pc-c4-06-control-op-surface-task.md`、test-spec
`docs/test-specs/pc-c4-06-control-op-surface.md`。

## Implementation

### Changes

- `crates/synvelis-app/Cargo.toml`: `ruviform-plugin-core` を
  `[dependencies]` へ追加（Coordinator 承認済み —
  `FastControlWrite.means: ParamWriteMeans` が lib code から
  名指し不能だった blocker の解消）。dev-dep 重複は除去。
- `crates/synvelis-app/src/engine.rs`: `control_frame_base()`
  （attached dispatch の verified `frame_base()`）と
  `control_last_block()`（`last_block()` の `Display`）の
  read-only accessor を追加。
- `crates/synvelis-app/src/ui_ops.rs`:
  - `UiOpsState.last_control_drain: ReportDrainReport`（default =
    drain 未実施を正直に示す）。
  - wire projection helper: `time_base_json`/`media_time_json`/
    `surface_report_json`/`tick_report_json`/`report_drain_json`/
    `fast_control_block_json`/`fast_control_outcome_json`/
    `control_observation_json`/`applied_receipt_json` — counter・
    generation・tagged id・`Option` 区別を verbatim に投影し、
    contract serde を持たない型のみ variant 名/debug string 化。
  - wire parse helper: `write_means_param`/`i64_param`/
    `ObserveCallParam`/`ObserveFrameParam`/`ObserveBaseParam`/
    `ObserveSpanParam`/`ObserveParams` DTO +
    `observe_frame`/`transport_status_param`/
    `transport_span_param`。
  - `op_control_fast_control`: params → `FastControlWrite`
    （`position` は surface verified `frame_base` で stamp）→
    `control_fast_control` → `FastControlOutcome` 投影 +
    `Blocked` 時 `block_detail` → `ui_pump`。
  - `op_control_status`: `&self` snapshot — `engine`/`generation`/
    `active_generation`（`0`→`null` 規約）/`frame_base`/
    `last_error`/`surface`/`tick`/`drain`。
  - `op_control_observe`: `ObserveParams` parse → surface
    generation 一致 gate → `control_observe_cycle` →
    `feed_control_cycle`（`Reported` のみ、既存強制）→
    observations/receipts/`fed` を返却 → `ui_pump`。
  - `note_control_drain`: composition root の
    `ReportDrainReport` を `last_control_drain` に記録。
- `crates/synvelis-app/src/service.rs`: `OPS` に
  `syn.control.fast_control`/`syn.control.status`/
  `syn.control.observe` を登録、`dispatch` に 3 route 追加。
- `crates/synvelis-app/tests/pc_c4_06_control_op_surface.rs`:
  新規 headless fixture — `SharedService` wrapper で
  `AppService` を `Rc<RefCell>` 共有し、`UiRuntime::
  dispatch_op` 経由で 4 test（surface 無し fail-closed・typed
  outcome/status・observe stage 選別 + Reported feed・
  generation gate）。

### Decisions During Implementation

- `ParamWriteMeans` blocker: dev-dep のみ・re-export 無しで
  `synvelis-app` lib から名指し不能 → Coordinator 承認済みで
  `[dependencies]` 追加。player 変更不要。
- `position` は caller の frame 数 + surface verified
  `frame_base` で stamp — base を caller に主張させない。
- observe の span/call window/`plan_generation` は全て caller
  証拠。receipts を構造化 JSON（`status`/`plan_generation`/
  `transport_generation`/`clock_epoch`/`applied_frame`）で返し、
  driver の `TransportMirror` と同じ学習経路を提供。op は
  surface generation の一致のみを gate。
- drain は composition root のまま（`PluginHost` borrow は app
  所有外、`with_control_dispatch` seam が境界）。`note_control_drain`
  で typed report のみ status へ橋渡し。default は
  `attached:false` で「drain 未実施」を正直に示す。
- `active_generation` は `rt_status().active_generation` の
  `0`→`null` 規約 — 未観測を「active」と装わない。
- contract serde を持たない型（`HostError`/`DropReason`/
  `ApplyStatus`/`FastControlBlockKind`/`FastControlBlock` detail）
  のみ variant 名/debug string 化。stage 区別（`reported` vs
  `dispatched`）は wire でも偽装しない。

## Related Experiments

### Adopted

なし — `metadata.experiments.adopted` は空。

### Rejected

なし — `metadata.experiments.rejected` は空。

## Validation

- `mise run test`（`cargo test --workspace --locked`）: exits 0。
  新規 fixture `tests/pc_c4_06_control_op_surface.rs` は 4 test
  green（plan generation は fast=1・automation=2・observe 毎
  pump +1 の決定的規則どおり）。
- `mise run check` / `lint` / `fmt-check`: exits 0。
- `mise run test-rt`: exits 0。
- `mise run check-app-native` / `lint-app-native`: exits 0。
- `mise run test-msrv`（rust 1.89 toolchain）: exits 0。
- `mise run build`（`cargo build --workspace --locked`）: exits 0。

## Risks

- `plan_generation` の caller 供給は caller 負荷 — live plan
  generation を知るには receipts 等の evidence が必要で、初見
  caller は試行が要る。receipts を構造化 JSON で返すことで
  driver と同じ学習経路を提供するが、誤った generation の span
  は投影 gate で `Scheduled` 残留となる（誤配送ではなく未投影
  — fail-safe 側、design Risks）。
- `observe` op 末尾の `ui_pump` が automation dispatch で plan
  generation を進める — test は receipts で適用済み世代を確認し、
  次 span の generation を +1 規則で組み立てる（fixture 内で
  決定的）。
- `block_detail` は `Display` の string 化 — `FastControlBlock`
  は contract serde を持たないため、evidence の主張は構造化済み
  の `kind` が担い detail は診断用（design Risks）。
- dev-dep → dep 移動: `ruviform-plugin-core` は既に
  `synvelis-player`/`synvelis-project` が依存する workspace pin
  であり、新規外部依存・lockfile churn は無い。

## Follow-ups

- `observe_cycle` の per-block 物理 call-window の RT runtime 内
  自動供給・`drain_output_reports` の engine 定期配線・
  production composition root の `PluginHost` 接続（後続 slice）。
- 実 VST3/OS backend（ruviform#253）・実 GUI（#114/#102）・
  AudioLens（#294）・oracle（#103）— 全て NotRun。
- wire schema の L1 検証（実 GUI/driver 結合）。
- `status` への live `plan_generation` 露出（player accessor が
  必要 — observe receipts が代替証拠）。
