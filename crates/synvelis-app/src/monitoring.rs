//! RCD-03 app Arm/Monitoring UX — synvelis#117 slice。
//!
//! session-scope の monitor 宣言（Arm/Monitor mode/Input binding/
//! `input_output_ready`/Hardware Direct 観測）と monitor session
//! （`synvelis-recording::plan::preflight` が確定する
//! `PreparedRecordingPlan` + transport/punch マーク）を保持し、
//! `syn.monitoring.state` が `decide`/`MonitorProbe`/
//! `LatencyReport::resolve`/`prepare_low_latency` を消費した
//! 読み取り投影を返す。
//!
//! 責務分界:
//!
//! - route/gate/latency の意味論は recording/player crate が所有し、
//!   本モジュールは再実装しない。`decide` の呼出しと `MonitorProbe`
//!   の feed が route 投影の唯一の経路。
//! - 宣言・session・punch マーク・latency 注入は全て session-only
//!   であり `Project`/`ProjectController` を一切変更しない
//!   （`project.new`/`open` で reset）。Mute/Solo は project の
//!   strip control target から読み取るだけ。
//! - Hardware Direct Monitor は外部観測として verbatim に保持・
//!   投影する。`Unknown` を制御可能と偽らず、DAW Mute/Solo で
//!   その値を変更しない。
//! - 実 Device/Capture/Software Monitor の PCM 経路は本 slice の
//!   対象外 — `MonitorAdapter` のミックスや RT graph 搭載は
//!   行わない（latency 解決投影のみ消費）。

use std::collections::BTreeMap;

use serde_json::{Value, json};
use synvelis_player::{
    LatencyQuality, LatencyReport, LatencyResolutionBasis, LowLatencyMonitor, LowLatencyReadiness,
    LowLatencyReason, LowLatencyState, prepare_low_latency,
};
use synvelis_project::{
    ClockEpochId, LaneId, MusicalTick, Project, SessionRange, TempoMapRef, TrackId,
};
use synvelis_recording::{
    ArmState, BoundedVec, BufferBudget, CaptureTarget, ChannelMap, GateSpec, HardwareDirectMonitor,
    HardwareDirectState, InputBinding, MonitorProbe, MonitorRoute, MonitorSegment, MonitorState,
    PreparedRecordingPlan, RecordingRequest, TrackMonitorContext, TransportPhase, decide,
    preflight,
};

/// `syn.monitoring.state` の segments 投影で probe を回す track 数上限。
/// これを超える宣言 track は live route のみ投影し `tracks_omitted`
/// で明示する。
const PROBE_TRACK_CAP: usize = 32;

/// track あたりの segment 投影上限。超過時は `truncated: true`。
const SEGMENT_CAP: usize = 256;

/// probe feed の block サイズ [frame]。gate 境界は piece 単位で
/// 正確に分割されるため block 粒度は投影精度に影響しない。
/// 大きめに取り feed 回数を抑える — 出力 segment は
/// [`push_segment_merged`] で同一決定の隣接区間を併合するため、
/// 実際の route 遷移数だけが `SEGMENT_CAP` に効く。
const FEED_BLOCK: i64 = 1 << 24;

/// `syn.monitoring.session` start の既定バッファ予算
/// （`synvelis-recording` fixture と同値。`max_block_frames` は
/// `chunk_frames` 以下であることが preflight で検証される）。
const DEFAULT_BUDGET: BufferBudget = BufferBudget {
    chunks: 16,
    chunk_frames: 1024,
    spans_per_chunk: 8,
    filled_capacity: 16,
    lookahead_spans: 16,
};

/// `syn.monitoring.session` start の既定 `max_block_frames`。
const DEFAULT_MAX_BLOCK_FRAMES: u32 = 1024;

/// Track 単位の monitor 宣言（session-scope、永続化しない）。
///
/// `binding` は `synvelis-recording` の `InputBinding` をそのまま
/// 保持し、Arm/Monitor/ChannelMap の語彙を複製しない。
#[derive(Clone, Debug)]
pub struct MonitorTrackDecl {
    /// monitor session の capture lane。`None` は session 開始時に
    /// track の先頭 lane へ解決される。
    pub lane: Option<LaneId>,
    /// channel→input 写像 + Arm + Monitor mode。
    pub binding: InputBinding,
    /// 入力 Binding + Output Capability の検証済みフラグ
    /// （`TrackMonitorContext::input_output_ready` へそのまま渡す。
    /// 未検証を真としない）。
    pub input_output_ready: bool,
    /// 外部 Hardware Direct Monitor の観測/申告。`decide` が
    /// notice 判定に使い、app は値を変更しない。
    pub hardware_direct: HardwareDirectMonitor,
}

