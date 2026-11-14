---
id: "EXP-observewiring01a2b3c4d5e6f7"
type: "experiment"
status: "completed"
title: "PC-C4 observe engine wiring — call-window domain と receipt drain-once の検証"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/pc-c4-observe-engine-wiring.md"
  depends-on:
    - "docs/impl/ir/pc-c4-observe-engine-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/pc-c4-plugin-binding.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
    - "docs/test-specs/pc-c4-observe-engine-wiring.md"
  defers: []
  deferred-by: []
metadata:
  experiments: []
---

# PC-C4 observe engine wiring — experiment log

## EXP-1: `call_start` の frame domain — engine cursor vs node cursor

### 問い

`RoutingControlDispatch::observe_block(generation, call_start,
call_frames)` に渡す `call_start` はどの frame domain の値か。
Coordinator 分析は `RoutingEngine::cursor()`（pub）で block 開始
frame を取れると示唆していたが、`observe_block` が投影する
`TransportSpan` と node scheduler の call-window 会計がどちらの
domain を期待するかが実装前の分岐点だった。

### 検討した選択肢

1. **`RoutingEngine::cursor()` をそのまま渡す** — engine cursor は
   transport domain で、seek で再 anchor され、publish 前に処理
   済みの block 数を反映しない。plugin node の cursor は attach
   時に `EngineFrame(epoch=0, frame=0)` で始まり、その plan が
   active な間の `rt.process` 回数だけ進み、generation 切替で 0 に
   戻る。両者は一致しない。
2. **`GraphRuntime`/node 側の cursor を直接読む** — ruviform 側の
   node cursor は per-node 内部状態で、routing 層から公開
   accessor が無い（`frame_cursor` は runtime 全体のもので
   node domain ではない）。
3. **`RoutingEngine` が node-head を自前で追跡する** —
   `rt.process` 成功ごとに post-process `active_generation` を読み、
   generation が変われば head=0、同じなら前回 head+frames。
   観測しない block でも head は進める（全 process 成功を対象）。

### 結果

選択肢 3 を採用。`ObservedBlock { generation, call_start,
call_frames }` を `run_observed`/`seek_observed` の observer
callback で供給し、`call_start` は `ClockEpochId::new(0)` の
node-cursor domain とした（routing path は `SetClock`/
`DeviceRestart` を送らないため epoch は attach mint の 0 で一貫）。
`status_observe_section_tracks_node_cursor_domain` test が
2 block 後の `[256, 512)`・generation swap 後の `[0, 256)` restart
・engine cursor との乖離を pin する。

### 却下理由（選択肢 1・2）

- 選択肢 1: engine cursor を node domain として渡すと、seek 後や
  generation 切替後に `observe_block` の span 分割・ledger 投影が
  node 実体とずれ、`last_observed_span` が捏造 window を報告する。
- 選択肢 2: node cursor の公開 accessor が無く、追加には
  ruviform 側変更（本 slice の編集範囲外）が必要。engine 側の
  post-process generation 観測で同じ契約を満たせる。

## EXP-2: scheduler receipt の drain-once と explicit observe の競合

### 問い

`ParamDispatch::observe_cycle` は呼出しごとに node の report
channel を `drain_reports` する。`SessionRoutingEngine::run` が
block ごとに `observe_block` を駆動するようになると、RT が
`install_plan` apply で emit した `AppliedReceipt` は run 中の
観測で消費され、後続の明示 `syn.control.observe` op の
`receipts` が空になる。これは退行か、許容される意味変化か。

### 検討した選択肢

1. **`observe_cycle` を peek 型に変える** — receipt を drain
   せず複数 consumer が読めるようにする。`dispatch.rs` は本
   slice の編集範囲外（`routing_engine.rs` のみ可）であり、
   単一 drain owner という PC-C3-05 trace 契約
   （「the only report-channel drain path」）にも反する。
2. **`op_control_observe` が pending cycle の receipt を response
   に merge する** — drain-once を維持しつつ evidence を op
   response に残せるが、window 帰属の違う receipt を 1 response
   に混ぜることになり、通常運転では `ui_pump` が先に pending を
   drain して receipt を捨てるため効果が薄い。
3. **receipt の authoritative consumer は観測時点で完了済みと
   整理し、test を production 経路に合わせる** —
   `observe_node_block` 内の `transport.note_receipts` が
   mirror の transport/plan generation を補正するのが receipt の
   本来の役割であり、これは auto-observation でも同じく実行
   される。明示 op の `receipts` は「この call が drain した
   分」を正直に返す（多くの場合空）。

### 結果

選択肢 3 を採用。`pc_c4_06_control_op_surface` の
`observe_reports_stages_and_feeds_only_reported` を更新し、
applied plan generation の receipt 証拠は
`engine.control_drain_observations()` が返す buffered cycle で
assert、明示 op の `receipts` は drain-once で空であることを
明示 assert した。op の doc comment に drain-once 意味を記録。

### 却下理由（選択肢 1・2）

- 選択肢 1: 編集範囲外かつ単一 drain owner 契約に反する。
  receipt を複数回適用すると mirror 補正が二重計上される
  リスクがある。
- 選択肢 2: receipt の window 帰属を偽装する merge になり、
  `ui_pump` 先行 drain で大半の場合結局空になる — evidence を
  増やすのではなく semantics を濁すだけ。
