//! 非 RT 前処理（Preflight）と確定録音計画。
//!
//! `RecordingRequest` を検証し、RT 開始までに凍結する値
//! （Capture Target・Clock Epoch・TempoMap 写像・バッファ予算・
//! session 終端）を [`PreparedRecordingPlan`] として束ねる。
//! Gate 仕様は半開 `[in, out)` で受け取り、ここで範囲検証・
//! クランプ済みの形に正規化する。

use synvelis_project::{
    ClockEpochId, Error, LaneId, MusicalTick, Project, SessionFrame, TempoMapRef, TickTimeMapping,
};

use crate::gate::GateScheduler;

/// 録音モードに対応する Gate 仕様。全境界は半開 `[in, out)`。
///
/// `session` 開始は常に `transport_start`。`stop` は録音全体の
/// 停止位置（Loop では最終パスの途中停止を含む）。
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GateSpec {
    /// `open` から `stop` まで連続録音（Punch なし）。
    Normal {
        /// 録音開始 tick（`transport_start` と独立可）。
        open: MusicalTick,
    },
    /// `[punch_in, punch_out)` のみ保存。範囲外は走行のみ。
    Punch {
        /// Punch 開始 tick。
        punch_in: MusicalTick,
        /// Punch 終了 tick。`stop` を超える分は `stop` にクランプ。
        punch_out: MusicalTick,
    },
    /// `[loop_start, loop_end)` を `stop` まで繰り返す。
    Loop {
        /// Loop 区間先頭 tick。
        loop_start: MusicalTick,
        /// Loop 区間終端 tick（`loop_start` より大きいこと）。
        loop_end: MusicalTick,
    },
    /// Loop 内の `[punch_in, punch_out)` のみ保存。
    PunchInLoop {
        /// Loop 区間先頭 tick。
        loop_start: MusicalTick,
        /// Loop 区間終端 tick。
        loop_end: MusicalTick,
        /// Punch 開始 tick（Loop 区間内）。
        punch_in: MusicalTick,
        /// Punch 終了 tick（Loop 区間内）。
        punch_out: MusicalTick,
    },
}

/// 録音先の識別。各 Target は独立した SPSC 対を持つ単一ストリーム。
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureTarget {
    /// 録音先 lane。
    pub lane_id: LaneId,
    /// チャネル数（1 以上）。
    pub channels: u32,
}

/// 1 ストリーム分のバッファ予算。全容量は 1 以上。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BufferBudget {
    /// Chunk 数。Free Pool 容量と一致しなければならない。
    pub chunks: usize,
    /// 1 Chunk あたりの最大 frame 数。
    pub chunk_frames: u32,
    /// Chunk 内の `CaptureSpan` 上限。
    pub spans_per_chunk: usize,
    /// RT→非 RT Filled Queue 容量。
    pub filled_capacity: usize,
    /// Gate schedule 先読み span 数（1 以上）。
    pub lookahead_spans: usize,
}

/// Preflight 入力。`tempo_map` は Project の snapshot 参照（revision を検証する）。
#[derive(Clone, Debug)]
pub struct RecordingRequest {
    /// 録音先ストリーム（1 以上）。
    pub targets: Vec<CaptureTarget>,
    /// 期待する Clock Epoch。
    pub clock_epoch: ClockEpochId,
    /// プロジェクト `TempoMap` snapshot（revision/PPQN を照合する）。
    pub tempo_map: TempoMapRef,
    /// Transport 開始 tick（負 Pre-roll 可）。
    pub transport_start: MusicalTick,
    /// 停止 tick。
    pub stop: MusicalTick,
    /// Gate 仕様。
    pub gate: GateSpec,
    /// バッファ予算。
    pub budget: BufferBudget,
    /// 1 コールバックの最大 frame 数。
    pub max_block_frames: u32,
}

/// Preflight 合格後の確定計画。RT 開始後は全値が不変。
pub struct PreparedRecordingPlan {
    /// 凍結済み録音先。
    pub targets: Vec<CaptureTarget>,
    /// 凍結済み Clock Epoch。
    pub clock_epoch: ClockEpochId,
    /// Tick↔frame 写像（凍結済み revision 由来）。
    pub mapping: TickTimeMapping,
    /// `transport_start` の絶対 media frame。
    pub transport_start_frame: i64,
    /// 確定済み Gate（クランプ適用済み）。
    pub gate: GateSpec,
    /// session 終端の `SessionFrame`（> 0）。
    pub session_end: SessionFrame,
    /// バッファ予算。
    pub budget: BufferBudget,
    /// `max_block_frames`（検証済み: ≤ `chunk_frames`）。
    pub max_block_frames: u32,
    /// 構築時の session sample rate [Hz]。engine sample rate との
    /// 一致検証・frame↔tick 写像の基準として保持する。
    pub sample_rate: u32,
    /// Gate scheduler コンパイラ（全ストリームで共有・無状態）。
    pub scheduler: GateScheduler,
}

/// tick → 絶対 media frame（sample rate = mapping 時点の `num/den` = 1/rate）。
fn tick_frame(
    mapping: &TickTimeMapping,
    tick: MusicalTick,
    sample_rate: u32,
) -> Result<i64, Error> {
    let v = mapping.tick_to_media(tick, 1, sample_rate)?;
    Ok(v.value())
}