impl Default for MonitorTrackDecl {
    fn default() -> Self {
        Self {
            lane: None,
            binding: InputBinding {
                channel_map: ChannelMap::identity(2),
                arm: ArmState::Off,
                monitor: MonitorState::Off,
            },
            input_output_ready: false,
            hardware_direct: HardwareDirectMonitor::default(),
        }
    }
}

/// `syn.monitoring.set` の patch — `Some` の field のみ更新する。
#[derive(Default)]
pub struct TrackPatch {
    /// lane 指定。外側 `Some` = field 指定あり、内側 `None` =
    /// 「自動解決へ戻す」（宣言 lane をクリア）。
    pub lane: Option<Option<LaneId>>,
    /// Record Arm の新値。
    pub arm: Option<ArmState>,
    /// Monitor mode の新値。
    pub monitor: Option<MonitorState>,
    /// channel→input 写像の新値（arm/monitor は別 field）。
    pub channel_map: Option<ChannelMap>,
    /// `input_output_ready` の新値。
    pub input_output_ready: Option<bool>,
    /// Hardware Direct 観測の新値。
    pub hardware_direct: Option<HardwareDirectMonitor>,
}

/// Punch in/out マーク（project tick、半開 `[in, out)` の両端）。
/// session 開始時に `GateSpec` へ写像される — 両方揃えば
/// `GateSpec::Punch`、それ以外は `GateSpec::Normal`。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PunchMarks {
    /// Punch in tick。
    pub punch_in: Option<MusicalTick>,
    /// Punch out tick。
    pub punch_out: Option<MusicalTick>,
}

/// `syn.monitoring.transport` の action。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MonitorTransportAction {
    /// transport を再生開始。
    Play,
    /// 一時停止（位置保持・gate 閉）。
    Pause,
    /// 停止。
    Stop,
    /// 位置移動（`MusicalTick`）。
    Seek,
}

/// `syn.monitoring.session` start のパラメータ。
#[derive(Clone, Copy)]
pub struct MonitorSessionParams {
    /// Transport 開始 tick。
    pub transport_start: MusicalTick,
    /// 停止 tick（`transport_start` より後であること）。
    pub stop: MusicalTick,
    /// Clock Epoch（`None` は project の最小 epoch を使う）。
    pub clock_epoch: Option<ClockEpochId>,
    /// sample rate [frame/sec]。
    pub sample_rate: u32,
    /// 1 callback の最大 frame 数（`None` は既定 1024）。
    pub max_block_frames: Option<u32>,
}

/// 稼働中の monitor session — `preflight` 確定 plan と transport
/// 状態。gate/区間は plan に凍結され、以後の project/tempo 変更で
/// 再計算しない（録音 pass の binding 時凍結と同じ契約）。
pub struct MonitorSessionState {
    /// `preflight` が確定した録音計画（gate scheduler を共有）。
    pub plan: PreparedRecordingPlan,
    /// 現在位置（project tick）。
    pub position: MusicalTick,
    /// transport が rolling か。
    pub playing: bool,
    /// 一時停止中か（位置保持・gate 閉 — decide には `Stopped`
    /// 相当として写像する）。
    pub paused: bool,
    /// sample rate [frame/sec]（position→session frame 変換用）。
    pub sample_rate: u32,
    /// session に使った project Clock Epoch（tagged 投影用）。
    pub clock_epoch: ClockEpochId,
    /// session の transport 開始 tick。
    pub transport_start: MusicalTick,
    /// session の停止 tick。
    pub stop: MusicalTick,
}

