---
id: "SPEC-1umIWi6ogYs4kgcJeTyvJ8"
type: "spec"
status: "approved"
title: "TIM-03 session→driver/routing wiring — ProcessBoundarySession の composition 接続（headless slice）"
created: "2026-11-11"
updated: "2026-11-11"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/designs/tim03-process-boundary-design.md"
    - "docs/plans/tim03-process-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
  defers: []
  deferred-by: []
---

# TIM-03 session→driver/routing wiring — ProcessBoundarySession の composition 接続

## What Should Be Built

Issue #127（TIM-03 Plugin Process 境界）の次 slice。task1–3
（`docs/specs/tim03-process-boundary.md` 系）で完成した
`ProcessBoundarySession` oracle を、`synvelis-player` の 2 つの
composition 境界へ実際に配線する。

- **誰が session 寿命を持つか**: `HeadlessDriver` は attach された
  node ごとに `ProcessBoundarySession` を activation 世代で開き、
  `RoutingControlDispatch` は generation-local `GenerationSurface`
  の `NodeSurface` ごとに session を保持する。session は
  `PrecisionProvenance` pin（instance/node/capability generation/
  graph generation）を mint し、generation 交代で旧 pin は typed
  refusal になる。
- **session 入力の出所**: routing 経路では
  `PluginHostProcessors` が `connect` で投影したのと同一の
  boundary 入力（`ProcessCapability`/`ExecutionMode`/`DegradedConsent`/
  `ScheduleBudget`）を `ProcessorHost::boundary_attach` 経由で
  `ProcessorControl` に同梱し、`take_processor_control` →
  `ParamDispatch` の既存 seam を通して session が開かれる。
  driver 経路では `AttachedNode` が同じ入力 spec を運び、
  activation 時に driver が provenance を mint する。
- **driver transport の正直な metadata**: `observe_attached_block`
  が捏造していた固定値（`loop_ticks: None`・`LoopOccurrence::new(0)`・
  `TransportGeneration::new(0)`・`position == cursor`）を、driver が
  command した実 transport 状態（status/anchor/loop/occurrence の
  mirror 投影）と node 自身の `AppliedReceipt`（transport/plan
  generation・clock epoch）へ置き換える。loop wrap 後の in-loop
  event が occurrence で再発火するよう、position は wrap 規則
  （`pos_at`/`do_wrap` と同じ連続モデル）で投影する。
- **fail-closed staging**: driver の transport command
  （activation/seek の anchor、`set_loop`/`clear_loop`/`panic`/
  `resume`）は boundary 宣言済み node では `stage_window` を通し、
  oracle 拒否・provenance 不整合・`QueueFull` を typed に返す。
  宣言なし node は従来の lenient 経路を維持する。

## Why Now

`ProcessBoundarySession` は task3 で per-request API として完成し
mock fixture で検証済みだが、誰も session を開いていない — oracle
は dead code のまま。Issue #127 の consumer 契約は「capability/
provenance/frame-base/generation/policy の不変条件を保ったまま
session を driver/routing 境界へ接続する」ことを要求する。本 slice
はその配線本体のみを対象とし、Tempo/Loop 自動導出・precision
certification 実測・実 plugin/OS・consumer 照合は別 slice/gate
に残す。

## Users and Value

- `HeadlessDriver` の attached plugin 経路: param/MIDI dispatch と
  transport staging が generation pin 付き session 経由になり、
  旧 generation の配送混入・frame-base 不一致・未宣言 policy の
  すり抜けが typed refusal になる。
- `RoutingControlDispatch`: `GenerationSurface` が node ごとの
  boundary session を所有し、`take_processor_control` で vend された
  control から provenance pin 付き session が開く。`flush_batches`
  （CTL-02）の param 配送権限は変更しない — 第 2 の param 配送権限は
  作らない。
- `observe_attached_block` の span context が scheduler の実状態を
  反映し、loop wrap 後も in-loop plan event が正しく投影される
  （現行の `position == cursor` 固定値では wrap 後に投影が止まる）。

## Scope

### In Scope

- `crates/synvelis-player/src/process_boundary.rs`:
  `BoundarySessionSpec`（attach 解決済み session 入力
  capability/mode/consent/budget）の追加。
- `crates/synvelis-player/src/host.rs`:
  `ProcessorControl::boundary` field、`ProcessorHost::boundary_attach`
  （default `None`）、`PluginHostProcessors::boundary_spec` への
  `boundary_delivery` 集約と `boundary_attach` 実装。
- `crates/synvelis-player/src/routing_engine.rs`:
  `RetainedConnection::boundary`、`stage_build` での
  `boundary_attach` 呼出、`take_processor_control` での vend。
- `crates/synvelis-player/src/routing_control.rs`:
  `NodeSurface::boundary`、`vend_nodes` での session open、
  `boundary_session`/`boundary_provenance`/`boundary_plan_window`/
  `boundary_stage_window` 公開 API と `BoundaryStageError`。
- `crates/synvelis-player/src/driver.rs`:
  `AttachedNode::boundary`、`AttachedDispatch`（dispatch + session +
  `TransportMirror` + last span）、activation/seek/loop/panic/resume
  の session 経由 staging、`observe_attached_block` の receipt 反映、
  `attached_boundary`/`attached_span` accessor、
  `ActivateError::BoundarySession`。
