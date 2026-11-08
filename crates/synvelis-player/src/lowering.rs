//! Timeline lowering: project tracks/lanes/clips and the active comps'
//! Effective projections → a generic [`GraphSpec`] plus stable
//! node/port mappings ([`PlaybackBinding`]).
//!
//! The graph side receives only `AudioFormat`s — no Project, Track or
//! Clip type crosses the boundary. Source frames, engine frames and
//! musical ticks stay distinct: `Media` anchors convert exactly in
//! their declared time base, `Musical` anchors convert through the
//! project tempo map, and the only rounding is `floor` at the final
//! engine-frame boundary.
//!
//! An active comp supersedes the raw clips on its take group's owner
//! lane inside the group's `target_range` — the adopted regions own
//! that span, so a superseded clip is reported, never double-played.
//! Each adopted segment lowers to the same playback-node shape as a
//! clip; a silence region produces no node by contract. Boundary
//! transitions lower as two extra playback nodes — the left region's
//! read extended through `boundary → overlap.end`, the right's through
//! `overlap.start → boundary` — each shaped by its declared fade
//! curve, so the overlap is a real crossfade, not a unity-gain
//! double-play.

use synvelis_project::{
    Clip, ClipId, CompId, CompRegionId, CompTransitionId, EffectiveTransition, FadeShape,
    PlaybackMapping, Project, ProjectRange, ProjectTime, SourceBinding, SourceId, TakeGroupId,
    TickTimeMapping, project_comp,
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

/// A playback node's resolved window: engine frames
/// `[start, start+len)` map 1:1 onto source frames
/// `[source_start, source_start+source_len)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipWindow {
    /// First engine frame of the node.
    pub engine_start: u64,
    /// Node length in engine frames (`> 0`).
    pub engine_len: u64,
    /// First source frame the node reads.
    pub source_start: u64,
    /// Source frames the node covers.
    pub source_len: u64,
}

/// What a lowered playback node reads — its project provenance. The
/// generic graph sees none of this; the binding keeps it for stats,
/// prepared-source mapping and skip reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeContent {
    /// A project clip.
    Clip(ClipId),
    /// An adopted comp region's base read.
    CompSegment {
        /// The projected comp.
        comp_id: CompId,
        /// The comp region this node plays.
        region_id: CompRegionId,
    },
    /// A comp transition's extended read — one side of the bounded
    /// double-play that carries the crossfade through the overlap.
    CompOverlap {
        /// The projected comp.
        comp_id: CompId,
        /// The comp transition.
        transition_id: CompTransitionId,
        /// Which boundary side this node covers.
        side: OverlapSide,
    },
}

impl NodeContent {
    /// The comp this node's content belongs to — `None` for clips.
    #[must_use]
    pub const fn comp_id(&self) -> Option<CompId> {
        match *self {
            Self::Clip(_) => None,
            Self::CompSegment { comp_id, .. } | Self::CompOverlap { comp_id, .. } => Some(comp_id),
        }
    }
}

/// Which side of a transition's overlap a [`NodeContent::CompOverlap`]
/// node covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlapSide {
    /// The left region's read extended past the boundary.
    Left,
    /// The right region's read extended before the boundary.
    Right,
}

/// Fade direction across a [`FadeWindow`]'s domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FadeDirection {
    /// 0 → unity across the domain (the incoming side).
    In,
    /// unity → 0 across the domain (the outgoing side).
    Out,
}

/// A gain envelope over an engine-frame span, applied per-frame by the
/// node's processor. Only transition-side nodes carry one — base clips
/// and comp segments play at unity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FadeWindow {
    /// The declared curve.
    pub shape: FadeShape,
    /// Fade direction.
    pub direction: FadeDirection,
    /// First engine frame of the fade domain (the transition's
    /// resolved `overlap_range` start; may be negative pre-roll).
    pub engine_start: i64,
    /// Exclusive end of the fade domain.
    pub engine_end: i64,
}

