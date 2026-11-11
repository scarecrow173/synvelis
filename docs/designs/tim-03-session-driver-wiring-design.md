---
id: "DESIGN-MsBu5XoYY4Ex3QB0qmDvC5"
type: "design"
status: "approved"
title: "TIM-03 session→driver/routing wiring design — session 寿命・provenance mint・transport mirror"
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
  implements:
    - "docs/specs/tim-03-session-driver-wiring.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/designs/tim03-process-boundary-design.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim03-process-boundary.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/specs/tim03-process-boundary.md"
  defers: []
  deferred-by: []
---

# TIM-03 session→driver/routing wiring design

## Context

`ProcessBoundarySession`（task3）は per-request oracle への
provenance pin・frame-base 検証・staging ゲートを提供するが、
session を開く composition root が存在しない。2 つの既存境界が
session の自然な owner 候補:

- `HeadlessDriver`: `AttachedNode`（`PluginNode` + `ParamDispatch`）
  を activation ごとに受け取り、transport anchor/loop/panic を
  node の control channel へ直接 push している。span context は
  `observe_attached_block` が固定値で捏造している。
- `RoutingControlDispatch`: `RoutingEngine::take_processor_control`
  が generation-local `ProcessorControl`（node/control/frame_base/
  plugin identity）を一度だけ vend し、`vend_nodes` が
  `PluginBindingSession` + `ParamDispatch` を `NodeSurface` に
  保持する。

接続で満たすべき不変条件: capability/policy は attach 時に
`PluginHostProcessors::connect` が投影したものと同一であること、
provenance は権威ある状態から mint されること、frame base は
verified `AttachClockContext` のみ、旧 generation が新 session に
混ざらないこと、`ProcessorConnection` の形状は render が
exhaustive destructure するため変えられないこと。

## Scope

### In Scope

- `BoundarySessionSpec` の新設と host→engine→control の vend 経路。
- `HeadlessDriver` の per-node session + `TransportMirror` +
  receipt 反映 span context。
- `RoutingControlDispatch` の `NodeSurface` session + scoped
  transport staging API。
- headless test と doc chain。

### Out of Scope

spec の Out of Scope に同じ（project/app/render/他 crate、
Tempo/Loop 自動導出、実 plugin/OS、Issue Close）。

## Components and Boundaries

### `BoundarySessionSpec`（process_boundary.rs）

```rust
pub struct BoundarySessionSpec {
    pub capability: ProcessCapability,
    pub mode: ExecutionMode,
    pub consent: Option<DegradedConsent>,
    pub budget: ScheduleBudget,
}
```

`ProcessBoundarySession::open` の control/provenance 以外の引数を
そのまま運ぶ Copy 値。「attach 時に解決された boundary 入力」で
あり、session open 時に `attach_delivery` が再評価されるため、
spec と実 attach がずれていれば open が fail-closed する
（caller contract: spec は接続時と同じ入力でなければならない）。

### host→engine vend 経路（host.rs / routing_engine.rs）

`ProcessorConnection` は `synvelis-render/src/export.rs` が
exhaustive destructure するため field を追加できない。代わりに:

- `ProcessorHost` に default 実装つき
  `fn boundary_attach(&self, request: &ProcessorConnectRequest)
  -> Option<BoundarySessionSpec> { None }` を追加。
  `PluginHostProcessors` のみ override。
- `PluginHostProcessors::boundary_spec(request, process_config)`
  に `boundary_delivery` の capability/mode 解決を集約し、
  `connect` は spec→`attach_delivery` 投影、`boundary_attach` は
  spec をそのまま返す — 両者は必ず同一入力から導かれる。
- `stage_build` は `connect` の前に `boundary_attach(&request)` を
  呼び、結果を `RetainedConnection::boundary` に保持。
- `take_processor_control` が `ProcessorControl::boundary`
  （新規 pub field `Option<BoundarySessionSpec>`）として vend
  する。`ProcessorControl` は engine 内でのみ構築され、app は
  opaque に forward するだけなので field 追加は安全。

### `HeadlessDriver`（driver.rs）

- `AttachedNode` に `pub boundary: Option<BoundarySessionSpec>`。
  activation 時に driver が provenance を mint:
  `StablePluginInstanceId`（driver の `next_instance` 単調採番、
  activation ごとに新規）+ `node` + `capability_generation = 0`
  （driver-attached node に capability surface はなく、軸は
  instance + graph generation が担う — doc 明記）+
  `graph_generation = self.generation`（publish される plan 世代）。
- `self.attached: Vec<ParamDispatch>` →
  `Vec<AttachedDispatch>`:
  `{ dispatch, boundary: Option<ProcessBoundarySession>,
     transport: TransportMirror, last_span: Option<TransportSpan> }`。