/// session-scope の monitor UX 状態。`SynvelisApp` が保持し、
/// project 再生成で reset される。
#[derive(Default)]
pub struct MonitorUxState {
    /// `TrackId` → monitor 宣言（BTreeMap で決定的順序）。
    pub tracks: BTreeMap<TrackId, MonitorTrackDecl>,
    /// 稼働中の monitor session。
    pub session: Option<MonitorSessionState>,
    /// 宣言済み punch マーク。
    pub punch: PunchMarks,
    /// デバイス報告レイテンシ（headless 注入点。既定は空報告 →
    /// `quality: Unknown` / `Unavailable:NotPrepared` と正直に投影）。
    pub latency: LatencyReport,
    /// low-latency route の準備状態。
    pub low_latency: LowLatencyReadiness,
    /// 通常 monitor との音質差が既知か。
    pub tone_difference: bool,
}

impl MonitorUxState {
    /// `syn.monitoring.set` — track 宣言を patch 更新する。
    ///
    /// # Errors
    /// track が project に存在しない、指定 lane が project に
    /// 存在しない場合に `Err`（message は op 層が `INVALID_PARAMS`
    /// へ写す）。
    pub fn set_track(
        &mut self,
        project: &Project,
        track: TrackId,
        patch: TrackPatch,
    ) -> Result<(), String> {
        if !project.tracks.contains_key(&track) {
            return Err(format!("track {} not found in project", track.to_tagged()));
        }
        if let Some(Some(lane)) = patch.lane {
            // 宣言 lane はその track の lane_ids に含まれること —
            // 他 track の lane を誤って bind すると session が
            // 別 track の入力を capture する。
            let owned = project
                .tracks
                .get(&track)
                .is_some_and(|t| t.lane_ids.contains(&lane));
            if !owned {
                return Err(format!(
                    "lane {} is not a lane of track {}",
                    lane.to_tagged(),
                    track.to_tagged()
                ));
            }
        }
        let decl = self.tracks.entry(track).or_default();
        if let Some(lane) = patch.lane {
            decl.lane = lane;
        }
        if let Some(arm) = patch.arm {
            decl.binding.arm = arm;
        }
        if let Some(monitor) = patch.monitor {
            decl.binding.monitor = monitor;
        }
        if let Some(map) = patch.channel_map {
            decl.binding.channel_map = map;
        }
        if let Some(ready) = patch.input_output_ready {
            decl.input_output_ready = ready;
        }
        if let Some(hw) = patch.hardware_direct {
            decl.hardware_direct = hw;
        }
        Ok(())
    }

    /// `syn.monitoring.set` の `clear` — 宣言を削除する。
    /// 戻り値は実際に削除があったか。
    #[must_use]
    pub fn clear_track(&mut self, track: TrackId) -> bool {
        self.tracks.remove(&track).is_some()
    }

    /// punch in/out マークを両方設定する（`syn.monitoring.punch`
    /// の `"set"`）。片方だけの設定は [`Self::punch_mark`] を使う。
    ///
    /// # Errors
    /// session が active な場合（gate は開始時に確定済み）。
    pub fn punch_set(
        &mut self,
        punch_in: Option<MusicalTick>,
        punch_out: Option<MusicalTick>,
    ) -> Result<(), String> {
        self.require_no_session("punch marks")?;
        self.punch = PunchMarks {
            punch_in,
            punch_out,
        };
        Ok(())
    }

    /// punch in または out の片側マークを設定する
    /// （`syn.monitoring.punch` の `"in"`/`"out"`）。
    ///
    /// # Errors
    /// session が active な場合。
    pub fn punch_mark(&mut self, which: PunchSide, tick: MusicalTick) -> Result<(), String> {
        self.require_no_session("punch marks")?;
        match which {
            PunchSide::In => self.punch.punch_in = Some(tick),
            PunchSide::Out => self.punch.punch_out = Some(tick),
        }
        Ok(())
    }

    /// punch マークを全解除する（`syn.monitoring.punch` の
    /// `"clear"` — gate は Normal に戻る）。
    ///
    /// # Errors
    /// session が active な場合。
    pub fn punch_clear(&mut self) -> Result<(), String> {
        self.require_no_session("punch marks")?;
        self.punch = PunchMarks::default();
        Ok(())
    }

