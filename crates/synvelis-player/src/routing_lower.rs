//! SF-03 slice 1 — semantic `RoutingModel` → Ruviform typed `GraphSpec`
//! lowering plus the generation-scoped [`RoutingSourceMap`].
//!
//! Topology per strip (deterministic `BTreeMap` order):
//!
//! ```text
//!   clip/comp playback ─┐
//!   incoming Main/Send ─┼─→ [chain processor 0] → … → [chain n-1] ─┐
//!                        │        (empty chain: direct)          ▼
//!                        └──────────────────────────────→ strip fader/pan
//!                                                             │     │
//!                                          pre-fader tap ←────┘     └─→ post-fader out
//! ```
//!
//! The semantic main *input* port resolves to the chain head's input
//! (or the fader input when the chain is empty); the semantic main
//! *output* port resolves to the fader's post-out. The fader node also
//! exposes a dry `pre_out` carrying the post-chain/pre-fader signal
//! for `SendTap::PreFader`. Mute/solo are baked at lowering time via
//! [`RoutingModel::audible_strips`] — edges out of non-audible strips
//! are simply not emitted (the same authority the semantic oracle
//! uses), never a runtime mute guess.
//!
//! Chain processor nodes are emitted for every chain member; the
//! compile factory must supply an instance or the build fails closed
//! (`PluginUnavailable`) — no silent bypass, no fake silence.

use std::collections::{BTreeMap, BTreeSet};

use ruviform_audio::typed::{GraphProcessor, NodeIo, OfflineRenderSupport};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_graph::{
    Activation, Availability, ChannelMap as GraphChannelMap, ConnDef, ConnectionId, Direction,
    EdgeDelay, GraphBuilder, GraphSpec, Multiplicity, NodeDef, NodeId, NodeKind, PortDef,
    PortFormat, PortId, PortRef as GraphPortRef, PortRole as GraphPortRole,
};
use ruviform_rt::RtContext;
use synvelis_project::{
    BuildBudget, ChannelStrip, ConnectionKind, ControlOwner, ExplicitChannelMap, MappingResolution,
    NotLoweredReason, ParameterRole, PlaybackContentRef, PlaybackOverlapSide, PortDirection,
    PortOwner, PortRole, Processor, ProcessorId, Project, RoutingBuildKey, RoutingConnection,
    RoutingConnectionId, RoutingFault, RoutingModel, RoutingPlaybackRef, RoutingSourceMap,
    SemanticNodeRef, SemanticPortId, SendConfig, SendId, SendTap, SignalDomain, SourceId, StripId,
    UnsupportedReason,
};

use crate::lowering::{
    EngineConfig, InsertBinding, NodeContent, PlaybackBinding, PlaybackPlan, SkippedPlayback,
    plan_playbacks, playback_lane_id,
};

/// Deterministic lowering failures — each maps to a [`RoutingFault`]
/// at the engine boundary; none are silent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticLowerError {
    /// A semantic port's channel count cannot be represented by the
    /// engine format.
    ChannelMismatch {
        /// The offending port.
        port: SemanticPortId,
    },
    /// A non-audio signal domain reached the audio lowering.
    UnsupportedDomain {
        /// The offending port.
        port: SemanticPortId,
    },
    /// A semantic port reference cannot be resolved to a port entity.
    PortUnresolved {
        /// The dangling port reference.
        port: SemanticPortId,
    },
    /// A clip/comp playback's lane→track→strip chain is broken, or a
    /// referenced semantic entity is absent.
    MissingStrip {
        /// What could not be resolved.
        detail: String,
    },
    /// The project has no master bus/strip to terminate at.
    MasterMissing,
    /// A send references a gain target absent from the control model.
    DanglingGainTarget {
        /// The send.
        send: SendId,
    },
    /// The lowered graph exceeded the request's resource budget.
    BudgetExceeded {
        /// Which bound was crossed.
        detail: &'static str,
    },
    /// The requested output tap format cannot represent the engine
    /// format — the sink must share the engine's sample rate and
    /// sample format; only the channel count may differ, and that is
    /// bridged by the explicit channel map on the sink edge.
    OutputFormat {
        /// What mismatched.
        detail: &'static str,
    },
}

impl SemanticLowerError {
    /// The [`RoutingFault`] this lowering failure reports.
    #[must_use]
    pub const fn fault(&self) -> RoutingFault {
        match self {
            Self::ChannelMismatch { .. } | Self::UnsupportedDomain { .. } => {
                RoutingFault::UnsupportedFormatOrTimeMap
            }
            Self::PortUnresolved { .. } => RoutingFault::PortUnresolved,
            Self::MissingStrip { .. } | Self::MasterMissing | Self::DanglingGainTarget { .. } => {
                RoutingFault::InvalidRouting
            }
            Self::BudgetExceeded { .. } => RoutingFault::CompileResourceExceeded,
            Self::OutputFormat { .. } => RoutingFault::UnsupportedFormatOrTimeMap,
        }
    }
}

/// What runtime object a graph node plays — consumed by the engine's
/// compile factory to install the right [`GraphProcessor`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SemanticNodeRole {
    /// Index into [`SemanticLowered::playbacks`].
    Playback(usize),
    /// A strip's fader/pan stage; `gains` are the per-channel
    /// (left, right) linear gains already including pan law.
    StripFader {
        /// Owning strip.
        strip: StripId,
        /// `[left, right]` effective gains.
        gains: [f32; 2],
    },
    /// A send-level gain stage on one send edge.
    SendGain {
        /// Owning send.
        send: SendId,
        /// Linear gain (1.0 = unity).
        gain: f32,
    },
    /// A chain processor slot — the factory must supply the instance.
    ChainProcessor {
        /// Semantic processor ID.
        processor: ProcessorId,
    },
}