impl FadeWindow {
    /// The gain at `engine_frame`, with the fade parameter clamped to
    /// `[0, 1]` outside the domain. A degenerate domain holds unity —
    /// an overlap collapsed to a point applies no ramp.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // f64 curve → f32 sample gain
    #[allow(clippy::cast_precision_loss)] // engine frames stay far under 2^53
    pub fn gain_at(&self, engine_frame: i64) -> f32 {
        let span = self.engine_end.saturating_sub(self.engine_start);
        if span <= 0 {
            return 1.0;
        }
        let t =
            (engine_frame.saturating_sub(self.engine_start) as f64 / span as f64).clamp(0.0, 1.0);
        // `curve(t)` is the fade-in gain at t; a fade-out is the
        // mirrored curve — `cos(t·π/2)` is `sin((1-t)·π/2)`.
        let curve = |x: f64| match self.shape {
            FadeShape::Linear => x,
            FadeShape::EqualPower => (x * std::f64::consts::FRAC_PI_2).sin(),
        };
        let gain = match self.direction {
            FadeDirection::In => curve(t),
            FadeDirection::Out => curve(1.0 - t),
        };
        gain as f32
    }
}

/// Stable mapping for one lowered playback node.
#[derive(Clone, Debug)]
pub struct PlaybackBinding {
    /// What the node plays — clip, comp segment, or transition side.
    pub content: NodeContent,
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
    /// Crossfade envelope — `Some` only on transition-side nodes.
    pub fade: Option<FadeWindow>,
}

/// One resolved playback node's plan — everything needed to emit a
/// graph node, without the node itself. [`plan_playbacks`] produces
/// these in deterministic order (clips in project order, then each
/// active comp's segments and transition sides in take-group order);
/// both the legacy clip→master lowering and the SF-03 semantic
/// lowering consume the same plans so source-window resolution has a
/// single authority.
#[derive(Clone, Debug)]
pub struct PlaybackPlan {
    /// What the node plays — clip, comp segment, or transition side.
    pub content: NodeContent,
    /// The verified source it reads.
    pub source_id: SourceId,
    /// Resolved engine/source window.
    pub window: ClipWindow,
    /// The source's canonical output format.
    pub canonical: AudioFormat,
    /// Crossfade envelope — `Some` only on transition-side nodes.
    pub fade: Option<FadeWindow>,
}

/// The lane a playback plan's content sits on — clips via their lane,
/// comp content via the take group's owner lane. `None` when the
/// provenance cannot be resolved (foreign/missing entities).
#[must_use]
pub fn playback_lane_id(
    project: &Project,
    content: &NodeContent,
) -> Option<synvelis_project::LaneId> {
    match *content {
        NodeContent::Clip(clip_id) => project.clips.get(&clip_id).map(|c| c.lane_id),
        NodeContent::CompSegment { comp_id, .. } | NodeContent::CompOverlap { comp_id, .. } => {
            let comp = project.comp.comps.get(&comp_id)?;
            let group = project.comp.take_groups.get(&comp.take_group_id)?;
            Some(group.owner_lane_id)
        }
    }
}

/// Why a comp-produced node could not be lowered — mirrors the
/// clip-skip rules at the projection boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompSkipReason {
    /// The bound source is absent from the project or its canonical
    /// format could not be resolved.
    MissingSource(SourceId),
    /// The take group's `source_rate` or the source's canonical rate
    /// differs from the engine rate — implicit resampling is forbidden.
    RateMismatch(u32),
    /// The range/boundary could not be resolved to engine frames
    /// (unresolvable project time or a degenerate overlap).
    Unresolvable,
    /// A `Loop`/`SimpleStretch` mapping — the 1:1 clip-player graph
    /// cannot lower it (same rule as clips).
    UnsupportedMapping,
}