    /// `syn.monitoring.transport` — session の transport を動かす。
    ///
    /// # Errors
    /// session が無い場合、または `Seek` で `position` が無い場合。
    pub fn transport(
        &mut self,
        action: MonitorTransportAction,
        position: Option<MusicalTick>,
    ) -> Result<(), String> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| "no active monitor session".to_owned())?;
        match action {
            MonitorTransportAction::Play => {
                session.paused = false;
                session.playing = true;
            }
            MonitorTransportAction::Pause => {
                if session.playing {
                    session.playing = false;
                    session.paused = true;
                }
            }
            MonitorTransportAction::Stop => {
                session.playing = false;
                session.paused = false;
            }
            MonitorTransportAction::Seek => {
                session.position = position
                    .ok_or_else(|| "params.position (tick) required for seek".to_owned())?;
            }
        }
        Ok(())
    }

    /// `syn.monitoring.session` start — 宣言 track から
    /// `RecordingRequest` を組み立て `preflight` で plan を確定する。
    ///
    /// # Errors
    /// session 既稼働・宣言 track なし・参照先（track/lane/clock
    /// epoch）不在・`preflight` 検証失敗の場合に `Err`。
    pub fn start_session(
        &mut self,
        project: &Project,
        params: MonitorSessionParams,
    ) -> Result<(), String> {
        if self.session.is_some() {
            return Err("monitor session already active — stop it first".to_owned());
        }
        if self.tracks.is_empty() {
            return Err("no monitor tracks declared — syn.monitoring.set first".to_owned());
        }
        let mut targets = Vec::with_capacity(self.tracks.len());
        for (track_id, decl) in &self.tracks {
            let track = project.tracks.get(track_id).ok_or_else(|| {
                format!("declared track {} no longer exists", track_id.to_tagged())
            })?;
            let lane = decl
                .lane
                .or_else(|| track.lane_ids.first().copied())
                .ok_or_else(|| {
                    format!(
                        "track {} has no lane for monitor capture",
                        track_id.to_tagged()
                    )
                })?;
            targets.push(CaptureTarget {
                lane_id: lane,
                channels: u32::try_from(decl.binding.channel_map.len()).map_err(|_| {
                    format!("track {} channel count overflows u32", track_id.to_tagged())
                })?,
            });
        }
        let clock_epoch = params
            .clock_epoch
            .or_else(|| project.clock_epochs.keys().next().copied());
        let clock_epoch = clock_epoch.ok_or_else(|| {
            "project has no clock epoch — register one before starting a monitor session".to_owned()
        })?;
        let gate = match (self.punch.punch_in, self.punch.punch_out) {
            (Some(punch_in), Some(punch_out)) => GateSpec::Punch {
                punch_in,
                punch_out,
            },
            _ => GateSpec::Normal {
                open: params.transport_start,
            },
        };
        let request = RecordingRequest {
            targets,
            clock_epoch,
            tempo_map: TempoMapRef {
                revision: project.tempo_map.revision,
            },
            transport_start: params.transport_start,
            stop: params.stop,
            gate,
            budget: DEFAULT_BUDGET,
            max_block_frames: params.max_block_frames.unwrap_or(DEFAULT_MAX_BLOCK_FRAMES),
        };
        let plan = preflight(project, request, params.sample_rate)
            .map_err(|e| format!("monitor session preflight: {e}"))?;
        self.session = Some(MonitorSessionState {
            plan,
            position: params.transport_start,
            playing: false,
            paused: false,
            sample_rate: params.sample_rate,
            clock_epoch,
            transport_start: params.transport_start,
            stop: params.stop,
        });
        Ok(())
    }

    /// `syn.monitoring.session` stop — session を破棄する。
    /// 宣言・punch マーク・latency 注入は保持される。
    pub fn stop_session(&mut self) {
        self.session = None;
    }

    /// `syn.monitoring.latency` — 報告値を patch 更新する
    /// （`None` の field は現行値を維持）。
    pub fn set_latency(
        &mut self,
        report: Option<LatencyReport>,
        readiness: Option<LowLatencyReadiness>,
        tone_difference: Option<bool>,
    ) {
        if let Some(report) = report {
            self.latency = report;
        }
        if let Some(readiness) = readiness {
            self.low_latency = readiness;
        }
        if let Some(tone_difference) = tone_difference {
            self.tone_difference = tone_difference;
        }
    }

    fn require_no_session(&self, what: &str) -> Result<(), String> {
        if self.session.is_some() {
            return Err(format!(
                "{what} cannot change while a monitor session is active — stop it first"
            ));
        }
        Ok(())
    }

    /// 宣言 track 全件の `TrackMonitorContext` を構築する。
    /// `any_solo` は宣言 track の strip 集合で評価する
    /// （monitor session の solo 文脈 — project 全 strip ではない）。
    fn contexts(&self, project: &Project) -> Vec<(TrackId, TrackMonitorContext)> {
        let soloed: Vec<bool> = self
            .tracks
            .keys()
            .map(|track_id| {
                Self::strip_of(project, *track_id)
                    .is_some_and(|s| project.routing.strip_soloed(&project.control, s))
            })
            .collect();
        let any_solo = soloed.iter().any(|s| *s);
        self.tracks
            .iter()
            .map(|(track_id, decl)| {
                let strip = Self::strip_of(project, *track_id);
                let ctx = TrackMonitorContext {
                    arm: decl.binding.arm,
                    monitor: decl.binding.monitor,
                    input_output_ready: decl.input_output_ready,
                    muted: strip.is_some_and(|s| project.routing.strip_muted(&project.control, s)),
                    soloed: strip
                        .is_some_and(|s| project.routing.strip_soloed(&project.control, s)),
                    any_solo,
                    hardware_direct: decl.hardware_direct,
                };
                (*track_id, ctx)
            })
            .collect()
    }

    /// track の strip id（strip が無ければ `None` — mute/solo の
    /// 権威が存在しないことを偽らない）。
    fn strip_of(project: &Project, track: TrackId) -> Option<synvelis_project::StripId> {
        project
            .tracks
            .get(&track)
            .and_then(|_| project.routing.strip_of_track(track))
            .map(|s| s.id)
    }

    /// `syn.monitoring.state` — 宣言・session・punch・latency の
    /// 読み取り投影。決して route/notices を捏造しない:
    /// session 未開始は `decide(ctx, Stopped, false)`、
    /// position が session 外は `gate_open = false` で判定する。
    #[must_use]
    pub fn state(&self, project: &Project) -> Value {
        let contexts = self.contexts(project);
        // position → session frame。in-session の gate_open は
        // segments（MonitorProbe 産物）から引くため、segments を
        // 先に計算する。
        let position_frame = self.position_session_frame();
        let segments = self.segments(&contexts);
        // gate_open は session gate の位置関数であり track 非依存
        // （全 probe が同一 scheduler を共有する）。position を含む
        // segment が投影済みならその gate_open を全 track の live
        // route に使う。session 外・停止中・segment 未カバーは
        // `false` — tick 空間で gate を再実装しない。
        let gate_open_now = position_frame
            .filter(|frame| self.in_session(*frame))
            .and_then(|frame| {
                segments.iter().find_map(|(_, view)| {
                    view.segments
                        .iter()
                        .find(|&seg| segment_contains(seg, frame))
                        .map(|seg| seg.gate_open)
                })
            })
            .unwrap_or(false);
        let live = self.live_routes(&contexts, gate_open_now);
        let mut tracks_json = Vec::with_capacity(self.tracks.len());
        let mut notices = json!({
            "double_monitoring": [],
            "hardware_direct_unknown": [],
            "input_unavailable": [],
        });
        for (track_id, ctx) in &contexts {
            let decl = &self.tracks[track_id];
            let decision = live
                .get(track_id)
                .copied()
                .unwrap_or_else(|| decide(ctx, TransportPhase::Stopped, false));
            push_notice(&mut notices, "double_monitoring", *track_id, decision);
            push_notice(&mut notices, "hardware_direct_unknown", *track_id, decision);
            push_notice(&mut notices, "input_unavailable", *track_id, decision);
            tracks_json.push(json!({
                "track": track_id.to_tagged(),
                "in_project": project.tracks.contains_key(track_id),
                "lane": decl.lane.map(LaneId::to_tagged),
                "arm": arm_str(decl.binding.arm),
                "monitor": monitor_str(decl.binding.monitor),
                "binding": {
                    "kind": binding_kind(&decl.binding.channel_map),
                    "channels": decl.binding.channel_map.channels(),
                },
                "input_output_ready": decl.input_output_ready,
                "hardware_direct": {
                    "state": hw_state_str(decl.hardware_direct.state),
                    "observed_active": decl.hardware_direct.observed_active,
                },
                "muted": ctx.muted,
                "soloed": ctx.soloed,
                "route": route_str(decision.route),
                "notices": notices_json(decision),
            }));
        }
        let segments_json: Vec<Value> = segments
            .iter()
            .map(|(track_id, view)| {
                json!({
                    "track": track_id.to_tagged(),
                    "truncated": view.truncated,
                    "segments": view.segments.iter().map(segment_json).collect::<Vec<_>>(),
                })
            })
            .collect();
        json!({
            "session": self.session_json(position_frame),
            "punch_marks": {
                "punch_in": self.punch.punch_in.map(MusicalTick::value),
                "punch_out": self.punch.punch_out.map(MusicalTick::value),
            },
            "tracks": tracks_json,
            "segments": segments_json,
            "tracks_omitted": self.tracks.len().saturating_sub(PROBE_TRACK_CAP),
            "notices": notices,
            "latency": self.latency_json(),
        })
    }

    /// position の session-frame 位置。session 無し・tick→media
    /// 変換失敗時は `None`。
    fn position_session_frame(&self) -> Option<i64> {
        let session = self.session.as_ref()?;
        let media = session
            .plan
            .mapping
            .tick_to_media(session.position, 1, session.sample_rate)
            .ok()?;
        media
            .value()
            .checked_sub(session.plan.transport_start_frame)
    }

    /// session の表示用 JSON。`phase` は `decide` 語彙
    /// （`TransportPhase`）の wire 表現 — `PunchOpen`/`PostRoll` は
    /// `gate_open` が担うため非停止相の区別は punch マークとの
    /// 位置比較で導出する。
    fn session_json(&self, position_frame: Option<i64>) -> Value {
        let Some(session) = self.session.as_ref() else {
            return json!({
                "active": false,
                "transport_start": null,
                "stop": null,
                "gate": null,
                "punch_in": null,
                "punch_out": null,
                "position": null,
                "position_frame": null,
                "in_session": null,
                "transport": null,
                "phase": null,
                "sample_rate": null,
                "session_end_frames": null,
                "clock_epoch": null,
            });
        };
        let (gate, punch_in, punch_out) = match session.plan.gate {
            GateSpec::Punch {
                punch_in,
                punch_out,
            } => ("punch", Some(punch_in.value()), Some(punch_out.value())),
            GateSpec::Normal { .. } => ("normal", None, None),
            // 本 slice は Normal/Punch のみ構築する。万が一 plan が
            // 他 variant を持っても gate 名を偽らない。
            GateSpec::Loop { .. } => ("loop", None, None),
            GateSpec::PunchInLoop { .. } => ("punch_in_loop", None, None),
        };
        let transport = if session.playing {
            "playing"
        } else if session.paused {
            "paused"
        } else {
            "stopped"
        };
        let phase = if session.playing {
            match (self.punch.punch_in, self.punch.punch_out) {
                (Some(punch_in), Some(punch_out)) => {
                    let pos = session.position;
                    if pos < punch_in {
                        "punch_wait"
                    } else if pos < punch_out {
                        "punch_open"
                    } else {
                        "post_roll"
                    }
                }
                _ => "playing",
            }
        } else {
            // paused も gate 閉の「停止中」として `Stopped` に写像
            // する（decide 語彙に Paused は無い。transport field が
            // paused を正直に保持する）。
            "stopped"
        };
        let in_session = position_frame.is_some_and(|f| self.in_session(f));
        json!({
            "active": true,
            "transport_start": session.transport_start.value(),
            "stop": session.stop.value(),
            "gate": gate,
            "punch_in": punch_in,
            "punch_out": punch_out,
            "position": session.position.value(),
            "position_frame": position_frame,
            "in_session": in_session,
            "transport": transport,
            "phase": phase,
            "sample_rate": session.sample_rate,
            "session_end_frames": session.plan.session_end.value(),
            "clock_epoch": session.clock_epoch.to_tagged(),
        })
    }

    /// 宣言 track ごとの `MonitorProbe` segment 列（bounded）。
    /// `phase` は `Playing` 固定で feed する — `decide` は
    /// `Stopped` 以外を同一視し、区間の開閉は scheduler 由来の
    /// `gate_open` が担うため、tick 空間の gate 再実装をしない。
    fn segments(
        &self,
        contexts: &[(TrackId, TrackMonitorContext)],
    ) -> Vec<(TrackId, TrackSegments)> {
        let Some(session) = self.session.as_ref() else {
            return Vec::new();
        };
        contexts
            .iter()
            .take(PROBE_TRACK_CAP)
            .map(|(track_id, ctx)| {
                let (segments, truncated) = probe_segments(&session.plan, ctx);
                (
                    *track_id,
                    TrackSegments {
                        segments,
                        truncated,
                    },
                )
            })
            .collect()
    }

    /// session 内の session-frame か。
    fn in_session(&self, frame: i64) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| (0..s.plan.session_end.value()).contains(&frame))
    }

    /// live route — transport 位相と `gate_open_now` で `decide`
    /// を呼ぶ。停止中（paused 含む）は `Stopped` 相当に写像する。
    fn live_routes(
        &self,
        contexts: &[(TrackId, TrackMonitorContext)],
        gate_open_now: bool,
    ) -> BTreeMap<TrackId, synvelis_recording::MonitorDecision> {
        let playing = self.session.as_ref().is_some_and(|s| s.playing);
        let phase = if playing {
            TransportPhase::Playing
        } else {
            TransportPhase::Stopped
        };
        contexts
            .iter()
            .map(|(track_id, ctx)| (*track_id, decide(ctx, phase, gate_open_now)))
            .collect()
    }

    /// latency 注入値の解決投影 — `LatencyReport::resolve` と
    /// `prepare_low_latency` の結果を verbatim に写す。
    fn latency_json(&self) -> Value {
        let low: LowLatencyMonitor =
            prepare_low_latency(self.low_latency, self.tone_difference, &self.latency);
        let resolution = &low.latency;
        json!({
            "resolution": {
                "total_frames": resolution.total_frames,
                "quality": quality_str(resolution.quality),
                "basis": resolution.basis.map(resolution_basis_str),
                "clock_epoch": resolution.clock_epoch.map(ruviform_media_core::ClockEpochId::get),
                "excluded_reported": resolution.excluded_reported,
            },
            "low_latency": {
                "state": low_latency_state_str(low.state),
                "reason": low.reason.map(low_latency_reason_str),
                "tone_difference": low.tone_difference,
            },
        })
    }
}