/// Semantic lowering output: the executable spec plus everything
/// needed to bind processors and read the output tap.
#[derive(Clone, Debug)]
pub struct SemanticLowered {
    /// The typed graph spec (graph-local IDs only).
    pub spec: GraphSpec,
    /// Semantic→runtime map for this build generation.
    pub source_map: RoutingSourceMap,
    /// Playback bindings in plan order — `prepared` input lists and
    /// `node_roles::Playback(i)` index into this.
    pub playbacks: Vec<PlaybackBinding>,
    /// Node → runtime role for the compile factory.
    pub node_roles: BTreeMap<NodeId, SemanticNodeRole>,
    /// Device sink input port — the graph output tap.
    pub device_in: PortId,
    /// Explicit skips from the shared playback planner.
    pub skipped: Vec<SkippedPlayback>,
    /// Output-stage insert bindings in signal order — empty unless
    /// the lowering ran with [`SemanticOutput::insert_count`] > 0.
    /// These nodes carry no semantic meaning: they never enter
    /// `node_roles` or the source map, and the consumer binds one
    /// processor per binding (the offline render path's master-bus
    /// DSP slots).
    pub inserts: Vec<InsertBinding>,
}

/// The explicit output stage an offline-render lowering adds on top
/// of the semantic graph — the only consumer that needs it is
/// `synvelis-render`. [`SemanticOutput::default`] reproduces the
/// realtime lowering exactly (no inserts, sink at engine format).
///
/// The output stage lives strictly at the sink boundary: insert
/// nodes run at the *engine* format inside the graph, and a
/// differing [`Self::format`] channel count is bridged ONLY by the
/// explicit channel map on the edge into the device sink — never an
/// implicit mix inside the semantic graph (Issue #106 forbids
/// implicit channel-layout conversion; an explicit request-declared
/// output format is not one).
#[derive(Clone, Copy, Debug, Default)]
pub struct SemanticOutput {
    /// Master-bus insert slots spliced between the master strip's
    /// post-fader output and the device sink, in signal order. The
    /// count bounds the emitted nodes — the consumer zips
    /// [`SemanticLowered::inserts`] with its own DSP specs.
    pub insert_count: usize,
    /// Device-sink tap format override. `None` taps at the engine
    /// format. `Some` must share the engine's sample rate and sample
    /// format ([`SemanticLowerError::OutputFormat`] otherwise); a
    /// differing channel count is bridged by the explicit channel
    /// map on the sink edge.
    pub format: Option<AudioFormat>,
}

/// Per-strip resolved wiring.
#[derive(Clone, Copy)]
struct StripWiring {
    /// Fader/pan node.
    fader_node: NodeId,
    /// Semantic main-input resolution: chain head input or fader in.
    entry: GraphPortRef,
    /// Semantic main-output resolution: fader post-out.
    main_out: GraphPortRef,
    /// Pre-fader tap resolution: fader pre-out.
    pre_tap: GraphPortRef,
}

/// Per-processor resolved wiring.
struct ProcessorWiring {
    node: NodeId,
    /// Sidechain input ports keyed by semantic port.
    sidechain: BTreeMap<SemanticPortId, PortId>,
}

/// Resolved ends of one routing connection.
struct ConnectionEnds {
    /// Strip the source port belongs to (audibility gate, tap owner).
    src_strip: StripId,
    /// Runtime port the semantic destination resolves to.
    dst_ref: GraphPortRef,
    /// Strip whose audibility gates the destination.
    dst_strip: StripId,
}

/// Strip fader/pan stage — passthrough pre-tap plus gained post-tap.
/// Fan-in summation is the runtime's input materialization, identical
/// to the legacy clip→master path.
pub struct StripProcessor {
    gains: [f32; 2],
}

impl StripProcessor {
    /// `gains` are `[left, right]` linear gains.
    #[must_use]
    pub const fn new(gains: [f32; 2]) -> Self {
        Self { gains }
    }
}

impl GraphProcessor for StripProcessor {
    fn offline_support(&self) -> OfflineRenderSupport {
        OfflineRenderSupport::Verified
    }

    fn process(&mut self, io: &mut NodeIo<'_>, _ctx: &RtContext) {
        let frames = io.frames();
        if frames == 0 {
            return;
        }
        let input = io.input(0);
        let mut post = io.output(0);
        let mut pre = io.output(1);
        for c in 0..2usize {
            let gain = self.gains[c.min(1)];
            let src_len = input
                .as_ref()
                .and_then(|i| i.channel(c))
                .map_or(0, <[f32]>::len);
            let n = frames.min(src_len);
            if let Some(dst) = pre.as_mut().and_then(|v| v.channel_mut(c)) {
                match input.as_ref().and_then(|i| i.channel(c)) {
                    Some(src) => {
                        dst[..n].copy_from_slice(&src[..n]);
                        dst[n..frames].fill(0.0);
                    }
                    None => dst[..frames].fill(0.0),
                }
            }
            if let Some(dst) = post.as_mut().and_then(|v| v.channel_mut(c)) {
                match input.as_ref().and_then(|i| i.channel(c)) {
                    Some(src) => {
                        for (d, s) in dst[..n].iter_mut().zip(&src[..n]) {
                            *d = s * gain;
                        }
                        dst[n..frames].fill(0.0);
                    }
                    None => dst[..frames].fill(0.0),
                }
            }
        }
    }
}

/// Linear gain stage for send levels and test-injected DSP.
pub struct GainProcessor {
    gain: f32,
}

impl GainProcessor {
    /// `gain` is linear (1.0 = unity).
    #[must_use]
    pub const fn new(gain: f32) -> Self {
        Self { gain }
    }
}

impl GraphProcessor for GainProcessor {
    fn offline_support(&self) -> OfflineRenderSupport {
        OfflineRenderSupport::Verified
    }

    fn process(&mut self, io: &mut NodeIo<'_>, _ctx: &RtContext) {
        let frames = io.frames();
        if frames == 0 {
            return;
        }
        let input = io.input(0);
        let mut out = io.output(0);
        for c in 0..2usize {
            let src_len = input
                .as_ref()
                .and_then(|i| i.channel(c))
                .map_or(0, <[f32]>::len);
            let n = frames.min(src_len);
            if let Some(dst) = out.as_mut().and_then(|v| v.channel_mut(c)) {
                match input.as_ref().and_then(|i| i.channel(c)) {
                    Some(src) => {
                        for (d, s) in dst[..n].iter_mut().zip(&src[..n]) {
                            *d = s * self.gain;
                        }
                        dst[n..frames].fill(0.0);
                    }
                    None => dst[..frames].fill(0.0),
                }
            }
        }
    }
}

