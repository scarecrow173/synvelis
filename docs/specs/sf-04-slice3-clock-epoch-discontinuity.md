---
id: "SPEC-034UL2whPqBifdr1Jsrkn7"
type: "spec"
status: "approved"
title: "SF-04 slice3 — probe clock epoch discontinuity・capture segment cut・fault/DSP invariance"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
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
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
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
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/designs/sf-04-slice1-semantic-probe-adapter-design.md"
  defers: []
  deferred-by: []
---

# SF-04 slice3 — probe clock epoch discontinuity・capture segment cut・fault/DSP invariance

## What Should Be Built

Issue #107（SF-04 Semantic Probe Adapter）の headless 残件 slice として、
slice 1 の `ProbeSession`/segment ledger と slice 2 の
ControlSession→ControlTrace 配線の上に、cursor-series の clock epoch
不連続（transport seek 等）に対する fail-closed 挙動を実装する:

- `synvelis-project` の `CaptureSegmentEnd` に `ClockEpoch` を追加し、
  capture segment が plan generation 境界に加えて cursor clock-epoch
  変化も跨がないことを契約化する。
- `synvelis-player` の `ProbeSession` に `bump_clock_epoch()`（Ruviform
  `ProbeBroker::bump_epoch` への委譲）と `time_context()` を追加し、
  drain 経路で open segment の clock context と record/status の
  `ClockContextRef` を照合して epoch 変化時に segment を切断する。
- `RoutingEngine::seek` が probe clock epoch を bump し、
  `probe_time_context()` で `ProbeTimeContext`（Ruviform #309）を
  公開する。
- headless fixture で、seek による epoch 進行・segment の
  `ClockEpoch` 切断・暗黙の再 capture 不在・probe mode が master
  PCM を変えないこと（SF-PROBE-02）・bounded fault
  （probe table capacity・rebind 中の command channel full が atomic
  に失敗し次 publish で回復すること）を検証する。

## Why Now

slice 1/2 で semantic 解決・generation-scoped binding・segment
ledger・trace 配線は確立したが、cursor-series の時間不連続に対する
契約が未定義だった。RT frame cursor は seek を跨いでも単調増加する
ため、sample position だけでは pre/post-seek のデータを区別できず、
旧 segment に新 epoch の PCM/status が混入する経路が残っていた。
Ruviform #309 の `ProbeTimeContext`/epoch/clock-mark 機構は既に
pin 済み依存に存在するため、Synvelis 側は「どの不連続で epoch を
進めるか」「segment をどう切るか」の adapter 責務だけを実装すれば
よく、generic clock alignment の再実装は不要。

## Users and Value

- #108 UI 実装者が、seek/loop/device restart を跨ぐ probe 観測が
  単一の連続 span として表示されないことを契約として信頼できる。
- #109 実測/oracle 作業が、epoch 境界をまたぐ計測値の provenance
  （`ClockContextRef`/`ProbeTimeContext`）を独立に照合できる。
- headless fixture が、probe lifecycle（Monitor/Capture）が DSP
  出力を変えないことと bounded fault の fail-closed 性を
  regression として固定できる。

## Scope

### In Scope

- `crates/synvelis-project/src/probe.rs`: `CaptureSegmentEnd::ClockEpoch`
  追加と `ProbeCaptureSegment` doc の境界明文化。
- `crates/synvelis-player/src/probe.rs`: `ProbeSession::bump_clock_epoch`/
  `time_context`、`observe_meta`/`observe_status` の epoch 照合と
  `cut_segment_on_epoch_change`。
- `crates/synvelis-player/src/routing_engine.rs`: `RoutingEngine::seek`
  の epoch bump、`probe_time_context()` accessor。
- headless テスト `crates/synvelis-player/tests/sf04_s3_probe_discontinuity.rs`。
- `docs/` 文書チェーン（spec/design/plan/task/test-spec/impl record）。

### Out of Scope

- `crates/synvelis-app/`・`crates/synvelis-workspace/` の変更
  （slice 2 配線はそのまま消費する）。
- UI 表示（#108）、実測・oracle gate（#109）、実 OS/plugin/RT 観測
  （#114 gate）。
- ruviform 側の probe 本体・clock alignment・pin/rev 変更。
- loop point・device restart 等 seek 以外の不連続点の個別対応
  （epoch bump の呼び出し点は seek のみ — 他の不連続宣言は将来の
  transport 契約拡張に委ねる）。
