---
id: "EXP-tim03-loop-boundary-mirror-vs-wrapsim"
type: "experiment"
status: "completed"
title: "TIM-03 Loop boundary 導出 — mirror 読み出し vs maintained WrapSim の試行"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added:
      - type: "file"
        path: "docs/impl/exp/tim-03-loop-boundary.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/tim-03-loop-boundary.md"
  depends-on:
    - "docs/designs/tim-03-loop-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-03-loop-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-03-loop-boundary.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/tim-03-loop-boundary.md"
  defers: []
  deferred-by: []
---

# TIM-03 Loop boundary 導出 — mirror 読み出し vs maintained WrapSim の試行

## 問題設定

loop wrap `BoundaryPoint` の導出に必要な scheduler-equivalent
state（`status`/`anchor`/`loop`/`pos_at`）を、
`RoutingControlDispatch` が保持する既存状態からどう得るか。
初期設計（design doc 初版）は `TransportMirror` の
commanded state を読み出して window start で再構成する
方式だった。実装前に upstream `TransportScheduler`
（Ruviform #190）と mirror の fold 規則を精読した結果、
この方式では mid-stream `SetLoop`/`ClearLoop` をまたぐ
window で scheduler `pos_at` を再構成できないことが
判明した。

## E1 mirror unfold-line の再構成限界

`TransportMirror` は anchor/unfold line を保持し、
`position_at`/`raw_position_at` は **現在の** loop
region で lazy fold する。scheduler の `pos_at` は
Playing 中 `anchor_pos + (frame - anchor_frame)` を
返し、wrap（`do_wrap`）のたびに
`anchor=(wrap_frame, loop_start + overflow)` へ
再錨定する scalar state である。

`SetLoop`/`ClearLoop` が window 途中に適用されると:

- scheduler は wrap ごとに当時の loop region で
  再錨定した anchor を持ち続ける。
- mirror は過去の unfold line を *現在*（= apply 後）の
  loop region で fold するため、loop region が変わった
  時点より前の wrap frame を別 region で fold し直し、
  `pos_at` が乖離する。

つまり「mirror から scalar state を読み出して window
start で再構成」は SetLoop/ClearLoop またぎで
scheduler と一致しない。これは derivation の核心
fixture（mid-window SetLoop）で直接顕在化する。

## E2 maintained WrapSim の採用

scheduler と同一 algebra の scalar state `WrapSim`
（`status`/`anchor_frame`/`anchor_pos`/`loop_ticks`/
`frame_base`）を `process_boundary.rs` に `pub(crate)`
で実装し、`NodeSurface` が 1 つ保持する。
`observe_node_block` は staged→scheduled 解決と
due sort の後、`due < end_frame` の pending を集めて
`wrap_sim.walk_window(start_frame, window_frames,
&pending)` を呼び、観測 window ごとに sim を進める。

`boundary_loop_wraps` は `wrap_sim.clone()` に対して
`schedule+staged` merge・sort した pending を渡して
preview walk する — 実 state（mirror/scheduled/
staged/sim）は一切変更しない pure read。

利点:

- wrap は scheduler と同じ逐次再錨定で追跡されるため、
  SetLoop/ClearLoop/ClearLoop→SetLoop の任意の列で
  scheduler `pos_at` と一致する。
- mirror の既存契約（span projection・receipts）を
  変更しない — sim は独立 state。
- command の適用列は `observe_node_block` と同一の
  `scheduled_transport` を共有するため二重適用・
  取りこぼしがない。

コスト:

- `NodeSurface` に `Copy` scalar state が 1 つ増える
  （~40 bytes、allocation なし）。
- `observe_node_block` に `walk_window` 呼出が 1 つ
  増える（window あたり `O(applies + wraps)`、wrap ごとに
  loop_len 以上進むか cascade は同 frame coalesce）。

## E3 却下した代替案

- **mirror 読み出し + window-start 再構成**: E1 の理由で
  棄却。SetLoop/ClearLoop またぎの fixture で scheduler
  と乖離する。
- **`TransportScheduler` を node ごとに複製**: upstream
  scheduler は RT callback・span dispatch・receipt を
  持つ重い state であり、wrap 導出に必要な scalar
  trajectory だけを複製する `WrapSim` の方が境界が
  明確（generation/receipt/plan を持たない）。
- **window ごとの全履歴 replay**: `observe_node_block`
  が既に全 pending を順に適用しているため、sim を
  維持して walk する方が単純で allocation も少ない。

## 結論

maintained `WrapSim` を採用。`docs/impl/ir/
tim-03-loop-boundary.md` に実装記録、`evidence/sv127/
verification.md` に検証証拠を記録。