/// Mutable lowering state — the builder plus every map the
/// [`RoutingSourceMap`] is assembled from. Kept as one struct so each
/// lowering phase is a method with `&mut self`, not a free function
/// threading a dozen arguments.
struct Lower<'a> {
    builder: GraphBuilder,
    project: &'a Project,
    routing: &'a RoutingModel,
    engine_format: AudioFormat,
    engine_channels: u16,
    /// Device-sink port format — `engine_format` unless the caller
    /// declared an explicit [`SemanticOutput::format`].
    sink_format: AudioFormat,
    /// Explicit channel map bridging graph format → sink format on
    /// the edge into the sink — `None` when the formats agree.
    output_channel_map: Option<GraphChannelMap>,
    /// Output insert nodes in signal order (empty for realtime).
    output_inserts: Vec<InsertBinding>,
    audible: BTreeSet<StripId>,
    strip_wiring: BTreeMap<StripId, StripWiring>,
    processor_wiring: BTreeMap<ProcessorId, ProcessorWiring>,
    /// Send gain stages created lazily in first-use order:
    /// send → (node, in port, out port).
    send_gain_nodes: BTreeMap<SendId, (NodeId, PortId, PortId)>,
    device: Option<(NodeId, PortId)>,
    ports_map: BTreeMap<SemanticPortId, MappingResolution<GraphPortRef>>,
    nodes_map: BTreeMap<SemanticNodeRef, MappingResolution<NodeId>>,
    connections_map: BTreeMap<RoutingConnectionId, MappingResolution<ConnectionId>>,
    send_taps: BTreeMap<SendId, GraphPortRef>,
    playback_bindings: Vec<RoutingPlaybackRef>,
    node_roles: BTreeMap<NodeId, SemanticNodeRole>,
}

impl<'a> Lower<'a> {
    fn new(project: &'a Project, engine: &EngineConfig, output: SemanticOutput) -> Self {
        let engine_format = engine_format(engine);
        let sink_format = output.format.unwrap_or(engine_format);
        let output_channel_map = (sink_format.layout.channels() != engine_format.layout.channels())
            .then(|| {
                // Same convention as the playback edges: sink channel `d`
                // reads graph channel `min(d, src-1)` — an explicit map,
                // never an implicit mix.
                let src = u16::try_from(engine_format.layout.channels()).unwrap_or(u16::MAX);
                let dst = u16::try_from(sink_format.layout.channels()).unwrap_or(u16::MAX);
                GraphChannelMap {
                    entries: (0..dst)
                        .map(|d| (d.min(src.saturating_sub(1)), d))
                        .collect(),
                }
            });
        Self {
            builder: GraphBuilder::new(),
            project,
            routing: &project.routing,
            engine_format,
            engine_channels: u16::try_from(engine_format.layout.channels()).unwrap_or(u16::MAX),
            sink_format,
            output_channel_map,
            output_inserts: Vec::with_capacity(output.insert_count),
            audible: project.routing.audible_strips(&project.control),
            strip_wiring: BTreeMap::new(),
            processor_wiring: BTreeMap::new(),
            send_gain_nodes: BTreeMap::new(),
            device: None,
            ports_map: BTreeMap::new(),
            nodes_map: BTreeMap::new(),
            connections_map: BTreeMap::new(),
            send_taps: BTreeMap::new(),
            playback_bindings: Vec::new(),
            node_roles: BTreeMap::new(),
        }
    }

    /// Device sink node — the single graph output tap, at the
    /// declared sink format (engine format unless overridden).
    fn emit_device_sink(&mut self) {
        let (node, ports) = self.builder.add_node(&NodeDef {
            kind: NodeKind::Sink,
            ports: vec![audio_port(
                Direction::Input,
                GraphPortRole::Main,
                self.sink_format,
                Multiplicity::unbounded(0),
            )],
        });
        self.device = Some((node, ports[0]));
    }

    /// Output insert slots between the master output and the sink —
    /// engine-format processor nodes, one input + one output each,
    /// exactly one edge per port. They carry no semantic meaning and
    /// never enter `node_roles`/the source map.
    fn emit_output_inserts(&mut self, count: usize) {
        for _ in 0..count {
            let (node, ports) = self.builder.add_node(&NodeDef {
                kind: NodeKind::Processor,
                ports: vec![
                    audio_port(
                        Direction::Input,
                        GraphPortRole::Main,
                        self.engine_format,
                        Multiplicity::exactly(1),
                    ),
                    audio_port(
                        Direction::Output,
                        GraphPortRole::Main,
                        self.engine_format,
                        Multiplicity::exactly(1),
                    ),
                ],
            });
            self.output_inserts.push(InsertBinding {
                node_id: node,
                in_port: ports[0],
                out_port: ports[1],
            });
        }
    }

    /// Every strip: fader node, chain nodes, intra-strip wiring and
    /// semantic port resolutions.
    fn emit_strips(&mut self) -> Result<(), SemanticLowerError> {
        let strips: Vec<StripId> = self.routing.strips.keys().copied().collect();
        for strip_id in strips {
            let strip = &self.routing.strips[&strip_id];
            self.emit_strip(strip)?;
        }
        Ok(())
    }