/// A clip or comp-produced node that produced no graph node — always
/// explicit, never silent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkippedPlayback {
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
    /// The playback mapping is not lowerable by the 1:1 clip-player
    /// graph (loop/stretch need a different node family — MD-A scope
    /// keeps them explicit skips, never silent).
    UnsupportedMapping {
        /// The clip.
        clip_id: ClipId,
    },
    /// The clip overlaps an active comp's target range on the comp's
    /// owner lane — the adopted regions own that span, so the raw
    /// clip is superseded rather than double-played.
    SupersededByComp {
        /// The superseded clip.
        clip_id: ClipId,
        /// The take group whose active comp owns the span.
        take_group_id: TakeGroupId,
    },
    /// Adopted silence region — produces silence by contract.
    CompSilence {
        /// The projected comp.
        comp_id: CompId,
        /// The silence region.
        region_id: CompRegionId,
    },
    /// The active comp could not be projected at lowering time — the
    /// registry failed to satisfy its own invariants (cannot happen to
    /// a state admitted by `validate_references`, so this means the
    /// project was mutated outside the controller).
    CompProjection {
        /// The failing comp.
        comp_id: CompId,
        /// Projection error detail.
        detail: String,
    },
    /// An adopted comp segment that produced no node.
    CompSegment {
        /// The projected comp.
        comp_id: CompId,
        /// The skipped region.
        region_id: CompRegionId,
        /// Why it could not be lowered.
        reason: CompSkipReason,
    },
    /// A transition side that produced no node. The base segments
    /// still play — the boundary degenerates to a hard cut, reported
    /// here rather than silently lost.
    CompOverlap {
        /// The projected comp.
        comp_id: CompId,
        /// The comp transition.
        transition_id: CompTransitionId,
        /// Which side could not be lowered.
        side: OverlapSide,
        /// Why it could not be lowered.
        reason: CompSkipReason,
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
    /// One entry per lowered playback node: clips first (project
    /// order), then each active comp's segments and transition sides
    /// (take-group order) — the order `prepared` input lists follow.
    pub playbacks: Vec<PlaybackBinding>,
    /// The master sink's input port (the graph output tap).
    pub master_in: PortId,
    /// Master-bus insert chain in signal order — every clip edge lands
    /// on the first insert (or the sink when empty).
    pub inserts: Vec<InsertBinding>,
    /// Clips and comp nodes that produced no node, with reasons.
    pub skipped: Vec<SkippedPlayback>,
}

/// An active take group's comping zone on its owner lane.
struct CompZone {
    group_id: TakeGroupId,
    comp_id: CompId,
    lane_id: synvelis_project::LaneId,
    /// The group's declared media↔source base rate.
    source_rate: u32,
    /// `target_range` resolved to engine frames — `None` when it
    /// cannot be resolved (the group then supersedes no clip; its own
    /// segments report resolvability individually).
    span: Option<(i64, i64)>,
}

/// Shared lowering context: the graph builder plus the project and
/// wiring constants every playback node connects through.
struct LowerEnv<'a> {
    builder: &'a mut GraphBuilder,
    fan_in: PortRef,
    engine_format: AudioFormat,
    engine_channels: u16,
}