/// punch マークの片側指定。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PunchSide {
    /// punch in。
    In,
    /// punch out。
    Out,
}

/// 1 track 分の segment 投影結果。
struct TrackSegments {
    /// session-frame 半開区間の連続列。
    segments: Vec<MonitorSegment>,
    /// `SEGMENT_CAP` または probe fault で打ち切られたか。
    truncated: bool,
}

/// `frame` が segment の半開 `[start, end)` に含まれるか。
fn segment_contains(seg: &MonitorSegment, frame: i64) -> bool {
    let start = i64::from(seg.session.start());
    let end = i64::from(seg.session.end());
    start <= frame && frame < end
}

/// `plan` の gate scheduler を `MonitorProbe` で全 session 分
/// 駆動し、segment を収集する（非 RT・bounded）。
fn probe_segments(
    plan: &PreparedRecordingPlan,
    ctx: &TrackMonitorContext,
) -> (Vec<MonitorSegment>, bool) {
    let mut probe = MonitorProbe::new(plan, ctx.clone());
    let mut out = BoundedVec::with_capacity(64);
    let mut all = Vec::new();
    let end = plan.session_end.value();
    let mut truncated = false;
    while !probe.finished() {
        let remaining = end - probe.cursor().value();
        if remaining <= 0 {
            break;
        }
        let Ok(frames) = u32::try_from(remaining.min(FEED_BLOCK)) else {
            break; // remaining.min(FEED_BLOCK) ≤ 2^24 なので到達不能
        };
        probe.feed(frames, TransportPhase::Playing, &mut out);
        if probe.faulted() {
            truncated = true;
            break;
        }
        for seg in out.as_slice() {
            if push_segment_merged(&mut all, *seg) {
                continue;
            }
            if all.len() >= SEGMENT_CAP {
                truncated = true;
                break;
            }
            all.push(*seg);
        }
        if truncated {
            break;
        }
    }
    (all, truncated || !probe.finished())
}

