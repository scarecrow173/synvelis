//! MIDI 1.0 Standard MIDI File (SMF) Format 0/1 codec and the
//! project provenance document (issue #126, TIM-02).
//!
//! [`parse_smf`] decodes SMF bytes into a [`SmfDocument`] — an
//! event-level, source-native representation that preserves tick
//! positions, track/channel identity, ordering, tempo/time-signature
//! meta events, system-exclusive payloads, and every other meta event
//! as raw bytes. The document is *provenance*: it is never executed
//! directly. Note and controller *projections* ([`SmfDocument::
//! paired_notes`], [`SmfDocument::controller_events`]) are explicit,
//! deterministic derivations — unknown event kinds stay inside the
//! document and cannot silently become playable MIDI.
//!
//! [`export_smf`] renders a project's MIDI source back to SMF bytes.
//! A source carrying a provenance document re-emits that document
//! (optionally re-divided and/or re-formatted); a source without one
//! synthesizes a minimal Format 0/1 file from its [`MidiNote`]s.
//! Every native→target tick conversion is accounted for in
//! [`SmfConversionReport`] — there is no implicit quantization.
//!
//! Scope is deliberately narrow: Format 0 and Format 1 with PPQN
//! division only. SMPTE time bases, Format 2, and MIDI 2.0/MPE are
//! rejected as [`SmfError::Unsupported`] — never silently accepted.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    id::SourceId,
    midi_comp::{MidiControllerEvent, MidiControllerKind},
    project::Project,
    time::MidiTick,
};

/// SMF format selector — the only two formats this slice supports.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmfFormat {
    /// Single interleaved track.
    Format0,
    /// Conductor/meta track plus one or more event tracks.
    Format1,
}

/// One SMF event kind. Channel events carry their channel; meta and
/// sysex events do not. Unknown meta types are preserved verbatim as
/// [`SmfEventKind::MetaOther`] — typed provenance, never executable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SmfEventKind {
    /// Note On (velocity 1–127; a wire velocity-0 Note On is
    /// normalized to [`SmfEventKind::NoteOff`] at parse time).
    NoteOn {
        /// MIDI channel (0–15).
        channel: u8,
        /// Key (0–127).
        key: u8,
        /// Note-on velocity (1–127).
        velocity: u8,
    },
    /// Note Off (release velocity 0–127).
    NoteOff {
        /// MIDI channel (0–15).
        channel: u8,
        /// Key (0–127).
        key: u8,
        /// Release velocity (0–127).
        velocity: u8,
    },
    /// Polyphonic key pressure.
    PolyAftertouch {
        /// MIDI channel (0–15).
        channel: u8,
        /// Key (0–127).
        key: u8,
        /// Pressure value (0–127).
        value: u8,
    },
    /// Control change — CC64 sustain is one instance.
    Cc {
        /// MIDI channel (0–15).
        channel: u8,
        /// Controller number (0–127).
        controller: u8,
        /// Controller value (0–127).
        value: u8,
    },
    /// Program change.
    ProgramChange {
        /// MIDI channel (0–15).
        channel: u8,
        /// Program number (0–127).
        program: u8,
    },
    /// Channel pressure.
    ChannelAftertouch {
        /// MIDI channel (0–15).
        channel: u8,
        /// Pressure value (0–127).
        value: u8,
    },
    /// Pitch bend (14-bit, 0–16383; 8192 is center).
    PitchBend {
        /// MIDI channel (0–15).
        channel: u8,
        /// Bend value (0–16383).
        value: u16,
    },
    /// End of track meta event.
    EndOfTrack,
    /// Tempo meta event — microseconds per quarter note.
    Tempo {
        /// µs per quarter note (fits the SMF u24 domain).
        us_per_quarter: u32,
    },
    /// Time signature meta event. `denominator` preserves the raw
    /// SMF byte — the actual denominator is `2^denominator`.
    TimeSignature {
        /// Beats per bar (as stored).
        numerator: u8,
        /// Log2 of the beat unit denominator (raw SMF byte).
        denominator: u8,
        /// 32nd notes per quarter note.
        thirtyseconds: u8,
        /// MIDI clocks per metronome click.
        clocks: u8,
    },
    /// Track name meta event (raw bytes — not necessarily UTF-8).
    TrackName {
        /// Name payload bytes.
        bytes: Vec<u8>,
    },
    /// Any other meta event, preserved verbatim.
    MetaOther {
        /// Meta event type byte.
        meta_kind: u8,
        /// Meta payload bytes.
        data: Vec<u8>,
    },
    /// System-exclusive event. `prefix` is `0xF0` or `0xF7`.
    Sysex {
        /// `0xF0` (start) or `0xF7` (continuation/end).
        prefix: u8,
        /// Sysex payload bytes.
        data: Vec<u8>,
    },
}

/// One timed SMF event in source-native ticks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmfEvent {
    /// Absolute tick within the track (non-decreasing per track).
    pub tick: MidiTick,
    /// The event payload.
    pub kind: SmfEventKind,
}

/// One SMF track — an ordered event list.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmfTrack {
    /// Events in non-decreasing tick order.
    pub events: Vec<SmfEvent>,
}

/// Wire form for [`SmfDocument`]; validates on conversion so a
/// crafted snapshot cannot smuggle an out-of-domain document past
/// deserialization.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SmfDocumentRepr {
    /// SMF format.
    format: SmfFormat,
    /// PPQN ticks per quarter note (1–32767).
    division: u32,
    /// Tracks in file order.
    tracks: Vec<SmfTrack>,
}

