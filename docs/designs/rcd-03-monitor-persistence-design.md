---
id: "DESIGN-034VpQd8lN3xO0wS5uZ7vK"
type: "design"
status: "approved"
title: "RCD-03 Arm/Monitor/Punch Project 永続投影 design"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-03-monitor-persistence.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-03-app-arm-monitoring-ux.md"
    - "docs/designs/rcd-03-monitoring-punch-latency.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-monitor-persistence.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-03-monitor-persistence.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 Arm/Monitor/Punch Project 永続投影 design

## Overview

`docs/designs/rcd-03-app-arm-monitoring-ux.md` は Arm/Monitor 宣言・
Hardware Direct 観測・punch マークを session-only とし、
`project_new`/`project_open`/`project_reload` で reset する契約を
approved 済みである。本 design はそのうち **authored 宣言**
（Arm/Monitor/channel_map/lane/punch marks）を `Project` の永続
モデルへ投影する slice を定義する。session 観測値
（`input_output_ready` / `hardware_direct` / 稼働 session /
latency 注入）は非永続のまま `MonitorUxState` に残る。

```
wire (syn.monitoring.*)
  → ui_ops: patch 分解
      persisted 部 → ProjectOp::{SetTrackMonitor,ClearTrackMonitor,
                                SetPunchMarks}
                    → ProjectSession::apply → ProjectController
                       (validate → clone apply → validate_references
                        → revision++ → undo stack)
      session 部   → MonitorUxState::set_track_session（観測値のみ）
  → syn.monitoring.state: project.monitoring + session overlay
     + decide/MonitorProbe/preflight/LatencyReport を合成
```

`Project` が宣言の唯一の正本（single authority）であり、
`MonitorUxState` はキャッシュを持たない read-through 投影とする。
これにより undo/redo・save/load・他経路（control.dispatch 等）からの
project 変更が全て `syn.monitoring.state` に自動反映され、
app 側に二重の宣言状態を持たない。

## Persistent Data Model (`synvelis-project::monitoring_data`)

`recording_data.rs` と同じ POD パターン（serde derive +
`deny_unknown_fields`、crate 依存なし）。recording 側の
`ArmState`/`MonitorState`/`ChannelMap` と**同名・同値・別型**で
定義し、app 層で写像する（project→recording の依存は禁止のため
型共有不可。wire/serde 表現は同一語彙を維持する）。

```rust
/// Arm 状態（serde: "off" | "on"）
pub enum ArmState { Off, On }

/// Monitor mode（serde: "off" | "auto" | "on"）
pub enum MonitorState { Off, Auto, On }

/// output channel → input channel の写像。非空。
pub struct MonitorChannelMap { pub channels: Vec<u32> }

/// track 単位の authored monitor 宣言。
pub struct TrackMonitorDecl {
    /// 明示 lane。`None` は「track の先頭 lane を使う」宣言
    /// （session 開始時に recording 側が解決する既存 semantics）。
    pub lane: Option<LaneId>,
    pub channel_map: MonitorChannelMap,
    pub arm: ArmState,
    pub monitor: MonitorState,
}

/// project 全体の punch マーク（project tick、負値可）。
/// 両方 `Some` のときのみ recording 側が Punch gate を構成する
/// （既存契約をそのまま維持）。
pub struct PunchMarks {
    pub punch_in: Option<MusicalTick>,
    pub punch_out: Option<MusicalTick>,
}

/// `Project::monitoring` の値型。
pub struct MonitoringData {
    /// keyed by TrackId。宣言の存在自体が「この track は monitor
    /// 管理下」という authored 意味を持つ。
    pub tracks: BTreeMap<TrackId, TrackMonitorDecl>,
    pub punch: PunchMarks,
}
```

`Default` は「宣言なし・punch なし」。`TrackMonitorDecl::default()`
は `lane: None` / `channel_map: [0,1]`（旧 app の
`ChannelMap::identity(2)` と同値）/ `arm: Off` / `monitor: Off`
— 旧 session-only 実装の default decl と一致させ、wire 投影の
既定値を変えない。

## Project Integration

- `Project::monitoring: MonitoringData` — `#[serde(default)]` 付き
  （v11 body は空既定で deserialize される）。
