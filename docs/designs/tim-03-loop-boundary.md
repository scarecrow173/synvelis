---
id: "DESIGN-034XzNPr4B2cwT30gBhGCN"
type: "design"
status: "approved"
title: "TIM-03 Loop/Tempo boundary 自動導出 — 設計"
created: "2026-11-14"
updated: "2026-11-14"
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
    - "docs/specs/tim-03-loop-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-03-loop-boundary.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  defers: []
  deferred-by: []
---

# TIM-03 Loop/Tempo boundary 自動導出 — 設計

## Context

`ProcessBoundarySession::{plan_window,stage_window,stage_plan}` は
`boundaries: Vec<BoundaryPoint>` を caller 供給とし、command
由来 point（`command_boundary`/`window_offset`）のみ自動付加する。
loop wrap は position 由来の discontinuity であり、caller が
transport 状態を再実装して列挙する現状は mirror/scheduler との
二重管理になる。`RoutingControlDispatch` の `NodeSurface` は
既に `TransportMirror`（commanded status/anchor/loop）と
`scheduled_transport`（absolute apply frame）+
`staged_transport`（head-relative due）を保持し、
`observe_node_block`/`note_transport_command` が upstream
scheduler と同一の validity・apply 規則で mirror を畳み込んでいる。

ただし mirror は unfold line を *現在の* loop region で lazy
fold する投影であり、途中の `SetLoop`/`ClearLoop` をまたぐと
scheduler の `pos_at`（wrap ごとに再錨定する scalar state）を
再構成できない。そこで各 control node に scheduler 等価の
`WrapSim` を維持し、`observe_node_block` が観測する全 window
を同一 pending command 列で walk して進める。
`boundary_loop_wraps` はその sim を clone して read-only で
preview walk する — mirror/scheduled queue を一切変更しない。

upstream `TransportScheduler`（Ruviform #190）の wrap 意味論:
`pos_at(frame)` は Playing 中 `anchor_pos + (frame -
anchor_frame)`、それ以外は `anchor_pos` で凍結。`next_wrap
(after)` は Playing かつ loop 設定時に `after + max(loop_end -
pos_at(after), 0)` を返す。`do_wrap(frame)` は
`anchor=(frame, loop_start + overflow)`・occurrence+1 に
再錨定する。run_callback は pending apply due と wrap frame の
小さい方を境界とし、**同 frame の tie では apply が先**、
wrap は新 state で再評価される。plan install は
status/anchor/loop を変えない（pg/tempo_rev/plan のみ）。

## Scope

### In Scope

- `process_boundary.rs` への `pub(crate)` scheduler 等価
  scalar state `WrapSim`
  （`status`/`anchor_frame`/`anchor_pos`/`loop_ticks`/
  `frame_base`）: `pos_at`/`next_wrap`/`wrap`/
  `apply_command`/`walk_window` を持つ `Copy` state。
  `apply_command` は `media_time_ticks` + scheduler
  validity 規則（Seek 非負・SetLoop 非空）を
  `note_transport_command` と同一に適用する。
  generation/receipt/plan は持たない — wrap は
  それらに依存しない。
- `routing_control.rs` への
  `RoutingControlDispatch::boundary_loop_wraps(&self,
  processor, project, call_start: EngineFrame,
  call_frames: u32) -> Result<Vec<BoundaryPoint>,
  BoundaryStageError>`: surface ゲート → node 取得 →
  `wrap_sim` を clone し、scheduled+staged を absolute
  due へ merge・sort して `walk_window` で preview
  walk する。`NodeSurface.wrap_sim` は
  `observe_node_block` が各観測 window を
  `walk_window` で進めて維持する（mirror fold と
  同じ pending 適用列を共有し、command の二重
  適用はない — sim は独立 state）。
- `tests/tim03_loop_boundary.rs`（新規）と docs チェーン、
  `evidence/sv127/`。

### Out of Scope

- spec の Out of Scope と同一（consumer 照合・実
  plugin/OS/GUI・driver.rs 配線・plan install tracking・
  依存変更・他ファイル）。

## Components and Boundaries

- **`WrapSim`（process_boundary.rs, `pub(crate)`）**:
  `pos_at`/`next_wrap`/`wrap`/`apply_command`/
  `walk_window` を持つ `Copy` scalar state。
  `NodeSurface` が 1 つ保持し、vend 時に
  scheduler-initial state（`Stopped`, anchor `(0,0)`,
  loop 無し）で初期化される。`pos_at` は Playing 中
  `anchor_pos + (frame - anchor_frame)`、それ以外は
  凍結 — loop の lazy fold を持たず、wrap は
  `wrap(frame)` が `anchor=(frame, loop_start +
  overflow % len)` に再錨定することでのみ起きる
  （upstream `SimState` と同一 algebra）。
  `apply_command` は `media_time_ticks` + scheduler
  validity 規則（Seek 非負・SetLoop 非空）を
  `note_transport_command` と同一に適用する。
  generation/receipt/plan は持たない — wrap は
  それらに依存しない。
