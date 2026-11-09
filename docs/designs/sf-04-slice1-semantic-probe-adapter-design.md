---
id: "DESIGN-034Um5591oRMfPg7cVthyU"
type: "design"
status: "approved"
title: "SF-04 slice1 設計 — Semantic Probe Adapter（project contract + player runtime adapter）"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice1 設計 — Semantic Probe Adapter（project contract + player runtime adapter）

## Context

SF-03 slice1..5 で semantic routing → typed graph lowering・generation-scoped
`RoutingSourceMap`・`RoutingEngine`（prepare/publish/reconcile）が確立済み。
Ruviform pin `0f5b5737` は `ruviform_audio::typed::probe`（`ProbeBroker` /
`ProbeDeck` / `ProbeCollector` / `pair()`）、#308 の staged rebind
（`RebindBegin/Stage/Commit` + `RebindReceipt` + `CapturePolicy`）、#309 の
clock identity（`ProbeClockMark` / `ProbeTimeContext` /
`align_observation_points` / `ObservationCoverage`）を提供する。

本 slice はその上に Synvelis 側の semantic 層を載せる: 安定した semantic
point（entity + boundary + action position）を generation-scoped SourceMap と
published `GraphSpec` に解決し、runtime `ProbePoint{node, port}` へ bind する
adapter を project（契約）/player（runtime）の 2 層で定義する。
ruviform の汎用機構（capture/rebind/clock）は再実装しない。

## Scope

### In Scope

- `synvelis-project/src/probe.rs`: semantic contract 型と pure 解決関数
  `resolve_probe_point(project, source_map, spec, point)`。
- `synvelis-player/src/probe.rs`: `ProbeSession`（broker+collector+binding
  ledger+capture segment ledger）と `RoutingEngine` への配線。
- headless テスト `sf04_*.rs`。

### Out of Scope

- `synvelis-app`/`synvelis-workspace` への配線、UI、実 RT 計測、複数 window
  ownership、capture PCM の永続化、probe capture の Recording Writer 統合。
- ruviform 側の変更。

## Components and Boundaries

### synvelis-project（契約層・永続化可能な semantic 意図のみ）

- `SignalBoundary`: `PreInsert | PostInsert | PreFader | PostFader | Input |
  Output | Sidechain`。DAW semantic 概念であり ruviform `TapKind` とは
  1:1 対応しない — 解決結果として runtime `ProbePoint` が決まるだけ。
- `SemanticSignalEntityRef`: `Track(TrackId) | Bus(BusId) | Strip(StripId) |
  Processor(ProcessorId) | Send(SendId)`。
- `SemanticProbePoint { entity, boundary, port: Option<SemanticPortId> }`:
  `port` は sidechain 等の曖昧さ解消と明示 port 指定に使う optional disambiguator。
- `ProjectSessionId(u64)` / `CaptureSessionId(u64)` / `RuntimeProbeId(u64)`:
  session/broker 寿命の ID。`RuntimeProbeId` は ruviform `ProbeId` の
  semantic mirror であり **Project には保存しない**（binding は player 層の
  runtime 状態）。
- `ProbeMode { Off | Monitor | Capture }`: project 側の要求 mode。
  ruviform `ProbeMode` とは player 層でのみ相互変換する。
- `SubscriptionKind { Metrics | Pcm }`、`ProbeSubscriptionKey { session_id,
  point, kind }`。
- `ProbeResolveState`: `Requested | Unresolved | Unavailable | Unsupported |
  QueuedForBinding | PendingRtInstall | BoundActive | StaleGeneration |
  PartiallyRebound | Lost | Stopped`。
- `ProbeStatusKind`/`ProbeStopReason`/`ProbeUnobservedReason`/
  `ProbeStatusReceipt`: ruviform `StatusKind` 系の semantic mirror
  （project は ruviform-audio に依存できないため）。
- `SemanticProbeBinding`: point + project_revision + runtime_instance +
  active_plan_generation + source_map_fingerprint + runtime_probe_id +
  requested/confirmed mode + state + last_status。
- `ProbeCaptureSegment`（`recording_data::CaptureSegment` とは別概念・
  probe 観測区間）: capture_session_id + segment_index + point +
  active_plan_generation + clock_context + frame_range + discontinuities +
  completeness + end reason。
- 補助型: `ClockContextRef { domain, epoch }`（media-core ID）、
  `ProbeFrameInterval`、`ObservationGap`/`ObservationGapKind`、
  `CaptureCompleteness`。

### Boundary → runtime port 解決規則（`resolve_probe_point`）

解決は `map.ports`（semantic port → `MappingResolution<GraphPortRef>`）、
`map.send_taps`、`map.semantic_nodes`、published `GraphSpec` のみを使う
pure 関数。entity → subject の写像:

- `Track(t)` → `routing.strip_of_track(t)` の strip（無ければ `Unresolved`）。
- `Bus(b)` → `routing.buses[b].strip`。
- `Strip(s)` → そのまま。
- `Processor(p)` → `routing.processors[p]`。
- `Send(s)` → `routing.sends[s]`。