- transport command の session 経由化:
  - activation/resume anchor: `set_clock`（clock rebase であり
    transport command ではない — session の window gate 対象外、
    ただし `QueueFull` は同一 bounded channel の拒否として
    `BoundarySessionError::QueueFull` に写像）+ `stage_window`
    `[(Seek,0),(Play,0)]`。activation では publish 前に staging
    するため失敗は `ActivateError::BoundarySession` で何も
    publish されない。非宣言 node は従来の `let _ =` lenient
    経路を維持。
  - `seek` 再 anchor: `set_clock` + `stage_window [(Seek,0)]`。
    signature を `Result<(), BoundarySessionError>` に変更
    （sessioned node の失敗を silent divergence にしない）。
  - `set_loop`/`clear_loop`/`panic`: 戻り値を
    `Result<(), BoundarySessionError>` に変更。sessioned は
    `stage_window`、非 sessioned は raw `request` で
    `QueueFull` → `BoundarySessionError::QueueFull`（同一チャネル
    の正直な拒否）。
  - `resume_attached_transport`: `Result<(), BoundarySessionError>`。
- `window_frames` は常に `engine.block_frames`（次 callback =
  driver が処理する唯一の window）。`boundaries` は `vec![]` —
  loop wrap 自動導出は別 slice。

### `TransportMirror`（driver.rs、private）

`observe_attached_block` が捏造する `SpanContext` の正直な供給源。
driver が command した transport のみを追跡する per-node mirror:

- `status`/`anchor_frame`/`anchor_pos`/`loop_ticks`/`occurrence`:
  `TransportScheduler::apply`/`do_wrap` と同じ規則で投影。
  position は wrap を畳み込んだ値（`pos_at`/`do_wrap` の連続
  モデルと代数的に等価 — 下記「position モデル」参照）。
- `transport_generation`/`clock_epoch`: node 自身の
  `AppliedReceipt`（`observe_cycle` の drain）から更新。
  `pending_generation` は driver が staging に成功した
  tg-bump 系 command（Play/Pause/Stop/Seek — SetLoop/ClearLoop/
  InstallPlan は bump しない）の未確認分で、span 構築時に
  `confirmed + pending` を予測値として使い、receipt 到着で
  reset する。caller-side command（`dispatch` の Play、
  `stage_window`/`stage_plan`）は mirror に見えず receipt が
  次 block で補正する（bound event を持たない ParamDispatch
  plan では tg は投影に効かない — doc 明記）。
- `plan_generation`: 従来どおり `dispatch.last_plan_generation()`
  — projection gate（`scheduler_gen`）と一致する値であり、
  receipt 由来の live pg と到達可能な全ケースで一致する。
- `tempo_revision`: `TempoMapRevision::new(0)` のまま —
  `ParamDispatch`/`plan_builder` 経由の plan は revision 0 で、
  実値源は Tempo slice の範囲（doc 明記）。

**position モデル**: mirror は scheduler と同じ
`(status, anchor_frame, anchor_pos, loop_ticks, occurrence)` を保持
し、`position_at(frame)` は

- 非 Playing: `(anchor_pos, occurrence)`（`pos_at` と同じ frozen）
- Playing: `p = anchor_pos + (frame - anchor_frame)`（飽和）;
  `loop=(s,e)`, `e>s`, `p>=e` なら `w=(p-e)/(e-s)+1` で
  `p-=w*(e-s)`, `occ+=w`。

これは scheduler の逐次 `do_wrap`（`anchor_pos=ls+overflow`,
`overflow=pos-le`）と代数的に等価: `p-w*len ∈ [s,e)` に収まり、
`w` は cascade 回数と一致する。`pos>=e` の即時 cascade
（SetLoop 適用時に既に loop end を越えている場合）も同じ式で
正しい occurrence 合計になる。Degraded deferral（wrap が legal
boundary にずれ `overflow>0`）は本 composition の activated
contract が `min_frames=1` で全非零 span legal のため到達不能
— 将来 contract が変わる slice で再検討（spec Deferred）。

**mirror 更新点**: activation/resume anchor（Playing,
anchor=(cursor,cursor), pending+=2）、seek（anchor=(engine_frame,
engine_frame), status 不変, pending+=1）、panic（Stopped,
anchor_pos=0, anchor_frame=次 head=cursor, pending+=1）、
set_loop（loop=(start,end), occurrence=0）、clear_loop
（loop=None, occurrence 不変）。いずれも staging 成功時のみ更新
— `stage_window` の `QueueFull` は先行 command が残り得るため
mirror は最後に完全成功した command 状態を保持する
（best-effort、doc 明記）。

### `RoutingControlDispatch`（routing_control.rs）

- `NodeSurface` に `boundary: Option<ProcessBoundarySession>`。
- `vend_nodes`: `PluginBindingSession::attach` と同じ `instance`
  （`StablePluginInstanceId`）を使い、provenance は
  `PrecisionProvenance::new(instance, control.node,
  surface.capability_generation, surface.generation)`。
  `ParamDispatch::new` が `control.control` を move する前に
  `ProcessBoundarySession::open(&control.control, spec…)` を呼ぶ。
  open 失敗は `without_control` に typed で落とす
  （`connect` が同一 `attach_delivery` を先に通すため実際には
  到達不能 — fail-closed の保険）。
