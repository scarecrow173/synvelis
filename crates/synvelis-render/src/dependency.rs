//! Typed semantic dependency closure + recipe resolver
//! (synvelis#290, RCP-01).
//!
//! [`resolve_recipe`] walks the *resolved* project view
//! ([`synvelis_project::render_input::resolve_render_project`] output)
//! backwards from the render boundary and records every semantic
//! entity that can affect the output as a typed
//! [`DependencyNode`]/[`DependencyEdge`] pair. Edge kinds follow the
//! RCP-00 vocabulary: `Audio`, `Event`, `Control`, `Parameter`,
//! `Sidechain`, `ExternalMedia`, `Transport`, `TimeMapping`.
//!
//! Fail-closed rules (RCP-REC-02): unresolved port/plugin bindings,
//! missing plugin-state evidence or degraded capture quality,
//! unpinned/weak external assets, unverified sources, live clocks and
//! unsupported scopes all surface as [`UnknownDependency`]s — the
//! resolver returns [`RecipeResolution::Unknown`], never a partial
//! "best effort" recipe. Cycle rejection reuses the routing model's
//! own zero-delay acyclicity check ([`RoutingModel::validate`]).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use synvelis_project::diff::{ObjectKind, SemanticObjectRef};
use synvelis_project::id::{
    AssetId, BusId, ChainId, ClipId, CompId, CompRegionId, LaneId, ModulationSourceId,
    ParameterTargetId, ProcessorId, SemanticPortId, SendId, SourceId, StripId, TakeId, TrackId,
};
use synvelis_project::plugin_state::StateCaptureQuality;
use synvelis_project::project::AssetDigest;
use synvelis_project::render_input::canonical_entity_digest;
use synvelis_project::routing::{ConnectionKind, ProcessorBinding, StripOwner};
use synvelis_project::snapshot::ContentDigest;
use synvelis_project::{
    AutomationTimeScope, CompCandidateRef, ControlOwner, PortBinding, PortDirection, PortOwner,
    Project, SemanticPort, SourceState,
};

use crate::recipe::{
    AUDIO_SEMANTICS_VERSION, CANONICALIZATION_VERSION, ClockDependency, OutputContract,
    RECIPE_DOMAIN_TAG, RECIPE_HASH_ALGORITHM, RECIPE_SCHEMA_VERSION, RecipeBranchPin,
    RecipeResolution, RecipeTarget, RenderBoundary, RenderRecipeV1, RendererProfile,
    ResolutionProvenance, TemporalDomain, UnknownCause, UnknownDependency, UnsupportedReason,
};

// ---------------------------------------------------------------------------
// typed closure
// ---------------------------------------------------------------------------

/// Semantic edge kind — the RCP-00 dependency vocabulary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyEdgeKind {
    /// Audio-rate signal path (ports, chains, processors, buses).
    Audio,
    /// Event/MIDI/content path (clips, notes, sources, takes).
    Event,
    /// Control modulation path (modulation sources/assignments).
    Control,
    /// Parameter binding path (targets, automation lanes).
    Parameter,
    /// Sidechain/key input path.
    Sidechain,
    /// External media content dependency (asset bytes).
    ExternalMedia,
    /// Transport/clock dependency.
    Transport,
    /// Musical-time mapping dependency (tempo map, PPQN, placement).
    TimeMapping,
}

/// One node of the typed dependency closure: a semantic object plus
/// its canonical content digest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyNode {
    /// Semantic identity (`kind` + tagged stable ID).
    pub object: SemanticObjectRef,
    /// SHA-256 of the canonical, name-free entity value.
    pub digest: ContentDigest,
}

/// One typed edge `from → to`: `from` is upstream — a change in `from`
/// can invalidate `to`. `detail` carries edge-specific canonical data
/// (connection kind, channel map, send tap, chain index, …) so edge
/// semantics are part of recipe identity, not just endpoints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyEdge {
    /// Upstream (depended-upon) object.
    pub from: SemanticObjectRef,
    /// Downstream (dependent) object.
    pub to: SemanticObjectRef,
    /// Semantic edge kind.
    pub kind: DependencyEdgeKind,
    /// Canonical edge payload (sorted-key JSON).
    pub detail: Value,
}

/// Plugin-state evidence required for Exact resolution of a `Resolved`
/// processor — supplied by the host/plugin-state port, never inferred.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginStateEvidence {
    /// Digest of the captured plugin-state bytes.
    pub digest: ContentDigest,
    /// Observed capture quality — must be `exact_eligible()`.
    pub quality: StateCaptureQuality,
}

/// External asset pin status supplied by the asset/verification port.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "pin", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ExternalPin {
    /// Bytes verified against `digest` — the asset is pinned.
    Pinned {
        /// Verified content digest.
        digest: ContentDigest,
    },
    /// The asset is known but not pinned to verified content.
    Unpinned,
    /// The asset is a live device/stream — cannot be pinned.
    Live,
}

/// Port the resolver queries for per-processor plugin-state evidence.
pub trait PluginStateProbe {
    /// Evidence for `processor`, or `None` when unknown.
    fn evidence(&self, processor: ProcessorId) -> Option<PluginStateEvidence>;
}

/// Port the resolver queries for external-asset pin status.
pub trait ExternalAssetProbe {
    /// Pin status for `asset`.
    fn pin(&self, asset: AssetId) -> ExternalPin;
}

/// No plugin-state evidence — every `Resolved` processor resolves
/// `Unknown` (fail closed).
#[derive(Clone, Debug, Default)]
pub struct NoPluginStates;

impl PluginStateProbe for NoPluginStates {
    fn evidence(&self, _processor: ProcessorId) -> Option<PluginStateEvidence> {
        None
    }
}

/// Static plugin-state evidence table (fixtures/tests).
#[derive(Clone, Debug, Default)]
pub struct StaticPluginStates(pub BTreeMap<ProcessorId, PluginStateEvidence>);

