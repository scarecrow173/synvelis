---
id: "DESIGN-034UunDPfVm34dBXKZHHG0"
type: "design"
status: "approved"
title: "SF-04 slice2 設計 — ControlSession probe observation → ControlTrace 配線"
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
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/sf-04-slice1-semantic-probe-adapter-design.md"
    - "docs/designs/pc-c3-05-rt-trace-adapter-design.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
  defers: []
  deferred-by: []
---

# SF-04 slice2 設計 — ControlSession probe observation → ControlTrace 配線

## Context

slice 1 で確立した経路の終端が未接続:

- `RoutingEngine::probe_*`（slice 1）— semantic request/lifecycle/drain。
- `DispatchTraceAdapter::record_probe`/`trace`/`traces`（PC-C3-05）—
  `ControlTrace.observed` slot への証拠記録。adapter は **node ごとの
  `ParamDispatch` を所有**する per-node オブジェクト。
- `ControlSession`（SF-03 slice5）— session-scoped UI/control/
  recording/observation 状態。`ProjectSession::control_mut()` 経由。
  `EnginePort`（`session.rs`）が「workspace が engine を所有しない」
  port パターンの先例。

本 slice はこの 3 者を port 契約で接続する。`synvelis-workspace` は
`synvelis-player` に依存できない（依存方向:
recording-poc → synvelis-recording → synvelis-project、player →
project、workspace → project）ため、port trait は両者が依存する
`synvelis-project` に置く。これは project が既に永続化ポートを所有
する層であることとも整合する。

## Scope

### In Scope

- `synvelis-project/src/probe_trace.rs`: `ProbeTracePort` trait、
  `ProbeObservationUpdate`、`ProbePortError`。
- `synvelis-player/src/probe_trace.rs`: `ProbeTraceBridge`（port の
  参照実装）と `ProbeEvent` → `ProbeObservation` translation。
- `synvelis-workspace`: `ControlSession` への probe-trace state + API
  （`probe_request`/`probe_set_mode`/`probe_capture`/
  `probe_stop_capture`/`probe_remove`/`probe_drain`/`probe_bindings`/
  `probe_segments`/`probe_trace`/`probe_traces`/
  `probe_observed_freshness`）。
- headless テスト 2 本。

### Out of Scope

- `synvelis-app`/`synvelis-recording`、UI、実測、ruviform 変更、
  `ControlTrace`/adapter 公開契約の破壊的変更、legacy
  `ControlObservation`/`feed_probe` 経路。

## Components and Boundaries

### synvelis-project — `probe_trace` module（port 契約）

`ProbeObservationPort` ではなく **`ProbeTracePort`** と名付ける —
probe lifecycle と trace 記録の双方を担う session 向け境界である
ことを示す。

```rust
pub trait ProbeTracePort {
    // -- lifecycle（非 RT orchestration）--
    fn probe_request(&mut self, project: &Project,
                     key: ProbeSubscriptionKey, mode: ProbeMode)
        -> Result<(), ProbePortError>;
    fn probe_set_mode(&mut self, key: ProbeSubscriptionKey, mode: ProbeMode)
        -> Result<(), ProbePortError>;
    fn probe_capture(&mut self, key: ProbeSubscriptionKey, max_frames: u64)
        -> Result<(), ProbePortError>;
    fn probe_stop_capture(&mut self, key: ProbeSubscriptionKey)
        -> Result<(), ProbePortError>;
    fn probe_remove(&mut self, key: ProbeSubscriptionKey)
        -> Result<(), ProbePortError>;

    /// 唯一の drain 経路。contract-level update のみを返す
    /// （runtime ProbeId・ruviform 型は越境しない）。
    fn probe_drain(&mut self) -> Vec<ProbeObservationUpdate>;

    // -- state reads --
    fn probe_bindings(&self)
        -> &BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding>;
    fn probe_segments(&self) -> &[ProbeCaptureSegment];

    // -- trace channel --
    /// observed slot への証拠記録。target に trace route が無い
    /// 場合 false（silent drop ではなく呼び出し側が観測できる）。
    fn record_probe(&mut self, target: ParameterTargetId,
                    observation: ProbeObservation) -> bool;
    fn trace(&self, target: ParameterTargetId) -> Option<&ControlTrace>;
    fn traces(&self) -> Vec<(ParameterTargetId, ControlTrace)>;
}
```