/// Emits one playback-source node feeding `fan_in`, with the explicit
/// [`ChannelMap`] whenever the canonical layout differs from the
/// engine's (mono→stereo duplicates the single channel; stereo→mono
/// selects channel 0 — a map entry cannot express a sum).
fn emit_node(env: &mut LowerEnv<'_>, canonical: AudioFormat) -> (NodeId, PortId) {
    let src_channels = u16::try_from(canonical.layout.channels()).unwrap_or(u16::MAX);
    let channel_map = if canonical.layout == env.engine_format.layout {
        None
    } else {
        Some(ChannelMap {
            entries: (0..env.engine_channels)
                .map(|d| (d.min(src_channels.saturating_sub(1)), d))
                .collect(),
        })
    };
    let (node_id, out_ports) = env.builder.add_node(&NodeDef {
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
    env.builder.connect(&ConnDef {
        from: PortRef {
            node: node_id,
            port: out_ports[0],
        },
        to: env.fan_in,
        delay: EdgeDelay::Zero,
        channel_map,
    });
    (node_id, out_ports[0])
}

/// Resolves a [`ProjectRange`] to engine frames `[start, end)` —
/// `None` when the domain cannot be resolved under `map`.
fn range_span(
    range: &ProjectRange,
    map: Option<&TickTimeMapping>,
    rate: u32,
) -> Option<(i64, i64)> {
    match range {
        ProjectRange::Media(span) => {
            Some((span.start().frames_at(rate), span.end().frames_at(rate)))
        }
        ProjectRange::Musical(range) => Some((
            map?.tick_to_media(range.start(), 1, rate).ok()?.value(),
            map?.tick_to_media(range.end(), 1, rate).ok()?.value(),
        )),
    }
}

/// Resolves a [`ProjectTime`] to an engine frame — `None` when it
/// cannot be resolved under `map`.
fn point_frame(time: &ProjectTime, map: Option<&TickTimeMapping>, rate: u32) -> Option<i64> {
    match time {
        ProjectTime::Media(v) => Some(v.frames_at(rate)),
        ProjectTime::Musical(tick) => Some(map?.tick_to_media(*tick, 1, rate).ok()?.value()),
    }
}

/// Resolution context shared by every lowering path — the project,
/// the canonical source lookup and the engine geometry. No graph
/// builder: [`plan_playbacks`] resolves *what* plays, callers decide
/// *where* each node connects.
struct PlanEnv<'a> {
    project: &'a Project,
    canonical_of: &'a dyn Fn(SourceId) -> Option<AudioFormat>,
    engine: &'a EngineConfig,
}

/// Lowers one comp-side [`SourceBinding`] into a playback *plan* —
/// the same resolution [`lower_comp_node`] performed, minus node
/// emission.
///
/// The node plays engine span `play_span` reading `binding`'s source
/// from `read_start` — the caller derives `read_start` from
/// `binding.window` (a segment plays the whole base window; a
/// transition side plays only the extended head/tail slice, since the
/// base read belongs to the region's own segment node). Only `Natural`
/// mappings lower — the 1:1 contract clips already enforce.
fn comp_node_plan(
    env: &PlanEnv<'_>,
    binding: &SourceBinding,
    group_rate: u32,
    play_span: (i64, i64),
    read_start: i64,
) -> Result<(ClipWindow, AudioFormat), CompSkipReason> {
    if !env.project.sources.contains_key(&binding.source_id) {
        return Err(CompSkipReason::MissingSource(binding.source_id));
    }
    let Some(canonical) = (env.canonical_of)(binding.source_id) else {
        return Err(CompSkipReason::MissingSource(binding.source_id));
    };
    if canonical.sample_rate.get() != env.engine.sample_rate {
        return Err(CompSkipReason::RateMismatch(canonical.sample_rate.get()));
    }
    if group_rate != env.engine.sample_rate {
        // The projection's source windows are expressed in
        // `group.source_rate` media frames — a different engine rate
        // would silently resample, so it refuses.
        return Err(CompSkipReason::RateMismatch(group_rate));
    }
    if !matches!(binding.mapping, PlaybackMapping::Natural) {
        return Err(CompSkipReason::UnsupportedMapping);
    }
    let (start, end) = play_span;
    // Clamp a negative engine start: the skipped head shifts the
    // source window forward — never silently repositioned.
    let lead = u64::try_from(start.checked_neg().unwrap_or(i64::MAX).max(0)).unwrap_or(u64::MAX);
    let engine_start = u64::try_from(start.max(0)).unwrap_or(0);
    let Some(len) = end
        .checked_sub(start)
        .and_then(|v| u64::try_from(v).ok())
        .map(|v| v.saturating_sub(lead))
        .filter(|v| *v > 0)
    else {
        return Err(CompSkipReason::Unresolvable);
    };
    let source_start = u64::try_from(read_start).unwrap_or(0).saturating_add(lead);
    Ok((
        ClipWindow {
            engine_start,
            engine_len: len,
            source_start,
            source_len: len,
        },
        canonical,
    ))
}

/// Plans one active comp's Effective projection into playback nodes:
/// base segments in order, then each transition's extended sides with
/// their declared fades.
fn plan_comp(
    env: &PlanEnv<'_>,
    zone: &CompZone,
    tempo: Option<&TickTimeMapping>,
    plans: &mut Vec<PlaybackPlan>,
    skipped: &mut Vec<SkippedPlayback>,
) {
    let comp_id = zone.comp_id;
    let effective = match project_comp(env.project, comp_id) {
        Ok(e) => e,
        Err(e) => {
            skipped.push(SkippedPlayback::CompProjection {
                comp_id,
                detail: e.to_string(),
            });
            return;
        }
    };
    for segment in &effective.segments {
        let region_id = segment.region_id;
        let Some(binding) = &segment.binding else {
            skipped.push(SkippedPlayback::CompSilence { comp_id, region_id });
            continue;
        };
        let content = NodeContent::CompSegment { comp_id, region_id };
        let Some(span) = range_span(&segment.project_range, tempo, env.engine.sample_rate) else {
            skipped.push(SkippedPlayback::CompSegment {
                comp_id,
                region_id,
                reason: CompSkipReason::Unresolvable,
            });
            continue;
        };
        match comp_node_plan(
            env,
            binding,
            zone.source_rate,
            span,
            binding.window.start().value(),
        ) {
            Ok((window, canonical)) => plans.push(PlaybackPlan {
                content,
                source_id: binding.source_id,
                window,
                canonical,
                fade: None,
            }),
            Err(reason) => skipped.push(SkippedPlayback::CompSegment {
                comp_id,
                region_id,
                reason,
            }),
        }
    }
    for transition in &effective.transitions {
        plan_transition(env, zone, transition, tempo, plans, skipped);
    }
}

/// Plans one Effective transition into its two extended-read nodes.
/// A `None` side is a silence region's extension — nothing plays,
/// nothing is skipped (the region already reported its silence).
fn plan_transition(
    env: &PlanEnv<'_>,
    zone: &CompZone,
    transition: &EffectiveTransition,
    tempo: Option<&TickTimeMapping>,
    plans: &mut Vec<PlaybackPlan>,
    skipped: &mut Vec<SkippedPlayback>,
) {
    let comp_id = zone.comp_id;
    let transition_id = transition.transition_id;
    let overlap = range_span(&transition.overlap_range, tempo, env.engine.sample_rate);
    let boundary = point_frame(&transition.boundary, tempo, env.engine.sample_rate);
    let degenerate = match (overlap, boundary) {
        (Some((o_start, o_end)), Some(b)) => b <= o_start || b >= o_end,
        _ => true,
    };
    if degenerate {
        for side in [OverlapSide::Left, OverlapSide::Right] {
            if transition_side(transition, side).is_some() {
                skipped.push(SkippedPlayback::CompOverlap {
                    comp_id,
                    transition_id,
                    side,
                    reason: CompSkipReason::Unresolvable,
                });
            }
        }
        return;
    }
    let (o_start, o_end) = overlap.unwrap_or_default();
    let boundary = boundary.unwrap_or_default();
    for side in [OverlapSide::Left, OverlapSide::Right] {
        let Some(binding) = transition_side(transition, side) else {
            continue;
        };
        // The extended window widens the region's base read; the side
        // node plays only the overhang — `boundary → overlap.end` for
        // the left, `overlap.start → boundary` for the right. For a
        // `Natural` binding the played source length equals the played
        // engine length, so `read_start` is the window's tail (left)
        // or head (right) slice.
        let (play_span, read_start, shape, direction) = match side {
            OverlapSide::Left => {
                let Some(read_start) = binding
                    .window
                    .end()
                    .value()
                    .checked_sub(o_end.saturating_sub(boundary))
                else {
                    skipped.push(SkippedPlayback::CompOverlap {
                        comp_id,
                        transition_id,
                        side,
                        reason: CompSkipReason::Unresolvable,
                    });
                    continue;
                };
                (
                    (boundary, o_end),
                    read_start,
                    transition.fade_out,
                    FadeDirection::Out,
                )
            }
            OverlapSide::Right => (
                (o_start, boundary),
                binding.window.start().value(),
                transition.fade_in,
                FadeDirection::In,
            ),
        };
        // The envelope domain is the side's own overhang span: the
        // left tail starts at unity at the boundary and decays to
        // zero, the right head rises from zero to unity — each ends
        // exactly where its segment's base read owns unity.
        let fade = FadeWindow {
            shape,
            direction,
            engine_start: play_span.0,
            engine_end: play_span.1,
        };
        let content = NodeContent::CompOverlap {
            comp_id,
            transition_id,
            side,
        };
        match comp_node_plan(env, binding, zone.source_rate, play_span, read_start) {
            Ok((window, canonical)) => plans.push(PlaybackPlan {
                content,
                source_id: binding.source_id,
                window,
                canonical,
                fade: Some(fade),
            }),
            Err(reason) => skipped.push(SkippedPlayback::CompOverlap {
                comp_id,
                transition_id,
                side,
                reason,
            }),
        }
    }
}

/// One side's extended binding — `None` when that region is silence.
fn transition_side(transition: &EffectiveTransition, side: OverlapSide) -> Option<&SourceBinding> {
    match side {
        OverlapSide::Left => transition.left.as_ref(),
        OverlapSide::Right => transition.right.as_ref(),
    }
}

/// Lowers the project's clips and active comps into a generic audio
/// graph spec.
///
/// `canonical_of(source_id)` supplies each resolved source's canonical
/// format (the resolver's output); `None` means the source could not
/// be prepared and its nodes are skipped as `MissingSource`.
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
#[allow(clippy::too_many_lines)] // one pass over clips plus comp projections
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

    let (plans, skipped) = plan_playbacks(project, canonical_of, engine);
    let mut env = LowerEnv {
        builder: &mut builder,
        fan_in,
        engine_format,
        engine_channels: engine.channels,
    };
    let mut playbacks = Vec::with_capacity(plans.len());
    for plan in plans {
        let (node_id, out_port) = emit_node(&mut env, plan.canonical);
        playbacks.push(PlaybackBinding {
            content: plan.content,
            source_id: plan.source_id,
            node_id,
            out_port,
            window: plan.window,
            canonical: plan.canonical,
            fade: plan.fade,
        });
    }

    Lowered {
        spec: builder.build(),
        playbacks,
        master_in,
        inserts,
        skipped,
    }
}