- `surface_is_current`/`refresh_surface` の変更なし —
  boundary session の pin は binding session と同じ
  capability/graph generation を共有し、surface の staleness
  判定がそのまま session の寿命を規定する。
- 公開 API（`&self`/`&mut self`）:
  - `boundary_session(processor) -> Option<&ProcessBoundarySession>`
    — `plan_window`（純粋）と provenance 証拠用。session の
    request method は `&mut PluginNodeControl`/`&mut ParamDispatch`
    を必要とし、それらは surface 外に出ないため inert。
  - `boundary_provenance(processor) -> Option<PrecisionProvenance>`
    — `current` として提示する pin。
  - `boundary_plan_window(...)` / `boundary_stage_window(...)` —
    node の session + `dispatch.control()` を内部で borrow し、
    session メソッドへそのまま委譲。session 不在は
    `BoundaryStageError::NoSession`、session 拒否は
    `BoundaryStageError::Session(BoundarySessionError)` で
    typed 保存。
- `stage_plan`/`dispatch_params`/`dispatch_midi` は公開しない:
  param 配送権限は `PluginBindingSession`/`flush_batches` が唯一
  （第 2 権限を作らない制約）。`stage_plan` は任意 event plan を
  install でき binding 検証を迂回し得るため同じ理由で除外。
  MIDI dispatch は routing control surface の責務外。

## Data and Control Flow

```text
PluginHostProcessors::connect
  └─ boundary_spec(request) ──► BoundarySessionSpec
        ├─► attach_delivery → AttachConfig::{policy,budget}（既存）
        └─► ProcessorHost::boundary_attach → stage_build
              → RetainedConnection::boundary
              → ProcessorControl::boundary（take_processor_control）
              → vend_nodes → ProcessBoundarySession::open
                   provenance = (instance, node, cap_gen, gen)

HeadlessDriver::activate_with_attached
  └─ AttachedNode{node, dispatch, boundary: Option<Spec>}
       → AttachedDispatch{dispatch, session?, TransportMirror, last_span}
       → anchor staging（publish 前）: set_clock + stage_window[Seek,Play]
       → run(): observe_attached_block
            span = mirror.position_at + receipts-derived generations
            observe_cycle → receipts → mirror.note_receipts
```

## Non-Functional Constraints

- 非 RT orchestration のみ — oracle 評価・session open・staging は
  全て audio callback 外。`TransportMirror`/`observe_attached_block`
  も従来どおり非 RT 側。
- upstream typed error を保持（`BoundarySessionError` 各 variant、
  `ProcessBoundaryError`、`QueueFull`、`InstallPlanError`、
  `DispatchError`）。
- RT-suitable 性を悪化させない — 追加処理は全て non-RT 経路。

## Risks and Trade-offs

- **signature 変更**: `seek`/`resume_attached_transport` が `()` →
  `Result<(), BoundarySessionError>`、`set_loop`/`clear_loop`/
  `panic` が `QueueFull` → `BoundarySessionError`。
  player crate 内 API で consumer は test のみ — 追従コストは
  限定的。`BoundarySessionError::QueueFull` が同一 bounded
  channel の拒否を正直に表す。
- **sessioned/非 sessioned の非対称**: 宣言 node は strict typed
  staging、未宣言 node は legacy lenient — 「宣言 = 境界契約を
  負う」という一貫した規則として文書化。legacy 挙動の温存は
  既存 test 互換のため。
- **mirror の限界**: caller-side out-of-band staging・Degraded
  deferral・tempo revision は追跡しない — 全て doc 明記済みで、
  receipt が generation を補正し、position/status は
  driver-commanded state の投影に留まる。
- **`boundary_attach` の二重導出**: `connect` と vend で同じ
  spec を二度計算するが、`boundary_spec` 一箇所に集約し決定的
  （`ProcessConfig::new` は cheap pure）— 乖離は構造的に不可能。

## Deferred Design Concerns

spec の Deferred Design Concerns に同じ。加えて:

- `AttachedNode.boundary` の caller contract（spec が実 attach
  入力と一致すること）は検証不能 — `AttachConfig::{policy,
  budget}` は control から読み戻せない。session open の
  `attach_delivery` 再評価が capability-level の不一致は
  fail-closed するが、policy/budget の一致自体は caller 責任。
  `PluginHostProcessors::boundary_attach` を使う composition
  root では構造的に保証される。

## Verification Notes

- `tests/tim03_session_wiring.rs`（新規）+ `tim03_request_plan.rs`
  回帰 + 既存 driver/routing test 追従。
- span context の honesty は `attached_span` accessor 経由で
  loop wrap 後の `loop_ticks`/`occurrence`/wrapped
  `position_ticks`/receipt-derived `transport_generation` を
  assert する。