- `ProbeObservationUpdate { key: ProbeSubscriptionKey,
  observation: ProbeObservation }` — drain 1 event あたり高々 1 件。
  `ProbeObservation`（trace.rs の既存 enum）を再利用し、別語彙を
  作らない。
- `ProbePortError`（`#[non_exhaustive]`）: `UnknownKey | NotBound |
  AlreadyCapturing | ProjectMismatch | NoActivePlan | Failed(String)`。
  player の `ProbeRequestError` を写像（`Broker(ProbeError)` は
  project が ruviform-audio に依存できないため `Failed` に畳む）。
- trait は object-safe。`probe_drain` が `Vec` を返すのは非 RT
  orchestration 経路であり、telemetry channel 容量で有界。

### synvelis-player — `ProbeTraceBridge`（port 参照実装）

```rust
pub struct ProbeTraceBridge {
    engine: RoutingEngine,
    /// node → その node の ParamDispatch を所有する trace adapter。
    adapters: BTreeMap<NodeId, DispatchTraceAdapter>,
    /// ParameterTargetId → 所有 adapter の node。
    /// composition root が bind 時に登録する（推測しない）。
    routes: BTreeMap<ParameterTargetId, NodeId>,
}
```

- `new(engine, adapters)` + `route_target(target, node)` /
  `unroute_target(target)` / `adapter_mut(node)` 等の additive API。
- lifecycle 5 メソッドは `engine.probe_*` へそのまま委譲し、
  `ProbeRequestError` → `ProbePortError` を写像。
- `probe_bindings`/`probe_segments` は engine の ledger をそのまま
  返す。
- `record_probe(target, obs)`: `routes[target]` → `adapters[node]`
  → `DispatchTraceAdapter::record_probe`。route 無しは `false`。
- `trace(target)`/`traces()`: routes 経由で所有 adapter を引き、
  `traces()` は全 adapter の trace を merge して返す。

#### `probe_drain` の translation（fail-closed）

1. drain 前に `engine.probe_bindings()` から
   `ProbeId → (key, active_plan_generation)` の snapshot を作る
   （`&mut drain` と immutable borrow が衝突するため事前取得。
   `active_plan_generation` は publish 時のみ変化し drain 中は不変）。
2. `engine.probe_drain(|event| …)` 内で各 `ProbeEvent` を翻訳:

   - `Metric(MetricRecord)`:
     - `meta.probe` が snapshot に無い → skip（未知/除去済み probe）。
     - `meta.generation != binding.active_plan_generation` →
       **skip（stale-generation reject — trace に記録しない）**。
     - `sample_position` が `i64` に収まらない → skip（stamp を
       正直に作れない）。
     - stamp: `MediaTime::new(position, TimeBase::sample_rate(
       meta.sample_rate))`。`meta.sample_rate == 0` または
       `TimeBase` 構築失敗 → skip。
     - provenance: `clock_ref(meta.clock)` が `Some` → `Derived`、
       `None`（clock 未配線）→ `Estimated`（Current に到達しない）。
     - value: `levels[0..n]` の max `peak`。全 channel が
       `peak == 0.0 && rms == 0.0` → `MeasuredValue::Silence`。
       それ以外 → `MeasuredValue::Signal(ParamValue::new(peak))`
       （`ParamValue::new` は finite を [0,1] に clamp — >0 dBFS の
       飽和は normalized 表示値として正直に丸められる。
       非 finite は `Err` → skip）。`n == 0` → skip。
     - → `ProbeObservation::Measuring { stamp, value }`。
   - `Status(StatusRecord)`:
     - `meta.probe` → key の解決は Metric と同じ snapshot。
     - kind 写像（観測 slot に関係するものだけ update を出す）:
       - `LostBlocks` → `Lost { count: counts.lost_blocks }`
       - `LostRecords` → `Lost { count: counts.lost_records }`
         （`counts` は drain 前の `engine.probe_counts()`…ではなく
         `ProbeSession::counts()` snapshot — cumulative 値である
         ことを doc に明記）
       - `Removed` → `Missing`
       - `ModeChanged { to: RtProbeMode::Off }` → `Disabled`
       - `RebindDropped` → `Disabled`（subscription は残るが観測は
         次の publish rebind まで停止 — binding 生存・観測停止を
         `Disabled` で表す。`Missing` ではない: contract binding は
         存在する）
       - `Installed`/`ModeChanged{Monitor|Capture}`/
         `CaptureStarted`/`CaptureStopped`/`Unbound`/`Unobserved`/
         `Unknown` → update 無し（transient/per-block の状態は
         binding ledger と segment gap が既に担う。slot は最後の
         `Measuring` を保持し freshness が age で Stale に落とす）
     - `Captured(ProbeChunk)` → update 無し（PCM 証拠は segment
       ledger の責務。`MeasuredValue` は PCM を運べない）。

   generation 照合は Metric のみに適用する: status は binding ledger
   （slice 1 の `observe_status` が generation guard 済み）を通る
   lifecycle 証拠であり、slot 更新は「今この channel が何を報告
   できるか」の写像であって stale data の主張ではない。

