//! Timeline lowering: project tracks/lanes/clips → a generic
//! [`GraphSpec`] plus a stable node/port mapping ([`ClipBinding`]).
//!
//! The graph side receives only `AudioFormat`s — no Project, Track or
//! Clip type crosses the boundary. Source frames, engine frames and
//! musical ticks stay distinct: `Media` anchors convert exactly in
//! their declared time base, `Musical` anchors convert through the
//! project tempo map, and the only rounding is `floor` at the final
//! engine-frame boundary.

use synvelis_project::{
    ClipId, MediaSpan, MediaTimeValue, Project, SourceId, TickTimeMapping, TimeAnchor,
};

use ruviform_audio_core::AudioFormat;
use ruviform_graph::{
    Activation, Availability, ChannelMap, ConnDef, Direction, EdgeDelay, GraphBuilder, GraphSpec,
    Multiplicity, NodeDef, NodeId, NodeKind, PortDef, PortFormat, PortId, PortRef, PortRole,
};

/// Headless engine geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineConfig {
    /// Engine sample rate.
    pub sample_rate: u32,
    /// Master output channel count.
    pub channels: u16,
    /// Frames per process block.
    pub block_frames: usize,
}

/// A clip's resolved window: engine frames `[start, start+len)` map
/// 1:1 onto source frames `[source_start, source_start+source_len)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipWindow {
    /// First engine frame of the clip.
    pub engine_start: u64,
    /// Clip length in engine frames (`> 0`).
    pub engine_len: u64,
    /// First source frame the clip reads.
    pub source_start: u64,
    /// Source frames the clip covers.
    pub source_len: u64,
}

/// Stable mapping for one lowered clip.
#[derive(Clone, Debug)]
pub struct ClipBinding {
    /// The project clip this node plays.
    pub clip_id: ClipId,
    /// The verified source it reads.
    pub source_id: SourceId,
    /// Stable graph node issued by the builder.
    pub node_id: NodeId,
    /// The node's audio output port.
    pub out_port: PortId,
    /// Resolved engine/source window.
    pub window: ClipWindow,
    /// The source's canonical output format.
    pub canonical: AudioFormat,
}

/// A clip that produced no node — always explicit, never silent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkippedClip {
    /// Muted clip — lowers to silence by contract.
    Muted {
        /// The muted clip.
        clip_id: ClipId,
    },
    /// Source rate differs from the engine rate — implicit resampling
    /// is forbidden, so the clip is skipped with its rate recorded.
    RateMismatch {
        /// The clip.
        clip_id: ClipId,
        /// The source's native rate.
        source_rate: u32,
    },
    /// Anchor could not be converted to engine frames (empty or
    /// unresolvable time range).
    UnresolvableAnchor {
        /// The clip.
        clip_id: ClipId,
    },
    /// Referenced source is absent or could not be resolved.
    MissingSource {
        /// The clip.
        clip_id: ClipId,
        /// The unresolved source.
        source_id: SourceId,
    },
}

/// Lowering result: the generic spec plus every stable binding.
#[derive(Clone, Debug)]
pub struct Lowered {
    /// The generic graph spec — free of project types.
    pub spec: GraphSpec,
    /// One entry per clip that produced a node.
    pub clips: Vec<ClipBinding>,
    /// The master sink's input port (the graph output tap).
    pub master_in: PortId,
    /// Clips that produced no node, with reasons.
    pub skipped: Vec<SkippedClip>,
}

/// `floor(n/d)` on `i128` — the only rounding applied, at the final
/// frame boundary.
fn floor_div(numer: i128, denom: i128) -> i64 {
    let q = numer / denom;
    let r = numer % denom;
    let floored = if r < 0 { q - 1 } else { q };
    i64::try_from(floored).unwrap_or(i64::MAX)
}

/// `MediaTimeValue` → engine frames: `value * num * rate / den`,
/// floored at the boundary.
fn media_frames(value: MediaTimeValue, rate: u32) -> i64 {
    floor_div(
        i128::from(value.value) * i128::from(value.numerator) * i128::from(rate),
        i128::from(value.denominator),
    )
}

fn media_span_frames(span: &MediaSpan, rate: u32) -> (i64, i64) {
    (
        media_frames(span.start(), rate),
        media_frames(span.end(), rate),
    )
}

