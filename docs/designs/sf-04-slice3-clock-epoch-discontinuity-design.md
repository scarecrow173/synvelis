---
id: "DESIGN-034UL2WkinplQmMD9VPyeX"
type: "design"
status: "approved"
title: "SF-04 slice3 設計 — probe clock epoch discontinuity・capture segment cut"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/sf-04-slice1-semantic-probe-adapter-design.md"
    - "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
  defers: []
  deferred-by: []
---

# SF-04 slice3 設計 — probe clock epoch discontinuity・capture segment cut

## Context

slice 1 の `ProbeSession` は `ProbeCollector` の telemetry/chunk を
drain し、`RecordMeta`/`StatusRecord` の generation 照合で
segment ledger を更新する。slice 2 はその上に session→trace 配線を
載せた。しかし cursor-series の時間軸については 1 つの穴が残って
いた:

- RT frame cursor（`RecordMeta.sample_position`）は処理フレーム数を
  数えるため、transport seek を跨いでも単調増加する。
- seek は「同じ position 軸上の別の時間位置への跳躍」であり、
  pre-seek と post-seek の record を同一 span として扱うと、
  capture segment の `frame_range`/`discontinuities` が実際には
  連続していない区間を連続として記録してしまう。

Ruviform #309 はこの問題を cursor-series の clock epoch で解く
機構を既に提供する: `ProbeBroker` が `ProbeTimeContext`（domain/
epoch/runtime instance/sample rate/provenance/anchors/generation
window）を保持し、`bump_epoch()` で epoch を進めて
`wire_clock_mark()` 経由で deck に `SetClockMark` を送る。以降の
record/status は新 epoch の `ProbeClockMark` を運び、Ruviform 側の
align 分類（`EpochMismatch`/`DiscontinuityInWindow`）が効く。
Synvelis 側の責務は「どの transport 不連続で epoch を進めるか」
「segment ledger が epoch 境界を跨がないこと」の 2 点のみで、
generic clock alignment 自体は Ruviform に委譲する（再実装禁止の
scope 条件どおり）。

## Scope

### In Scope

- `synvelis-project` `probe.rs`: `CaptureSegmentEnd::ClockEpoch`。
- `synvelis-player` `probe.rs`: `ProbeSession::bump_clock_epoch`/
  `time_context`、`observe_meta`/`observe_status` の epoch 照合、
  `cut_segment_on_epoch_change`。
- `synvelis-player` `routing_engine.rs`: `RoutingEngine::seek` の
  epoch bump、`probe_time_context()`。
- headless テスト `sf04_s3_probe_discontinuity.rs`（5 件）。

### Out of Scope

- spec の Out of Scope と同じ（app/workspace 変更・UI・実測・
  ruviform 変更・seek 以外の不連続点・trace 翻訳契約変更）。

## Components and Boundaries

### synvelis-project — `CaptureSegmentEnd::ClockEpoch`

```rust
pub enum CaptureSegmentEnd {
    Requested,
    BudgetExhausted,
    GenerationBoundary,
    RebindDropped,
    Removed,
    /// The cursor-series clock epoch changed (transport
    /// discontinuity such as a seek) — the segment never spans it.
    ClockEpoch,
    Unknown,
}
```

`ProbeCaptureSegment` の doc を「generation 境界 **または** cursor
clock-epoch 変化を跨がない」に更新。enum は既に
`#[non_exhaustive]` 相当の契約語彙であり、variant 追加は additive。

### synvelis-player — `ProbeSession` epoch API

```rust
/// Declares a cursor-series discontinuity (transport seek) —
/// bumps the clock epoch so records after the boundary classify
/// as `EpochMismatch`/`DiscontinuityInWindow` instead of silently
/// aligning with pre-discontinuity spans (Ruviform #309).
pub fn bump_clock_epoch(&mut self) -> Result<(), ProbeRequestError>;

/// The probe cursor-series time context (Ruviform #309).
pub fn time_context(&self) -> &ProbeTimeContext;
```

`bump_clock_epoch` は `self.broker.bump_epoch()` への薄い委譲。
Ruviform `ProbeBroker::bump_epoch` は **先に `self.time.epoch` を
increment してから `wire_clock_mark()` を push** するため、
command channel full で `ProbeError` が返っても broker epoch は
進んだままになる。この非対称は意図的に維持する: broker/deck の
epoch 乖離は `probe_time_context().epoch` と record の
`ClockContextRef.epoch` の差として観測可能であり、Synvelis 側で
「失敗したら epoch を巻き戻す」補償を入れると silent align に
なり得るため行わない。

### synvelis-player — drain 側の epoch 照合（lazy cut）

segment の epoch 切断は **eager ではなく lazy** に行う。設計判断:

- `bump_clock_epoch` 呼出時点では RT 側にまだ旧 epoch の chunk が
  残っている可能性がある（command channel の FIFO — bump の
  `SetClockMark` が適用される前に処理された block の chunk は旧
  epoch を運ぶ）。eager に open segment を閉じると、それらの正当な
  旧 epoch chunk が帰属先を失う。
- そこで cut は「新 epoch の evidence が drain された瞬間」に
  行う: `observe_meta`（`Captured` chunk の `RecordMeta`）と
  `observe_status`（closing kind 以外の `StatusRecord`）が
  `clock_ref(record.clock)` を得た時点で、open segment の保持する
  `clock` と比較し、不一致なら旧 segment を `ClockEpoch` で閉じる。

```rust
fn cut_segment_on_epoch_change(
    open: &mut BTreeMap<ProbeId, OpenSegment>,
    segments: &mut Vec<ProbeCaptureSegment>,
    probe: ProbeId,
    clock: ClockContextRef,
)
```

- open segment が無い → no-op（新 epoch chunk は捨てられる。
  segment ledger は `observe_meta` の `open.get` で既に帰属を
  要求するため、暗黙の新 segment は構造上発生しない）。
- `segment.clock == None`（clock 未配線/Unknown provenance）→
  no-op。wiring 前の record は epoch を持たず、照合不能を理由に
  segment を切らない — Unknown を推測で塗り潰さない。
- `old == clock` → no-op（同一 epoch の record は通常経路へ）。
- それ以外 → `open.remove` + `close_segment(segment,
  Some(ClockEpoch), Some(clock))`。`close_segment` の
  `clock_context` は `segment.clock.or(status_clock)` なので、
  旧 epoch の `clock_context` が保持される（`status_clock` は
  `segment.clock` が `None` の場合の fallback であり、本経路では
  常に `Some(old)` が優先される）。

`observe_meta` では cut 後に `open.contains_key` を再確認し、
cut された場合は **triggering record を return で捨てる** —
post-epoch の `sample_position`/`frames` を旧 segment の
`frame_range` に fold しない。`observe_status` では cut を gap/
stop の bookkeeping **より先**に行い、post-epoch の
`Unobserved`/`Lost*` が旧 segment の `discontinuities` に
記録されるのを防ぐ。closing kind
（`CaptureStopped`/`Removed`/`RebindDropped`）は自身の end reason
を持つため epoch cut の対象外とし、通常の close 経路に任せる。

この設計により:

- pre-epoch の chunk はすべて旧 segment に正しく帰属する
  （lazy cut なので cut 前に drain された分は影響を受けない）。
- epoch を跨ぐ segment は存在しない（`ended_by == ClockEpoch` で
  閉じ、後続は新 segment のみ）。
- 新 epoch の capture は **明示的な `probe_capture` のみ**が開始
  する — RT 側の probe mode は seek で変わらず capture budget も
  継続するが、post-cut chunk は open segment が無いため捨てられ、
  segment ledger は増えない。

### synvelis-player — `RoutingEngine::seek`

```rust
pub fn seek(&mut self, engine_frame: u64) { ... }
```

既存の transport re-anchor（cursor 更新・handle seek 通知・
drain_nonces settle pass）の **後** に
`let _ = self.probes.bump_clock_epoch();` を追加。順序の根拠:

- drain pass が先に走るため通常経路では command channel は空で、
  `SetClockMark` はほぼ確実に積まれる。
- re-anchor 後に bump することで、「seek が起きた」という宣言と
  epoch 進行が同一の engine mutation 内で不可分になる。
- 失敗（channel full）は `let _ =` で握り潰すが silent ではない:
  broker epoch は進んでいるため `probe_time_context().epoch` と
  deck mark が乖離し、乖離自体が観測証拠になる（`Err` を呼び出し
  元に伝える設計も検討したが、`seek` は transport 操作であり
  probe clock の配送失敗で seek 自体を失敗させるべきではない —
  乖離の可観測性を優先）。

`probe_time_context()` は `ProbeSession::time_context()` の
passthrough で、`&ProbeTimeContext`（domain/epoch/runtime
instance/sample rate/provenance/anchors/generation window）を返す。
UI/診断/将来の align 検証が読む公開面。

### RT 側の非変更

`ProbeDeck`/`ProbeCollector`/`ProbeBroker` の RT 面・command
channel・chunk ring には一切触れない。epoch bump は非 RT の
command push、cut は非 RT の drain bookkeeping であり、RT
callback の no-alloc/no-lock/no-I/O 契約は不変。

## Data and Control Flow