    /// One strip's fader + chain + port map entries.
    fn emit_strip(&mut self, strip: &ChannelStrip) -> Result<(), SemanticLowerError> {
        let (fader_node, fader_ports) = self.builder.add_node(&NodeDef {
            kind: NodeKind::Processor,
            ports: vec![
                audio_port(
                    Direction::Input,
                    GraphPortRole::Main,
                    self.engine_format,
                    Multiplicity::unbounded(0),
                ),
                audio_port(
                    Direction::Output,
                    GraphPortRole::Main,
                    self.engine_format,
                    Multiplicity::unbounded(0),
                ),
                audio_port(
                    Direction::Output,
                    GraphPortRole::Main,
                    self.engine_format,
                    Multiplicity::unbounded(0),
                ),
            ],
        });
        let fader_in = fader_ports[0];
        let post_out = fader_ports[1];
        let pre_out = fader_ports[2];
        let gains = strip_gains(self.project, strip.id, self.engine_channels);
        self.node_roles.insert(
            fader_node,
            SemanticNodeRole::StripFader {
                strip: strip.id,
                gains,
            },
        );

        let chain_nodes = self.emit_chain(strip)?;

        // Entry = chain head input, or the fader input for an empty
        // chain; last chain output feeds the fader input.
        let entry = match chain_nodes.first() {
            Some(&(_, node, main_in, _)) => GraphPortRef {
                node,
                port: main_in,
            },
            None => GraphPortRef {
                node: fader_node,
                port: fader_in,
            },
        };
        for pair in chain_nodes.windows(2) {
            self.connect(
                GraphPortRef {
                    node: pair[0].1,
                    port: pair[0].3,
                },
                GraphPortRef {
                    node: pair[1].1,
                    port: pair[1].2,
                },
                None,
            );
        }
        if let Some(&(_, node, _, main_out)) = chain_nodes.last() {
            self.connect(
                GraphPortRef {
                    node,
                    port: main_out,
                },
                GraphPortRef {
                    node: fader_node,
                    port: fader_in,
                },
                None,
            );
        }

        self.strip_wiring.insert(
            strip.id,
            StripWiring {
                fader_node,
                entry,
                main_out: GraphPortRef {
                    node: fader_node,
                    port: post_out,
                },
                pre_tap: GraphPortRef {
                    node: fader_node,
                    port: pre_out,
                },
            },
        );
        self.nodes_map.insert(
            SemanticNodeRef::Strip(strip.id),
            MappingResolution::Resolved(fader_node),
        );
        self.map_strip_ports(strip)?;
        Ok(())
    }

    /// Chain processor nodes in declared order plus their semantic
    /// port/node map entries. Returns `(processor, node, main_in,
    /// main_out)` per member.
    fn emit_chain(
        &mut self,
        strip: &ChannelStrip,
    ) -> Result<Vec<(ProcessorId, NodeId, PortId, PortId)>, SemanticLowerError> {
        let chain = self.routing.chains.get(&strip.chain).ok_or_else(|| {
            SemanticLowerError::MissingStrip {
                detail: format!("strip {} references missing chain", strip.id),
            }
        })?;
        let mut chain_nodes = Vec::with_capacity(chain.processors.len());
        for processor_id in &chain.processors {
            chain_nodes.push(self.emit_chain_member(*processor_id)?);
        }
        Ok(chain_nodes)
    }

    /// One chain member: processor node, role registration and port
    /// resolutions.
    fn emit_chain_member(
        &mut self,
        processor_id: ProcessorId,
    ) -> Result<(ProcessorId, NodeId, PortId, PortId), SemanticLowerError> {
        let processor = self.routing.processors.get(&processor_id).ok_or_else(|| {
            SemanticLowerError::MissingStrip {
                detail: format!("chain references missing processor {processor_id}"),
            }
        })?;
        let (node, port_ids) = self.emit_processor_node(processor)?;
        let mut main_in = None;
        let mut main_out = None;
        let mut sidechain = BTreeMap::new();
        for (sem_id, graph_id) in processor.port_ids.iter().zip(port_ids.iter()) {
            let Some(port) = self.routing.ports.get(sem_id) else {
                continue;
            };
            match (port.direction, port.role) {
                (PortDirection::Input, PortRole::Main) => main_in = Some(*graph_id),
                (PortDirection::Output, PortRole::Main) => main_out = Some(*graph_id),
                (PortDirection::Input, PortRole::Sidechain) => {
                    sidechain.insert(*sem_id, *graph_id);
                }
                _ => {}
            }
            self.ports_map.insert(
                *sem_id,
                MappingResolution::Resolved(GraphPortRef {
                    node,
                    port: *graph_id,
                }),
            );
        }
        let (Some(mi), Some(mo)) = (main_in, main_out) else {
            return Err(SemanticLowerError::MissingStrip {
                detail: format!("processor {processor_id} lacks main ports"),
            });
        };
        self.node_roles.insert(
            node,
            SemanticNodeRole::ChainProcessor {
                processor: processor_id,
            },
        );
        self.nodes_map.insert(
            SemanticNodeRef::Processor(processor_id),
            MappingResolution::Resolved(node),
        );
        self.processor_wiring
            .insert(processor_id, ProcessorWiring { node, sidechain });
        Ok((processor_id, node, mi, mo))
    }

    /// Strip semantic ports → runtime ports. Domain/channel agreement
    /// with the engine format is enforced — a mismatch is an explicit
    /// failure, never an implicit map.
    fn map_strip_ports(&mut self, strip: &ChannelStrip) -> Result<(), SemanticLowerError> {
        let wiring = self.strip_wiring[&strip.id];
        for port_id in &strip.port_ids {
            let Some(port) = self.routing.ports.get(port_id) else {
                continue;
            };
            check_port_domain(*port_id, port.domain, port.channels, self.engine_format)?;
            let resolved = match (port.direction, port.role) {
                (PortDirection::Input, PortRole::Main) => MappingResolution::Resolved(wiring.entry),
                (PortDirection::Output, PortRole::Main) => {
                    MappingResolution::Resolved(wiring.main_out)
                }
                _ => MappingResolution::Unsupported(UnsupportedReason::TopologyUnsupported),
            };
            self.ports_map.insert(*port_id, resolved);
        }
        Ok(())
    }

    /// Bus entities resolve through their strip.
    fn map_bus_nodes(&mut self) {
        for bus in self.routing.buses.values() {
            if let Some(w) = self.strip_wiring.get(&bus.strip) {
                self.nodes_map.insert(
                    SemanticNodeRef::Bus(bus.id),
                    MappingResolution::Resolved(w.fader_node),
                );
            }
        }
    }