/// `seg` が末尾 segment と同一決定（`gate_open`/`route`/`notices`
/// 一致・半開連続）なら末尾の終端を伸ばして併合し `true` を返す。
/// `MonitorProbe` の segment は feed block 粒度で分割されるため、
/// 併合しないと単純 gate でも session 長に比例した本数になり
/// `SEGMENT_CAP` が実用 session を打ち切ってしまう。併合は表示上の
/// 区間統合であり gate 意味論の再実装ではない（scheduler 由来の
/// `gate_open` と `decide` 結果をそのまま比較するだけ）。
fn push_segment_merged(all: &mut [MonitorSegment], seg: MonitorSegment) -> bool {
    let Some(last) = all.last_mut() else {
        return false;
    };
    if last.gate_open != seg.gate_open
        || last.route != seg.route
        || last.notices != seg.notices
        || last.session.end() != seg.session.start()
    {
        return false;
    }
    let Ok(range) = SessionRange::try_new(last.session.start(), seg.session.end()) else {
        return false;
    };
    last.session = range;
    true
}

fn arm_str(arm: ArmState) -> &'static str {
    match arm {
        ArmState::Off => "off",
        ArmState::On => "on",
    }
}

fn monitor_str(monitor: MonitorState) -> &'static str {
    match monitor {
        MonitorState::Off => "off",
        MonitorState::Auto => "auto",
        MonitorState::On => "on",
    }
}