/// A parsed SMF file: format, PPQN division, and ordered tracks.
///
/// `division` is the source-native PPQN resolution — the same role
/// [`crate::MidiDivision::Ppqn`] plays for a [`crate::MidiSource`].
/// The SMF SMPTE time base has no representation here; parsing one is
/// an [`SmfError::Unsupported`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SmfDocumentRepr", into = "SmfDocumentRepr")]
pub struct SmfDocument {
    /// SMF format (0 or 1).
    pub format: SmfFormat,
    /// PPQN ticks per quarter note (1–32767).
    pub division: u32,
    /// Tracks in file order.
    pub tracks: Vec<SmfTrack>,
}

impl SmfDocument {
    /// Enforces the document's structural contract: division in the
    /// SMF PPQN range, per-track non-decreasing ticks, and every
    /// event field inside its wire domain.
    ///
    /// # Errors
    /// [`Error::Validation`] on any violation.
    pub fn validate(&self) -> Result<(), Error> {
        if !(1..=0x7FFF).contains(&self.division) {
            return Err(Error::Validation(format!(
                "smf division {} outside 1..=32767",
                self.division
            )));
        }
        // The SMF header carries the track count as u16 and a file
        // with no tracks is not a valid document — both bounds match
        // what `parse_smf` can produce.
        if self.tracks.is_empty() || self.tracks.len() > usize::from(u16::MAX) {
            return Err(Error::Validation(format!(
                "smf track count {} outside 1..=65535",
                self.tracks.len()
            )));
        }
        for (ti, track) in self.tracks.iter().enumerate() {
            let mut prev = MidiTick::ZERO;
            for (ei, event) in track.events.iter().enumerate() {
                if ei > 0 && event.tick < prev {
                    return Err(Error::Validation(format!(
                        "smf track {ti} event {ei} tick {} precedes {}",
                        event.tick.value(),
                        prev.value()
                    )));
                }
                prev = event.tick;
                validate_event_kind(&event.kind)
                    .map_err(|m| Error::Validation(format!("smf track {ti} event {ei}: {m}")))?;
            }
        }
        Ok(())
    }

    /// Derives the note projection: `(channel, key)` FIFO pairing of
    /// Note On/Off **within each track**. A wire velocity-0 Note On
    /// is already normalized to `NoteOff` at parse time.
    ///
    /// Returns specs sorted by `(start, key, channel, velocity, len)`
    /// — the deterministic order `ImportMidiSmf` uses to draw
    /// `NoteId`s. Zero-length notes (on and off at the same tick)
    /// carry no sounding duration and are excluded from the
    /// projection; they remain in the document. A `NoteOff` with no
    /// pending `NoteOn` is ignored for projection (preserved in the
    /// document).
    ///
    /// # Errors
    /// [`Error::Validation`] when a track ends with unclosed
    /// `NoteOn`s — the file is truncated/malformed for note purposes
    /// and the import must not silently auto-close them.
    pub fn paired_notes(&self) -> Result<Vec<SmfNoteSpec>, Error> {
        let mut specs = Vec::new();
        for track in &self.tracks {
            // (channel, key) → FIFO of pending onsets (tick, velocity).
            let mut pending: BTreeMap<(u8, u8), VecDeque<(MidiTick, u8)>> = BTreeMap::new();
            for event in &track.events {
                match event.kind {
                    SmfEventKind::NoteOn {
                        channel,
                        key,
                        velocity,
                    } => {
                        pending
                            .entry((channel, key))
                            .or_default()
                            .push_back((event.tick, velocity));
                    }
                    SmfEventKind::NoteOff { channel, key, .. } => {
                        if let Some(on) = pending
                            .get_mut(&(channel, key))
                            .and_then(VecDeque::pop_front)
                        {
                            let len = event.tick.value() - on.0.value();
                            if len > 0 {
                                specs.push(SmfNoteSpec {
                                    channel,
                                    key,
                                    velocity: on.1,
                                    start: on.0,
                                    len: midi_tick(len).map_err(|e| {
                                        Error::Validation(format!(
                                            "smf note length {len} unrepresentable: {e}"
                                        ))
                                    })?,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(((channel, key), _)) = pending.iter().find(|(_, q)| !q.is_empty()) {
                return Err(Error::Validation(format!(
                    "smf track ends with unclosed note on channel {channel} key {key}"
                )));
            }
        }
        specs.sort_by_key(|s| (s.start.value(), s.key, s.channel, s.velocity, s.len.value()));
        Ok(specs)
    }

    /// Derives the controller projection consumed by
    /// [`crate::compile_midi_comp`]: every `Cc` and `PitchBend`
    /// event across all tracks as [`MidiControllerEvent`]s sorted by
    /// `(tick, track index, event index)` — the lane's documented
    /// chronological order (ties keep vector order, last write wins).
    #[must_use]
    pub fn controller_events(&self) -> Vec<MidiControllerEvent> {
        let mut tagged: Vec<(i64, usize, usize, MidiControllerEvent)> = Vec::new();
        for (ti, track) in self.tracks.iter().enumerate() {
            for (ei, event) in track.events.iter().enumerate() {
                let (kind, value, channel) = match event.kind {
                    SmfEventKind::Cc {
                        channel,
                        controller,
                        value,
                    } => (
                        MidiControllerKind::Cc(controller),
                        i32::from(value),
                        channel,
                    ),
                    SmfEventKind::PitchBend { channel, value } => {
                        (MidiControllerKind::PitchBend, i32::from(value), channel)
                    }
                    _ => continue,
                };
                tagged.push((
                    event.tick.value(),
                    ti,
                    ei,
                    MidiControllerEvent {
                        tick: event.tick,
                        channel,
                        kind,
                        value,
                    },
                ));
            }
        }
        tagged.sort_by_key(|(t, ti, ei, _)| (*t, *ti, *ei));
        tagged.into_iter().map(|(_, _, _, e)| e).collect()
    }
}

impl From<SmfDocument> for SmfDocumentRepr {
    fn from(d: SmfDocument) -> Self {
        Self {
            format: d.format,
            division: d.division,
            tracks: d.tracks,
        }
    }
}

impl TryFrom<SmfDocumentRepr> for SmfDocument {
    type Error = Error;
    fn try_from(repr: SmfDocumentRepr) -> Result<Self, Self::Error> {
        let doc = Self {
            format: repr.format,
            division: repr.division,
            tracks: repr.tracks,
        };
        doc.validate()?;
        Ok(doc)
    }
}

/// A note derived from [`SmfDocument::paired_notes`] — the deterministic
/// input `ImportMidiSmf` turns into [`crate::MidiNote`]s. `channel` is
/// provenance only; [`crate::MidiNote`] itself carries no channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmfNoteSpec {
    /// MIDI channel the note sounds on.
    pub channel: u8,
    /// Key (0–127).
    pub key: u8,
    /// Note-on velocity (1–127).
    pub velocity: u8,
    /// Source-native start tick.
    pub start: MidiTick,
    /// Length in source-native ticks (> 0).
    pub len: MidiTick,
}

/// Typed SMF boundary failures. Malformed/overflow/unsupported are
/// distinct so callers can tell a corrupt file from an out-of-scope
/// one — SMPTE/MIDI 2.0 never report success.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SmfError {
    /// Structurally invalid bytes or an inconsistent document
    /// (`offset` is the byte position for parse failures, `0` for
    /// document-level failures).
    Malformed {
        /// Byte offset of the failure.
        offset: usize,
        /// What was wrong.
        detail: String,
    },
    /// Well-formed SMF outside the supported subset (Format 2,
    /// SMPTE division, MIDI 2.0/MPE markers) or an export parameter
    /// outside the SMF domain.
    Unsupported {
        /// What was unsupported.
        detail: String,
    },
    /// Checked arithmetic overflowed (tick accumulation, tick
    /// conversion, or delta encoding).
    Overflow {
        /// What overflowed.
        detail: String,
    },
    /// The named project entity does not exist (export lookup).
    NotFound(String),
}

impl std::fmt::Display for SmfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed { offset, detail } => {
                write!(f, "smf malformed at byte {offset}: {detail}")
            }
            Self::Unsupported { detail } => write!(f, "smf unsupported: {detail}"),
            Self::Overflow { detail } => write!(f, "smf overflow: {detail}"),
            Self::NotFound(what) => write!(f, "smf export target not found: {what}"),
        }
    }
}

impl std::error::Error for SmfError {}

/// Export parameters: output format and target PPQN division.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmfExportOptions {
    /// Output SMF format.
    pub format: SmfFormat,
    /// Target PPQN ticks per quarter note (1–32767).
    pub division: u32,
}

/// A rational number `num/den` with `den > 0`, reduced by gcd — used
/// for exact max-error reporting in [`SmfConversionReport`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmfRational {
    /// Numerator (`>= 0` for error reporting).
    pub num: i64,
    /// Denominator (`> 0`).
    pub den: i64,
}

