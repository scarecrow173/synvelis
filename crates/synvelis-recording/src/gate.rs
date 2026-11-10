//! Gate schedule: 録音対象区間の有限先読み表現。
//!
//! `GateScheduler` が `GateSpec` を「パス基準 session-frame オフセット +
//! project tick 区間」のテンプレート列へコンパイルする（非 RT）。
//! `TempoMap` 境界では span を分割し、各 pass は同一テンプレートを
//! `pass_len` ずつ平行移動した span 列になる。`GateSchedule` は
//! 有界 window + watermark（`scheduled_until`）+ 生成カーソルを持ち、
//! `extend`（非 RT）で先読みを補充し、`evaluate_gate`（RT）が
//! ブロックとの交差 `CapturePiece` を engine 所有 scratch へ書き出す。

#![allow(clippy::module_name_repetitions)]

use synvelis_project::{Error, MusicalTick, SessionFrame, SessionRange, TickTimeMapping};

use crate::{
    bounded::{BoundedFull, BoundedVec},
    plan::GateSpec,
};

/// 1 本の open 区間（半開 `[in, out)`）。Loop では `pass_index` が
/// pass を区別し、`project_*` は同一 project 区間を指す。
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GateSpan {
    /// session 上の半開区間。
    pub session: SessionRange,
    /// Loop pass 番号（非 Loop では 0）。
    pub pass_index: u32,
    /// 区間先頭に対応する project tick。
    pub project_in: MusicalTick,
    /// 区間終端（exclusive）に対応する project tick。
    pub project_out: MusicalTick,
}

/// ブロック内の 1 保存片。`session_in`/`session_out` は半開。
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CapturePiece {
    /// 発生した pass。
    pub pass_index: u32,
    /// piece 先頭の `SessionFrame`。
    pub session_in: SessionFrame,
    /// piece 終端（exclusive）。
    pub session_out: SessionFrame,
    /// piece 先頭に対応する project tick。
    pub project_in: MusicalTick,
    /// piece 終端に対応する project tick。
    pub project_out: MusicalTick,
}

/// Gate 評価の失敗分類（RT 上で Fault bit へ変換される）。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GateError {
    /// `scheduled_until` を超えるブロックを評価しようとした。
    Underrun,
    /// piece を入れる scratch/span 容量不足。
    SpanCapacity,
}

/// パス内の 1 span テンプレート（pass 先頭からの frame オフセット）。
#[derive(Copy, Clone, Debug)]
struct Template {
    off_in: i64,
    off_out: i64,
    project_in: MusicalTick,
    project_out: MusicalTick,
}

/// 生成カーソル。`GateSchedule` 内に保持し、stream ごとに独立する。
#[derive(Copy, Clone, Debug)]
struct SpanCursor {
    pass: u32,
    template: usize,
    exhausted: bool,
}

/// `GateSpec` をコンパイルした schedule 生成器。無状態で全 stream に共有。
pub struct GateScheduler {
    templates: Vec<Template>,
    /// Loop 系: `loop_start` の session-frame。非 Loop: 0。
    pass_base: i64,
    /// 1 pass あたりの frame 長。非 Loop では使用しない。
    pass_len: i64,
    /// Loop 系では true（無限 pass）、Normal/Punch は pass 0 のみ。
    repeating: bool,
    session_end: SessionFrame,
}

/// 有界 lookahead window。`scheduled_until` 未満の全 frame は
/// 「open span か closed gap」のいずれかに分類済みであることを保証する。
pub struct GateSchedule {
    spans: BoundedVec<GateSpan>,
    scheduled_until: SessionFrame,
    cursor: SpanCursor,
}

impl GateSchedule {
    /// `lookahead` 本の span を保持する空 schedule。
    #[must_use]
    pub fn new(lookahead: usize) -> Self {
        Self {
            spans: BoundedVec::with_capacity(lookahead),
            scheduled_until: SessionFrame::ZERO,
            cursor: SpanCursor {
                pass: 0,
                template: 0,
                exhausted: false,
            },
        }
    }

    /// 全 frame `< scheduled_until` は分類済み。
    #[must_use]
    pub fn scheduled_until(&self) -> SessionFrame {
        self.scheduled_until
    }

    /// 未消費 span 数。
    #[must_use]
    pub fn pending(&self) -> usize {
        self.spans.len()
    }

    /// 未消費 span のスライス（テスト・診断用）。
    #[must_use]
    pub fn spans(&self) -> &[GateSpan] {
        self.spans.as_slice()
    }
}

/// 先頭から `frame_span` frame の session 範囲 ↔ project 範囲
/// `[p_in, p_out)` の線形対応で、オフセット `off` の project tick を返す。
/// `off` は `[0, frame_span]` 内を仮定（端点は厳密に project 境界へ写る）。
#[must_use]
fn interp_project(frame_span: i64, p_in: MusicalTick, p_out: MusicalTick, off: i64) -> MusicalTick {
    let tick_span = i128::from(p_out.value() - p_in.value());
    let tick = i128::from(p_in.value()) + i128::from(off) * tick_span / i128::from(frame_span);
    // 補間結果は project_in..=project_out に収まる（frame_span > 0、off >= 0）。
    #[allow(clippy::cast_possible_truncation)]
    MusicalTick::new(tick as i64)
}