fn hw_state_str(state: HardwareDirectState) -> &'static str {
    match state {
        HardwareDirectState::DeviceControlled => "device_controlled",
        HardwareDirectState::UserReported => "user_reported",
        HardwareDirectState::Unknown => "unknown",
    }
}

fn route_str(route: MonitorRoute) -> &'static str {
    match route {
        MonitorRoute::Input => "input",
        MonitorRoute::Playback => "playback",
        MonitorRoute::Muted => "muted",
    }
}

fn binding_kind(map: &ChannelMap) -> &'static str {
    match map.len() {
        1 => "mono",
        2 => "stereo",
        _ => "multichannel",
    }
}

fn notices_json(decision: synvelis_recording::MonitorDecision) -> Value {
    json!({
        "double_monitoring": decision.notices.double_monitoring,
        "hardware_direct_unknown": decision.notices.hardware_direct_unknown,
        "input_unavailable": decision.notices.input_unavailable,
    })
}

fn push_notice(
    notices: &mut Value,
    key: &str,
    track: TrackId,
    decision: synvelis_recording::MonitorDecision,
) {
    let flag = match key {
        "double_monitoring" => decision.notices.double_monitoring,
        "hardware_direct_unknown" => decision.notices.hardware_direct_unknown,
        _ => decision.notices.input_unavailable,
    };
    if flag && let Some(list) = notices.get_mut(key).and_then(Value::as_array_mut) {
        list.push(json!(track.to_tagged()));
    }
}