/// `request` を `project` に照らして検証し、凍結済み計画を返す。
///
/// # Errors
/// [`Error::Validation`]/[`Error::NotFound`]/[`Error::Overflow`]：
/// 参照不整合・範囲違反・容量違反・`TempoMap` revision/PPQN 不一致。
#[allow(clippy::too_many_lines)] // sequential validation checks
pub fn preflight(
    project: &Project,
    request: RecordingRequest,
    sample_rate: u32,
) -> Result<PreparedRecordingPlan, Error> {
    // --- 参照・容量 ---
    if request.targets.is_empty() {
        return Err(Error::Validation("no capture targets".into()));
    }
    for target in &request.targets {
        if !project.lanes.contains_key(&target.lane_id) {
            return Err(Error::NotFound(format!("lane {}", target.lane_id)));
        }
        if target.channels == 0 {
            return Err(Error::Validation("channels must be >= 1".into()));
        }
    }
    if !project.clock_epochs.contains_key(&request.clock_epoch) {
        return Err(Error::NotFound(format!(
            "clock epoch {}",
            request.clock_epoch
        )));
    }
    // TempoMap revision / PPQN は request snapshot と Project 側で一致必須。
    if request.tempo_map.revision != project.tempo_map.revision {
        return Err(Error::Validation(format!(
            "tempo map revision mismatch: request {} vs project {}",
            request.tempo_map.revision, project.tempo_map.revision
        )));
    }
    let mapping = TickTimeMapping::new(&project.tempo_map)?;
    if mapping.ppqn() != project.ppqn.0 {
        return Err(Error::Validation(format!(
            "ppqn mismatch: request {} vs project {}",
            mapping.ppqn(),
            project.ppqn.0
        )));
    }

    let b = request.budget;
    if b.chunks == 0
        || b.chunk_frames == 0
        || b.spans_per_chunk == 0
        || b.filled_capacity == 0
        || b.lookahead_spans == 0
    {
        return Err(Error::Validation("budget capacities must be >= 1".into()));
    }
    if b.filled_capacity > b.chunks {
        return Err(Error::Validation(
            "filled_capacity exceeds chunk count".into(),
        ));
    }
    if request.max_block_frames == 0 {
        return Err(Error::Validation("max_block_frames must be >= 1".into()));
    }
    if request.max_block_frames > b.chunk_frames {
        return Err(Error::Validation(
            "max_block_frames exceeds chunk_frames".into(),
        ));
    }

    // --- Gate 境界（tick 空間で半開検証。stop は transport_start より後）---
    let ts = request.transport_start;
    let stop = request.stop;
    if stop <= ts {
        return Err(Error::Validation(
            "stop must be after transport_start".into(),
        ));
    }
    let gate = match request.gate {
        GateSpec::Normal { open } => {
            if open < ts {
                return Err(Error::Validation(
                    "normal open precedes transport_start".into(),
                ));
            }
            if open >= stop {
                return Err(Error::Validation("normal open is at/after stop".into()));
            }
            GateSpec::Normal { open }
        }
        GateSpec::Punch {
            punch_in,
            punch_out,
        } => {
            if punch_out <= punch_in {
                return Err(Error::Validation("punch_out must be after punch_in".into()));
            }
            if punch_in < ts {
                return Err(Error::Validation(
                    "punch_in precedes transport_start".into(),
                ));
            }
            if punch_in >= stop {
                return Err(Error::Validation("punch_in is at/after stop".into()));
            }
            // punch_out > stop は stop へクランプ（半開上端は超過可）。
            GateSpec::Punch {
                punch_in,
                punch_out: punch_out.min(stop),
            }
        }
        GateSpec::Loop {
            loop_start,
            loop_end,
        } => {
            if loop_end <= loop_start {
                return Err(Error::Validation(
                    "loop_end must be after loop_start".into(),
                ));
            }
            if loop_start >= stop {
                return Err(Error::Validation("loop_start is at/after stop".into()));
            }
            GateSpec::Loop {
                loop_start,
                loop_end,
            }
        }
        GateSpec::PunchInLoop {
            loop_start,
            loop_end,
            punch_in,
            punch_out,
        } => {
            if loop_end <= loop_start {
                return Err(Error::Validation(
                    "loop_end must be after loop_start".into(),
                ));
            }
            if punch_out <= punch_in || punch_in < loop_start || punch_out > loop_end {
                return Err(Error::Validation(
                    "punch range must be non-empty and inside loop range".into(),
                ));
            }
            if loop_start >= stop {
                return Err(Error::Validation("loop_start is at/after stop".into()));
            }
            GateSpec::PunchInLoop {
                loop_start,
                loop_end,
                punch_in,
                punch_out,
            }
        }
    };

    // --- session 終端（> 0 必須）---
    let transport_start_frame = tick_frame(&mapping, ts, sample_rate)?;
    let stop_frame = tick_frame(&mapping, stop, sample_rate)?;
    let end_value = stop_frame
        .checked_sub(transport_start_frame)
        .ok_or(Error::Overflow)?;
    if end_value <= 0 {
        return Err(Error::Validation("session_end must be positive".into()));
    }
    let session_end = SessionFrame::new(end_value)?;

    let tempo_ticks: Vec<i64> = project
        .tempo_map
        .tempo_events
        .iter()
        .map(|e| e.tick)
        .collect();
    let scheduler = GateScheduler::new(
        &mapping,
        sample_rate,
        stop,
        gate,
        transport_start_frame,
        session_end,
        &tempo_ticks,
    )?;

    Ok(PreparedRecordingPlan {
        targets: request.targets,
        clock_epoch: request.clock_epoch,
        mapping,
        transport_start_frame,
        gate,
        session_end,
        budget: request.budget,
        max_block_frames: request.max_block_frames,
        sample_rate,
        scheduler,
    })
}