strip subject の boundary 規則（fader node = `semantic_nodes[Strip]`、
post_out = `ports[main_out semantic port]` の解決先、pre_out = 同 node の
残り Main output port、fader_in = 同 node の唯一の input port）:

| boundary | 解決先 |
| --- | --- |
| `Input` / `PreInsert` | semantic main-in port → `ports_map`（= entry: chain head input または fader in。lowering に pre-strip 独立段は無く両者は同一点） |
| `PostInsert` | fader node の input port（post-chain/pre-fader 信号点） |
| `PreFader` | fader node の post_out 以外の Main output port（= pre_out。pre-fader send tap と同一点） |
| `Output` / `PostFader` | semantic main-out port → `ports_map`（= fader post-out） |
| `Sidechain` | `Unsupported`（strip に sidechain port は存在しない） |

processor subject:

| boundary | 解決先 |
| --- | --- |
| `Input` / `PreInsert` | main-in semantic port → `ports_map` |
| `Output` / `PostInsert` | main-out semantic port → `ports_map` |
| `Sidechain` | `point.port` 指定の sidechain input port、または sidechain port が 1 つだけならそれ。複数で未指定は `Unresolved` |
| `PreFader` / `PostFader` | `Unsupported`（processor に fader 概念は無い） |

send subject:

| boundary | 解決先 |
| --- | --- |
| `Input` | `send_taps[send]`（source strip 上の設定済み tap 点） |
| `Output` | send connection の destination semantic port → `ports_map`（destination の入力合流点。send 単独分離はこの lowering に無く、観測点は共有 port であることを binding が示す） |
| その他 | `Unsupported` |

`ports_map` の `MappingResolution` はそのまま probe resolution へ写像する:
`Resolved(PortRef)` → `Resolved`、`Unavailable(_)` → `Unavailable`、
`Unsupported(_)` → `Unsupported`、`NotLowered(_)` → `Unavailable`
（mute/disabled で runtime object が無い = 物理 slot 不在。理由の詳細は
lowering 側の SourceMap が保持する）。entity/port 参照自体が無い場合は
`Unresolved`。明示 `point.port` は entity 所有・domain Audio・boundary の
方向と整合することを検証し、不整合は `Unresolved`。

`ProbePointResolution`（project 層の解決結果）:
`Resolved(GraphPortRef) | Unresolved | Unavailable | Unsupported |
NotLowered(NotLoweredReason)` — `NotLowered` は binding state では
`Unavailable` に写像するが、解決関数の戻り値として理由を保持する。

### synvelis-player（runtime adapter 層）

`ProbeSession` — `probe::pair(&ProbeConfig)` の broker/collector を所有し、
deck は `RoutingEngine::new` で `rt.set_probe_deck()` する。

- `request(runtime, spec, project, map, key, mode)`:
  冪等（同 key は既存 binding を返す）。`resolve_probe_point` →
  `Resolved` なら `broker.install(runtime, spec, ProbePoint{node,port}, mode)`。
  解決失敗系は broker を呼ばず binding を `Unresolved`/`Unavailable`/
  `Unsupported` で記録（fail-closed の正直な報告）。install の
  `PointUnavailable{availability}` も同様に state へ写像。
  `CapacityExceeded`/`CommandChannelFull` は `ProbeRequestError` として返す。
- `set_mode` / `capture(max_frames)` / `stop_capture` / `remove`:
  binding → `RuntimeProbeId` → broker command。`requested_mode` を更新し、
  RT 確認は status drain まで `confirmed_mode` に反映しない
  （Requested/Queued ≠ RT-installed）。
- `on_publish(runtime, spec)`: `broker.on_publish` → `RebindReceipt`。
  `bound` → `active_plan_generation` 更新 + `PendingRtInstall`。
  `unbound` → `StaleGeneration`（RT の `RebindDropped` status で `Lost`）。
  `capture_stopped` の segment close は RT status
  （`CaptureStopped{GenerationBoundary}`、sample_position 付き）で行う。
  `CommandChannelFull` は live bindings が旧 generation guard で `Unbound`
  → `StaleGeneration` へ落ちるため握り潰さず binding 状態に委ねる。
  `CapturePolicy` は default `StopAtBoundary` のまま（auto-recapture しない）。
- `drain(f)`: `collector.drain` を wrap し、各 `ProbeEvent` で binding
  ledger と open segment を更新してから caller の `f` へ転送する
  （split borrow で `collector` と ledger を分離）。
  - `Status`: `last_status` 更新 + state 遷移。`Installed` は
    `s.generation == binding.active_plan_generation` のときのみ
    `BoundActive`（不一致は `StaleGeneration`）。`ModeChanged` →
    `confirmed_mode`。`CaptureStarted` → `confirmed_mode=Capture` +
    segment open 確認。`CaptureStopped` → `confirmed_mode=Monitor` +
    segment close（reason を end marker に記録）。`Unbound` →
    `StaleGeneration`。`RebindDropped` → `Lost`。`Removed` → `Stopped`。
    `Unobserved`/`LostBlocks`/`LostRecords` → open segment に
    `ObservationGap` を記録。
  - `Captured`/`Metric`: open segment の frame_range を
    `[sample_position, +frames)` で拡張（`meta.generation` が segment の
    generation と一致する場合のみ — stale record は捨てる）。
