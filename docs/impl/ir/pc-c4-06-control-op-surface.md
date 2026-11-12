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

## 実装サマリ

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

## 判断記録

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

## NotRun / 未検証

- `observe_cycle` の per-block 物理 call-window の RT runtime 内
  自動供給・`drain_output_reports` の engine 定期配線・
  production composition root の `PluginHost` 接続（後続 slice）。
- 実 VST3/OS backend（ruviform#253）・実 GUI（#114/#102）・
  AudioLens（#294）・oracle（#103）— 全て NotRun。
- wire schema の L1 検証（実 GUI/driver 結合）。
- `status` への live `plan_generation` 露出（player accessor が
  必要 — observe receipts が代替証拠）。

## 検証

- `mise run test`（`cargo test --workspace --locked`）: exits 0。
  新規 fixture `tests/pc_c4_06_control_op_surface.rs` は 4 test
  green（plan generation は fast=1・automation=2・observe 毎
  pump +1 の決定的規則どおり）。
- `mise run check` / `lint` / `fmt-check`: exits 0。
- `mise run test-rt`: exits 0。
- `mise run check-app-native` / `lint-app-native`: exits 0。
- `mise run test-msrv`（rust 1.89 toolchain）: exits 0。
- `mise run build`（`cargo build --workspace --locked`）: exits 0。
