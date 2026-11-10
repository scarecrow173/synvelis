---
id: "DESIGN-034VdWksbk4tfLCaf6HPoj"
type: "design"
status: "approved"
title: "RCD-03 app Arm/Monitoring UX + monitor session projection"
created: "2026-11-10"
updated: "2026-11-10"
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
  depends-on:
    - "SPEC-034VdSvFA9X0ESbckrSvBI"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "DESIGN-034UtlI2uLUF1lr2g3CUM0"
  refines:
    - "DESIGN-034UtlI2uLUF1lr2g3CUM0"
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-app-arm-monitoring-ux.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 app Arm/Monitoring UX + monitor session projection

## Context

crate 層 RCD-03（`synvelis-recording::monitor` の `decide`/`MonitorProbe`/
`HardwareDirectMonitor`、`synvelis-player::monitor` の `MonitorAdapter`/
`LatencyReport`/`prepare_low_latency`）は PR #214 で完了済み。
本 design はその上に載る app 面の UX op と読み取り投影だけを対象とし、
route/gate/latency の意味論は一切再実装しない。

`SynvelisApp` は `workspace: WorkspaceManager<ShellWindowHost>` 経由で
`ProjectSession`（唯一の project writer 経路・`project()`/`control()`/
`transport()` アクセサ）を持ち、`routing_engine`/`recorder` 等の
session-scope slot を field で保持する既存パターンがある。
本 slice は同パターンで `monitoring` slot を追加する。

## Scope

### In Scope

- `crates/synvelis-app/src/monitoring.rs`（新規）: session-scope の
  monitor UX 状態と投影ロジック。
- `crates/synvelis-app/src/ui_ops.rs`: `op_monitoring_*` ハンドラ。
- `crates/synvelis-app/src/service.rs`: `syn.monitoring.*` の OPS 登録と
  dispatch。
- `crates/synvelis-app/src/app.rs`: `monitoring` field 追加と
  `project_new`/`project_open` での reset。
- `crates/synvelis-app/Cargo.toml`: `ruviform-media-core` 依存追加
  （`LatencyComponent.clock_epoch` の wire 写像に必要）。
- `crates/synvelis-app/tests/monitor_arm*.rs`: headless fixture。

### Out of Scope

- `synvelis-project` / `synvelis-recording` / `synvelis-player` /
  `synvelis-workspace` の変更（消費のみ。project 投影は既存の
  strip mute/solo 読み取りで足りるため `project/src/monitor.rs` は
  作らない — spec の「不要なら触らない」分岐）。
- `MonitorAdapter` PCM 経路、RT graph monitor node、実デバイス
  latency 実測の供給。
- Loop/PunchInLoop gate、count-in/pre-roll の UX 宣言。
- Arm/Monitor 宣言・Hardware Direct 観測の永続化。

## Components and Boundaries

- **`monitoring.rs` — `MonitorUxState`**（`SynvelisApp` の session-scope
  slot、project 再生成で reset）:
  - `tracks: BTreeMap<TrackId, MonitorTrackDecl>` —
    `MonitorTrackDecl { lane: Option<LaneId>, binding: InputBinding,
    input_output_ready: bool, hardware_direct: HardwareDirectMonitor }`。
    `binding` は `synvelis-recording::input::InputBinding`（channel_map +
    arm + monitor）をそのまま保持し、語彙を複製しない。
  - `session: Option<MonitorSessionState>` —
    `MonitorSessionState { plan: PreparedRecordingPlan, position:
    MusicalTick, playing: bool, paused: bool, sample_rate: u32,
    clock_epoch: synvelis_project::ClockEpochId, transport_start:
    MusicalTick, stop: MusicalTick, punch_in/out: Option<MusicalTick> }`。
    plan は `synvelis-recording::plan::preflight` の産物で、Capture
    Gate scheduler を `MonitorProbe` と共有する。
  - `punch: PunchMarks { punch_in: Option<MusicalTick>, punch_out:
    Option<MusicalTick> }` — session 非依存のマーク。session 開始時に
    gate へ写像される（両方揃えば `GateSpec::Punch`、それ以外は
    `GateSpec::Normal { open: transport_start }`）。
  - `latency: LatencyReport`、`low_latency: LowLatencyReadiness`、
    `tone_difference: bool` — デバイス報告の headless 注入点
    （既定値は `LatencyReport::default()` 等で、未注入は
    `quality: Unknown` / `Unavailable:NotPrepared` と正直に投影）。