    /// Every routing connection — disabled ones are recorded
    /// `NotLowered`, enabled ones lower to graph edges.
    fn lower_connections(&mut self) -> Result<(), SemanticLowerError> {
        let conns: Vec<RoutingConnectionId> = self.routing.connections.keys().copied().collect();
        for conn_id in conns {
            let conn = &self.routing.connections[&conn_id];
            if !conn.enabled {
                self.connections_map.insert(
                    conn_id,
                    MappingResolution::NotLowered(NotLoweredReason::DisabledConnection),
                );
                continue;
            }
            let resolution = self.lower_connection(conn)?;
            self.connections_map.insert(conn_id, resolution);
        }
        Ok(())
    }

    /// Orphan processors (declared but in no chain) lower to nothing —
    /// recorded explicitly, never silently dropped.
    fn mark_orphan_processors(&mut self) {
        for processor in self.routing.processors.values() {
            if !self.processor_wiring.contains_key(&processor.id) {
                self.nodes_map.insert(
                    SemanticNodeRef::Processor(processor.id),
                    MappingResolution::NotLowered(NotLoweredReason::OutOfScope),
                );
            }
        }
    }

    /// Master strip's main output → device sink input.
    fn connect_master_sink(&mut self) -> Result<(), SemanticLowerError> {
        let master_strip = self
            .routing
            .buses
            .get(&self.routing.master)
            .map(|b| b.strip)
            .ok_or(SemanticLowerError::MasterMissing)?;
        let master_wiring = self
            .strip_wiring
            .get(&master_strip)
            .copied()
            .ok_or(SemanticLowerError::MasterMissing)?;
        let (device_node, device_in) = self.device.ok_or(SemanticLowerError::MasterMissing)?;
        let sink = GraphPortRef {
            node: device_node,
            port: device_in,
        };
        // master post-out → insert[0].in → insert[0].out → … →
        // insert[n-1].out → sink; with no inserts the master output
        // feeds the sink directly. The explicit channel map rides
        // only the edge into the sink.
        let mut prev = master_wiring.main_out;
        for insert in self.output_inserts.clone() {
            self.connect(
                prev,
                GraphPortRef {
                    node: insert.node_id,
                    port: insert.in_port,
                },
                None,
            );
            prev = GraphPortRef {
                node: insert.node_id,
                port: insert.out_port,
            };
        }
        self.connect(prev, sink, self.output_channel_map.clone());
        Ok(())
    }

    /// Playback nodes → owning strip main input.
    fn emit_playbacks(
        &mut self,
        canonical_of: &dyn Fn(SourceId) -> Option<AudioFormat>,
        engine: &EngineConfig,
    ) -> Result<(Vec<PlaybackBinding>, Vec<SkippedPlayback>), SemanticLowerError> {
        let (plans, skipped) = plan_playbacks(self.project, canonical_of, engine);
        let mut playbacks = Vec::with_capacity(plans.len());
        for plan in &plans {
            playbacks.push(self.emit_playback(plan, playbacks.len())?);
        }
        Ok((playbacks, skipped))
    }

    /// Assemble the finished [`SemanticLowered`] — build, budget
    /// check, source map.
    fn finish(
        self,
        key: &RoutingBuildKey,
        generation: u64,
        budget: &BuildBudget,
        playbacks: Vec<PlaybackBinding>,
        skipped: Vec<SkippedPlayback>,
    ) -> Result<SemanticLowered, SemanticLowerError> {
        let spec = self.builder.build();
        if spec.nodes().len() > budget.max_nodes {
            return Err(SemanticLowerError::BudgetExceeded {
                detail: "max_nodes",
            });
        }
        if spec.connections().len() > budget.max_connections {
            return Err(SemanticLowerError::BudgetExceeded {
                detail: "max_connections",
            });
        }
        let device_in = self
            .device
            .map(|(_, port)| port)
            .ok_or(SemanticLowerError::MasterMissing)?;
        Ok(SemanticLowered {
            spec,
            source_map: RoutingSourceMap {
                project_id: self.project.project_id,
                project_revision: key.project_revision,
                build_key: key.clone(),
                generation,
                semantic_nodes: self.nodes_map,
                ports: self.ports_map,
                connections: self.connections_map,
                send_taps: self.send_taps,
                playback_bindings: self.playback_bindings,
            },
            playbacks,
            node_roles: self.node_roles,
            device_in,
            skipped,
            inserts: self.output_inserts,
        })
    }

    /// Connect two runtime ports (no channel map).
    fn connect(
        &mut self,
        from: GraphPortRef,
        to: GraphPortRef,
        channel_map: Option<GraphChannelMap>,
    ) {
        self.builder.connect(&ConnDef {
            from,
            to,
            delay: EdgeDelay::Zero,
            channel_map,
        });
    }

    /// Emits one processor node whose ports mirror the semantic port
    /// list.
    fn emit_processor_node(
        &mut self,
        processor: &Processor,
    ) -> Result<(NodeId, Vec<PortId>), SemanticLowerError> {
        let mut defs = Vec::with_capacity(processor.port_ids.len());
        for port_id in &processor.port_ids {
            let port = self
                .routing
                .ports
                .get(port_id)
                .ok_or(SemanticLowerError::PortUnresolved { port: *port_id })?;
            check_port_domain(*port_id, port.domain, port.channels, self.engine_format)?;
            defs.push(audio_port(
                match port.direction {
                    PortDirection::Input => Direction::Input,
                    PortDirection::Output => Direction::Output,
                },
                match port.role {
                    PortRole::Main => GraphPortRole::Main,
                    PortRole::Sidechain => GraphPortRole::Sidechain,
                },
                self.engine_format,
                Multiplicity::unbounded(0),
            ));
        }
        Ok(self.builder.add_node(&NodeDef {
            kind: NodeKind::Processor,
            ports: defs,
        }))
    }