impl PluginStateProbe for StaticPluginStates {
    fn evidence(&self, processor: ProcessorId) -> Option<PluginStateEvidence> {
        self.0.get(&processor).cloned()
    }
}

/// No external pins — weak digests resolve `Unknown` (fail closed).
#[derive(Clone, Debug, Default)]
pub struct NoExternalAssets;

impl ExternalAssetProbe for NoExternalAssets {
    fn pin(&self, _asset: AssetId) -> ExternalPin {
        ExternalPin::Unpinned
    }
}

/// Static external-asset pin table (fixtures/tests).
#[derive(Clone, Debug, Default)]
pub struct StaticExternalAssets(pub BTreeMap<AssetId, ExternalPin>);

impl ExternalAssetProbe for StaticExternalAssets {
    fn pin(&self, asset: AssetId) -> ExternalPin {
        self.0.get(&asset).cloned().unwrap_or(ExternalPin::Unpinned)
    }
}

/// External evidence ports the resolver consults.
pub struct RecipeEnvironment<'a> {
    /// Plugin-state evidence port.
    pub plugin_states: &'a dyn PluginStateProbe,
    /// External-asset pin port.
    pub external_assets: &'a dyn ExternalAssetProbe,
}

/// The typed dependency closure plus the external evidence it
/// resolved against.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyClosure {
    /// Closure nodes sorted by `(project_id, kind, stable_id)`.
    pub nodes: Vec<DependencyNode>,
    /// Closure edges sorted by `(from, to, kind, detail)`.
    pub edges: Vec<DependencyEdge>,
    /// Plugin-state evidence consumed (processor → digest+quality).
    pub plugin_states: BTreeMap<ProcessorId, PluginStateEvidence>,
    /// Verified external pins consumed (asset → digest).
    pub external_pins: BTreeMap<AssetId, ContentDigest>,
}

// ---------------------------------------------------------------------------
// resolver
// ---------------------------------------------------------------------------

/// One recipe-resolution request.
pub struct RecipeRequest<'a> {
    /// The resolved project view (output of
    /// [`synvelis_project::render_input::resolve_render_project`]).
    pub project: &'a Project,
    /// Render boundary.
    pub boundary: RenderBoundary,
    /// Temporal domain.
    pub temporal: TemporalDomain,
    /// Renderer profile.
    pub renderer: RendererProfile,
    /// Output contract.
    pub output: OutputContract,
    /// External evidence ports.
    pub environment: &'a RecipeEnvironment<'a>,
    /// Branch pin recorded inside the recipe target (non-hashed
    /// provenance fields stay on [`ResolutionProvenance`]).
    pub branch: Option<RecipeBranchPin>,
    /// Provenance recorded on the resolution (non-hashed).
    pub provenance: ResolutionProvenance,
    /// Project schema version of the resolved view.
    pub project_schema_version: u32,
    /// Render-input adapter schema version.
    pub render_input_schema_version: u32,
}

struct Resolver<'a> {
    project: &'a Project,
    env: &'a RecipeEnvironment<'a>,
    nodes: BTreeSet<SemanticObjectRef>,
    /// `(from, to, kind, detail-json)` dedup key — iteration order is
    /// the canonical edge order.
    edges: BTreeSet<(
        SemanticObjectRef,
        SemanticObjectRef,
        DependencyEdgeKind,
        String,
    )>,
    edge_details: BTreeMap<
        (
            SemanticObjectRef,
            SemanticObjectRef,
            DependencyEdgeKind,
            String,
        ),
        Value,
    >,
    unknowns: BTreeSet<UnknownDependency>,
    plugin_states: BTreeMap<ProcessorId, PluginStateEvidence>,
    external_pins: BTreeMap<AssetId, ContentDigest>,
    seen: BTreeSet<(ObjectKind, String)>,
    visited_targets: BTreeSet<ParameterTargetId>,
}

impl<'a> Resolver<'a> {
    fn new(project: &'a Project, env: &'a RecipeEnvironment<'a>) -> Self {
        Self {
            project,
            env,
            nodes: BTreeSet::new(),
            edges: BTreeSet::new(),
            edge_details: BTreeMap::new(),
            unknowns: BTreeSet::new(),
            plugin_states: BTreeMap::new(),
            external_pins: BTreeMap::new(),
            seen: BTreeSet::new(),
            visited_targets: BTreeSet::new(),
        }
    }

    fn object(&self, kind: ObjectKind, stable_id: String) -> SemanticObjectRef {
        SemanticObjectRef {
            project_id: self.project.project_id,
            kind,
            stable_id,
            lineage: None,
        }
    }

    /// Registers a node and returns its ref.
    fn node(&mut self, kind: ObjectKind, stable_id: String) -> SemanticObjectRef {
        let r = self.object(kind, stable_id);
        self.nodes.insert(r.clone());
        r
    }

    /// `true` the first time `(kind, stable_id)` is visited — guards
    /// every `visit_*` against diamond re-traversal.
    fn first_visit(&mut self, kind: ObjectKind, stable_id: &str) -> bool {
        self.seen.insert((kind, stable_id.to_owned()))
    }

    /// Records `from → to`. Both endpoints are auto-registered as
    /// nodes so every edge endpoint is a closure node by construction.
    fn edge(
        &mut self,
        from: &SemanticObjectRef,
        to: &SemanticObjectRef,
        kind: DependencyEdgeKind,
        detail: Value,
    ) {
        self.nodes.insert(from.clone());
        self.nodes.insert(to.clone());
        let detail_json = serde_json::to_string(&detail).unwrap_or_default();
        let key = (from.clone(), to.clone(), kind, detail_json);
        self.edges.insert(key.clone());
        self.edge_details.insert(key, detail);
    }