- **`ui_ops.rs` — op ハンドラ**: params 検証・enum 文字列写像・
  `synvelis_project`/`synvelis_recording`/`synvelis_player` 型への
  変換を担う薄い層。状態遷移の実体は `monitoring.rs` のメソッド。
- **`service.rs`**: OPS 6 件 + dispatch 6 arm のみ。

## Data and Control Flow

### `syn.monitoring.set`

`{track, lane?, arm?, monitor?, binding?{channels:[u32,..]},
input_output_ready?, hardware_direct?{state,observed_active?}, clear?}`
→ track 存在検証（`project.tracks`）→ lane 存在 + track 帰属検証
（`track.lane_ids` 所属 — 他 track の lane 誤 bind を拒否）→
`ChannelMap::new` 検証 → 宣言を merge upsert（未指定 field は維持）。
`clear: true` で宣言削除。project には一切触れない（session-only）。

### `syn.monitoring.punch`

`{action: "set"|"in"|"out"|"clear", punch_in?, punch_out?}`（tick は i64）
→ マーク更新。session active 中は `INVALID_PARAMS`（gate は session
開始時に確定済み — 途中変更を黙って無効化しない）。

### `syn.monitoring.transport`

`{action: "play"|"pause"|"stop"|"seek", position?}` — session 必須。
`position` は `MusicalTick`。play/pause/stop は `playing`/`paused`
フラグを更新、seek は `position` を更新。位相語彙は state 投影で
導出する。

### `syn.monitoring.session`

`{action: "start"|"stop", transport_start?, stop?, clock_epoch?,
sample_rate?, max_block_frames?}`。
start: 宣言 track 全件から `CaptureTarget`（lane 解決: 宣言 lane →
track の先頭 lane、channels = `channel_map.len()`）を構築し、
`preflight(project, RecordingRequest, sample_rate)` で plan 確定。
失敗は `INVALID_PARAMS`（preflight の Error を message に載せる）。
既に active なら `INVALID_PARAMS`（明示 stop が先）。stop: session
破棄。gate は punch マークから導出（Loop 系は本 slice 非対応）。
budget/max_block_frames は既定値
（`chunks:16, chunk_frames:1024, spans_per_chunk:8,
filled_capacity:16, lookahead_spans:16`、block 1024 — recording
fixture と同値）で、op 引数で上書き可能。

### `syn.monitoring.latency`

`{report?{round_trip?,input?,output?,reported[],reported_verified},
readiness?{prepared,input_ready,output_ready}, tone_difference?}`
→ `LatencyReport`/`LowLatencyReadiness` を patch 保持。
`clock_epoch` は u64 ↔ `ruviform_media_core::ClockEpochId`。

### `syn.monitoring.state`

読み取り専用。以下を合成:

1. **session ブロック**: active/transport_start/stop/gate/punch
   マーク/position/transport(playing|paused|stopped)/phase/
   sample_rate/session_end_frames/clock_epoch(tagged)。
   phase 導出: 非再生 → `stopped`。再生中は punch マークで
   `punch_wait`/`punch_open`/`post_roll`、無ければ `playing`。
   paused は transport=`paused` と正直に報告し、decide 用位相は
   `Stopped` に写像（停止中扱い — gate は閉じている）。
2. **tracks ブロック**: 宣言済み track ごとに arm/monitor/binding
   （kind は channels.len() から導出: 1=mono,2=stereo,他
   =multichannel）/input_output_ready/hardware_direct（state +
   observed_active を verbatim 投影、`controllable` 等の推測 field
   は置かない）/strip 由来の muted・soloed/`in_session`。
3. **route/notices（live）**: position → session frame 変換
   （`plan.mapping.tick_to_media(position,1,sample_rate) -
   transport_start_frame`）。session 内なら対応 segment の
   `gate_open` を採用し、`decide(ctx, live_phase, gate_open)` を
   呼ぶ。session 外・位相 stopped は `gate_open=false` で
   `decide(ctx, Stopped, false)`。route は `null` ではなく
   decide の結果を常に投影（decide は Stopped でも route を返す）。
   これにより「現在の可聴 route」が timeline segment と同一の
   decide 経路で得られる。
