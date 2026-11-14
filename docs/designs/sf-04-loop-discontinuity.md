---
id: "DESIGN-492syZg75ShHdZRAeG2DRE"
type: "design"
status: "approved"
title: "SF-04 loop-discontinuity 設計 — 検出 accessor・per-block 宣言結線・OpName wire"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-04-loop-discontinuity.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-loop-discontinuity.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/sf-04-slice3-clock-epoch-discontinuity-design.md"
    - "docs/designs/sf-04-slice6-probe-compare-design.md"
    - "docs/designs/sf-04-slice8-probe-faults-declare-design.md"
    - "docs/designs/tim-03-loop-boundary.md"
  defers: []
  deferred-by: []
---

# SF-04 loop-discontinuity 設計 — 検出 accessor・per-block 宣言結線・OpName wire

## Context

SF-PROBE-07 の Loop 脚に必要な機構は両側とも実装済み:

- **検出側（TIM-03/sv#127）**: `NodeSurface.wrap_sim`
  （scheduler-faithful `WrapSim`）が `observe_node_block`
  内で `walk_window` により各観測 window を走査し、
  `LoopWrap` `BoundaryPoint` を返す（現行は `let _ =`
  で破棄）。`scheduled_transport` の fold ループが
  `SetLoop`/`ClearLoop` を `note_transport_command`
  経由で mirror へ適用する。
- **宣言側（SF-04 slice8）**: `RoutingEngine::
  declare_probe_discontinuity` → `probes.bump_clock_epoch`
  が epoch cut を行い、境界跨ぎ compare を
  `EpochMismatch`/`DiscontinuityInWindow` で fail-closed
  にする。app passthrough
  `SessionRoutingEngine::declare_probe_discontinuity`
  も実装済み。

欠けているのは「どの block で不連続が起きたか」を
observer（`RoutingControlDispatch`）から run-loop
呼出し側（`SessionRoutingEngine::run`）へ伝える検出
accessor と、その読み取り→宣言の結線だけである。

## Scope

### In Scope

- `crates/synvelis-player/src/routing_control.rs`:
  `RoutingControlDispatch` への検出フラグ + read-only
  accessor、`observe_block`/`observe_node_block` 内の
  検出配線。
- `crates/synvelis-app/src/engine.rs`:
  `SessionRoutingEngine::run` の per-block 観測→検出→
  宣言結線。
- `ui/src/ipc.ts`: `OpName` union への
  `"syn.probe.compare"` 追加。
- `crates/synvelis-player/tests/
  sf04_loop_discontinuity.rs`（新規）。
- `docs/` 文書チェーン・slice8 deferred 解消記録・
  `evidence/sv107/`。

### Out of Scope

- `probe.rs`/`routing_engine.rs`/`driver.rs`/
  `process_boundary.rs`/`monitor_engine.rs` 他 player
  src、`synvelis-app` の `monitoring.rs`/`ui_ops.rs`/
  `service.rs`/tests、`ui/src/store.ts`（呼出し経路が
  型を要求しない限り）、SF-PROBE-08/09、#108/#109、
  実機 gate。

## 設計判断 1 — 検出源は `WrapSim::walk_window` + fold 済み apply、occurrence 差分ではない

`observe_node_block` は既に 2 つの権威ある信号を計算
している:

1. `node.wrap_sim.walk_window(start, frames, &pending)`
   — 戻り値 `Vec<BoundaryPoint>` は当該 window 内の
   `LoopWrap` 点のみ（`BoundaryKind` は `LoopWrap`
   以外を返さない契約）。scheduler の `run_callback`
   と同じ境界選択（head apply → `min(next apply,
   next wrap)` 逐次・apply が tie に勝つ・wrap は
   re-anchor 後に再評価）を再現する。
2. `scheduled_transport.drain(..fold)` — window 内で
   fold される `(due, TransportCommand)`。ここで
   `SetLoop`/`ClearLoop` を検出する。

**occurrence 差分（`span.ctx.occurrence` の前回値との
比較）は採用しない**:

- `span.ctx.occurrence` は `TransportMirror`（lazy-fold
  mirror）経由の近似値で、`WrapSim` doc が明記する
  通り mid-stream `SetLoop`/`ClearLoop` 後に scheduler
  の `pos_at` と発散しうる。検出に使うと「mirror が
  起こさなかった wrap」を見逃す/起こしたと誤認する
  余地がある。
