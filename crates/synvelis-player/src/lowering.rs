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

/// A master-bus insert node between the clip fan-in and the sink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InsertBinding {
    /// Stable graph node issued by the builder.
    pub node_id: NodeId,
    /// The node's audio input port.
    pub in_port: PortId,
    /// The node's audio output port.
    pub out_port: PortId,
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
    /// Master-bus insert chain in signal order — every clip edge lands
    /// on the first insert (or the sink when empty).
    pub inserts: Vec<InsertBinding>,
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
/// `min(d, src-1)` — mono→stereo duplicates the single channel; a
/// stereo→mono engine selects channel 0 (the map carries one entry,
/// it does not sum). The engine format supports the mono/stereo
/// scope of the L1/L2 slices: callers must keep `engine.channels`
/// in `1..=2` — wider counts coerce to stereo here and are the
/// caller's responsibility to refuse earlier.
/// # Panics
/// When `engine.sample_rate` is zero (engine config invariant).
#[must_use]
#[allow(clippy::too_many_lines)] // one pass over a flat clip table
pub fn lower(
    project: &Project,
    canonical_of: &dyn Fn(SourceId) -> Option<AudioFormat>,
    engine: &EngineConfig,
) -> Lowered {
    lower_with_inserts(project, canonical_of, engine, 0)
}

/// [`lower`] plus a master-bus insert chain of `insert_count`
/// processor nodes between the clip fan-in and the sink — the offline
/// render path installs deterministic DSP there. Every insert carries
/// one input and one output port in the engine format; the first
/// insert's input is unbounded (it receives the whole clip fan-in).
/// # Panics
/// When `engine.sample_rate` is zero (engine config invariant).
#[must_use]
#[allow(clippy::too_many_lines)] // one pass over a flat clip table
pub fn lower_with_inserts(
    project: &Project,
    canonical_of: &dyn Fn(SourceId) -> Option<AudioFormat>,
    engine: &EngineConfig,
    insert_count: usize,
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

    let mut inserts = Vec::with_capacity(insert_count);
    for index in 0..insert_count {
        let (node_id, ports) = builder.add_node(&NodeDef {
            kind: NodeKind::Processor,
            ports: vec![
                PortDef {
                    direction: Direction::Input,
                    role: PortRole::Main,
                    format: PortFormat::Audio(engine_format),
                    multiplicity: if index == 0 {
                        Multiplicity::unbounded(0)
                    } else {
                        Multiplicity::exactly(1)
                    },
                    capabilities: Vec::new(),
                    availability: Availability::Always,
                    activation: Activation::Enabled,
                },
                PortDef {
                    direction: Direction::Output,
                    role: PortRole::Main,
                    format: PortFormat::Audio(engine_format),
                    multiplicity: Multiplicity::exactly(1),
                    capabilities: Vec::new(),
                    availability: Availability::Always,
                    activation: Activation::Enabled,
                },
            ],
        });
        inserts.push(InsertBinding {
            node_id,
            in_port: ports[0],
            out_port: ports[1],
        });
    }
    // Wire the chain tail-to-head: each insert's output feeds the next
    // insert (or the sink), so clip edges land on `fan_in`.
    let fan_in = inserts.first().map_or_else(
        || PortRef {
            node: master_node,
            port: master_in,
        },
        |first| PortRef {
            node: first.node_id,
            port: first.in_port,
        },
    );
    for (index, insert) in inserts.iter().enumerate() {
        let to = inserts.get(index + 1).map_or_else(
            || PortRef {
                node: master_node,
                port: master_in,
            },
            |next| PortRef {
                node: next.node_id,
                port: next.in_port,
            },
        );
        builder.connect(&ConnDef {
            from: PortRef {
                node: insert.node_id,
                port: insert.out_port,
            },
            to,
            delay: EdgeDelay::Zero,
            channel_map: None,
        });
    }

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
        // `start == i64::MIN` saturates rather than overflows; the
        // `len` check below refuses the clip in that case anyway.
        let lead =
            u64::try_from(start.checked_neg().unwrap_or(i64::MAX).max(0)).unwrap_or(u64::MAX);
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
        // A ChannelMap is required whenever the *layout* differs — not
        // only the channel count: an unpositioned canonical layout
        // (`Custom`) feeding a `Mono`/`Stereo` master validates as an
        // implicit conversion without one. The entries also cover the
        // count mismatch: mono→stereo duplicates, and a stereo→mono
        // engine selects channel 0 (a map entry cannot express a sum).
        let channel_map = if canonical.layout == engine_format.layout {
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
            to: fan_in,
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
        inserts,
        skipped,
    }
}