- `Project::new` は `MonitoringData::default()`。
- `validate_references` に追加:
  - `monitoring.tracks` の各 key は `project.tracks` に存在
    （`Error::NotFound`）。
  - `decl.lane` が `Some(l)` のとき `l ∈ track.lane_ids`
    （`Error::Validation`）。
  - `decl.channel_map.channels` が非空（`Error::Validation`）。
  - punch marks は参照を持たないため検査不要。
- `max_issuable_id_raw`: monitor decl は既存 `TrackId`/`LaneId` を
  key/参照に使うのみで新規 ID 型を導入しないため変更不要。
- `RemoveTrack::apply` は `project.monitoring.tracks.remove(track_id)`
  を cascade する（track 削除で decl が孤立するのを防ぐ。
  undo は snapshot 復元で戻る）。`RemoveLane::validate` に
  「monitor decl が参照する lane は削除不可」の検査を追加する。
- semantic diff（`diff/engine.rs`）への ObjectKind 追加は本 slice
  の対象外 — diff は既知 entity のみ列挙するため、未列挙の
  `monitoring` 変更は diff に現れないが破損はしない。follow-up
  として記録する。

## Schema Migration (v11 → v12)

`store.rs` の `SCHEMA_VERSION` を 12 に更新し、履歴コメントに
v12（monitoring projection 追加）を追記。`migration.rs` の STEPS
末尾に `{ from: 11, migrate: migrate_v11_monitoring }` を追加。
step は新フィールドが `#[serde(default)]` で既に空既定になって
いるため実質 identity（`migrate_v8_smf_documents` と同型）だが、
step の存在自体が「v11 body を v12 として受理した」ことの
監査証跡になる。

前方互換: v12 body（`monitoring` フィールド入り）を v11 build で
読むと `deny_unknown_fields` で reject — fail-loud を維持。

## Operations (`ProjectOp`)

```rust
/// track の monitor 宣言を置き換える（upsert）。
SetTrackMonitor { track: TrackId, decl: TrackMonitorDecl },
/// track の monitor 宣言を削除する。
ClearTrackMonitor { track: TrackId },
/// punch マークを絶対値で置き換える（両 None = 解除）。
SetPunchMarks { punch_in: Option<MusicalTick>,
                punch_out: Option<MusicalTick> },
```

- `SetTrackMonitor::validate`: track 存在、`decl.lane` が
  `track.lane_ids` に属する、`channel_map` 非空。
- `ClearTrackMonitor::validate`: track 存在（decl 無しでも
  apply は no-op で成功 — app は decl 有無を見て op 発行を
  制御する）。
- `SetPunchMarks::validate`: 常に成功（tick は参照を持たない）。
- apply は `project.monitoring` のみを変更。undo/redo は
  controller の project snapshot 復元にそのまま乗る。
- patch 型の op ではなく **full-decl upsert** を採用: app が
  現在 decl（無ければ default）に patch を畳んで送る。
  `Option<Option<LaneId>>` のような入れ子 Option を避け、
  Composite 内での合成も「最終形を構築してから積む」明確な
  意味論になる。

## App Integration (`synvelis-app`)

### `MonitorUxState` の再構成

保持するもの（session 観測値のみ）:

```rust
pub struct MonitorUxState {
    /// track → session 観測 overlay（永続化しない）。
    track_sessions: BTreeMap<TrackId, TrackSessionState>,
    session: Option<MonitorSessionState>,   // 稼働 session（既存）
    latency: LatencyReport,                  // 注入点（既存）
    low_latency: LowLatencyReadiness,        // 既存
    tone_difference: bool,                   // 既存
}
struct TrackSessionState {
    input_output_ready: bool,       // 既定 false（旧契約どおり）
    hardware_direct: HardwareDirectMonitor, // 既定 Unknown
}
```

保持しなくなるもの: `tracks` 宣言 map、`punch`、
`MonitorTrackDecl`（wire patch 型 `TrackPatch` は残す）。

### write-through（`ui_ops.rs`）