    fn unknown(&mut self, cause: UnknownCause, locator: String) {
        self.unknowns.insert(UnknownDependency { cause, locator });
    }

    // -- entity lookups -------------------------------------------------

    fn port(&self, id: SemanticPortId) -> Option<&'a SemanticPort> {
        self.project.routing.ports.get(&id)
    }

    // -- traversal --------------------------------------------------------

    /// Explains what feeds `port`: node + owner edge + incoming
    /// enabled connections (recursively upstream).
    fn visit_port(&mut self, port_id: SemanticPortId) {
        if !self.first_visit(ObjectKind::SemanticPort, &port_id.to_tagged()) {
            return;
        }
        let Some(port) = self.port(port_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: port_id.to_tagged(),
                },
                format!("port {}", port_id.to_tagged()),
            );
            return;
        };
        let port_ref = self.node(ObjectKind::SemanticPort, port_id.to_tagged());

        match &port.binding {
            PortBinding::Unresolved { plugin, advertised } => {
                self.unknown(
                    UnknownCause::UnresolvedPort {
                        locator: port_id.to_tagged(),
                    },
                    format!(
                        "port {} plugin {plugin} advertised {advertised}",
                        port_id.to_tagged()
                    ),
                );
            }
            PortBinding::Internal | PortBinding::PluginDeclared { .. } => {}
        }

        // Ownership edge: output ports are produced by their owner,
        // input ports feed their owner.
        let owner_ref = match port.owner {
            PortOwner::Strip(s) => self.node(ObjectKind::Strip, s.to_tagged()),
            PortOwner::Processor(p) => self.node(ObjectKind::Processor, p.to_tagged()),
        };
        match port.direction {
            PortDirection::Output => self.edge(
                &owner_ref,
                &port_ref,
                DependencyEdgeKind::Audio,
                json!({"role": port.role}),
            ),
            PortDirection::Input => self.edge(
                &port_ref,
                &owner_ref,
                DependencyEdgeKind::Audio,
                json!({"role": port.role}),
            ),
        }
        match port.owner {
            PortOwner::Strip(s) => self.visit_strip(s),
            PortOwner::Processor(p) => self.visit_processor(p),
        }

        // Incoming enabled connections into this port.
        for conn in self.project.routing.connections.values() {
            if !conn.enabled || conn.destination != port_id {
                continue;
            }
            let kind = match conn.kind {
                ConnectionKind::Main | ConnectionKind::Send(_) => DependencyEdgeKind::Audio,
                ConnectionKind::Sidechain => DependencyEdgeKind::Sidechain,
            };
            let detail = json!({
                "connection": conn.id.to_tagged(),
                "conn_kind": conn.kind,
                "channel_map": conn.channel_map,
            });
            let src_ref = self.node(ObjectKind::SemanticPort, conn.source.to_tagged());
            self.edge(&src_ref, &port_ref, kind, detail);
            if let ConnectionKind::Send(send_id) = conn.kind {
                self.visit_send(send_id);
            }
            self.visit_port(conn.source);
        }
    }

    fn visit_strip(&mut self, strip_id: StripId) {
        if !self.first_visit(ObjectKind::Strip, &strip_id.to_tagged()) {
            return;
        }
        let strip_ref = self.node(ObjectKind::Strip, strip_id.to_tagged());
        let Some(strip) = self.project.routing.strips.get(&strip_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: strip_id.to_tagged(),
                },
                format!("strip {}", strip_id.to_tagged()),
            );
            return;
        };
        match strip.owner {
            StripOwner::Track(t) => {
                let tref = self.node(ObjectKind::Track, t.to_tagged());
                self.edge(&tref, &strip_ref, DependencyEdgeKind::Audio, json!({}));
                self.visit_track(t);
            }
            StripOwner::Bus(b) => {
                let bref = self.node(ObjectKind::Bus, b.to_tagged());
                self.edge(&bref, &strip_ref, DependencyEdgeKind::Audio, json!({}));
                self.visit_bus(b);
            }
        }
        for port_id in &strip.port_ids {
            let pref = self.node(ObjectKind::SemanticPort, port_id.to_tagged());
            self.edge(&strip_ref, &pref, DependencyEdgeKind::Audio, json!({}));
            self.visit_port(*port_id);
        }
        self.visit_chain(strip.chain, &strip_ref);
        self.visit_control_owner(ControlOwner::Strip(strip_id));
        // Send configs tapping this strip's ports.
        for send in self.project.routing.sends.values() {
            if let Some(conn) = self.project.routing.connections.get(&send.connection)
                && let Some(port) = self.project.routing.ports.get(&conn.source)
                && matches!(port.owner, PortOwner::Strip(s) if s == strip_id)
            {
                self.visit_send(send.id);
            }
        }
    }

    fn visit_bus(&mut self, bus_id: BusId) {
        if !self.first_visit(ObjectKind::Bus, &bus_id.to_tagged()) {
            return;
        }
        let bus_ref = self.node(ObjectKind::Bus, bus_id.to_tagged());
        let Some(bus) = self.project.routing.buses.get(&bus_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: bus_id.to_tagged(),
                },
                format!("bus {}", bus_id.to_tagged()),
            );
            return;
        };
        if self.project.routing.master == bus_id {
            let rref = self.node(
                ObjectKind::RoutingModel,
                self.project.project_id.to_tagged(),
            );
            self.edge(&rref, &bus_ref, DependencyEdgeKind::Transport, json!({}));
        }
        let sref = self.node(ObjectKind::Strip, bus.strip.to_tagged());
        self.edge(
            &bus_ref,
            &sref,
            DependencyEdgeKind::Audio,
            json!({"bus_kind": bus.kind}),
        );
    }

    fn visit_chain(&mut self, chain_id: ChainId, strip_ref: &SemanticObjectRef) {
        if !self.first_visit(ObjectKind::ProcessorChain, &chain_id.to_tagged()) {
            return;
        }
        let chain_ref = self.node(ObjectKind::ProcessorChain, chain_id.to_tagged());
        self.edge(strip_ref, &chain_ref, DependencyEdgeKind::Audio, json!({}));
        let Some(chain) = self.project.routing.chains.get(&chain_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: chain_id.to_tagged(),
                },
                format!("chain {}", chain_id.to_tagged()),
            );
            return;
        };
        let mut prev: Option<SemanticObjectRef> = None;
        for (index, proc_id) in chain.processors.iter().enumerate() {
            let proc_ref = self.node(ObjectKind::Processor, proc_id.to_tagged());
            self.edge(
                &chain_ref,
                &proc_ref,
                DependencyEdgeKind::Audio,
                json!({"index": index}),
            );
            if let Some(p) = &prev {
                self.edge(
                    p,
                    &proc_ref,
                    DependencyEdgeKind::Audio,
                    json!({"order": index}),
                );
            }
            prev = Some(proc_ref);
            self.visit_processor(*proc_id);
        }
    }

    fn visit_processor(&mut self, proc_id: ProcessorId) {
        if !self.first_visit(ObjectKind::Processor, &proc_id.to_tagged()) {
            return;
        }
        let proc_ref = self.node(ObjectKind::Processor, proc_id.to_tagged());
        let Some(processor) = self.project.routing.processors.get(&proc_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: proc_id.to_tagged(),
                },
                format!("processor {}", proc_id.to_tagged()),
            );
            return;
        };
        match &processor.binding {
            ProcessorBinding::Unresolved { descriptor } => {
                self.unknown(
                    UnknownCause::UnresolvedProcessor {
                        locator: proc_id.to_tagged(),
                    },
                    format!("processor {} descriptor {descriptor}", proc_id.to_tagged()),
                );
            }
            ProcessorBinding::Resolved(plugin) => {
                match self.env.plugin_states.evidence(proc_id) {
                    None => {
                        self.unknown(
                            UnknownCause::PluginStateUnknown {
                                locator: proc_id.to_tagged(),
                            },
                            format!(
                                "processor {} plugin {}",
                                proc_id.to_tagged(),
                                plugin.descriptor
                            ),
                        );
                    }
                    Some(ev) => {
                        if ev.quality.exact_eligible() {
                            self.plugin_states.insert(proc_id, ev);
                        } else {
                            self.unknown(
                                UnknownCause::PluginStateDegraded {
                                    locator: proc_id.to_tagged(),
                                    quality: format!("{:?}", ev.quality),
                                },
                                format!("processor {}", proc_id.to_tagged()),
                            );
                        }
                    }
                }
                if self
                    .project
                    .plugin_instances
                    .instances
                    .contains_key(&proc_id)
                {
                    let iref = self.node(ObjectKind::PluginInstance, proc_id.to_tagged());
                    self.edge(&proc_ref, &iref, DependencyEdgeKind::Audio, json!({}));
                }
            }
        }
        for port_id in &processor.port_ids {
            let pref = self.node(ObjectKind::SemanticPort, port_id.to_tagged());
            self.edge(&proc_ref, &pref, DependencyEdgeKind::Audio, json!({}));
            self.visit_port(*port_id);
        }
        self.visit_control_owner(ControlOwner::Processor(proc_id));
    }

    fn visit_send(&mut self, send_id: SendId) {
        if !self.first_visit(ObjectKind::SendConfig, &send_id.to_tagged()) {
            return;
        }
        let send_ref = self.node(ObjectKind::SendConfig, send_id.to_tagged());
        let Some(send) = self.project.routing.sends.get(&send_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: send_id.to_tagged(),
                },
                format!("send {}", send_id.to_tagged()),
            );
            return;
        };
        if let Some(target) = send.gain_target {
            let tref = self.node(ObjectKind::ParameterTarget, target.to_tagged());
            self.edge(
                &tref,
                &send_ref,
                DependencyEdgeKind::Parameter,
                json!({"role": "send-gain"}),
            );
            self.visit_parameter_target(target);
        }
        self.visit_control_owner(ControlOwner::Send(send_id));
        if let Some(conn) = self.project.routing.connections.get(&send.connection)
            && self.project.routing.ports.contains_key(&conn.destination)
        {
            let dref = self.node(ObjectKind::SemanticPort, conn.destination.to_tagged());
            self.edge(
                &send_ref,
                &dref,
                DependencyEdgeKind::Audio,
                json!({"tap": send.tap, "send": send_id.to_tagged()}),
            );
        }
    }

    fn visit_track(&mut self, track_id: TrackId) {
        if !self.first_visit(ObjectKind::Track, &track_id.to_tagged()) {
            return;
        }
        let track_ref = self.node(ObjectKind::Track, track_id.to_tagged());
        let Some(track) = self.project.tracks.get(&track_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: track_id.to_tagged(),
                },
                format!("track {}", track_id.to_tagged()),
            );
            return;
        };
        for lane_id in &track.lane_ids {
            let lref = self.node(ObjectKind::Lane, lane_id.to_tagged());
            self.edge(&lref, &track_ref, DependencyEdgeKind::Event, json!({}));
            self.visit_lane(*lane_id);
        }
        self.visit_control_owner(ControlOwner::Track(track_id));
    }

    fn visit_lane(&mut self, lane_id: LaneId) {
        if !self.first_visit(ObjectKind::Lane, &lane_id.to_tagged()) {
            return;
        }
        let lane_ref = self.node(ObjectKind::Lane, lane_id.to_tagged());
        let Some(_lane) = self.project.lanes.get(&lane_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: lane_id.to_tagged(),
                },
                format!("lane {}", lane_id.to_tagged()),
            );
            return;
        };
        for clip_id in &self.project.lanes[&lane_id].clip_ids {
            let cref = self.node(ObjectKind::Clip, clip_id.to_tagged());
            self.edge(&cref, &lane_ref, DependencyEdgeKind::Event, json!({}));
            self.visit_clip(*clip_id);
        }
        // Comping: take groups owned by this lane feed the lane's
        // playback when a comp is active.
        for group in self
            .project
            .comp
            .take_groups
            .values()
            .filter(|g| g.owner_lane_id == lane_id)
        {
            let gref = self.node(ObjectKind::TakeGroup, group.id.to_tagged());
            self.edge(&gref, &lane_ref, DependencyEdgeKind::Event, json!({}));
            if let Some(comp_id) = group.active_comp_id {
                self.visit_comp(comp_id);
            }
            for take_id in &group.take_ids {
                let tref = self.node(ObjectKind::Take, take_id.to_tagged());
                self.edge(&tref, &gref, DependencyEdgeKind::Event, json!({}));
                self.visit_take(*take_id);
            }
        }
    }

    fn visit_clip(&mut self, clip_id: ClipId) {
        if !self.first_visit(ObjectKind::Clip, &clip_id.to_tagged()) {
            return;
        }
        let clip_ref = self.node(ObjectKind::Clip, clip_id.to_tagged());
        let Some(clip) = self.project.clips.get(&clip_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: clip_id.to_tagged(),
                },
                format!("clip {}", clip_id.to_tagged()),
            );
            return;
        };
        if self.project.sources.contains_key(&clip.source_id) {
            let sref = self.node(ObjectKind::MediaSource, clip.source_id.to_tagged());
            self.edge(&sref, &clip_ref, DependencyEdgeKind::Event, json!({}));
            self.visit_media_source(clip.source_id);
        } else if self.project.midi_sources.contains_key(&clip.source_id) {
            let mref = self.node(ObjectKind::MidiSource, clip.source_id.to_tagged());
            self.edge(&mref, &clip_ref, DependencyEdgeKind::Event, json!({}));
            self.visit_midi_source(clip.source_id);
        } else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: clip.source_id.to_tagged(),
                },
                format!(
                    "clip {} source {}",
                    clip_id.to_tagged(),
                    clip.source_id.to_tagged()
                ),
            );
        }
        for note in self.project.notes.values().filter(|n| n.clip == clip_id) {
            let nref = self.node(ObjectKind::Note, note.id.to_tagged());
            self.edge(&nref, &clip_ref, DependencyEdgeKind::Event, json!({}));
            self.visit_control_owner(ControlOwner::Note(note.id));
        }
        self.visit_control_owner(ControlOwner::Clip(clip_id));
    }

    fn visit_media_source(&mut self, source_id: SourceId) {
        if !self.first_visit(ObjectKind::MediaSource, &source_id.to_tagged()) {
            return;
        }
        let sref = self.node(ObjectKind::MediaSource, source_id.to_tagged());
        let Some(source) = self.project.sources.get(&source_id) else {
            return;
        };
        if source.state == SourceState::Imported {
            self.unknown(
                UnknownCause::UnverifiedSource {
                    locator: source_id.to_tagged(),
                },
                format!("source {}", source_id.to_tagged()),
            );
        }
        let aref = self.node(ObjectKind::Asset, source.asset_id.to_tagged());
        self.edge(&aref, &sref, DependencyEdgeKind::ExternalMedia, json!({}));
        self.visit_asset(source.asset_id);
    }

    fn visit_asset(&mut self, asset_id: AssetId) {
        if !self.first_visit(ObjectKind::Asset, &asset_id.to_tagged()) {
            return;
        }
        self.node(ObjectKind::Asset, asset_id.to_tagged());
        let Some(asset) = self.project.assets.get(&asset_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: asset_id.to_tagged(),
                },
                format!("asset {}", asset_id.to_tagged()),
            );
            return;
        };
        let digest = AssetDigest::parse(&asset.digest);
        if digest.is_content_addressed() {
            // `sha256:` digests are content addresses — identity is
            // already exact; byte verification is the artifact store's
            // job (RCP-02).
            return;
        }
        match self.env.external_assets.pin(asset_id) {
            ExternalPin::Pinned { digest } => {
                self.external_pins.insert(asset_id, digest);
            }
            ExternalPin::Unpinned => {
                self.unknown(
                    UnknownCause::UnpinnedExternalAsset {
                        locator: asset_id.to_tagged(),
                    },
                    format!("asset {}", asset_id.to_tagged()),
                );
            }
            ExternalPin::Live => {
                self.unknown(
                    UnknownCause::OpaqueDependency {
                        locator: asset_id.to_tagged(),
                    },
                    format!("asset {} live", asset_id.to_tagged()),
                );
            }
        }
    }

    fn visit_midi_source(&mut self, source_id: SourceId) {
        if !self.first_visit(ObjectKind::MidiSource, &source_id.to_tagged()) {
            return;
        }
        let mref = self.node(ObjectKind::MidiSource, source_id.to_tagged());
        let Some(source) = self.project.midi_sources.get(&source_id) else {
            return;
        };
        for note in source.notes.values() {
            let nref = self.node(ObjectKind::MidiNote, note.id.to_tagged());
            self.edge(&nref, &mref, DependencyEdgeKind::Event, json!({}));
        }
        if self.project.smf_documents.contains_key(&source_id) {
            let dref = self.node(ObjectKind::SmfDocument, source_id.to_tagged());
            self.edge(&dref, &mref, DependencyEdgeKind::Event, json!({}));
        }
        // MIDI content is musical-time — tempo mapping applies.
        let pref = self.node(ObjectKind::Project, self.project.project_id.to_tagged());
        self.edge(&pref, &mref, DependencyEdgeKind::TimeMapping, json!({}));
    }

    fn visit_comp(&mut self, comp_id: CompId) {
        if !self.first_visit(ObjectKind::Comp, &comp_id.to_tagged()) {
            return;
        }
        let cref = self.node(ObjectKind::Comp, comp_id.to_tagged());
        let Some(comp) = self.project.comp.comps.get(&comp_id) else {
            return;
        };
        for region_id in &comp.region_ids {
            let rref = self.node(ObjectKind::CompRegion, region_id.to_tagged());
            self.edge(&rref, &cref, DependencyEdgeKind::Event, json!({}));
            self.visit_comp_region(*region_id);
        }
        for transition_id in &comp.transition_ids {
            let tref = self.node(ObjectKind::CompTransition, transition_id.to_tagged());
            self.edge(&tref, &cref, DependencyEdgeKind::Event, json!({}));
        }
    }

    fn visit_comp_region(&mut self, region_id: CompRegionId) {
        if !self.first_visit(ObjectKind::CompRegion, &region_id.to_tagged()) {
            return;
        }
        let rref = self.node(ObjectKind::CompRegion, region_id.to_tagged());
        let Some(region) = self.project.comp.regions.get(&region_id) else {
            return;
        };
        match &region.candidate {
            CompCandidateRef::Take { take_id, .. } => {
                let tref = self.node(ObjectKind::Take, take_id.to_tagged());
                self.edge(&tref, &rref, DependencyEdgeKind::Event, json!({}));
                self.visit_take(*take_id);
            }
            CompCandidateRef::ExistingSource { source_id, .. } => {
                let sref = self.node(ObjectKind::MediaSource, source_id.to_tagged());
                self.edge(&sref, &rref, DependencyEdgeKind::Event, json!({}));
                self.visit_media_source(*source_id);
                if self.project.midi_sources.contains_key(source_id) {
                    let mref = self.node(ObjectKind::MidiSource, source_id.to_tagged());
                    self.edge(&mref, &rref, DependencyEdgeKind::Event, json!({}));
                    self.visit_midi_source(*source_id);
                }
            }
            CompCandidateRef::Silence => {}
        }
        let pref = self.node(ObjectKind::Project, self.project.project_id.to_tagged());
        self.edge(&pref, &rref, DependencyEdgeKind::TimeMapping, json!({}));
    }

    fn visit_take(&mut self, take_id: TakeId) {
        if !self.first_visit(ObjectKind::Take, &take_id.to_tagged()) {
            return;
        }
        let tref = self.node(ObjectKind::Take, take_id.to_tagged());
        let Some(take) = self.project.recording.takes.get(&take_id) else {
            return;
        };
        let sref = self.node(ObjectKind::MediaSource, take.source_id.to_tagged());
        self.edge(&sref, &tref, DependencyEdgeKind::ExternalMedia, json!({}));
        self.visit_media_source(take.source_id);
    }

    // -- control closure ---------------------------------------------------

    fn visit_control_owner(&mut self, owner: ControlOwner) {
        for target in self
            .project
            .control
            .targets
            .values()
            .filter(|t| t.owner == owner)
        {
            self.visit_parameter_target(target.id);
        }
        for source in self
            .project
            .control
            .sources
            .values()
            .filter(|s| s.owner == owner)
        {
            self.visit_modulation_source(source.id);
        }
    }

    fn visit_parameter_target(&mut self, target_id: ParameterTargetId) {
        if !self.visited_targets.insert(target_id) {
            return;
        }
        let tref = self.node(ObjectKind::ParameterTarget, target_id.to_tagged());
        let Some(target) = self.project.control.targets.get(&target_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: target_id.to_tagged(),
                },
                format!("parameter target {}", target_id.to_tagged()),
            );
            return;
        };
        if target.time_scope == AutomationTimeScope::ProjectAbsolute {
            self.unknown(
                UnknownCause::OpaqueDependency {
                    locator: target_id.to_tagged(),
                },
                format!("target {} absolute scope", target_id.to_tagged()),
            );
        }
        if let Some(owner_ref) = self.control_owner_ref(target.owner) {
            self.edge(&owner_ref, &tref, DependencyEdgeKind::Parameter, json!({}));
        }
        for lane in self
            .project
            .control
            .lanes
            .values()
            .filter(|l| l.target == target_id)
        {
            let lref = self.node(ObjectKind::AutomationLane, lane.id.to_tagged());
            self.edge(&lref, &tref, DependencyEdgeKind::Control, json!({}));
            let pref = self.node(ObjectKind::Project, self.project.project_id.to_tagged());
            self.edge(&pref, &lref, DependencyEdgeKind::TimeMapping, json!({}));
        }
        for assignment in self
            .project
            .control
            .assignments
            .values()
            .filter(|a| a.enabled && a.target == target_id)
        {
            let aref = self.node(ObjectKind::ModulationAssignment, assignment.id.to_tagged());
            self.edge(&aref, &tref, DependencyEdgeKind::Control, json!({}));
            self.visit_modulation_source(assignment.source);
            if let Some(depth) = assignment.depth_source {
                let dref = self.node(ObjectKind::ParameterTarget, depth.to_tagged());
                self.edge(
                    &dref,
                    &aref,
                    DependencyEdgeKind::Control,
                    json!({"depth": true}),
                );
                self.visit_parameter_target(depth);
            }
        }
    }

    fn visit_modulation_source(&mut self, source_id: ModulationSourceId) {
        if !self.first_visit(ObjectKind::ModulationSource, &source_id.to_tagged()) {
            return;
        }
        let sref = self.node(ObjectKind::ModulationSource, source_id.to_tagged());
        let Some(source) = self.project.control.sources.get(&source_id) else {
            self.unknown(
                UnknownCause::MissingEntity {
                    locator: source_id.to_tagged(),
                },
                format!("modulation source {}", source_id.to_tagged()),
            );
            return;
        };
        if let Some(owner_ref) = self.control_owner_ref(source.owner) {
            self.edge(&owner_ref, &sref, DependencyEdgeKind::Control, json!({}));
        }
        let pref = self.node(ObjectKind::Project, self.project.project_id.to_tagged());
        self.edge(&pref, &sref, DependencyEdgeKind::TimeMapping, json!({}));
    }

    fn control_owner_ref(&self, owner: ControlOwner) -> Option<SemanticObjectRef> {
        let (kind, id) = match owner {
            ControlOwner::Project => return None,
            ControlOwner::Track(id) => (ObjectKind::Track, id.to_tagged()),
            ControlOwner::Strip(id) => (ObjectKind::Strip, id.to_tagged()),
            ControlOwner::Send(id) => (ObjectKind::SendConfig, id.to_tagged()),
            ControlOwner::Processor(id) => (ObjectKind::Processor, id.to_tagged()),
            ControlOwner::Clip(id) => (ObjectKind::Clip, id.to_tagged()),
            ControlOwner::Note(id) => (ObjectKind::Note, id.to_tagged()),
        };
        Some(self.object(kind, id))
    }

    // -- entry -------------------------------------------------------------

    fn run(&mut self, boundary: &RenderBoundary) {
        // Project singleton anchors musical-time consumers (tempo map,
        // PPQN — the canonical `Project` value carries exactly those).
        let pref = self.node(ObjectKind::Project, self.project.project_id.to_tagged());
        for port_id in &boundary.ports {
            self.visit_port(*port_id);
        }
        // Explicit TimeMapping edges from the project to every
        // musical-time consumer in the closure.
        let time_dependent: Vec<SemanticObjectRef> = self
            .nodes
            .iter()
            .filter(|n| {
                matches!(
                    n.kind,
                    ObjectKind::Clip
                        | ObjectKind::MidiSource
                        | ObjectKind::AutomationLane
                        | ObjectKind::Note
                        | ObjectKind::CompRegion
                        | ObjectKind::TakeGroup
                        | ObjectKind::Comp
                )
            })
            .cloned()
            .collect();
        for node in time_dependent {
            self.edge(&pref, &node, DependencyEdgeKind::TimeMapping, json!({}));
        }
    }
}