- segment lifecycle: `capture()` 成功で `CaptureSessionId` を mint し
  `SegmentAccum` を open。`CaptureStopped`/`RebindDropped`/`Removed`
  status で close して `ProbeCaptureSegment` として `segments` に push。
  completeness: gap 無しで data 有り → `Complete`、gap 有り →
  `Incomplete`、data 無し → `Unknown`。

### RoutingEngine 配線（routing_engine.rs のみ）

- field 追加: `probes: ProbeSession`、`published_spec: Option<GraphSpec>`。
- `new()`: `probe::pair(&ProbeConfig{ chunk_frames: engine.block_frames.max(1), ..Default::default() })`
  → `rt.set_probe_deck(parts.deck)`、`ProbeSession::from_parts` で broker/
  collector を保持。`time_context_mut().sample_rate` を engine の値で設定し
  `wire_clock_mark()` を試行（失敗時は `UNKNOWN` mark のまま — fail-closed）。
- `publish()` 成功 arm: `graph.publish` 後に `probes.on_publish(&graph, &spec)`、
  `published_spec`/`source_map` を更新。spec は `prepared.spec` を move。
- public passthrough: `probe_request` / `probe_set_mode` / `probe_capture` /
  `probe_stop_capture` / `probe_remove` / `probe_drain` / `probe_bindings` /
  `probe_segments`。request 系は `source_map`/`published_spec` 不在時に
  `ProbeRequestError::NoActivePlan`。

## Data and Control Flow

1. Caller（将来の app/session 層）が `SemanticProbePoint` を構築し
   `engine.probe_request(project, key, mode)` を呼ぶ。
2. Engine は active `source_map` + `published_spec` + `graph`/`rt` を
   `ProbeSession::request` に渡す。
3. `resolve_probe_point` が semantic point を `GraphPortRef` に解決
   （fail-closed）。`broker.install` が published plan に対し物理 slot を
   解決し `Bind` command を enqueue → binding state `QueuedForBinding`。
4. RT callback で deck が command を適用 → telemetry に `Installed` 等の
   status。`probe_drain` で `BoundActive`/`confirmed_mode` が確定する。
5. `publish`（generation 交代）→ `broker.on_publish` が staged rebind を
   enqueue → `RebindReceipt` で binding generation/state を更新、
   capture は `StopAtBoundary` で segment cut。

## Non-Functional Constraints

- RT 経路（`ProbeDeck`）は ruviform 提供の noalloc/no-lock/no-I/O/bounded
  実装をそのまま使う。Synvelis 側は deck を install するのみで RT 側に
  触れない。
- 非 RT 側（`ProbeSession`）の bookkeeping は `BTreeMap`/`Vec` の bounded
  構造（probe 数 ≤ `max_probes`、segment 数 ≤ capture 回数）。
- probe mode の既定は `Off`（binding 未作成 = probe 無し）。
- `Cargo.lock` pin は `0f5b5737` に統一（`check-ruviform-rev` green）。

## Risks and Trade-offs

- `Input`/`PreInsert`・`Output`/`PostFader` はこの lowering では同一点に
  解決される — 別境界として扱いたい将来の要件が出たら lowering 側の
  port 追加が必要（design に明記済み、silent な同一視ではなく規則として固定）。
- `Send+Output` は destination の共有入力合流点を観測する（send 単独の
  信号分離点は lowering に存在しない）。観測点の意味は binding の
  resolved `ProbePoint` が正直に示す。
- `NotLowered`（mute/disabled）は binding state `Unavailable` に写像 —
  一時的状態であることは SourceMap 側が保持し、probe 層は「この build で
  物理 slot が無い」という事実のみを報告する。
- capture PCM chunk は adapter が frame_range のみ記録し PCM 自体は保持
  しない（slice 1 は metadata/segment 契約まで。永続化は後続 slice）。

## Deferred Design Concerns

- app/session 層への配線・複数 window/broker 集約・capture ownership/ACL
  | 本 slice は headless project/player のみ | integration slice | session
  境界が確定してから接続する。
- `align_observation_points`/`ObservationCoverage` を使った clock/format
  比較診断 | #309 契約は pin 済みだが消費側は計測 slice | #109 | segment
  metadata 契約を先に固定する。
- capture PCM の bounded storage/retention・Recording Writer 統合
  | SF-PROBE-06 | 計測/録音統合 slice | 保持契約を分離してから実装する。

## Verification Notes

- [ ] `sf04_project_probe.rs`: 解決規則（strip/processor/send × boundary）、
  Unavailable/Unsupported/Unresolved の fail-closed、明示 port 検証。
- [ ] `sf04_probe_adapter.rs`: request→QueuedForBinding→drain で
  BoundActive、generation 不一致 status の StaleGeneration、publish での
  segment cut（`RebindReceipt.capture_stopped` + `CaptureStopped` status）、
  auto-recapture 無し、remove→Stopped。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `check-ruviform-rev` green。