impl Default for SmfRational {
    fn default() -> Self {
        Self::ZERO
    }
}

impl SmfRational {
    /// The zero rational.
    pub const ZERO: Self = Self { num: 0, den: 1 };

    /// Builds a reduced rational from `num/den` (`den != 0`).
    /// Sign is normalized so `den > 0`; reduced values outside the
    /// i64 domain saturate (reporting-only type, unreachable for SMF
    /// tick arithmetic).
    #[must_use]
    pub fn new(num: i64, den: i64) -> Self {
        if den == 0 {
            return Self::ZERO;
        }
        let negative = (num < 0) != (den < 0);
        let (n, d) = (num.unsigned_abs(), den.unsigned_abs());
        let g = gcd_u64(n, d);
        let num = i64::try_from(n / g).unwrap_or(i64::MAX);
        let den = i64::try_from(d / g).unwrap_or(i64::MAX);
        Self {
            num: if negative { -num } else { num },
            den,
        }
    }

    /// Exact rational comparison via cross-multiplication.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (i128::from(self.num) * i128::from(other.den))
            .cmp(&(i128::from(other.num) * i128::from(self.den)))
    }
}

fn gcd_u64(a: u64, b: u64) -> u64 {
    let (mut a, mut b) = (a, b);
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a.max(1)
}

/// An event the export could not represent — currently always empty
/// (every parsed kind round-trips); the typed slot exists so a
/// future drop path reports instead of silently losing data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmfUnsupportedEvent {
    /// Source track index the event came from.
    pub track: u16,
    /// Source-native tick.
    pub tick: MidiTick,
    /// The event kind that was not emitted.
    pub kind: SmfEventKind,
}

/// Accounting for one [`export_smf`] call: how many source-event
/// tick conversions were exact vs rounded, and the worst absolute
/// error in target ticks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmfConversionReport {
    /// Source events whose ticks were converted (excludes
    /// writer-appended `EndOfTrack` markers).
    pub event_count: u64,
    /// Conversions with zero remainder (exact).
    pub exact_count: u64,
    /// Conversions that required rounding.
    pub rounded_count: u64,
    /// Largest |exact − rounded| across all conversions, as a
    /// rational in target ticks (`<= 1/2`).
    pub max_error: SmfRational,
    /// Events not representable in the target encoding — empty in
    /// this slice.
    pub unsupported_events: Vec<SmfUnsupportedEvent>,
}

impl Default for SmfConversionReport {
    fn default() -> Self {
        Self {
            event_count: 0,
            exact_count: 0,
            rounded_count: 0,
            max_error: SmfRational::ZERO,
            unsupported_events: Vec::new(),
        }
    }
}