// ---------------------------------------------------------------------------
// public resolution
// ---------------------------------------------------------------------------

fn digest_closure(closure: &DependencyClosure) -> Result<ContentDigest, serde_json::Error> {
    Ok(ContentDigest::sha256(&serde_json::to_vec(closure)?))
}

fn digest_temporal(temporal: &TemporalDomain) -> Result<ContentDigest, serde_json::Error> {
    Ok(ContentDigest::sha256(&serde_json::to_vec(temporal)?))
}

fn digest_renderer(renderer: &RendererProfile) -> Result<ContentDigest, serde_json::Error> {
    Ok(ContentDigest::sha256(&serde_json::to_vec(renderer)?))
}

/// Processor-domain subset digest: plugin identities, state evidence,
/// external pins and the processor/port/plugin-instance/parameter
/// nodes that carry them.
fn digest_processor_closure(
    closure: &DependencyClosure,
) -> Result<ContentDigest, serde_json::Error> {
    const KINDS: [ObjectKind; 8] = [
        ObjectKind::Processor,
        ObjectKind::PluginInstance,
        ObjectKind::SemanticPort,
        ObjectKind::ParameterTarget,
        ObjectKind::AutomationLane,
        ObjectKind::ModulationSource,
        ObjectKind::ModulationAssignment,
        ObjectKind::SendConfig,
    ];
    let nodes: Vec<&DependencyNode> = closure
        .nodes
        .iter()
        .filter(|n| KINDS.contains(&n.object.kind))
        .collect();
    let payload = json!({
        "nodes": nodes,
        "plugin_states": closure.plugin_states,
        "external_pins": closure.external_pins,
    });
    Ok(ContentDigest::sha256(&serde_json::to_vec(&payload)?))
}