### synvelis-workspace — `ControlSession` probe-trace state

新規 module `src/probe_trace.rs`（`ProbeTraceSession` struct）+
`ControlSession` への薄い delegate。`control.rs` の private field
規約を守るため、state は `pub(crate)` field として `ControlSession`
に保持し、ロジックは `probe_trace.rs` の `ProbeTraceSession` が持つ
（`ControlSession` の他 field には触れない）。

```rust
pub struct ProbeTraceSession {
    /// この ControlSession が mint した session id — subscription
    /// key の名前空間。他 session/engine と衝突しない。
    session_id: ProjectSessionId,
    /// subscription key → この session が観測を trace に流す
    /// parameter target 群。1 key が複数 target を養う
    /// （fan-out）、1 target が複数 key を持ちうる。
    targets: BTreeMap<ProbeSubscriptionKey, BTreeSet<ParameterTargetId>>,
}
```

`session_id` は process 内 atomic counter で mint（`ProjectSessionId`
は session-scoped runtime id であり永続化しない）。

#### API（`impl ControlSession` in probe_trace.rs、port は per-call）

port を **per-call `&mut dyn ProbeTracePort`** で受ける設計を採る
理由: probe source（`RoutingEngine` 内 `ProbeSession`）と trace
drain owner（node 別 `DispatchTraceAdapter`）は同じ engine/adapter
群を app が別途駆動する共有資源であり、session が 'static
`Box<dyn>` を所有すると所有権が衝突する。`control_with(f, project)`
と同じ「呼び出しごとに文脈を渡す」既存パターンに一致する。
「drain 単一 owner」は `ControlSession::probe_drain` が session
唯一の drain API であることで表現する（port の `probe_drain` を
呼ぶ session 経路はここだけ）。

- `probe_request(project, port, target, point, kind, mode)
  -> Result<ProbeSubscriptionKey, ProbeTraceError>`:
  1. `project.control.targets` に `target` が無ければ
     `UnknownTarget`（fail-closed、port を呼ばない）。
  2. `key = { session_id, point, kind }` を構成し
     `port.probe_request(project, key, mode)`。
  3. `Ok` なら `targets[key].insert(target)`（冪等 — 同 key・同
     target の再 request は集合 insert で no-op。`ProbeSession::
     request` 自体も key 冪等）。`Err` は相関を残さず伝播
     （未解決 point でも binding record は engine 側に残るが、
     session 相関は「この session が trace に流す」宣言なので
     request 失敗時は登録しない）。
- `probe_set_mode`/`probe_capture`/`probe_stop_capture`/
  `probe_remove`: key を受け `port` へ委譲。`probe_remove` 成功時は
  `ProbeObservation::Missing` を相関 target 全員に `record_probe`
  で即時記録する（session が除去を同期的に知っている正直な状態 —
  RT `Removed` status が後で同じ値を再記録しても冪等）。