- `op_monitoring_set`（非 clear）:
  1. `TrackPatch` を wire から構築（既存 helper 再利用）。
  2. persisted field（`lane`/`channel_map`/`arm`/`monitor`）が
     1 つでもある場合のみ `SetTrackMonitor` op を発行する
     （現在 decl に patch を畳んだ full-decl、現在値と同一
     なら発行せず revision を消費しない）。session 観測 field
     のみの patch は decl を生やさない — 観測値は非永続であり、
     default decl を書くと save/load 後に幻の宣言が残るため。
     宣言無し track の `state` 投影上の可視性は、`state()` が
     decl ∪ overlay の和集合を走査することで維持する
     （旧 session-only 実装と同じく patch した track が
     見える）。
  3. op 成功後、session field（`input_output_ready` /
     `hardware_direct`）を `set_track_session` で overlay に
     書く。op 失敗時は overlay も更新しない（atomic）。
- `op_monitoring_set`（`clear: true`）:
  project に decl があれば `ClearTrackMonitor` op を発行し、
  overlay も除去する。decl が無く overlay のみなら overlay
  除去のみで op を発行しない（旧来の `cleared:false` 応答を維持）。
- `op_monitoring_punch`: `require_no_session` を通した上で、
  `project.monitoring.punch` に action を畳んだ新 `PunchMarks`
  を計算し `SetPunchMarks` を発行する。session active 中は
  従来どおり `INVALID_PARAMS`。

### read-through（`monitoring.rs`）

- `contexts(project)`: `project.monitoring.tracks` ∪
  `track_sessions` の和集合（`BTreeSet` で決定的順序）を走査し、
  project POD → recording 型への写像
  （`ArmState`/`MonitorState`/`MonitorChannelMap` →
  `ChannelMap::new`）+ overlay（`input_output_ready` /
  `hardware_direct`、未登録 track は既定値）+ strip mute/solo
  で `TrackMonitorContext` を構築。宣言の無い track は default
  宣言（Arm/Monitor Off）として `decide` に渡す — route を
  捏造しない。`decide` への入力意味論は不変。
- `start_session(project, params)`: 宣言 track・lane・channel
  数・punch marks を `project.monitoring` から読む。
  `preflight`/`GateSpec` 構成は不変。
- `state(project)`: `tracks_json` は decl ∪ overlay の和集合を
  走査し、宣言 field は project（無ければ default 宣言 +
  `declared: false`）、観測 field は overlay から投影。
  `in_project` は `project.tracks.contains_key` をそのまま
  返す（validate_references により常に true だが wire 形状は維持）。
  punch_marks は `project.monitoring.punch` を投影。
- `project_new`/`project_open`: `MonitorUxState::default()` で
  session 観測値を reset（既存動作）。宣言・punch は load 済み
  `project.monitoring` がそのまま正本となるため、コピー・
  rehydrate コードは不要 — `state()` が project を読むため
  自動的に再投影される。app.rs の変更はコメント更新が主。

## Compatibility / Migration Notes

- contract_change: project snapshot schema v11 → v12。
  v11 body は `#[serde(default)]` + migration step で空
  monitoring として load される（情報損失なし — v11 には
  monitor 宣言が存在しなかった）。v12 body を v11 build で
  読むことは `deny_unknown_fields` で拒否される（意図的な
  前方非互換）。
- wire 互換: `syn.monitoring.*` の op 名・params・state JSON
  形状は不変。`syn.monitoring.state` の track 列が覆う集合も
  不変 — decl ∪ overlay の和集合は旧 session-only 実装の
  `tracks` map と同じ可視範囲を再現する（宣言の保存場所が
  変わるだけで、投影される track 集合は同一）。
- `syn.project.undo`/`redo` が monitor 宣言・punch を含むように
  なる（revision が進むのは宣言変更時のみ — session 観測値の
  更新は revision を消費しない）。

## Risks / Trade-offs

- read-through 化により `syn.monitoring.state` は毎回
  `project.monitoring` を走査する。宣言 track 数は mixer 規模で
  bounded、`PROBE_TRACK_CAP` 等の既存 cap がそのまま効く。
- `RemoveTrack` の cascade は decl を暗黙に消すが、undo で復元
  される。`validate_references` が crafted snapshot の孤立 decl
  を拒否するため不変条件は維持される。
- VCS semantic diff が monitor 変更を検出しない（follow-up）。
  undo/redo・save/load は snapshot ベースのため影響なし。
