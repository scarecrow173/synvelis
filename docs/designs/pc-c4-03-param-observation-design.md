---
id: "DSGN-PCc4030bs3vN1pQ8wXyZ2aC"
type: "design"
status: "approved"
title: "PC-C4-03 param observation design — drain surface・binding 逆引き・gate"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/specs/pc-c4-03-param-observation.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-03-param-observation.md"
    - "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
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
  references:
    - "docs/specs/pc-c4-03-param-observation.md"
  defers: []
  deferred-by: []
---

# PC-C4-03 design — reported observation 消費

## 概要

`RoutingControlDispatch` に 2 つの generation-scoped 公開面を追加する。
drain（host→ledger の inbound 消費）と observe（ledger→observation の
projection）は cadence が異なる（cycle 単位 vs block window 単位）ため
別メソッドとし、共有する drain は存在しない。

## `drain_output_reports`

```rust
pub fn drain_output_reports(
    &mut self,
    endpoints: &[PluginStateEndpoint],
    generation: u64,
    active_generation: Option<u64>,
    drain: impl FnMut(PluginHandle, &mut Vec<Event>)
        -> Result<ReportDrain, HostError>,
) -> ReportDrainReport
```

- `endpoints` は caller が `RoutingEngine::plugin_state_handles
  (generation)` で vend した `(processor, PluginHandle)` 集合。
- `drain` は host borrow の注入 seam（本番:
  `|h, out| host.borrow_mut().drain_output_reports(h, out)`）。
  `PluginHost` 型への直接依存を持たず、`Unavailable` 分岐を L1 worker
  なしで検証可能にする。
- gate は 2 段: `generation != surface.generation` → `generation_gated`、
  `active_generation != Some(surface.generation)` → `inactive_gated`。
  いずれも endpoint に触れる前に return し、report は queued のまま残る。
- endpoint → node は `surface.nodes[processor]` で解決。node surface を
  持たない processor は `unrouted`。

## report → ledger 写像

| drain 結果 | ledger 操作 | report counter |
| --- | --- | --- |
| `Drained` + `EventKind::ParamValue` | session bindings で `ParamId → synvelis_target` 逆引き → `record_reported` | `reported` / `no_pending` / `unbound_param` |
| `Drained` + その他 kind | なし（`events` のみ計数） | — |
| `Unavailable` | `mark_reporting_unavailable` | `unavailable` |
| `Err(InvalidHandle \| Failed)` | `mark_reporting_unavailable` + `errors` | `unavailable` |
| `Err(その他)` / 未知 `Ok` variant | なし | `failed` + `errors` |

- 逆引きは `node.session.bindings()`（delivery と同一 table）を使うため、
  この generation が実際に bind した target にしか report は届かない。
  `param_in_use` 不変条件により `(node, ParamId)` の逆引きは一意。
- `record_reported` が `false`（eligible な `Scheduled`/`Dispatched`
  entry なし）を返す report は `no_pending` — plugin 起点の報告や
  終端済み write であり、偽の observation を生まない。
- `Event.frame_offset` は ledger semantics に含めない（`record_reported`
  の API が frame を取らない）。frame 帰属は後続 slice の検討事項。

## `observe_cycle`

```rust
pub fn observe_cycle(
    &mut self,
    processor: ProcessorId,
    generation: u64,
    span: &TransportSpan,
    call_start: EngineFrame,
    call_frames: u32,
) -> Option<DispatchCycle>
```

- `ParamDispatch::observe_cycle` の generation-scoped 版。node の
  scheduler report channel（`NodeReport`）の drain owner はここだけ。
- `generation`/`processor` 不一致は `None`（fail-closed — 他世代の
  window で `Dispatched` を mint しない）。
- span/call window は caller（RT runtime 側）が供給する —
  `ParamDispatch::observe` と同じ契約。

## 境界・非目標

- `PluginHost`/`Rc<RefCell<PluginHost>>` は routing_control の署名に
  現れない（`PluginHandle`/`ReportDrain`/`HostError`/`Event` の値型のみ）。
- app/workspace 層からの呼び出し配線は pc-c4-04。`HeadlessDriver` への
  組込みも本 slice では行わない。
- L1 slot の実値検証は pin bump 待ち `NotRun`。