- **`WrapSim::walk_window`（process_boundary.rs,
  `pub(crate)`）**: window start で head position を
  `pos_at(ws)` から再錨定（`f >= ws` の trajectory
  を厳密に再現 — head wrap は pre-wrap position
  `loop_end` として評価され offset 0 を記録）→
  due <= ws の applies を ws head で畳み込み →
  `(ws, we)` を walk し wrap frame を
  `BoundaryPoint{offset, LoopWrap}` として収集。
  offset 昇順・同一 offset coalesce（cascade は
  `wrap` の代数 collapse で 1 point）。window end
  の wrap は次 window へ帰属。`self` を post-window
  state へ進める — `observe_node_block` がこれを
  実 state に対して呼び、`boundary_loop_wraps` は
  clone に対して呼ぶ。
- **`boundary_loop_wraps`（routing_control.rs, `pub`）**:
  `&self` の pure read。`NoSurface`/`StaleSurface`/
  `NoSession`（node 不在 = vended control なし）のみ
  typed error。boundary session は要求しない
  （sim は全 vended node で維持）。

## Data and Control Flow

1. `observe_node_block`（RT 観測経路、既存）:
   staged→scheduled 解決と due sort の後、
   `due < end_frame` の pending を集めて
   `node.wrap_sim.walk_window(start_frame,
   window_frames, &pending)` を呼び、sim を
   post-window state へ進める。mirror fold・
   segment dispatch は従来どおり — sim walk は
   独立 state への追加 step であり、command の
   適用列は同一 `scheduled_transport` を共有する
   ため二重適用・取りこぼしはない。
2. caller が upcoming window `(call_start,
   call_frames)` を指定して `boundary_loop_wraps`
   を呼ぶ（`observe_block` と同一 window・同一
   順序 — sim はまだ pre-window state）。
3. `NodeSurface` から `scheduled_transport`
   （absolute due、`we` 未満）+ `staged_transport`
   （`ws + due` へ解決、push 順保持、`we` 未満）
   を `(due, cmd)` へ merge し due で stable sort
   （同 due は scheduled → staged の queue 順 —
   `observe_node_block` の解決順と一致）。
4. `node.wrap_sim` を clone して
   `walk_window(ws, call_frames, &pending)` を
   呼び、返り値 `Vec<BoundaryPoint>` を得る。
   clone のため mirror/scheduled/staged/sim
   いずれも変更されない。
5. caller が返り値を `boundary_plan_window`/
   `boundary_stage_window` の `boundaries` へ渡し、
   command 由来 point と offset dedup されて
   oracle が検査する。

## Non-Functional Constraints

- 非 RT orchestration のみ: 割当は導出 Vec のみ、
  RT callback・plugin process・`observe_block` の
  mutation 経路に触れない。`&self` pure read で
  mirror/scheduled state を変更しない。
- frame-linear position domain 前提:
  `position_base == frame_base`（1 tick == 1 engine
  frame）。tempo map revision は musical 情報専用で
  wrap frame を動かさない（`TransportPlan.
  tempo_map` は `SpanContext.tempo_revision` にのみ
  現れ、`pos_at`/`next_wrap` は tempo を参照しない）
  — この不変条件をコードコメントと文書に明記。
- bounded work: pending applies は node の
  `max_pending_applies` budget で既に bounded、
  wrap walk は `O(applies + wraps)` で window frames
  に比例しない（wrap ごとに loop_len 以上進むか
  cascade は同 frame coalesce）。

## Risks and Trade-offs

- **mirror commanded-state approximation**: scheduler
  側 reject・out-of-band `control()` push は mirror
  に見えない — 既存の documented limitation を継承し、
  receipts が generation 軸を補正する契約は変更しない。
  wrap 導出も同じ approximation の上に立つことを
  文書化する。
- **staged→absolute 解決の window 仮定**: staged
  `due_frames` は次の `observe_block` call_start に
  対して解決される。`boundary_loop_wraps` の
  `call_start` がその head と一致する契約を明記し、
  不一致時は導出がずれ得ることをリスクとして記録。
- **SetLoop+preempt の occurrence edge**: mirror の
  既知 edge（same-head SetLoop→Stop/Seek/ClearLoop
  で frozen occurrence が scheduler より進む）は
  evidence-level で wrap frame には影響しない
  （occurrence は BoundaryPoint に含めない）。
- **wrap at offset 0**: window head の wrap は
  offset 0 の `BoundaryPoint` として返す
  （head apply と同じ oracle 契約、cut には
  ならない）。

## Deferred Design Concerns

- consumer 照合・実 plugin/OS matrix・precision
  certification — spec の Deferred と同一。
- driver.rs（`HeadlessDriver`）側の wrap 導出 —
  `loop_wrap_boundaries` は `pub(crate)` で
  driver からも再利用可能だが、本 slice では
  配線しない。
- degraded deferral の wrap frame 予測 — 要求
  frame のみ導出し、実適用 frame は oracle/
  scheduler の責務（`LoopWrap` は deferral
  scan を bound しない既存規則）。

## Verification Notes

- [ ] `mise run fmt-check` / `check synvelis-player` /
  `lint synvelis-player` / `test synvelis-player` が
  pass。
- [ ] `tim03_loop_boundary.rs` が導出 offset と
  実 scheduler wrap frame（plugin `TransportInfo`
  position fold）の一致を Fake Plugin 経路で
  検証する。