/// Schema + clock gates evaluated before any traversal. Returns the
/// early typed resolution when the input cannot be expressed under
/// the supported contract.
fn schema_gates(request: &RecipeRequest<'_>) -> Option<RecipeResolution> {
    // Schema ladder: a view resolved under an unknown project or
    // adapter schema stays typed Unknown — never coerced into the
    // current semantics (RCP-REC-06).
    if request.project_schema_version != synvelis_project::SCHEMA_VERSION {
        return Some(RecipeResolution::Unknown {
            dependencies: vec![UnknownDependency {
                cause: UnknownCause::UnsupportedSchema {
                    found: request.project_schema_version,
                },
                locator: format!(
                    "project schema {} (supported {})",
                    request.project_schema_version,
                    synvelis_project::SCHEMA_VERSION
                ),
            }],
        });
    }
    if request.render_input_schema_version != synvelis_project::RENDER_INPUT_SCHEMA_VERSION {
        return Some(RecipeResolution::Unknown {
            dependencies: vec![UnknownDependency {
                cause: UnknownCause::UnsupportedSchema {
                    found: request.render_input_schema_version,
                },
                locator: format!(
                    "render-input schema {} (supported {})",
                    request.render_input_schema_version,
                    synvelis_project::RENDER_INPUT_SCHEMA_VERSION
                ),
            }],
        });
    }
    // Live clocks can never be Exact — fail before traversal.
    match request.temporal.clock {
        ClockDependency::Live => Some(RecipeResolution::Unsupported {
            reason: UnsupportedReason::LiveClock,
        }),
        ClockDependency::Unpinned => Some(RecipeResolution::Unknown {
            dependencies: vec![UnknownDependency {
                cause: UnknownCause::UnpinnedClock,
                locator: "temporal.clock".to_owned(),
            }],
        }),
        ClockDependency::None | ClockDependency::Pinned { .. } => None,
    }
}