/// span の `pos` に対応する project tick を線形補間する。
/// `pos` は `[in, out]` 内を仮定（端点は厳密に project 境界へ写る）。
#[must_use]
fn project_at(span: &GateSpan, pos: SessionFrame) -> MusicalTick {
    let in_v = span.session.start().value();
    interp_project(
        span.session.end().value() - in_v,
        span.project_in,
        span.project_out,
        pos.value() - in_v,
    )
}

/// tick 区間 `[a, b)` を `tempo_ticks` 内の厳密に内側の点で分割し、
/// 各区間を `(frame オフセット, project 区間)` の Template にする。
/// `base_frame` は `a` の絶対 frame（オフセット基準）。
fn compile_region(
    mapping: &TickTimeMapping,
    sample_rate: u32,
    base_frame: i64,
    region: (MusicalTick, MusicalTick),
    tempo_ticks: &[i64],
    templates: &mut Vec<Template>,
) -> Result<(), Error> {
    let (a, b) = region;
    let mut seg_in = a;
    for &t in tempo_ticks {
        if t > a.value() && t < b.value() {
            push_template(
                mapping,
                sample_rate,
                base_frame,
                seg_in,
                MusicalTick::new(t),
                templates,
            )?;
            seg_in = MusicalTick::new(t);
        }
    }
    push_template(mapping, sample_rate, base_frame, seg_in, b, templates)
}

fn push_template(
    mapping: &TickTimeMapping,
    sample_rate: u32,
    base_frame: i64,
    seg_in: MusicalTick,
    seg_out: MusicalTick,
    templates: &mut Vec<Template>,
) -> Result<(), Error> {
    let f_in = mapping.tick_to_media(seg_in, 1, sample_rate)?.value();
    let f_out = mapping.tick_to_media(seg_out, 1, sample_rate)?.value();
    templates.push(Template {
        off_in: f_in.checked_sub(base_frame).ok_or(Error::Overflow)?,
        off_out: f_out.checked_sub(base_frame).ok_or(Error::Overflow)?,
        project_in: seg_in,
        project_out: seg_out,
    });
    Ok(())
}

impl GateScheduler {
    /// Gate 仕様を session-frame オフセットのテンプレート列へコンパイルする。
    ///
    /// `tempo_ticks` は tempo event の tick 列（順不同可。内部で sort/dedup）。
    ///
    /// # Errors
    /// [`Error::Overflow`]：tick↔frame 変換または差分のオーバーフロー。
    pub(crate) fn new(
        mapping: &TickTimeMapping,
        sample_rate: u32,
        stop: MusicalTick,
        gate: GateSpec,
        transport_start_frame: i64,
        session_end: SessionFrame,
        tempo_ticks: &[i64],
    ) -> Result<Self, Error> {
        let mut ticks = tempo_ticks.to_vec();
        ticks.sort_unstable();
        ticks.dedup();
        let s = |tick: MusicalTick| -> Result<i64, Error> {
            let f = mapping.tick_to_media(tick, 1, sample_rate)?.value();
            f.checked_sub(transport_start_frame).ok_or(Error::Overflow)
        };

        let mut templates = Vec::new();
        let (pass_base, pass_len, repeating) = match gate {
            GateSpec::Normal { open } => {
                // 非 Loop: 単一 pass 0、基準は transport_start。
                compile_region(
                    mapping,
                    sample_rate,
                    transport_start_frame,
                    (open, stop),
                    &ticks,
                    &mut templates,
                )?;
                (0, 0, false)
            }
            GateSpec::Punch {
                punch_in,
                punch_out,
            } => {
                compile_region(
                    mapping,
                    sample_rate,
                    transport_start_frame,
                    (punch_in, punch_out),
                    &ticks,
                    &mut templates,
                )?;
                (0, 0, false)
            }
            GateSpec::Loop {
                loop_start,
                loop_end,
            }
            | GateSpec::PunchInLoop {
                loop_start,
                loop_end,
                ..
            } => {
                let base = s(loop_start)?;
                let len = s(loop_end)?.checked_sub(base).ok_or(Error::Overflow)?;
                let (cap_in, cap_out) = match gate {
                    GateSpec::Loop { .. } => (loop_start, loop_end),
                    GateSpec::PunchInLoop {
                        punch_in,
                        punch_out,
                        ..
                    } => (punch_in, punch_out),
                    GateSpec::Normal { .. } | GateSpec::Punch { .. } => unreachable!(),
                };
                let base_frame = mapping.tick_to_media(loop_start, 1, sample_rate)?.value();
                compile_region(
                    mapping,
                    sample_rate,
                    base_frame,
                    (cap_in, cap_out),
                    &ticks,
                    &mut templates,
                )?;
                (base, len, true)
            }
        };
        Ok(Self {
            templates,
            pass_base,
            pass_len,
            repeating,
            session_end,
        })
    }

