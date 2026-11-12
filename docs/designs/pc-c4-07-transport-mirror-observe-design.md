---
id: "DESIGN-7c4e9a1b2d3f4e5a6b7c8d9e0f1a"
type: "design"
status: "approved"
title: "PC-C4-07 transport mirror + observe_block design"
created: "2026-11-19"
updated: "2026-11-19"
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
  implements:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
    - "docs/designs/tim-03-session-driver-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c4-03-param-observation-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/test-specs/pc-c4-07-transport-mirror-observe.md"
---

# PC-C4-07 transport mirror + `observe_block` design

## 問題

`observe_cycle` は caller が `TransportSpan` を捏造して渡す契約で、
`ui_ops.rs` の `syn.control.observe` は呼出ごとに虚構 span を生成
していた — transport 位置・status・generation が runtime の実状態と
無関係に「それらしい」値になる。driver path は `TransportMirror`
で commanded transport → `SpanContext` を解決済みだが、routing
control surface には同等物がなかった。

## 設計

### mirror の再利用

`driver.rs` の `TransportMirror` を `pub(crate)` 化し、routing
control の `NodeSurface` が node ごとに 1 つ保持する。第二モデルを
作らない — driver と同一の anchor/position/loop/occurrence/
generation 予測 + receipt 補正ロジックを共有する。

### command ingress の記録

node control queue の `due_frames` は「push 時点の次 callback
head 相対」。mirror は staging 成功時に `(command, due_frames)` を
`staged_transport` へ記録し、`observe_block` が最初に観測する
`call_start` (= node が pop する次 head) を基準に絶対 apply frame
`call_start + due` へ解決して `scheduled_transport` へ移す。

記録点は 2 箇所のみで、いずれも「queue への push が成功した後」:

- `fast_control` / `flush_batches` の `session.dispatch` 成功後 —
  `ParamDispatch::dispatch` が内部で push する `Play` (due 0) を
  同じ due で記録 (`install_plan` は transport command ではない)。
- `boundary_stage_window` の `stage_window` 成功後 — commands を
  そのまま記録 (TIM-03 ingress 点と同一)。

### `observe_block` 契約

```text
observe_block(generation, call_start, call_frames)
    -> Option<BTreeMap<ProcessorId, DispatchCycle>>
```

- `generation` は attach 済み surface generation と一致必須 —
  不一致は `None` (fail-closed、`observe_cycle` と同じ gate)。
- `call_start`/`call_frames` は caller 供給の物理 window — node の
  cursor domain で、run loop が block ごとに 1 回ずつ順に渡す。
- node ごとに: staged → scheduled 解決 → apply frame で window を
  分割 → 各 segment で `span_context` を mirror から導出し
  `observe_cycle` へ供給 → receipt を merge → window 末尾以降の
  apply は scheduled に残す → merged receipts を
  `note_receipts` で generation 軸へ適用。
- observations は segment 順に merge、同一 target は後の segment
  が勝つ。`last_span` には先頭 segment の span を保持。

### 正直さの境界

commanded-state 投影である: push と callback head の race、
scheduler 側の reject は mirror に届かない。generation 軸
(transport/plan generation) は receipt が訂正するが、status/position
は commanded 近似のまま — driver mirror が文書化するのと同じ
honesty contract。caller が window を飛ばして観測した場合、staged
command の解決は最初に観測した `call_start` 基準になるため、run
loop の「毎 block 連続観測」契約が前提。

## capability rebind junction (test-only)

`PluginHostProcessors::register_params`/`register_boundary` は
registration epoch を無条件に bump し `capability_generation` を
動かす。テストは `SharedAdapter(Rc<RefCell<PluginHostProcessors>>)`
経由で engine 所有後も登録面を保持し、prepare→publish 間に drift
を注入して `PluginCapabilityChanged` block・`reconcile` の
`CapabilityGeneration` mismatch・再 prepare/republish・`sync`
rebind (`graph_generation` = 新 plan gen、instance log の新規
instance)・stale-generation gate (`observe_block` None、
`fast_control` Queued、`boundary_stage_window`
`InactiveGeneration`、`drain_output_reports` `generation_gated`)
を一気通貫で証明する。