- `walk_window` はまさに「この window で wrap が
  起きたか」を scheduler-faithful に答える既存計算
  であり、戻り値を捨てているだけだった。検出はこの
  権威ある信号をそのまま使うのが DRY かつ正確。
- occurrence は wrap 検出の内部手がかりに留め、
  `ProbeClockMark`/`ProbeTimeContext` の wire 語彙
  には追加しない（coordinator 確定ピン）。

**`SetLoop`/`ClearLoop` の検出は apply 成功時のみ**:
`note_transport_command` は `SetLoop` の range 検証
（非負 start・非空 region）を行い、upstream が拒否
する command は mirror に fold しない。検出を
`matches!(command, SetLoop|ClearLoop)` の command
variant 一致ではなく、**mirror へ実際に noted された
loop command** に限定するため、`note_transport_command`
を `bool`（loop 領域 command を noted したか）返しに
変更し、その戻り値を OR 集約する。検証を通らない
`SetLoop` は apply が起きないため宣言もしない
（scheduler が拒否する apply を discontinuity と
偽装しない）。

**`Seek` command は検出対象外**: session transport
seek は `RoutingEngine::seek` が cursor re-anchor +
`bump_clock_epoch` を自前で行う宣言経路を持つ。
node-level transport command の `Seek` は TIM-03
boundary transport 基盤の別経路であり、probe 配線の
要件は未確定 — coordinator 確定ピンの loop wrap +
`SetLoop`/`ClearLoop` に限定し、spec の Deferred
Design Concerns に記録する。

## 設計判断 2 — accessor は `RoutingControlDispatch` の per-`observe_block` フラグ

```rust
pub struct RoutingControlDispatch {
    // ...existing...
    /// `observe_block` が直近 window で観測した probe 関連
    /// transport 不連続（loop wrap / SetLoop・ClearLoop apply）
    /// — `observe_block` ごとに再計算される検出のみの evidence。
    last_observed_loop_discontinuity: bool,
}

#[must_use]
pub fn observed_loop_discontinuity(&self) -> bool
```

- `observe_block` 先頭で `false` に reset し、
  generation mismatch の早期 `None` return でも
  stale `true` を残さない。`Some` を返した呼出しの
  直後にのみ意味を持つ（`observe_cycle` の span
  evidence とは別経路 — block 集約の transport 証拠）。
- `observe_node_block` の戻り値を
  `(DispatchCycle, bool)` に変え、node ごとの検出を
  `observe_block` が OR 集約してフラグへ書く。
  node 単位の永続フラグは持たない — 「この block で
  起きたか」が契約であり、読み取りは常に直近
  `observe_block` に対して行われる。
- accessor は `&self` の read-only（`last_observed_span`
  先例）。observer→engine の逆行所有はしない —
  accessor は検出のみ、宣言は呼出し側が行う。

## 設計判断 3 — `SessionRoutingEngine::run` は per-block `run_observed(1)` + 宣言

**単一 `run_observed(N)` 呼出しでは正しく宣言できない**:
observe closure は `&mut RoutingEngine` 借用中に走るため
closure 内から `engine_mut().declare_probe_discontinuity`
は呼べず（`inner.bridge` は `run_observed` が占有）、
run 後にまとめて宣言すると wrap を含む block と run
末尾の間の全 block が旧 epoch を名乗る — post-wrap
証拠が pre-wrap epoch を騙る fail-open 窓ができる。

正しい境界は「不連続を観測した block の直後・次の
block の前」。`run_observed(1)` を block ごとに呼び、
各呼出しの observe closure で検出 flag を立て、戻り後に
`declare_probe_discontinuity` する per-block loop に
再構成する。これは `MonitorEngine::run`
（`monitor_engine.rs`）が「`run(1, ...)` 反復で pump
cadence が狂うのを防ぐ」ために確立した先例と同じ
パターン。

**外部契約の等価維持**:

- **PumpPolicy**: `run(N, Every(n))` は index `0..N` で
  `index % n == 0` の block で pump する（呼出しごとに
  index が 0 から再開 — 毎 run の先頭 block が pump
  される）。per-block 呼出しでは `run_observed(1)` の
  index が常に 0 なので、`index % n == 0` を局所 counter
  で再現し pump block にだけ `Every(1)`、それ以外に
  `Every(0)` を渡す。`MonitorEngine` の永続
  `blocks_run` counter は採用しない — あちらは call を
  跨ぐ連続 cadence が契約だが、こちらは call ごとの
  再開が `run_observed` の意味論。