/// Project-oracle gates evaluated before traversal: the routing
/// model's own cycle/owner validation, the project-wide reference
/// oracle, and boundary port sanity.
fn input_gates(request: &RecipeRequest<'_>) -> Option<RecipeResolution> {
    // The routing model's own oracle — cycle rejection is the
    // existing zero-delay check, never a parallel graph authority.
    if let Err(e) = request.project.routing.validate(request.project) {
        let detail = e.to_string();
        return Some(RecipeResolution::Unsupported {
            reason: if detail.contains("cycle") {
                UnsupportedReason::CycleDetected
            } else {
                UnsupportedReason::InvalidRouting
            },
        });
    }
    // The project-wide reference oracle — dangling refs, invalid
    // temporal contracts, broken ownership all fail closed.
    if let Err(e) = request.project.validate_references() {
        return Some(RecipeResolution::Unsupported {
            reason: UnsupportedReason::InputRejected {
                detail: e.to_string(),
            },
        });
    }
    // Boundary ports must exist and be outputs.
    for port_id in &request.boundary.ports {
        match request.project.routing.ports.get(port_id) {
            None => {
                return Some(RecipeResolution::Unknown {
                    dependencies: vec![UnknownDependency {
                        cause: UnknownCause::MissingEntity {
                            locator: port_id.to_tagged(),
                        },
                        locator: format!("boundary port {}", port_id.to_tagged()),
                    }],
                });
            }
            Some(port) if port.direction != PortDirection::Output => {
                return Some(RecipeResolution::Unknown {
                    dependencies: vec![UnknownDependency {
                        cause: UnknownCause::OpaqueDependency {
                            locator: port_id.to_tagged(),
                        },
                        locator: format!("boundary port {} is not an output", port_id.to_tagged()),
                    }],
                });
            }
            _ => {}
        }
    }
    None
}