4. **segments ブロック**: session active 時、宣言 track ごとに
   `MonitorProbe::new(&plan, ctx)` を作り 4096-frame block で
   `session_end` まで feed。`phase` は `Playing` 固定
   （decide は Stopped 以外を同一視し、gate_open が区間を担う —
   probe の `gate_open` は scheduler 由来で tick 空間の再実装を
   しない）。track あたり `MONITOR_SEGMENT_CAP = 256`、
   probe 対象 track は `MONITOR_PROBE_TRACK_CAP = 32` で打ち切り、
   `truncated`/`tracks_omitted` を明示。`ctx` は live route と同じ
   `TrackMonitorContext`（muted/soloed/any_solo は strip から、
   any_solo は宣言 track の strip 集合で評価）。
5. **latency ブロック**: `report.resolve()` +
   `prepare_low_latency(readiness, tone_difference, &report)` を
   verbatim 投影（enum は snake_case 文字列）。
6. **notices 集約**: double_monitoring / hardware_direct_unknown /
   input_unavailable が立つ track の tagged 一覧。

`project_new`/`project_open`/`project_reload` で `monitoring` を
default に reset（宣言は session-only、project を跨がない）。
view/window/focus op は monitoring state に触れない — fixture で
不変性を検証する。

## Non-Functional Constraints

- 非 RT のみ。probe feed は state 呼出しごとの bounded 計算
  （track ≤32 × session_end/4096 feed）。fixture/実用の session 長
  では十分小さいが、RT callback からは呼ばない（service op は
  UI/host thread）。
- `syn.monitoring.state` は project/engine に触れず、宣言と
  session state のみから投影（`&self` で完結）。
- 全ての失敗は `INVALID_PARAMS`/`INTERNAL` の `ErrorBody` で、
  捏造した route/latency を返さない。

## Risks and Trade-offs

- **position→session frame 変換の丸め**: `tick_to_media` は
  tempo map 経由で frame 量子化される。segment の `gate_open` は
  scheduler と同一写像を通るため boundary の 1-frame ずれは
  scheduler 自体の量子化と一致する（独自補正しない）。
- **paused→Stopped 写像**: decide 語彙に Paused が無い。
  「停止中 Input」semantics に合わせ Stopped へ写像し、transport
  status には `paused` を残して情報を失わない。
- **segments の再計算コスト**: state 呼出しごとに全 probe を再 feed
  する。宣言 track ≤32・segment cap で bounded。頻繁 poll する
  GUI が問題になれば cache を検討するが、本 slice では YAGNI。
- **project 変更（track/lane 削除・tempo map 変更）**: 宣言 track が
  消えた場合は投影から自然に落ちる（`project.tracks` に無い track
  は skip）。session plan は開始時 snapshot のまま — tempo map
  revision が進んだ場合、live decide の muted/solo は最新 project
  を読むが gate 区間は凍結 plan のまま。これは「録音 pass の
  binding 時凍結」semantics と一致し、docs に明記する。

## Deferred Design Concerns

- 実 Device/実 GUI/multi-window/focus・AudioLens 受入 — 外部 gate。
- `MonitorAdapter` PCM 経路・RT graph monitor node — player RT
  搭載は宣言 scope 外。
- Arm/Monitor 宣言の永続化・Undo — session-only が現行契約。
- Loop/PunchInLoop・count-in/pre-roll UX — GateSpec は対応済みだが
  UX 宣言は後続 slice。
- 旧 `TransportState`/`ControlSession` transport との統合 — 本 slice
  の monitor transport は session-scope の宣言状態。実 transport
  配線は device/RT slice で再検討。

## Verification Notes

- [ ] `crates/synvelis-app/tests/monitor_arm*.rs` で Off/Auto/On ×
  Arm × Transport × Punch × Mute/Solo の route 投影、Hardware
  Direct notice、double_monitoring、input_unavailable、
  window/focus 不変性、latency 投影を検証。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-app-native` / `test-msrv`。