    /// Resolves a connection's semantic ends to runtime ports —
    /// source must be a strip output; destination is a strip main
    /// input or a processor sidechain input.
    fn connection_ends(
        &self,
        conn: &RoutingConnection,
    ) -> Result<ConnectionEnds, SemanticLowerError> {
        let src_sem = self
            .routing
            .ports
            .get(&conn.source)
            .ok_or(SemanticLowerError::PortUnresolved { port: conn.source })?;
        let dst_sem = self.routing.ports.get(&conn.destination).ok_or(
            SemanticLowerError::PortUnresolved {
                port: conn.destination,
            },
        )?;

        let PortOwner::Strip(src_strip_id) = src_sem.owner else {
            return Err(SemanticLowerError::MissingStrip {
                detail: format!("connection {} source is not a strip port", conn.id),
            });
        };
        if src_sem.direction != PortDirection::Output {
            return Err(SemanticLowerError::MissingStrip {
                detail: format!("connection {} source is not an output", conn.id),
            });
        }
        if !self.strip_wiring.contains_key(&src_strip_id) {
            return Err(SemanticLowerError::MissingStrip {
                detail: format!(
                    "connection {} source strip {src_strip_id} not lowered",
                    conn.id
                ),
            });
        }

        let (dst_ref, dst_strip) = match dst_sem.owner {
            PortOwner::Strip(dst_strip_id) => {
                if dst_sem.direction != PortDirection::Input {
                    return Err(SemanticLowerError::MissingStrip {
                        detail: format!("connection {} destination is not an input", conn.id),
                    });
                }
                let w = self.strip_wiring.get(&dst_strip_id).ok_or_else(|| {
                    SemanticLowerError::MissingStrip {
                        detail: format!("connection {} destination strip not lowered", conn.id),
                    }
                })?;
                (w.entry, dst_strip_id)
            }
            PortOwner::Processor(proc_id) => {
                let w = self.processor_wiring.get(&proc_id).ok_or_else(|| {
                    SemanticLowerError::MissingStrip {
                        detail: format!("connection {} destination processor not lowered", conn.id),
                    }
                })?;
                let Some(&port) = w.sidechain.get(&conn.destination) else {
                    return Err(SemanticLowerError::MissingStrip {
                        detail: format!(
                            "connection {} destination is not a sidechain port",
                            conn.id
                        ),
                    });
                };
                let owner_strip = self.routing.strip_of_processor(proc_id).ok_or_else(|| {
                    SemanticLowerError::MissingStrip {
                        detail: format!("processor {proc_id} has no owning strip"),
                    }
                })?;
                (GraphPortRef { node: w.node, port }, owner_strip)
            }
        };
        Ok(ConnectionEnds {
            src_strip: src_strip_id,
            dst_ref,
            dst_strip,
        })
    }

    /// Lowers one enabled routing connection to graph edge(s).
    ///
    /// Returns the [`MappingResolution`] recorded for the semantic
    /// connection — `Resolved` carries the `ConnectionId` of the edge
    /// that delivers into the destination port.
    fn lower_connection(
        &mut self,
        conn: &RoutingConnection,
    ) -> Result<MappingResolution<ConnectionId>, SemanticLowerError> {
        let ends = self.connection_ends(conn)?;

        // Audibility gate — the same authority the semantic oracle
        // uses.
        if !self.audible.contains(&ends.src_strip) || !self.audible.contains(&ends.dst_strip) {
            return Ok(MappingResolution::NotLowered(NotLoweredReason::MutedStrip));
        }

        // Source tap point: main connections leave post-fader; sends
        // leave pre- or post-fader per `SendConfig::tap`.
        let src_w = self.strip_wiring[&ends.src_strip];
        let mut tap = src_w.main_out;
        if let ConnectionKind::Send(send_id) = conn.kind {
            let send = self
                .routing
                .sends
                .get(&send_id)
                .ok_or(SemanticLowerError::DanglingGainTarget { send: send_id })?;
            tap = match send.tap {
                SendTap::PreFader => src_w.pre_tap,
                SendTap::PostFader => src_w.main_out,
            };
            self.send_taps.insert(send_id, tap);
        }

        let graph_map = conn.channel_map.as_ref().map(graph_channel_map);
        let resolved = if let ConnectionKind::Send(send_id) = conn.kind {
            self.lower_send_edge(send_id, tap, ends.dst_ref, graph_map)?
        } else {
            self.connect_id(tap, ends.dst_ref, graph_map)
        };
        Ok(MappingResolution::Resolved(resolved))
    }

    /// Send edge: a bound gain target inserts a deterministic gain
    /// stage (tap → gain → destination); no target means a direct
    /// edge.
    fn lower_send_edge(
        &mut self,
        send_id: SendId,
        tap: GraphPortRef,
        dst_ref: GraphPortRef,
        channel_map: Option<GraphChannelMap>,
    ) -> Result<ConnectionId, SemanticLowerError> {
        let send = self
            .routing
            .sends
            .get(&send_id)
            .ok_or(SemanticLowerError::DanglingGainTarget { send: send_id })?;
        let Some(gain) = send_gain(self.project, send)? else {
            return Ok(self.connect_id(tap, dst_ref, channel_map));
        };
        let (gain_node, gain_in, gain_out) =
            if let Some(&ports) = self.send_gain_nodes.get(&send_id) {
                ports
            } else {
                let ports = self.emit_send_gain(send_id, gain);
                self.send_gain_nodes.insert(send_id, ports);
                ports
            };
        self.connect(
            tap,
            GraphPortRef {
                node: gain_node,
                port: gain_in,
            },
            None,
        );
        // The gain stage is diagonal — the semantic channel map applies
        // at the destination boundary.
        Ok(self.connect_id(
            GraphPortRef {
                node: gain_node,
                port: gain_out,
            },
            dst_ref,
            channel_map,
        ))
    }