```
transport seek ──► RoutingEngine::seek(engine_frame)
                     │ cursor/handle re-anchor + drain settle
                     ▼
                 probes.bump_clock_epoch()
                     │ ProbeBroker::bump_epoch
                     │   time.epoch += 1 → wire_clock_mark()
                     │   → command channel: SetClockMark(epoch+1)
                     ▼
RT block ──► ProbeDeck applies SetClockMark ──► records carry
        new ProbeClockMark{epoch+1}
                     │
                     ▼
cycle ──► ProbeSession drain (collector.telemetry/chunks)
           │ observe_status / observe_meta:
           │   clock_ref(record) != segment.clock
           │     → cut_segment_on_epoch_change
           │     → close_segment(ended_by=ClockEpoch,
           │                    clock_context=old epoch)
           │   triggering record discarded
           │   post-epoch chunks: open.get → None → drop
           ▼
segment ledger: [..., segment(ended_by=ClockEpoch,
               clock_context=epoch N)]
explicit probe_capture ──► new OpenSegment(epoch N+1 records)
               → close on BudgetExhausted/next boundary
```

## Non-Functional Constraints

- RT 経路不変（上記）。
- `bump_clock_epoch`/`time_context`/`probe_time_context` は非 RT
  orchestration のみ。
- segment ledger は従来どおり有界（publish あたりの segment 数は
  capture request 数に比例、chunk 蓄積は `max_frames` 予算で有界）。
- 公開 enum/struct の追加は additive。`unsafe` なし。

## Risks and Trade-offs

- **lazy cut の帰属境界**: cut は「最初の新 epoch evidence」で
  起きるため、epoch bump 後・`SetClockMark` 適用前に処理された
  block の record（旧 epoch）は正しく旧 segment に入る。逆に
  bump が channel full で失敗した場合、deck は旧 epoch のまま
  record を出し続け、segment は切れない — これは「不連続が
  証拠として届いていない」状態であり、切らないことが正直な
  挙動（epoch が進んでいない record を新 epoch と見なさない）。
- **`ClockContextRef` の同一性比較**: `cut_segment_on_epoch_change`
  は `ClockContextRef` 全体の等価で判定する。epoch だけでなく
  domain/runtime instance/sample rate 等の他成分が変わった場合も
  切断対象になる — cursor-series の同一性が変わった以上、同一
  span と見なさないのが契約として正しい。
- **status 経路の epoch cut**: closing kind を除く全 status が
  対象。`Installed`/`ModeChanged` 等の lifecycle status も epoch
  を運ぶため、capture 中の seek で最初に届くのが status でも
  segment は正しく切れる。
- **seek 以外の不連続**: loop・device restart・transport stop は
  本 slice では epoch bump を呼ばない。これらが実装された際に
  bump を追加するだけで同じ cut 経路が効く設計にしてあるが、
  「どの操作が不連続か」の宣言責務は transport 契約側に残る。

## Deferred Design Concerns

- `SetClockMark` 配送失敗時の deck/broker 乖離の能動的修復 |
  best-effort + 観測可能性で受容 | fault 注入拡充時 | 現状は
  `probe_time_context` と record epoch の差で検出可能。
- epoch 境界をまたぐ `ProbeTimeContext` の anchor/align 検証 |
  Ruviform #309 の align 分類に委譲 | #109 oracle | Synvelis 側は
  epoch の宣言と segment 切断のみを担う。

## Verification Notes

- `crates/synvelis-player/tests/sf04_s3_probe_discontinuity.rs`:
  - `sf04_s3_seek_bumps_cursor_clock_epoch_on_records`: Monitor
    probe の metric record が seek 前後で epoch0/epoch1 を運び、
    `sample_position` は単調増加のまま（epoch のみが境界証拠）。
  - `sf04_s3_capture_segment_cuts_at_clock_epoch`: capture 中の
    seek で segment が `ClockEpoch` 終了・旧 epoch
    `clock_context`/`frame_range` 保持・後続 run で segment 数不変
    （暗黙再 capture 無し）・明示 `probe_capture` が新
    `capture_session_id`/新 epoch の第 2 segment を `BudgetExhausted`
    で閉じる。
  - `sf04_s3_probe_modes_leave_master_output_identical`: 同一
    clip project を probe 無し/Monitor+Capture で run し master
    PCM・CRC が bit 一致（非 silence fixture）。
  - `sf04_s3_probe_table_capacity_is_bounded`: 32 個 install 後の
    33 個目が `ProbeError::CapacityExceeded`。
  - `sf04_s3_rebind_channel_full_stays_atomic_and_recovers`:
    channel を意図的に満杯にして publish すると rebind が atomic
    に失敗し全 subscription が `QueuedForBinding` のまま
    （`BoundActive` を偽装しない）、次 publish で全件
    `BoundActive{generation=3}` に回復。
- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `check-headless-deps` / `test-msrv`。