- **PlayReport**: per-block `pcm` を channel ごとに
  concat、`frames` を合算、`crc` は merge 後の pcm に
  対し `run_observed` と同一式（`crc*31 + sample bits`）
  で再計算 — bit 単位で一致。pcm は
  `config.engine.channels`/`block_frames` で事前確保し
  `blocks=0` でも `channels` 個の空 Vec を返す元の
  shape を維持。
- **error 伝播**: ある block の `rt.process` が失敗
  した場合、以降の block は処理されず `Err` を返す
  （元実装と同じ）。その block では observe が走らない
  ため検出 flag も立たず宣言も起きない。既に buffer
  した `observed` cycles は `pending_observe_cycles`
  へ append してから返す（元実装が Err 時も append
  する契約を維持）。
- **`pending_observe_cycles`**: block ごとの merged
  `DispatchCycle` を従来どおり蓄積 —
  `control_drain_observations` の意味は不変。

**宣言の失敗扱い**: `declare_probe_discontinuity` は
`Result<(), ProbeRequestError>` を返すが、`run` の
返り値は `ProcessError` であり型を跨げない。seek の
`let _ = self.probes.bump_clock_epoch()` 先例に倣い
best-effort とし、`CommandChannelFull` 時も broker
epoch は push 前に進むため deck との乖離が
`probe_time_context` で観測可能なまま残る
（fail-closed は epoch advance 自体が保証）。署名変更
や新規エラー記録 field の追加は caller（service/
monitoring — sv#117 占有領域）への波及を伴うため
本 slice では行わず、design/spec に判断を記録する。

## 設計判断 4 — `ui/src/ipc.ts` は union 追加のみ

service 側 `syn.probe.compare`（`OpInfo`・dispatch・
`op_probe_compare`）は slice10 で実装済み。
`OpName` union 末尾へ `"syn.probe.compare"` を追加する
だけで wire 到達性は完成する。`ui/src/store.ts` には
probe 関連の型補助が存在せず、`syn.probe.compare` を
呼ぶ既存/必要な呼出し経路も無いため変更しない
（YAGNI — coordinator 指針どおり）。

## Components and Boundaries

- `routing_control.rs`:
  - `RoutingControlDispatch` に
    `last_observed_loop_discontinuity: bool` field と
    `observed_loop_discontinuity()` accessor を追加。
  - `observe_block`: 先頭で flag reset、node loop で
    `observe_node_block` の検出を OR 集約。
  - `observe_node_block`: 戻り値を
    `(DispatchCycle, bool)` に変更。`walk_window`
    戻り値の `LoopWrap` 点と `note_transport_command`
    の loop-apply 戻り値から検出を計算。
  - `note_transport_command`: `-> bool`（noted した
    loop 領域 command か）に変更。
- `engine.rs` `SessionRoutingEngine::run`: per-block
  `run_observed(1)` loop + PumpPolicy emulation +
  PlayReport merge + 検出時の best-effort 宣言。
- `ui/src/ipc.ts`: `OpName` union に
  `"syn.probe.compare"`。

## Invariants

- 検出は post-window の観測済み状態からのみ読み、
  `boundary_loop_wraps` の pre-window pure read と
  `observe_block` の順序契約（pure read は window の
  `observe_block` より前）を壊さない。
- 宣言は block ごとに高々 1 回（bool flag — 同一
  window の複数 wrap/apply でも epoch cut は 1 つで
  十分。過剰宣言は比較可能性を不必要に削るだけで
  fail-open にはならないが、block 内の全不連続を
  1 cut でカバーするのが正しい粒度）。
- `observe_block` の戻り値（`BTreeMap<ProcessorId,
  DispatchCycle>`）・`observe_cycle`・
  `boundary_*window`・`last_observed_span` の契約は
  不変。
- `SessionRoutingEngine::run` の外部契約（戻り値・
  pump cadence・error 伝播・observe cycle buffering）
  は不変。
- node transport `Seek`/`Play`/`Pause`/`Stop` の
  apply は本 slice の宣言対象外（Seek は session
  transport の別経路が宣言済み、Play/Pause/Stop は
  cursor 連続性を破らない）。