- `crates/synvelis-player/tests/`: 既存 fixture の追従と新規
  `tim03_session_wiring.rs`。
- 本 doc chain（spec/design/plan/task/test-spec/impl record）。

### Out of Scope

- `crates/synvelis-project/`・`crates/synvelis-app/`・
  `crates/synvelis-workspace/`・`crates/synvelis-render/`・その他
  crate の変更（`ProcessorConnection` の形状は render が exhaustive
  destructure するため本 slice では変更しない）。
- Tempo/Loop wrap 位置の自動導出（`boundaries` は caller 供給の
  まま — 別 slice）。
- `TransportMirror` が追えない caller-side transport staging
  （`ParamDispatch::control()`/`session.stage_window` 経由の
  out-of-band command）の mirror 反映 — receipt が generation を
  修正するが status/anchor/loop は driver-commanded state のまま
  （文書化済み制限）。
- `stage_plan`/`dispatch_params`/`dispatch_midi` の routing_control
  公開（param 配送権限は `PluginBindingSession`/`flush_batches` が
  唯一のまま — 設計判断として記録）。
- `tempo_revision` の実値追跡（`ParamDispatch` plan は revision 0;
  Tempo slice で扱う）。
- 実 plugin・実 OS・実 DAW・AudioLens f64/両 Consumer・実 GUI
  （`NotRun`/`Unsupported` 記録）。
- Issue #127 の Close。

## Requirements

- session は node ごと・activation/generation ごとに開き、
  `PrecisionProvenance` は権威ある attach/routing 状態から mint
  する（caller 申告の偽 provenance を受け付けない）。
- `AttachClockContext` は `PluginNodeControl` 由来の verified
  frame base のみを使う。
- capability/policy/provenance/frame-base の fail-closed は
  staging/dispatch の前に完了する。
- transport/plan generation は upstream scheduler 所有の
  event-scoped 軸 — `PrecisionProvenance` に複製しない（task3 の
  設計を維持）。
- `observe_attached_block` の span context は driver-commanded
  transport mirror + scheduler receipt の合成であり、捏造固定値を
  残さない。
- 旧 generation の session/dispatch が新 surface に混ざらない
  （attach/detach 境界で typed refusal）。
- `ProcessBoundarySession` は非 RT orchestration API のまま —
  RT callback 内で oracle を評価しない。
- upstream typed error（`ProcessBoundaryError`/
  `BoundarySessionError`/`InstallPlanError`/`DispatchError`/
  `QueueFull`）を flatten しない。

## Acceptance Criteria

- `HeadlessDriver` の attached plugin 経路で `ProcessBoundarySession`
  が per-node に開かれ、`plan_window`/`stage_window`/`dispatch_*`
  が既存 seam（`take_processor_control`→`ParamDispatch` 相当の
  control/dispatch pair）を通して合成される。
- `observe_attached_block` の捏造 metadata が driver の実 transport
  状態（loop/occurrence/transport generation/tempo revision 到達
  範囲）に置き換わる。到達不能な残りは「未対応の作業」に記録。
- QueueFull・stale generation・attach/detach 境界の挙動が headless
  テストで固定される（旧 generation の配送が新 session に混ざら
  ない等）。
- `tests/tim03_request_plan.rs` 回帰 + 新規 driver/routing 統合
  テスト。実 plugin・実 OS・実 DAW・AudioLens f64/両 Consumer・
  実 GUI は `NotRun`/`Unsupported`。
- Issue #127 は Open のまま。

## Deferred Design Concerns

- Tempo/Loop wrap 位置・tempo map revision の自動導出は別 slice
  （`boundaries` caller 供給・`tempo_revision` 実値追跡は本 slice
  の到達範囲外）。
- Degraded deferral で wrap が legal boundary へずれた場合の
  mirror position 乖離 — 本 composition の activated contract は
  `min_frames=1` で全非零 span が legal のため到達不能。contract
  が変わる将来 slice で再検討。
- caller が `ParamDispatch::control()`/`stage_window` で
  out-of-band に transport を staging した場合の mirror 乖離 —
  `ParamDispatch::control()` doc と同じ注意クラスとして文書化。
- routing_control への `stage_plan`/param/MIDI session dispatch
  公開 — param 権限の単一化（`PluginBindingSession`）を優先し、
  必要になれば別 slice で binding-aware な形で設計する。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/127>
- 上流 TIM-03: `docs/specs/tim03-process-boundary.md`、
  `docs/designs/tim03-process-boundary-design.md`、
  `docs/plans/tim03-process-boundary.md`、
  `docs/test-specs/tim03-process-boundary.md`、
  `docs/tasks/tim03-task3-request-plan.md`
- PC-C4 binding: `docs/specs/pc-c4-plugin-param-binding.md`
- Ruviform: scheduler `TransportScheduler`/`AppliedReceipt`/
  `SpanContext`/`TransportSpan`（pin `0f5b573`）、
  `PluginNodeControl`/`AttachClockContext`（#307）。