    /// Gain stage node for one send — role-registered so the compile
    /// factory installs [`GainProcessor`].
    fn emit_send_gain(&mut self, send_id: SendId, gain: f32) -> (NodeId, PortId, PortId) {
        let (node, ports) = self.builder.add_node(&NodeDef {
            kind: NodeKind::Processor,
            ports: vec![
                audio_port(
                    Direction::Input,
                    GraphPortRole::Main,
                    self.engine_format,
                    Multiplicity::unbounded(0),
                ),
                audio_port(
                    Direction::Output,
                    GraphPortRole::Main,
                    self.engine_format,
                    Multiplicity::unbounded(0),
                ),
            ],
        });
        self.node_roles.insert(
            node,
            SemanticNodeRole::SendGain {
                send: send_id,
                gain,
            },
        );
        (node, ports[0], ports[1])
    }

    /// `connect` returning the issued [`ConnectionId`].
    fn connect_id(
        &mut self,
        from: GraphPortRef,
        to: GraphPortRef,
        channel_map: Option<GraphChannelMap>,
    ) -> ConnectionId {
        self.builder.connect(&ConnDef {
            from,
            to,
            delay: EdgeDelay::Zero,
            channel_map,
        })
    }

    /// Emits one playback node and connects it to its owning strip's
    /// main-input resolution. The playback's `PlaybackBinding` is
    /// returned for the engine's clip-player wiring; the semantic-side
    /// record lands in `playback_bindings`.
    fn emit_playback(
        &mut self,
        plan: &PlaybackPlan,
        ordinal: usize,
    ) -> Result<PlaybackBinding, SemanticLowerError> {
        let lane_id = playback_lane_id(self.project, &plan.content).ok_or_else(|| {
            SemanticLowerError::MissingStrip {
                detail: format!("playback {:?} has no owning lane", plan.content),
            }
        })?;
        let lane =
            self.project
                .lanes
                .get(&lane_id)
                .ok_or_else(|| SemanticLowerError::MissingStrip {
                    detail: format!("lane {lane_id} missing"),
                })?;
        let strip = self.routing.strip_of_track(lane.track_id).ok_or_else(|| {
            SemanticLowerError::MissingStrip {
                detail: format!("track {} has no strip", lane.track_id),
            }
        })?;
        let strip_id = strip.id;
        let wiring =
            *self
                .strip_wiring
                .get(&strip_id)
                .ok_or_else(|| SemanticLowerError::MissingStrip {
                    detail: format!("strip {strip_id} not lowered"),
                })?;
        let destination = strip
            .port_ids
            .iter()
            .find(|id| {
                self.routing.ports.get(id).is_some_and(|p| {
                    p.direction == PortDirection::Input && p.role == PortRole::Main
                })
            })
            .copied()
            .ok_or_else(|| SemanticLowerError::MissingStrip {
                detail: format!("strip {strip_id} has no main input port"),
            })?;

        let src_channels = u16::try_from(plan.canonical.layout.channels()).unwrap_or(u16::MAX);
        let channel_map = (src_channels != self.engine_channels).then(|| {
            // Same convention as the legacy lowering: destination
            // channel `d` reads source channel `min(d, src-1)`.
            GraphChannelMap {
                entries: (0..self.engine_channels)
                    .map(|d| (d.min(src_channels.saturating_sub(1)), d))
                    .collect(),
            }
        });

        let (node_id, out_ports) = self.builder.add_node(&NodeDef {
            kind: NodeKind::Processor,
            ports: vec![PortDef {
                direction: Direction::Output,
                role: GraphPortRole::Main,
                format: PortFormat::Audio(plan.canonical),
                multiplicity: Multiplicity::unbounded(0),
                capabilities: Vec::new(),
                availability: Availability::Optional,
                activation: Activation::Enabled,
            }],
        });
        let out_port = out_ports[0];
        self.connect(
            GraphPortRef {
                node: node_id,
                port: out_port,
            },
            wiring.entry,
            channel_map,
        );
        self.node_roles
            .insert(node_id, SemanticNodeRole::Playback(ordinal));
        self.playback_bindings.push(RoutingPlaybackRef {
            content: playback_content_ref(plan.content),
            node_id,
            out_port,
            destination,
        });
        Ok(PlaybackBinding {
            content: plan.content,
            source_id: plan.source_id,
            node_id,
            out_port,
            window: plan.window,
            canonical: plan.canonical,
            fade: plan.fade,
        })
    }
}

/// `NodeContent` → the semantic-side [`PlaybackContentRef`] record.
fn playback_content_ref(content: NodeContent) -> PlaybackContentRef {
    match content {
        NodeContent::Clip(clip) => PlaybackContentRef::Clip(clip),
        NodeContent::CompSegment { comp_id, region_id } => {
            PlaybackContentRef::CompSegment { comp_id, region_id }
        }
        NodeContent::CompOverlap {
            comp_id,
            transition_id,
            side,
        } => PlaybackContentRef::CompOverlap {
            comp_id,
            transition_id,
            side: match side {
                crate::lowering::OverlapSide::Left => PlaybackOverlapSide::Left,
                crate::lowering::OverlapSide::Right => PlaybackOverlapSide::Right,
            },
        },
    }
}

/// Lowers `project`'s semantic routing into a typed graph spec plus
/// its [`RoutingSourceMap`].
///
/// `key`/`generation` stamp the source map so it can never be
/// mistaken for a different build's mapping. `budget` bounds the
/// emitted node/connection counts.
///
/// # Errors
/// [`SemanticLowerError`] — deterministic, never a partial spec.
pub fn lower_semantic(
    project: &Project,
    canonical_of: &dyn Fn(SourceId) -> Option<AudioFormat>,
    engine: &EngineConfig,
    key: &RoutingBuildKey,
    generation: u64,
    budget: &BuildBudget,
) -> Result<SemanticLowered, SemanticLowerError> {
    lower_semantic_inner(
        project,
        canonical_of,
        engine,
        key,
        generation,
        budget,
        SemanticOutput::default(),
    )
}

/// [`lower_semantic`] plus an explicit [`SemanticOutput`] stage —
/// the offline-render entry point (`BuildMode::Offline` builds).
/// The semantic pipeline is identical; `output` only adds insert
/// slots and/or a sink-format tap at the output boundary.
///
/// # Errors
/// [`SemanticLowerError`] — deterministic, never a partial spec.
pub fn lower_semantic_offline(
    project: &Project,
    canonical_of: &dyn Fn(SourceId) -> Option<AudioFormat>,
    engine: &EngineConfig,
    key: &RoutingBuildKey,
    generation: u64,
    budget: &BuildBudget,
    output: SemanticOutput,
) -> Result<SemanticLowered, SemanticLowerError> {
    lower_semantic_inner(
        project,
        canonical_of,
        engine,
        key,
        generation,
        budget,
        output,
    )
}