/// Converts a finished [`Resolver`] run into a [`DependencyClosure`]:
/// canonical per-node digests (extraction failures — absent entity,
/// non-finite float — become typed unknowns, never silent nulls) and
/// typed edges with their detail payloads.
fn build_closure(
    project: &Project,
    resolver: Resolver<'_>,
) -> Result<DependencyClosure, RecipeResolution> {
    let mut nodes = Vec::with_capacity(resolver.nodes.len());
    let mut unknowns = Vec::new();
    for object in &resolver.nodes {
        match canonical_entity_digest(project, object) {
            Ok(digest) => nodes.push(DependencyNode {
                object: object.clone(),
                digest,
            }),
            Err(e) => unknowns.push(UnknownDependency {
                cause: UnknownCause::InvalidValue {
                    locator: object.stable_id.clone(),
                },
                locator: format!("{}: {e}", object.stable_id),
            }),
        }
    }
    if !unknowns.is_empty() {
        return Err(RecipeResolution::Unknown {
            dependencies: unknowns,
        });
    }

    let edges: Vec<DependencyEdge> = resolver
        .edges
        .iter()
        .map(|(from, to, kind, detail_json)| DependencyEdge {
            from: from.clone(),
            to: to.clone(),
            kind: *kind,
            detail: resolver
                .edge_details
                .get(&(from.clone(), to.clone(), *kind, detail_json.clone()))
                .cloned()
                .unwrap_or(Value::Null),
        })
        .collect();

    Ok(DependencyClosure {
        nodes,
        edges,
        plugin_states: resolver.plugin_states,
        external_pins: resolver.external_pins,
    })
}