/// Lowers the project's clips into a generic audio graph spec.
///
/// `canonical_of(source_id)` supplies each resolved source's canonical
/// format (the resolver's output); `None` means the source could not
/// be prepared and its clips are skipped as `MissingSource`.
///
/// Engine-channel coverage for differing layouts uses an explicit
/// [`ChannelMap`]: output channel `d` reads source channel
/// `min(d, src-1)` — mono→stereo duplicates, stereo→mono sums.
/// # Panics
/// When `engine.sample_rate` is zero (engine config invariant).
#[must_use]
#[allow(clippy::too_many_lines)] // one pass over a flat clip table
pub fn lower(
    project: &Project,
    canonical_of: &dyn Fn(SourceId) -> Option<AudioFormat>,
    engine: &EngineConfig,
) -> Lowered {
    let mut builder = GraphBuilder::new();
    let engine_format = AudioFormat {
        sample_rate: ruviform_audio_core::SampleRate::new(engine.sample_rate)
            .expect("engine rate must be non-zero"),
        layout: if engine.channels == 1 {
            ruviform_audio_core::ChannelLayout::Mono
        } else {
            ruviform_audio_core::ChannelLayout::Stereo
        },
        sample_format: ruviform_audio_core::SampleFormat::F32,
    };
    let (master_node, master_ports) = builder.add_node(&NodeDef {
        kind: NodeKind::Sink,
        ports: vec![PortDef {
            direction: Direction::Input,
            role: PortRole::Main,
            format: PortFormat::Audio(engine_format),
            multiplicity: Multiplicity::unbounded(0),
            capabilities: Vec::new(),
            availability: Availability::Always,
            activation: Activation::Enabled,
        }],
    });
    let master_in = master_ports[0];

    let tempo = TickTimeMapping::new(&project.tempo_map).ok();
    let mut clips = Vec::new();
    let mut skipped = Vec::new();

    for clip in project.clips.values() {
        if clip.muted {
            skipped.push(SkippedClip::Muted { clip_id: clip.id });
            continue;
        }
        if !project.sources.contains_key(&clip.source_id) {
            skipped.push(SkippedClip::MissingSource {
                clip_id: clip.id,
                source_id: clip.source_id,
            });
            continue;
        }
        let Some(canonical) = canonical_of(clip.source_id) else {
            skipped.push(SkippedClip::MissingSource {
                clip_id: clip.id,
                source_id: clip.source_id,
            });
            continue;
        };
        if canonical.sample_rate.get() != engine.sample_rate {
            skipped.push(SkippedClip::RateMismatch {
                clip_id: clip.id,
                source_rate: canonical.sample_rate.get(),
            });
            continue;
        }
        let (start, end) = match clip.anchor {
            TimeAnchor::Media(span) => media_span_frames(&span, engine.sample_rate),
            TimeAnchor::Musical(range) => {
                let Some(map) = &tempo else {
                    skipped.push(SkippedClip::UnresolvableAnchor { clip_id: clip.id });
                    continue;
                };
                let Ok(s) = map.tick_to_media(range.start(), 1, engine.sample_rate) else {
                    skipped.push(SkippedClip::UnresolvableAnchor { clip_id: clip.id });
                    continue;
                };
                let Ok(e) = map.tick_to_media(range.end(), 1, engine.sample_rate) else {
                    skipped.push(SkippedClip::UnresolvableAnchor { clip_id: clip.id });
                    continue;
                };
                (s.value, e.value)
            }
        };
        // Clamp a negative engine start: the skipped head shifts the
        // source window forward — never silently repositioned.
        let lead = u64::try_from(start.max(0) - start).unwrap_or(0);
        let engine_start = u64::try_from(start.max(0)).unwrap_or(0);
        let Some(len) = end
            .checked_sub(start)
            .and_then(|v| u64::try_from(v).ok())
            .map(|v| v.saturating_sub(lead))
        else {
            skipped.push(SkippedClip::UnresolvableAnchor { clip_id: clip.id });
            continue;
        };
        if len == 0 {
            skipped.push(SkippedClip::UnresolvableAnchor { clip_id: clip.id });
            continue;
        }
        let source_start = u64::try_from(clip.source_offset.value()).unwrap_or(0) + lead;
        let src_channels = u16::try_from(canonical.layout.channels()).unwrap_or(u16::MAX);
        let channel_map = if src_channels == engine.channels {
            None
        } else {
            Some(ChannelMap {
                entries: (0..engine.channels)
                    .map(|d| (d.min(src_channels.saturating_sub(1)), d))
                    .collect(),
            })
        };
        let (node_id, out_ports) = builder.add_node(&NodeDef {
            kind: NodeKind::Processor,
            ports: vec![PortDef {
                direction: Direction::Output,
                role: PortRole::Main,
                format: PortFormat::Audio(canonical),
                multiplicity: Multiplicity::exactly(1),
                capabilities: Vec::new(),
                availability: Availability::Always,
                activation: Activation::Enabled,
            }],
        });
        builder.connect(&ConnDef {
            from: PortRef {
                node: node_id,
                port: out_ports[0],
            },
            to: PortRef {
                node: master_node,
                port: master_in,
            },
            delay: EdgeDelay::Zero,
            channel_map,
        });
        clips.push(ClipBinding {
            clip_id: clip.id,
            source_id: clip.source_id,
            node_id,
            out_port: out_ports[0],
            window: ClipWindow {
                engine_start,
                engine_len: len,
                source_start,
                source_len: len,
            },
            canonical,
        });
    }
    Lowered {
        spec: builder.build(),
        clips,
        master_in,
        skipped,
    }
}