- `probe_drain(port) -> ProbeDrainReport`:
  ```rust
  pub struct ProbeDrainReport {
      /// port が返した contract update 数。
      pub updates: usize,
      /// session 相関を持たず記録しなかった update 数。
      pub uncorrelated: usize,
      /// port が受理した record_probe 呼出数。
      pub recorded: usize,
      /// port が拒否した（trace route 無し）呼出数。
      pub unrouted: usize,
  }
  ```
  `port.probe_drain()` → 各 update について `targets[key]` の
  target 全員に `port.record_probe(target, update.observation)`。
  相関無し update は `uncorrelated`。record の成否は port の bool
  で集計 — silent drop を隠さない。
- reads: `probe_bindings(port)`/`probe_segments(port)` は port の
  ledger をそのまま返す。`probe_trace(port, target)` は
  `port.trace(target)`。`probe_traces(port)` は `port.traces()`。
  `probe_observed_freshness(port, target, ctx)` は trace 不在なら
  `Unavailable{NoEvidence}`、あれば `trace.observed_freshness(ctx)`。
- `probe_subscriptions(target) -> Vec<ProbeSubscriptionKey>` /
  `probe_targets(key) -> Vec<ParameterTargetId>`: 相関の read-only
  view（UI/診断用、alloc は非 RT 経路のみ）。

`ProbeTraceError`（workspace、`#[non_exhaustive]`）:
`UnknownTarget(ParameterTargetId) | Port(ProbePortError)`。
lifecycle 系（key 指定）メソッドは `ProbePortError` を直接返す。

#### 相関のライフサイクル

- `probe_remove` は相関を**残す** — RT `Removed` status の update
  （→ `Missing`）が drain で届くまで target に記録する必要がある。
  再 `probe_request`（`Stopped` binding は request 可能）は同 key を
  再利用し、既存 target 集合に新 target が加わる。
- 相関 map は subscription 数に有界（`max_probes` 由来）。
  `note_project_changed` での掃除は行わない — binding の世代遷移は
  engine/port 側の責務であり、session 相関は semantic key 単位で
  安定。

## Data and Control Flow

```
UI/command ──► ControlSession::probe_request(project, port, target,
               point, kind, mode)
                 │ key={session_id,point,kind} mint + target 相関登録
                 ▼
             port.probe_request ──► ProbeTraceBridge ──► RoutingEngine
               .probe_request         .probe_request      .probe_request
                                                            │ ProbeSession
                                                            ▼ resolve_probe_point
                                                        ProbeBroker install
RT block ──► ProbeDeck ──► ProbeCollector ──► telemetry queue
                                        │
cycle ──► ControlSession::probe_drain(port)   ← 唯一の drain owner
           │ port.probe_drain()
           ▼ ProbeTraceBridge: ProbeEvent → ProbeObservationUpdate
             (probe_id→key snapshot、generation 照合、status 写像)
           │ for each update: targets[key] → port.record_probe(target, obs)
           ▼ ProbeTraceBridge: routes[target] → adapters[node]
             .record_probe ──► DispatchTraceAdapter.observed slot
           │
read ──► ControlSession::probe_trace / probe_observed_freshness
         ──► port.trace / trace.observed_freshness(ctx)
```

`RebindReceipt`/segment cut は `RoutingEngine::publish` →
`ProbeSession::on_publish` の既存経路が担い、本配線は publish を
経由しない（bridge が engine を所有するが publish 駆動は
composition root/テストの責務）。drain translation の generation
照合が「旧 generation の record を新しい観測として記録しない」
ことを保証する。

## Non-Functional Constraints

- RT 経路不変: `ProbeDeck`/`ProbeCollector`/`DispatchTraceAdapter` の
  RT 面に触れない。bridge/session の全処理は非 RT。
- `probe_drain`/`record_probe` 経路の allocation は bounded
  （`Vec<ProbeObservationUpdate>` ≤ telemetry capacity、相関 map ≤
  subscription 数、trace adapter 内部 cap は PC-C3-05 のまま）。