/// The result of [`export_smf`]: canonical SMF bytes plus the
/// conversion report.
#[derive(Clone, Debug, PartialEq)]
pub struct SmfFile {
    /// Deterministic SMF bytes.
    pub bytes: Vec<u8>,
    /// Conversion accounting.
    pub report: SmfConversionReport,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

fn malformed(offset: usize, detail: impl Into<String>) -> SmfError {
    SmfError::Malformed {
        offset,
        detail: detail.into(),
    }
}

fn unsupported(detail: impl Into<String>) -> SmfError {
    SmfError::Unsupported {
        detail: detail.into(),
    }
}

fn overflow(detail: impl Into<String>) -> SmfError {
    SmfError::Overflow {
        detail: detail.into(),
    }
}

fn be_u16(data: &[u8], pos: usize) -> u16 {
    u16::from_be_bytes([data[pos], data[pos + 1]])
}

fn be_u32(data: &[u8], pos: usize) -> u32 {
    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
}

/// Reads a variable-length quantity (7 bits per byte, MSB
/// continuation). Accepts non-minimal encodings up to 8 bytes.
fn read_varlen(data: &[u8], pos: &mut usize) -> Result<u64, SmfError> {
    let mut value: u64 = 0;
    for i in 0..8 {
        let Some(&b) = data.get(*pos) else {
            return Err(malformed(*pos, "truncated variable-length quantity"));
        };
        *pos += 1;
        value = (value << 7) | u64::from(b & 0x7F);
        if b & 0x80 == 0 {
            return Ok(value);
        }
        if i == 7 {
            return Err(malformed(*pos, "variable-length quantity too long"));
        }
    }
    unreachable!()
}

/// Parses SMF Format 0/1 bytes into a [`SmfDocument`].
///
/// # Errors
/// [`SmfError::Malformed`] on structural violations,
/// [`SmfError::Unsupported`] on Format 2 / SMPTE division /
/// out-of-subset content, [`SmfError::Overflow`] on tick
/// accumulation overflow.
pub fn parse_smf(bytes: &[u8]) -> Result<SmfDocument, SmfError> {
    if bytes.len() < 8 || &bytes[0..4] != b"MThd" {
        return Err(malformed(0, "missing MThd header"));
    }
    let header_len = be_u32(bytes, 4) as usize;
    if header_len < 6 {
        return Err(malformed(4, format!("MThd length {header_len} < 6")));
    }
    if 8 + header_len > bytes.len() {
        return Err(malformed(8, "MThd chunk truncated"));
    }
    let format_raw = be_u16(bytes, 8);
    let ntracks = usize::from(be_u16(bytes, 10));
    let division_raw = be_u16(bytes, 12);
    let mut pos = 8 + header_len;

    let format = match format_raw {
        0 => SmfFormat::Format0,
        1 => SmfFormat::Format1,
        2 => {
            return Err(unsupported(format!(
                "SMF format {format_raw} is outside the supported Format 0/1 subset"
            )));
        }
        other => {
            return Err(unsupported(format!(
                "SMF format {other} is outside the supported Format 0/1 subset"
            )));
        }
    };
    if division_raw & 0x8000 != 0 {
        return Err(unsupported(format!(
            "SMPTE time base (division {division_raw:#06x}) is not supported"
        )));
    }
    let division = u32::from(division_raw & 0x7FFF);
    if division == 0 {
        return Err(malformed(12, "SMF division is zero"));
    }

    let mut tracks = Vec::with_capacity(ntracks);
    while pos < bytes.len() {
        if bytes.len() - pos < 8 {
            return Err(malformed(pos, "trailing bytes after last chunk"));
        }
        let chunk_id = &bytes[pos..pos + 4];
        let chunk_len = be_u32(bytes, pos + 4) as usize;
        pos += 8;
        if pos + chunk_len > bytes.len() {
            return Err(malformed(
                pos - 8,
                format!("chunk length {chunk_len} exceeds remaining file"),
            ));
        }
        let data = &bytes[pos..pos + chunk_len];
        if chunk_id == b"MTrk" {
            tracks.push(SmfTrack {
                events: parse_track(data)?,
            });
        }
        // Unknown chunk IDs are skipped — forward compatibility for
        // non-track chunks without pretending they are events.
        pos += chunk_len;
    }
    if tracks.len() != ntracks {
        return Err(malformed(
            8,
            format!(
                "MThd declares {ntracks} tracks but {} MTrk chunks were found",
                tracks.len()
            ),
        ));
    }
    if tracks.is_empty() {
        return Err(malformed(8, "SMF contains no tracks"));
    }

    let doc = SmfDocument {
        format,
        division,
        tracks,
    };
    doc.validate()
        .map_err(|e| malformed(0, format!("parsed document failed validation: {e}")))?;
    Ok(doc)
}

/// Parses one `MTrk` payload into absolute-tick events.
fn parse_track(data: &[u8]) -> Result<Vec<SmfEvent>, SmfError> {
    let mut events = Vec::new();
    let mut pos = 0usize;
    let mut tick_acc: i128 = 0;
    // Running status: last channel-event status byte (0x80–0xEF).
    // Meta/sysex events do not participate and do not clear it.
    let mut running: Option<u8> = None;

    while pos < data.len() {
        let delta = read_varlen(data, &mut pos)?;
        tick_acc += i128::from(delta);
        if tick_acc > i128::from(i64::MAX) {
            return Err(overflow(format!(
                "track tick accumulation exceeds i64 at byte {pos}"
            )));
        }
        if pos >= data.len() {
            return Err(malformed(pos, "delta time not followed by an event"));
        }
        let first = data[pos];
        let status = if first & 0x80 != 0 {
            pos += 1;
            first
        } else {
            let Some(status) = running else {
                return Err(malformed(
                    pos,
                    "data byte without a preceding channel status (no running status)",
                ));
            };
            status
        };

        let kind = if (0x80..=0xEF).contains(&status) {
            running = Some(status);
            decode_channel_event(data, &mut pos, status)?
        } else if status == 0xFF {
            decode_meta_event(data, &mut pos)?
        } else if status == 0xF0 || status == 0xF7 {
            decode_sysex(data, &mut pos, status)?
        } else {
            // 0xF1–0xF6 / 0xF8–0xFE: system common/realtime statuses
            // are not legal inside SMF track data.
            return Err(malformed(
                pos.saturating_sub(1),
                format!("system status byte {status:#04x} is not valid in SMF track data"),
            ));
        };

        let tick =
            i64::try_from(tick_acc).map_err(|_| overflow("track tick exceeds i64".to_string()))?;
        let tick =
            MidiTick::new(tick).map_err(|_| malformed(pos, "negative midi tick".to_string()))?;
        events.push(SmfEvent { tick, kind });
    }
    Ok(events)
}

/// Decodes one channel event (status `0x80`–`0xEF`) whose status byte
/// is already consumed; `pos` sits at the first data byte.
fn decode_channel_event(
    data: &[u8],
    pos: &mut usize,
    status: u8,
) -> Result<SmfEventKind, SmfError> {
    let channel = status & 0x0F;
    let data_len = match status & 0xF0 {
        0xC0 | 0xD0 => 1,
        _ => 2,
    };
    if *pos + data_len > data.len() {
        return Err(malformed(*pos, "truncated channel event data"));
    }
    let d1 = data[*pos];
    let d2 = data.get(*pos + 1).copied().unwrap_or(0);
    *pos += data_len;
    Ok(match status & 0xF0 {
        0x80 => SmfEventKind::NoteOff {
            channel,
            key: d1,
            velocity: d2,
        },
        0x90 => {
            if d2 == 0 {
                // Velocity-0 Note On *is* Note Off on the wire.
                SmfEventKind::NoteOff {
                    channel,
                    key: d1,
                    velocity: 0,
                }
            } else {
                SmfEventKind::NoteOn {
                    channel,
                    key: d1,
                    velocity: d2,
                }
            }
        }
        0xA0 => SmfEventKind::PolyAftertouch {
            channel,
            key: d1,
            value: d2,
        },
        0xB0 => SmfEventKind::Cc {
            channel,
            controller: d1,
            value: d2,
        },
        0xC0 => SmfEventKind::ProgramChange {
            channel,
            program: d1,
        },
        0xD0 => SmfEventKind::ChannelAftertouch { channel, value: d1 },
        0xE0 => SmfEventKind::PitchBend {
            channel,
            value: u16::from(d2) << 7 | u16::from(d1),
        },
        _ => unreachable!(),
    })
}

/// Decodes one meta event (`FF` status already consumed); `pos` sits
/// at the meta type byte.
fn decode_meta_event(data: &[u8], pos: &mut usize) -> Result<SmfEventKind, SmfError> {
    if *pos >= data.len() {
        return Err(malformed(*pos, "meta event missing type byte"));
    }
    let meta_kind = data[*pos];
    *pos += 1;
    let len = read_varlen(data, pos)?;
    let len = usize::try_from(len).map_err(|_| malformed(*pos, "meta length exceeds usize"))?;
    if *pos + len > data.len() {
        return Err(malformed(*pos, "meta payload truncated"));
    }
    let payload = &data[*pos..*pos + len];
    *pos += len;
    Ok(match meta_kind {
        // EOT is defined as `FF 2F 00`; a nonzero-length 0x2F
        // is not an end-of-track — keep it as raw meta
        // provenance instead of dropping its payload.
        0x2F if len == 0 => SmfEventKind::EndOfTrack,
        0x51 if len == 3 => SmfEventKind::Tempo {
            us_per_quarter: u32::from(payload[0]) << 16
                | u32::from(payload[1]) << 8
                | u32::from(payload[2]),
        },
        0x58 if len == 4 => SmfEventKind::TimeSignature {
            numerator: payload[0],
            denominator: payload[1],
            thirtyseconds: payload[2],
            clocks: payload[3],
        },
        0x03 => SmfEventKind::TrackName {
            bytes: payload.to_vec(),
        },
        other => SmfEventKind::MetaOther {
            meta_kind: other,
            data: payload.to_vec(),
        },
    })
}

/// Decodes one system-exclusive event (`0xF0`/`0xF7` status already
/// consumed); `pos` sits at the length varlen.
fn decode_sysex(data: &[u8], pos: &mut usize, prefix: u8) -> Result<SmfEventKind, SmfError> {
    let len = read_varlen(data, pos)?;
    let len = usize::try_from(len).map_err(|_| malformed(*pos, "sysex length exceeds usize"))?;
    if *pos + len > data.len() {
        return Err(malformed(*pos, "sysex payload truncated"));
    }
    let payload = data[*pos..*pos + len].to_vec();
    *pos += len;
    Ok(SmfEventKind::Sysex {
        prefix,
        data: payload,
    })
}

fn validate_event_kind(kind: &SmfEventKind) -> Result<(), String> {
    let channel_ok = |c: u8| c <= 15;
    match kind {
        SmfEventKind::NoteOn {
            channel,
            key,
            velocity,
        } => {
            if !channel_ok(*channel) {
                return Err(format!("channel {channel} > 15"));
            }
            if *key > 127 {
                return Err(format!("key {key} > 127"));
            }
            if !(1..=127).contains(velocity) {
                return Err(format!("note-on velocity {velocity} outside 1..=127"));
            }
        }
        SmfEventKind::NoteOff {
            channel,
            key,
            velocity,
        } => {
            if !channel_ok(*channel) {
                return Err(format!("channel {channel} > 15"));
            }
            if *key > 127 {
                return Err(format!("key {key} > 127"));
            }
            if *velocity > 127 {
                return Err(format!("note-off velocity {velocity} > 127"));
            }
        }
        SmfEventKind::PolyAftertouch {
            channel,
            key,
            value,
        } => {
            if !channel_ok(*channel) || *key > 127 || *value > 127 {
                return Err("poly aftertouch field out of domain".into());
            }
        }
        SmfEventKind::Cc {
            channel,
            controller,
            value,
        } => {
            if !channel_ok(*channel) || *controller > 127 || *value > 127 {
                return Err("cc field out of domain".into());
            }
        }
        SmfEventKind::ProgramChange { channel, program } => {
            if !channel_ok(*channel) || *program > 127 {
                return Err("program change field out of domain".into());
            }
        }
        SmfEventKind::ChannelAftertouch { channel, value } => {
            if !channel_ok(*channel) || *value > 127 {
                return Err("channel aftertouch field out of domain".into());
            }
        }
        SmfEventKind::PitchBend { channel, value } => {
            if !channel_ok(*channel) || *value > 16_383 {
                return Err("pitch bend field out of domain".into());
            }
        }
        SmfEventKind::Tempo { us_per_quarter } => {
            if *us_per_quarter > 0xFF_FFFF {
                return Err("tempo exceeds SMF u24 domain".into());
            }
        }
        SmfEventKind::Sysex { prefix, .. } => {
            if *prefix != 0xF0 && *prefix != 0xF7 {
                return Err(format!("sysex prefix {prefix:#04x} not 0xF0/0xF7"));
            }
        }
        SmfEventKind::EndOfTrack
        | SmfEventKind::TimeSignature { .. }
        | SmfEventKind::TrackName { .. }
        | SmfEventKind::MetaOther { .. } => {}
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/// One event queued for export. `tick` is in the source division
/// when gathered and the target division after `convert_tick`.
struct EmitItem {
    /// Event tick (source or target division depending on stage).
    tick: i64,
    /// Event kind (`EndOfTrack` items are handled per format).
    kind: SmfEventKind,
    /// Origin track index (format-0 merge tiebreak).
    track: u16,
    /// Origin sequence within the track (stable tiebreak).
    seq: u32,
}

/// Deterministic same-tick ordering: meta/sysex establish context,
/// note-offs release before controllers and note-ons retrigger.
fn emit_rank(kind: &SmfEventKind) -> u8 {
    match kind {
        SmfEventKind::Tempo { .. }
        | SmfEventKind::TimeSignature { .. }
        | SmfEventKind::TrackName { .. }
        | SmfEventKind::MetaOther { .. } => 1,
        SmfEventKind::Sysex { .. } => 2,
        SmfEventKind::NoteOff { .. } => 3,
        SmfEventKind::PolyAftertouch { .. }
        | SmfEventKind::Cc { .. }
        | SmfEventKind::ProgramChange { .. }
        | SmfEventKind::ChannelAftertouch { .. }
        | SmfEventKind::PitchBend { .. } => 4,
        SmfEventKind::NoteOn { .. } => 5,
        SmfEventKind::EndOfTrack => 6,
    }
}

/// Converts one source tick to the target division with round-half-up
/// and reports exactness + absolute error.
fn convert_tick(
    tick: i64,
    from_div: u32,
    to_div: u32,
    report: &mut SmfConversionReport,
) -> Result<i64, SmfError> {
    report.event_count += 1;
    let num = i128::from(tick) * i128::from(to_div);
    let den = i128::from(from_div);
    let rounded = (2 * num + den) / (2 * den);
    let out = i64::try_from(rounded).map_err(|_| {
        overflow(format!(
            "tick {tick} at division {from_div}→{to_div} exceeds i64"
        ))
    })?;
    let rem = num % den;
    if rem == 0 {
        report.exact_count += 1;
    } else {
        report.rounded_count += 1;
        // |exact − rounded| = |num − rounded·den| / den.
        let err_num = (num - i128::from(out) * den).unsigned_abs();
        let err_num = i64::try_from(err_num)
            .map_err(|_| overflow("conversion error numerator overflow".to_string()))?;
        let err = SmfRational::new(err_num, i64::from(from_div));
        if report.max_error.cmp(&err) == std::cmp::Ordering::Less {
            report.max_error = err;
        }
    }
    Ok(out)
}

fn midi_tick(v: i64) -> Result<MidiTick, Error> {
    MidiTick::new(v)
}

/// Exports a project's MIDI source to SMF bytes.
///
/// A source with a provenance [`SmfDocument`] in
/// `project.smf_documents` re-emits that document — all event kinds,
/// ordering, and meta/sysex payloads preserved — converted to
/// `options.division` and shaped as `options.format`. A source
/// without provenance synthesizes notes-only Format 0/1 output on
/// channel 0 (the note model carries no channel).
///
/// # Errors
/// [`SmfError::NotFound`] when `source_id` is not a MIDI source,
/// [`SmfError::Unsupported`] for out-of-domain parameters,
/// [`SmfError::Malformed`]/[`SmfError::Overflow`] for unrepresentable
/// source data.
pub fn export_smf(
    project: &Project,
    source_id: &SourceId,
    options: &SmfExportOptions,
) -> Result<SmfFile, SmfError> {
    if !(1..=0x7FFF).contains(&options.division) {
        return Err(unsupported(format!(
            "export division {} outside SMF PPQN range 1..=32767",
            options.division
        )));
    }
    let source = project
        .midi_sources
        .get(source_id)
        .ok_or_else(|| SmfError::NotFound(format!("midi source {source_id}")))?;
    let mut report = SmfConversionReport::default();

    // (source tick, kind, track, seq) items awaiting conversion.
    let mut items: Vec<EmitItem> = Vec::new();
    let from_div = if let Some(doc) = project.smf_documents.get(source_id) {
        // Fail closed on a hand-built divergent project: the
        // document must satisfy its own domain contract before its
        // fields are trusted as wire bytes.
        doc.validate()
            .map_err(|e| malformed(0, format!("smf document failed validation: {e}")))?;
        for (ti, track) in doc.tracks.iter().enumerate() {
            for (si, event) in track.events.iter().enumerate() {
                items.push(EmitItem {
                    tick: event.tick.value(),
                    kind: event.kind.clone(),
                    track: u16::try_from(ti).unwrap_or(u16::MAX),
                    seq: u32::try_from(si).unwrap_or(u32::MAX),
                });
            }
        }
        doc.division
    } else {
        if source.division.ticks_per_quarter() > 0x7FFF {
            return Err(unsupported(format!(
                "midi source division {} exceeds SMF PPQN range",
                source.division.ticks_per_quarter()
            )));
        }
        for note in source.notes.values() {
            let end = note.end().map_err(|e| match e {
                Error::Overflow => overflow(format!("midi note {} end overflows", note.id)),
                other => malformed(0, format!("midi note {} invalid: {other}", note.id)),
            })?;
            if note.len.value() <= 0 {
                return Err(malformed(
                    0,
                    format!("midi note {} has non-positive length", note.id),
                ));
            }
            if note.key > 127 || !(1..=127).contains(&note.velocity) {
                return Err(malformed(
                    0,
                    format!(
                        "midi note {} fields outside SMF domain (key {}, velocity {})",
                        note.id, note.key, note.velocity
                    ),
                ));
            }
            items.push(EmitItem {
                tick: note.start.value(),
                kind: SmfEventKind::NoteOn {
                    channel: 0,
                    key: note.key,
                    velocity: note.velocity,
                },
                track: 0,
                seq: 0,
            });
            items.push(EmitItem {
                tick: end.value(),
                kind: SmfEventKind::NoteOff {
                    channel: 0,
                    key: note.key,
                    velocity: 0,
                },
                track: 0,
                seq: 1,
            });
        }
        source.division.ticks_per_quarter()
    };
    if from_div == 0 {
        return Err(malformed(0, "source division is zero"));
    }

    // Convert every item tick into the target division, then shape
    // tracks per format.
    let mut converted: Vec<EmitItem> = Vec::with_capacity(items.len());
    for item in items {
        let tick = convert_tick(item.tick, from_div, options.division, &mut report)?;
        converted.push(EmitItem { tick, ..item });
    }
    let provenance_tracks = project
        .smf_documents
        .get(source_id)
        .map(|doc| doc.tracks.len());
    let out_tracks = shape_tracks(options.format, provenance_tracks, converted);

    let bytes = encode_smf(options.format, options.division, &out_tracks)?;
    Ok(SmfFile { bytes, report })
}

/// Groups converted items into output tracks of `(tick, kind)` pairs
/// per SMF format, applying deterministic same-tick ordering
/// (`NoteOff` before `NoteOn` via [`emit_rank`]).
fn shape_tracks(
    format: SmfFormat,
    doc_track_count: Option<usize>,
    converted: Vec<EmitItem>,
) -> Vec<Vec<(i64, SmfEventKind)>> {
    let mut out_tracks: Vec<Vec<(i64, SmfEventKind)>> = Vec::new();
    match format {
        SmfFormat::Format0 => {
            // Merge all tracks; EndOfTrack items are dropped and a
            // single terminal EOT is appended by the writer.
            let mut merged: Vec<EmitItem> = converted
                .into_iter()
                .filter(|c| !matches!(c.kind, SmfEventKind::EndOfTrack))
                .collect();
            merged.sort_by(|a, b| {
                (a.tick, emit_rank(&a.kind), a.track, a.seq).cmp(&(
                    b.tick,
                    emit_rank(&b.kind),
                    b.track,
                    b.seq,
                ))
            });
            let last_tick = merged.last().map_or(0, |c| c.tick);
            let mut track: Vec<(i64, SmfEventKind)> =
                merged.into_iter().map(|c| (c.tick, c.kind)).collect();
            track.push((last_tick, SmfEventKind::EndOfTrack));
            out_tracks.push(track);
        }
        SmfFormat::Format1 => {
            if let Some(n) = doc_track_count {
                // Preserve track structure; within a track, order by
                // (converted tick, emit rank, original seq) so
                // same-tick `NoteOff` precedes `NoteOn`
                // deterministically.
                let mut items = converted;
                items.sort_by(|a, b| {
                    (a.track, a.tick, emit_rank(&a.kind), a.seq).cmp(&(
                        b.track,
                        b.tick,
                        emit_rank(&b.kind),
                        b.seq,
                    ))
                });
                let mut iter = items.into_iter().peekable();
                for ti in 0..n {
                    let mut track: Vec<(i64, SmfEventKind)> = Vec::new();
                    while let Some(c) = iter.next_if(|c| usize::from(c.track) == ti) {
                        track.push((c.tick, c.kind));
                    }
                    let last_tick = track.last().map_or(0, |(t, _)| *t);
                    if !matches!(track.last().map(|(_, k)| k), Some(SmfEventKind::EndOfTrack)) {
                        track.push((last_tick, SmfEventKind::EndOfTrack));
                    }
                    out_tracks.push(track);
                }
            } else {
                // Synthesized Format 1: empty conductor track + one
                // event track.
                out_tracks.push(vec![(0, SmfEventKind::EndOfTrack)]);
                let mut events: Vec<(i64, SmfEventKind)> =
                    converted.into_iter().map(|c| (c.tick, c.kind)).collect();
                events.sort_by_key(|a| (a.0, emit_rank(&a.1)));
                let last_tick = events.last().map_or(0, |(t, _)| *t);
                events.push((last_tick, SmfEventKind::EndOfTrack));
                out_tracks.push(events);
            }
        }
    }
    out_tracks
}

fn push_varlen(out: &mut Vec<u8>, mut value: u64) {
    let mut buf = [0u8; 8];
    let mut n = 0;
    loop {
        buf[n] = (value & 0x7F) as u8;
        value >>= 7;
        n += 1;
        if value == 0 {
            break;
        }
    }
    for i in (0..n).rev() {
        let mut b = buf[i];
        if i != 0 {
            b |= 0x80;
        }
        out.push(b);
    }
}

fn encode_smf(
    format: SmfFormat,
    division: u32,
    tracks: &[Vec<(i64, SmfEventKind)>],
) -> Result<Vec<u8>, SmfError> {
    let ntracks = u16::try_from(tracks.len())
        .map_err(|_| overflow("track count exceeds SMF u16".to_string()))?;
    let division = u16::try_from(division)
        .map_err(|_| malformed(0, "division exceeds SMF u16".to_string()))?;
    let mut out = Vec::new();
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    let format_raw: u16 = match format {
        SmfFormat::Format0 => 0,
        SmfFormat::Format1 => 1,
    };
    out.extend_from_slice(&format_raw.to_be_bytes());
    out.extend_from_slice(&ntracks.to_be_bytes());
    out.extend_from_slice(&division.to_be_bytes());

    for track in tracks {
        let mut body = Vec::new();
        let mut prev_tick = 0i64;
        for (tick, kind) in track {
            let delta = tick
                .checked_sub(prev_tick)
                .ok_or_else(|| overflow("event tick moved backwards".to_string()))?;
            if delta < 0 {
                return Err(malformed(
                    0,
                    "event tick decreased inside an output track".to_string(),
                ));
            }
            let delta = u64::try_from(delta)
                .map_err(|_| overflow("event delta exceeds u64".to_string()))?;
            push_varlen(&mut body, delta);
            encode_event(&mut body, kind)?;
            prev_tick = *tick;
        }
        let body_len = u32::try_from(body.len())
            .map_err(|_| overflow("track body exceeds SMF u32".to_string()))?;
        out.extend_from_slice(b"MTrk");
        out.extend_from_slice(&body_len.to_be_bytes());
        out.extend_from_slice(&body);
    }
    Ok(out)
}

fn encode_event(out: &mut Vec<u8>, kind: &SmfEventKind) -> Result<(), SmfError> {
    match kind {
        SmfEventKind::NoteOn {
            channel,
            key,
            velocity,
        } => {
            out.extend_from_slice(&[0x90 | channel, *key, *velocity]);
        }
        SmfEventKind::NoteOff {
            channel,
            key,
            velocity,
        } => {
            out.extend_from_slice(&[0x80 | channel, *key, *velocity]);
        }
        SmfEventKind::PolyAftertouch {
            channel,
            key,
            value,
        } => {
            out.extend_from_slice(&[0xA0 | channel, *key, *value]);
        }
        SmfEventKind::Cc {
            channel,
            controller,
            value,
        } => {
            out.extend_from_slice(&[0xB0 | channel, *controller, *value]);
        }
        SmfEventKind::ProgramChange { channel, program } => {
            out.extend_from_slice(&[0xC0 | channel, *program]);
        }
        SmfEventKind::ChannelAftertouch { channel, value } => {
            out.extend_from_slice(&[0xD0 | channel, *value]);
        }
        SmfEventKind::PitchBend { channel, value } => {
            let lsb = u8::try_from(value & 0x7F)
                .map_err(|_| malformed(0, "pitch bend lsb out of range".to_string()))?;
            let msb = u8::try_from(value >> 7)
                .map_err(|_| malformed(0, "pitch bend msb out of range".to_string()))?;
            out.extend_from_slice(&[0xE0 | channel, lsb, msb]);
        }
        SmfEventKind::EndOfTrack => {
            out.extend_from_slice(&[0xFF, 0x2F, 0x00]);
        }
        SmfEventKind::Tempo { us_per_quarter } => {
            if *us_per_quarter > 0xFF_FFFF {
                return Err(malformed(0, "tempo exceeds SMF u24 domain".to_string()));
            }
            out.extend_from_slice(&[0xFF, 0x51, 0x03]);
            out.extend_from_slice(&us_per_quarter.to_be_bytes()[1..]);
        }
        SmfEventKind::TimeSignature {
            numerator,
            denominator,
            thirtyseconds,
            clocks,
        } => {
            out.extend_from_slice(&[
                0xFF,
                0x58,
                0x04,
                *numerator,
                *denominator,
                *thirtyseconds,
                *clocks,
            ]);
        }
        SmfEventKind::TrackName { bytes } => {
            out.push(0xFF);
            out.push(0x03);
            push_varlen(out, bytes.len() as u64);
            out.extend_from_slice(bytes);
        }
        SmfEventKind::MetaOther { meta_kind, data } => {
            out.push(0xFF);
            out.push(*meta_kind);
            push_varlen(out, data.len() as u64);
            out.extend_from_slice(data);
        }
        SmfEventKind::Sysex { prefix, data } => {
            out.push(*prefix);
            push_varlen(out, data.len() as u64);
            out.extend_from_slice(data);
        }
    }
    Ok(())
}