    /// カーソル位置の次 span（未生成）。
    fn next_span(&self, cursor: SpanCursor) -> Option<(i64, i64, u32, MusicalTick, MusicalTick)> {
        if cursor.exhausted || cursor.template >= self.templates.len() {
            return None;
        }
        if !self.repeating && cursor.pass > 0 {
            return None;
        }
        let t = self.templates[cursor.template];
        let base = self.pass_base + i64::from(cursor.pass) * self.pass_len;
        let session_in = base.checked_add(t.off_in)?;
        let session_out = base.checked_add(t.off_out)?;
        Some((
            session_in,
            session_out,
            cursor.pass,
            t.project_in,
            t.project_out,
        ))
    }

    /// `schedule` の lookahead window を `until` まで補充する（非 RT）。
    ///
    /// watermark は `min(until, 次 span の session_in)`（枯渇時は `until`）。
    /// すべての frame `< scheduled_until` が分類済みになる。
    ///
    /// # Errors
    /// 現時点では infallible（window 満杯は水位制御で扱う）。将来の
    /// 生成側失敗に備え `Result` を返す。
    pub fn extend(&self, schedule: &mut GateSchedule, until: SessionFrame) -> Result<(), Error> {
        schedule.spans.compact();
        while let Some((in_v, out_v, pass, p_in, p_out)) = self.next_span(schedule.cursor) {
            if in_v >= self.session_end.value() {
                // session 以降の span は生成しない（最終 pass の途中停止）。
                schedule.cursor.exhausted = true;
                break;
            }
            if in_v >= until.value() {
                break;
            }
            let out_capped = out_v.min(self.session_end.value());
            // session_end で打ち切った span の project_out は、打ち切り
            // 位置を template の session↔project 対応で補間し直す。
            // template 終端をそのまま名乗ると、短い末尾区間が gate 全域へ
            // 引き伸ばされて commit の segment/clip 配置がずれる。
            let p_out = if out_capped < out_v {
                interp_project(out_v - in_v, p_in, p_out, out_capped - in_v)
            } else {
                p_out
            };
            let session_in = SessionFrame::new(in_v)?;
            let session_out = SessionFrame::new(out_capped)?;
            let span = GateSpan {
                session: SessionRange::try_new(session_in, session_out)?,
                pass_index: pass,
                project_in: p_in,
                project_out: p_out,
            };
            if schedule.spans.push(span).is_err() {
                break;
            }
            schedule.cursor.template += 1;
            if schedule.cursor.template >= self.templates.len() {
                schedule.cursor.template = 0;
                if self.repeating {
                    schedule.cursor.pass = schedule.cursor.pass.saturating_add(1);
                } else {
                    schedule.cursor.exhausted = true;
                }
            }
        }
        // watermark: 次 span の in、枯渇なら i64::MAX 扱い。
        let next_in = self
            .next_span(schedule.cursor)
            .map_or(i64::MAX, |(in_v, ..)| in_v);
        let wm = until.value().min(next_in);
        schedule.scheduled_until = SessionFrame::new(wm.max(0))?;
        Ok(())
    }
}

/// `block`（半開 session 区間）と window 内 span の交差を `out` へ列挙する（RT）。
///
/// `block.end > scheduled_until` は [`GateError::Underrun`]。先に
/// `extend` で `until >= block.end` を満たしてから呼ぶこと。
/// 期限切れ span（`end <= block.start`）は `head` 前進で消費する。
///
/// # Errors
/// [`GateError::Underrun`]：watermark 不足。
/// [`GateError::SpanCapacity`]：`out` の容量不足。
pub fn evaluate_gate(
    schedule: &mut GateSchedule,
    block: SessionRange,
    out: &mut BoundedVec<CapturePiece>,
) -> Result<(), GateError> {
    if block.end() > schedule.scheduled_until {
        return Err(GateError::Underrun);
    }
    // 期限切れ span の消費。
    while let Some(s) = schedule.spans.front() {
        if s.session.end() <= block.start() {
            schedule.spans.advance(1);
        } else {
            break;
        }
    }
    for span in schedule.spans.as_slice() {
        if span.session.start() >= block.end() {
            break;
        }
        let piece_in = span.session.start().max(block.start());
        let piece_out = span.session.end().min(block.end());
        if piece_in >= piece_out {
            continue;
        }
        let piece = CapturePiece {
            pass_index: span.pass_index,
            session_in: piece_in,
            session_out: piece_out,
            project_in: project_at(span, piece_in),
            project_out: project_at(span, piece_out),
        };
        out.push(piece)
            .map_err(|BoundedFull(_)| GateError::SpanCapacity)?;
    }
    Ok(())
}