fn lower_semantic_inner(
    project: &Project,
    canonical_of: &dyn Fn(SourceId) -> Option<AudioFormat>,
    engine: &EngineConfig,
    key: &RoutingBuildKey,
    generation: u64,
    budget: &BuildBudget,
    output: SemanticOutput,
) -> Result<SemanticLowered, SemanticLowerError> {
    if let Some(format) = output.format {
        let engine_format = engine_format(engine);
        if format.sample_rate != engine_format.sample_rate
            || format.sample_format != engine_format.sample_format
        {
            return Err(SemanticLowerError::OutputFormat {
                detail: "sink format must share the engine sample rate and sample format",
            });
        }
    }
    let mut lower = Lower::new(project, engine, output);

    lower.emit_device_sink();
    lower.emit_strips()?;
    lower.map_bus_nodes();
    lower.lower_connections()?;
    lower.mark_orphan_processors();
    lower.emit_output_inserts(output.insert_count);
    lower.connect_master_sink()?;
    let (playbacks, skipped) = lower.emit_playbacks(canonical_of, engine)?;
    lower.finish(key, generation, budget, playbacks, skipped)
}

/// `SemanticPort::channels`/domain must agree with the engine format —
/// anything else is an explicit failure, never an implicit map.
fn check_port_domain(
    port: SemanticPortId,
    domain: SignalDomain,
    channels: u32,
    engine_format: AudioFormat,
) -> Result<(), SemanticLowerError> {
    if domain != SignalDomain::Audio {
        return Err(SemanticLowerError::UnsupportedDomain { port });
    }
    if channels != engine_format.layout.channels() {
        return Err(SemanticLowerError::ChannelMismatch { port });
    }
    Ok(())
}

/// Engine `EngineConfig` → `AudioFormat` (mono for 1 channel, stereo
/// otherwise — the same convention the legacy lowering uses).
///
/// # Panics
///
/// Panics if `engine.sample_rate` is zero (`SampleRate` rejects it).
#[must_use]
pub fn engine_format(engine: &EngineConfig) -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(engine.sample_rate).expect("engine rate must be non-zero"),
        layout: if engine.channels == 1 {
            ChannelLayout::Mono
        } else {
            ChannelLayout::Stereo
        },
        sample_format: SampleFormat::F32,
    }
}

fn audio_port(
    direction: Direction,
    role: GraphPortRole,
    format: AudioFormat,
    multiplicity: Multiplicity,
) -> PortDef {
    PortDef {
        direction,
        role,
        format: PortFormat::Audio(format),
        multiplicity,
        capabilities: Vec::new(),
        availability: Availability::Optional,
        activation: Activation::Enabled,
    }
}

/// Plain-domain value of a control target — `base` denormalized
/// through its declared mapping and capability range (internal
/// targets default to the `[0, 1]` plain domain).
fn target_plain(target: &synvelis_project::ParameterTarget) -> f64 {
    let (min, max) = target
        .capability
        .map_or((0.0, 1.0), |c| (c.min_value, c.max_value));
    target.mapping.denormalize(target.base, min, max)
}

/// Strip fader/pan gains — the plain domain is treated as a linear
/// gain clamped to `[0, 1]` (the bootstrap convention: master fader
/// `base` 1.0 = unity). A strip with no `Pan` target is unity on
/// both channels — the common case must stay bit-exact with the
/// legacy clip→master path. When a `Pan` target exists its `base`
/// ∈ [0,1] maps through a constant-power law (0 → hard left,
/// 0.5 → equal-power center, 1 → hard right). Missing fader target
/// means unity — the bootstrap model only seeds the master fader.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)] // bounded [0,1] plain → f32 gain
fn strip_gains(project: &Project, strip: StripId, engine_channels: u16) -> [f32; 2] {
    let fader = project
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Strip(strip) && t.role == ParameterRole::Fader)
        .map_or(1.0, target_plain);
    let gain = fader.clamp(0.0, 1.0) as f32;
    if engine_channels <= 1 {
        return [gain, gain];
    }
    let Some(pan_target) = project
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Strip(strip) && t.role == ParameterRole::Pan)
    else {
        return [gain, gain];
    };
    let pan = target_plain(pan_target);
    // Constant-power pan: base 0 → hard left, 0.5 → equal-power
    // center, 1 → hard right.
    let angle = (pan.clamp(0.0, 1.0) * std::f64::consts::FRAC_PI_2) as f32;
    [gain * angle.cos(), gain * angle.sin()]
}

/// Send level gain — the `SendLevel` target's plain domain is the
/// linear send gain (1.0 = unity per `SendGainSpec`). `None` means no
/// gain stage (direct edge).
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)] // plain-domain gain → f32
fn send_gain(project: &Project, send: &SendConfig) -> Result<Option<f32>, SemanticLowerError> {
    let Some(target_id) = send.gain_target else {
        return Ok(None);
    };
    let target = project
        .control
        .targets
        .get(&target_id)
        .ok_or(SemanticLowerError::DanglingGainTarget { send: send.id })?;
    Ok(Some(target_plain(target).max(0.0) as f32))
}

/// `ExplicitChannelMap` (destination→source indices) → graph
/// `(src, dst)` pairs — the same conversion `routing_tx`'s generic
/// projection performs.
fn graph_channel_map(map: &ExplicitChannelMap) -> GraphChannelMap {
    GraphChannelMap {
        entries: map
            .entries
            .iter()
            .enumerate()
            .map(|(d, s)| {
                (
                    u16::try_from(*s).unwrap_or(u16::MAX),
                    u16::try_from(d).unwrap_or(u16::MAX),
                )
            })
            .collect(),
    }
}