fn segment_json(seg: &MonitorSegment) -> Value {
    json!({
        "start": i64::from(seg.session.start()),
        "end": i64::from(seg.session.end()),
        "gate_open": seg.gate_open,
        "route": route_str(seg.route),
        "notices": notices_json(synvelis_recording::MonitorDecision {
            route: seg.route,
            notices: seg.notices,
        }),
    })
}

fn quality_str(q: LatencyQuality) -> &'static str {
    match q {
        LatencyQuality::Unknown => "unknown",
        LatencyQuality::Estimated => "estimated",
        LatencyQuality::Reported => "reported",
        LatencyQuality::Measured => "measured",
    }
}

fn resolution_basis_str(b: LatencyResolutionBasis) -> &'static str {
    match b {
        LatencyResolutionBasis::DeviceTimestamp => "device_timestamp",
        LatencyResolutionBasis::CallbackReported => "callback_reported",
        LatencyResolutionBasis::RoundTripMeasured => "round_trip_measured",
    }
}

fn low_latency_state_str(s: LowLatencyState) -> &'static str {
    match s {
        LowLatencyState::Ready => "ready",
        LowLatencyState::Degraded => "degraded",
        LowLatencyState::Unavailable => "unavailable",
    }
}

fn low_latency_reason_str(r: LowLatencyReason) -> &'static str {
    match r {
        LowLatencyReason::NotPrepared => "not_prepared",
        LowLatencyReason::InputNotReady => "input_not_ready",
        LowLatencyReason::OutputNotReady => "output_not_ready",
    }
}