/// Resolves the project's clips and active comps into ordered
/// [`PlaybackPlan`]s plus explicit skips — the single authority for
/// *what* plays. Node emission and edge wiring are the caller's
/// concern: the legacy lowering feeds every plan to one fan-in, the
/// SF-03 semantic lowering feeds each plan to its owning strip's
/// main input.
///
/// Order is deterministic: clips in `project.clips` (`BTreeMap`)
/// order, then each active take group's comp segments and transition
/// sides in take-group order. Skips are always explicit — muted,
/// superseded, missing-source, rate-mismatch, unresolvable-anchor and
/// unsupported-mapping clips each land in `skipped` with a reason.
#[must_use]
pub fn plan_playbacks(
    project: &Project,
    canonical_of: &dyn Fn(SourceId) -> Option<AudioFormat>,
    engine: &EngineConfig,
) -> (Vec<PlaybackPlan>, Vec<SkippedPlayback>) {
    let env = PlanEnv {
        project,
        canonical_of,
        engine,
    };
    let tempo = TickTimeMapping::new(&env.project.tempo_map).ok();

    // Active comps supersede raw clips inside their target range on the
    // owner lane — collect the zones up front so the clip pass can mark
    // superseded clips explicitly.
    let mut zones = Vec::new();
    for (group_id, group) in &env.project.comp.take_groups {
        let Some(comp_id) = group.active_comp_id else {
            continue;
        };
        zones.push(CompZone {
            group_id: *group_id,
            comp_id,
            lane_id: group.owner_lane_id,
            source_rate: group.source_rate,
            span: range_span(&group.target_range, tempo.as_ref(), env.engine.sample_rate),
        });
    }

    let mut plans = Vec::new();
    let mut skipped = Vec::new();
    for clip in env.project.clips.values() {
        plan_clip(&env, clip, &zones, tempo.as_ref(), &mut plans, &mut skipped);
    }

    // Comp pass: active comps in take-group order contribute their
    // adopted segments and transition sides.
    for zone in &zones {
        plan_comp(&env, zone, tempo.as_ref(), &mut plans, &mut skipped);
    }

    (plans, skipped)
}