/// Resolves a canonical [`RenderRecipeV1`] + [`crate::recipe::RecipeKey`]
/// for `request`, or a typed `Unknown`/`Unsupported` resolution.
///
/// # Errors
/// Never fails with `Err` — every failure mode is a typed
/// [`RecipeResolution`] variant (fail closed).
#[must_use]
pub fn resolve_recipe(request: &RecipeRequest<'_>) -> RecipeResolution {
    if let Some(early) = schema_gates(request).or_else(|| input_gates(request)) {
        return early;
    }

    let mut resolver = Resolver::new(request.project, request.environment);
    resolver.run(&request.boundary);

    if !resolver.unknowns.is_empty() {
        return RecipeResolution::Unknown {
            dependencies: resolver.unknowns.into_iter().collect(),
        };
    }

    let closure = match build_closure(request.project, resolver) {
        Ok(closure) => closure,
        Err(resolution) => return resolution,
    };

    let (Ok(inputs_digest), Ok(processor_digest), Ok(temporal_digest), Ok(renderer_digest)) = (
        digest_closure(&closure),
        digest_processor_closure(&closure),
        digest_temporal(&request.temporal),
        digest_renderer(&request.renderer),
    ) else {
        return RecipeResolution::Unsupported {
            reason: UnsupportedReason::Canonicalization {
                detail: "digest serialization failed".to_owned(),
            },
        };
    };

    let target = RecipeTarget {
        boundary: request.boundary.clone(),
        project_id: request.project.project_id,
        project_schema_version: request.project_schema_version,
        render_input_schema_version: request.render_input_schema_version,
        branch: request.branch.clone(),
    };

    let recipe = RenderRecipeV1 {
        schema_version: RECIPE_SCHEMA_VERSION,
        canonicalization_version: CANONICALIZATION_VERSION,
        audio_semantics_version: AUDIO_SEMANTICS_VERSION,
        hash_algorithm: RECIPE_HASH_ALGORITHM.to_owned(),
        domain_separator: RECIPE_DOMAIN_TAG.to_owned(),
        target,
        inputs_digest,
        processor_digest,
        temporal_digest,
        temporal: request.temporal.clone(),
        renderer_digest,
        output_contract: request.output.clone(),
        closure,
    };

    match recipe.key() {
        Ok(key) => RecipeResolution::Resolved {
            recipe: Box::new(recipe),
            key,
            provenance: Box::new(request.provenance.clone()),
        },
        Err(e) => RecipeResolution::Unsupported {
            reason: UnsupportedReason::Canonicalization {
                detail: e.to_string(),
            },
        },
    }
}