- `ControlTrace`/`ProbeTraceBridge` の翻訳契約変更。

## Requirements

- capture segment は plan generation 境界と cursor clock-epoch 変化の
  両方を跨がない。epoch 変化で閉じた segment の `ended_by` は
  `ClockEpoch`、`clock_context` は旧 epoch を保持する。
- epoch 変化を示す最初の record/status は旧 segment を閉じる引き金に
  なり、その record 自体は旧 segment に帰属しない。新 epoch の
  chunk/status は open segment が無い限り捨てられ、暗黙の新規
  capture segment は開始しない。
- segment が clock context を持たない（clock 未配線）場合は epoch
  照合を行わず、Unknown provenance を正直に維持する。
- `RoutingEngine::seek` は transport 再 anchor 後に epoch を bump
  する。bump は best-effort — command channel full で deck への
  clock mark 配送が失敗しても broker epoch は進み、乖離は
  `probe_time_context`/record の epoch で観測可能なまま残す
  （silent align しない）。
- probe lifecycle（request/set_mode/capture/stop/remove）が master
  PCM・graph 動作を変えない（SF-PROBE-02）。
- bounded fault は fail-closed: probe table 上限超過は
  `CapacityExceeded` で拒否、rebind 中の channel full は atomic に
  失敗し subscription は `QueuedForBinding` のまま RT install を
  偽装しない。次の publish で全体が再試行され回復する。
- RT 経路（`ProbeDeck`/`ProbeCollector`）の no-alloc/no-lock/no-I/O
  契約を維持する。epoch 照合・segment 切断は非 RT drain 側のみ。

## Acceptance Criteria

- [ ] seek 後の probe record が新しい clock epoch を運び、
  `probe_time_context().epoch` が単調に進む。
- [ ] capture 中の seek で open segment が `ClockEpoch` で閉じ、
  旧 epoch の `clock_context`/`frame_range`/completeness を保持し、
  後続 run でも segment 数が増えない（暗黙再 capture 無し）。
- [ ] 明示的な新規 `probe_capture` が新 epoch の新 segment を
  作り、新しい `capture_session_id` を持つ。
- [ ] Monitor/Capture probe が master PCM と CRC を変えない
  （非 silence fixture で比較）。
- [ ] 33 個目の probe request が `CapacityExceeded` で拒否される。
- [ ] rebind 中の channel full で全 subscription が
  `QueuedForBinding` に留まり（`BoundActive` を偽装しない）、
  次 publish で `BoundActive` に回復する。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `check-headless-deps`（存在する範囲）が green。

## Deferred Design Concerns

- #108 UI 接続 | 本 slice は headless contract/fixture に限定 | #108
  着手時 | UI が無いと epoch 表示の ergonomics は未検証のまま残る。
- #109 実測・oracle（SF-06 cross gate）| 実 plugin/OS/RT 観測は
  #114 gate | gate 通過後 | headless fixture は実計測の正しさを
  証明しない。
- loop point・device restart・transport stop/start 等の他の
  不連続宣言点 | 本 slice は `seek` のみを epoch bump 点とする |
  transport 契約拡張時 | 同じ `bump_clock_epoch` API を再利用
  できるが、宣言点の網羅は別途契約化が必要。
- `SetClockMark` 配送失敗時の deck/broker epoch 乖離の自己修復 |
  best-effort bump + 次回 wire で resync | fault 注入拡充時 |
  現状は乖離が record epoch で観測可能なことを契約とする。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/107>（SF-PROBE-01..09）
- 前提: SF-04 slice1（semantic contract・`ProbeSession`・engine
  probe passthrough）、SF-04 slice2（ControlSession→ControlTrace
  配線）、SF-03（Active SourceMap/Graph Generation）、PC-C3-05
  （RT trace adapter）。
- Ruviform: `ruviform_audio::typed::probe`（`ProbeBroker::
  bump_epoch`/`time_context`/`wire_clock_mark`、`ProbeTimeContext`、
  `RecordMeta`/`ProbeClockMark`/`ClockContextRef`）、Ruviform #308
  （graph swap rebind・`StopAtBoundary`）、#309（clock/epoch/latency
  alignment）。
- 本 spec は Issue #107 のうち slice 3（clock epoch discontinuity・
  capture segmentation・fault/DSP invariance）を対象とし、Issue は
  Open 継続が前提。