/// Plans one clip into a [`PlaybackPlan`] or an explicit skip —
/// muted, superseded-by-comp, missing-source, rate-mismatch,
/// unresolvable-anchor and unsupported-mapping each land in
/// `skipped` with a reason.
fn plan_clip(
    env: &PlanEnv<'_>,
    clip: &Clip,
    zones: &[CompZone],
    tempo: Option<&TickTimeMapping>,
    plans: &mut Vec<PlaybackPlan>,
    skipped: &mut Vec<SkippedPlayback>,
) {
    if clip.muted {
        skipped.push(SkippedPlayback::Muted { clip_id: clip.id });
        return;
    }
    // A clip inside an active comp's target span is superseded —
    // the adopted regions own that span, so the clip neither plays
    // nor is health-checked.
    if let Ok(span) = clip.placement.resolve_span(tempo, env.engine.sample_rate) {
        let (clip_start, clip_end) = (span.start().value(), span.end().value());
        if let Some(zone) = zones.iter().find(|z| {
            z.lane_id == clip.lane_id && z.span.is_some_and(|(s, e)| clip_start < e && s < clip_end)
        }) {
            skipped.push(SkippedPlayback::SupersededByComp {
                clip_id: clip.id,
                take_group_id: zone.group_id,
            });
            return;
        }
    }
    let Some(canonical) = playable_clip(env, clip, skipped) else {
        return;
    };
    let Ok(resolved) = clip
        .placement
        .resolve(&clip.playback, tempo, env.engine.sample_rate)
    else {
        skipped.push(SkippedPlayback::UnresolvableAnchor { clip_id: clip.id });
        return;
    };
    if !matches!(resolved.mapping, PlaybackMapping::Natural) {
        skipped.push(SkippedPlayback::UnsupportedMapping { clip_id: clip.id });
        return;
    }
    let start = resolved.media_span.start().value();
    let end = resolved.media_span.end().value();
    // Clamp a negative engine start: the skipped head shifts the
    // source window forward — never silently repositioned.
    // `start == i64::MIN` saturates rather than overflows; the
    // `len` check below refuses the clip in that case anyway.
    let lead = u64::try_from(start.checked_neg().unwrap_or(i64::MAX).max(0)).unwrap_or(u64::MAX);
    let engine_start = u64::try_from(start.max(0)).unwrap_or(0);
    let Some(len) = end
        .checked_sub(start)
        .and_then(|v| u64::try_from(v).ok())
        .map(|v| v.saturating_sub(lead))
    else {
        skipped.push(SkippedPlayback::UnresolvableAnchor { clip_id: clip.id });
        return;
    };
    if len == 0 {
        skipped.push(SkippedPlayback::UnresolvableAnchor { clip_id: clip.id });
        return;
    }
    let source_start = u64::try_from(resolved.source_start.value()).unwrap_or(0) + lead;
    plans.push(PlaybackPlan {
        content: NodeContent::Clip(clip.id),
        source_id: clip.source_id,
        window: ClipWindow {
            engine_start,
            engine_len: len,
            source_start,
            source_len: len,
        },
        canonical,
        fade: None,
    });
}

/// Source/canonical/rate gates shared by clip planning — pushes the
/// matching skip and returns `None`, or returns the canonical format.
fn playable_clip(
    env: &PlanEnv<'_>,
    clip: &Clip,
    skipped: &mut Vec<SkippedPlayback>,
) -> Option<AudioFormat> {
    if !env.project.sources.contains_key(&clip.source_id) {
        skipped.push(SkippedPlayback::MissingSource {
            clip_id: clip.id,
            source_id: clip.source_id,
        });
        return None;
    }
    let Some(canonical) = (env.canonical_of)(clip.source_id) else {
        skipped.push(SkippedPlayback::MissingSource {
            clip_id: clip.id,
            source_id: clip.source_id,
        });
        return None;
    };
    if canonical.sample_rate.get() != env.engine.sample_rate {
        skipped.push(SkippedPlayback::RateMismatch {
            clip_id: clip.id,
            source_rate: canonical.sample_rate.get(),
        });
        return None;
    }
    Some(canonical)
}