- `record_probe` は `DispatchTraceAdapter` 側で RT-safe と明記済み
  だが、本経路は非 RT orchestration からのみ呼ぶ。
- 公開 enum は全て `#[non_exhaustive]`。`unsafe` なし。lint は
  workspace 共通（`exhaustive_enums` warn → `-D warnings`）。

## Risks and Trade-offs

- **port を per-call にした判断**: session が port を所有しないため
  「うっかり engine.probe_drain を直接呼ぶ」 bypass を型では防げない。
  ただし所有権共有（engine は app が build/publish にも使う）を
  壊さないことが優先 — `EnginePort` 同様、契約は「session の
  drain API が唯一の *session 経路*」であり、engine 直叩きは
  composition root の責務外行為として残る。将来 app 側で port を
  'static 所有に変える選択肢は残る（trait は object-safe）。
- **`record_probe` の target → node routing は明示登録**:
  `ParamDispatch`/`DispatchTraceAdapter` は target→node の逆引きを
  公開しないため、composition root が bind 時に `route_target` で
  登録する。未登録 target は `record_probe → false` で観測可能
  （`ProbeDrainReport.unrouted` に計上）— silent drop にしない。
- **peak → `ParamValue` の正規化**: block peak は 0 dBFS 超で clamp
  される。`MeasuredValue::Signal` は「正規化された観測信号値」で
  あり絶対レベル計測ではない — この意味論は doc comment に明記し、
  #109 の実測 slice でより精密な写像が必要になれば port/translation
  を拡張する（`MeasuredValue` は `#[non_exhaustive]`）。
- **status → slot 写像の粗さ**: `Unobserved`/`Unbound` は update を
  出さず slot は最後の `Measuring` を保持 → freshness が
  `Stale{Age}` に落ちるまで「古いが存在する観測」を示す。これは
  「この block は観測されなかった」ことを slot が主張しない正直な
  挙動（per-block gap は segment ledger の `ObservationGap` が担う）。
- **`RebindDropped` → `Disabled` の写像**: `Missing`（binding 無し）
  でも `Lost`（block 喪失）でもなく「subscription 生存・観測停止」
  を表す既存 variant として `Disabled` を選択。freshness は
  `Unavailable{ProbeDisabled}` — `Missing` の `ProbeMissing` と
  同じく Current にはならない。

## Deferred Design Concerns

- app composition root の port 実装（`SessionRoutingEngine` の
  engine 共有方法・node 別 adapter 生成/route 登録のタイミング）|
  `synvelis-app` は別 dispatch | SF-05 integration slice | port
  契約が app 側の全要件を満たすかは app 実装時に検証。
- probe capture PCM の trace 接続 | `MeasuredValue` は PCM を運ば
  ない | 計測/録音統合 slice | segment ledger が既に frame
  range/gap を保持。
- `Unobserved`/`Unbound` を slot に反映する finer-grained な
  `ProbeObservation` variant | 現行 4 variant で freshness は
  正直に落ちる | 必要になった時点で project 契約を拡張 |
  現状の age-based Stale で受入条件は満たせる。

## Verification Notes

- workspace fake-port E2E `sf04_s2_probe_trace.rs`:
  observation update → `probe_drain` → `observed` slot `Present` →
  `observed_freshness`/`freshness` 分類、fail-closed（port が
  未相関 update を返しても記録されない・`trace` 不在は
  `Unavailable{NoEvidence}`）、stale reject（port が stale update
  を返さない契約の確認 + generation 違い update の扱い）、
  remove → `Missing` 記録、複数 target fan-out。
- player bridge `sf04_s2_probe_trace_bridge.rs`:
  実 `RoutingEngine` + 実 `DispatchTraceAdapter`（PC-C3-05 と同じ
  in-process fake plugin 構成）で Metric → `Measuring` 記録、
  generation 不一致 Metric の skip、`Removed` → `Missing`、
  `LostRecords` → `Lost`、route 無し target の `record_probe`
  false、`probe_drain` の update 内容。
- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `check-ruviform-rev` / `check-headless-deps`。
