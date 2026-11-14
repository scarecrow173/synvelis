//! RCP-04 typed graph model — the `ResolvedGraphSnapshot` contract the
//! missing-plugin resolver consumes.
//!
//! This module owns the *headless domain model* for Issue #293: a
//! pinned, immutable typed-graph view (semantic-stable node/port/edge
//! IDs, Audio/TimedEvent/Control domains, multiplicity, channel maps,
//! PDC/latency context, sidechain/send/return edge kinds) plus the
//! plugin declarations, environment capability snapshot and render
//! candidate index the resolver evaluates against.
//!
//! Boundary contract: nothing here performs I/O, lowers to runtime
//! IDs, or mutates `Project`. The workspace semantic→selection
//! integration that *produces* these snapshots is a deferred residual
//! (separate lease); this slice defines the contract and the pure
//! resolution/reconciliation logic on top of it. Port attribute
//! vocabulary (`Direction`/`PortRole`/`PortFormat`/`Multiplicity`/
//! `Availability`/`Activation`/`SignalDomain`/`ChannelMap`/`EdgeDelay`)
//! is reused from `ruviform_graph` so the snapshot speaks the same
//! typed-graph language the compiler consumes — it is not a second
//! port model.

use std::collections::{BTreeMap, BTreeSet};

use ruviform_graph::{
    Activation, Availability, ChannelMap, Direction, EdgeDelay, Multiplicity, PortFormat, PortRole,
    SignalDomain,
};
use synvelis_project::{
    OpaqueStateStatus, ProjectId, ProjectRevision, RuntimeClockEpochRef, RuntimeInstanceId,
    StateCaptureQuality,
};

/// Snapshot-local stable node identity. Issued by whoever builds the
/// [`ResolvedGraphSnapshot`] (the deferred semantic→selection
/// integration); never a runtime `NodeId` and never persisted into
/// `Project` — runtime IDs are reissued per build generation per the
/// `RoutingSourceMap` contract.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RcpNodeId(pub u64);

/// Snapshot-local stable port identity (unique within the snapshot).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RcpPortId(pub u32);

/// Snapshot-local stable edge identity.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RcpEdgeId(pub u64);

/// One endpoint of a signal path: a node plus one of its ports.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PortEndpoint {
    /// Owning node.
    pub node: RcpNodeId,
    /// Port on that node.
    pub port: RcpPortId,
}

/// Structural classification of a snapshot node.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NodeClass {
    /// Media/MIDI source — executable when its verified content is
    /// available; never a plugin.
    Source,
    /// Built-in DSP (gain/mix/pan/route) — always executable.
    Builtin,
    /// Hosted plugin processor — executability is decided by the
    /// [`super::verify`] pass against [`PluginCapabilitySnapshot`].
    Plugin,
    /// Output sink — terminal, never substitutable.
    Sink,
}

/// The plugin format an authored instance targets.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PluginFormat {
    /// VST3.
    Vst3,
    /// CLAP.
    Clap,
    /// Audio Unit.
    Au,
    /// LV2.
    Lv2,
    /// Built-in/internal processor format.
    Internal,
}

/// Authored plugin identity — what the project pinned when the
/// processor was bound. `binary_digest` is the optional content
/// digest of the resolved binary; `None` means identity rests on
/// format+uid+version alone (weaker evidence, still explicit).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PluginIdentity {
    /// Plugin format.
    pub format: PluginFormat,
    /// Vendor string as authored.
    pub vendor: String,
    /// Plugin-unique id within the format.
    pub uid: String,
    /// Authored version string.
    pub version: String,
    /// Optional binary content digest.
    pub binary_digest: Option<String>,
}

/// Environment-side lookup key for a plugin: format + uid. Version
/// and binary digest are compared as *evidence* during verification,
/// not as key material — a version drift must surface as a typed
/// verdict, not a silent `Missing`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PluginKey {
    /// Plugin format.
    pub format: PluginFormat,
    /// Hashed uid — see [`PluginEnvironment::uid_hash`].
    pub uid_hash: u64,
}

/// One advertised/declared port in a plugin layout — the unit port
/// layout comparison runs on. `name` is the plugin-advertised port
/// identity; direction/domain/format must match the authored
/// declaration exactly or the layout verdict is a mismatch (never a
/// guess).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeclaredPort {
    /// Plugin-advertised port name/identity.
    pub name: String,
    /// Port direction.
    pub direction: Direction,
    /// Signal domain.
    pub domain: SignalDomain,
    /// Payload format (audio carries rate/layout/encoding).
    pub format: PortFormat,
}

/// Opaque plugin-state evidence carried by a [`PluginDeclaration`].
/// `digest`/`quality`/`status` mirror the TIM-04 capture vocabulary:
/// the resolver consumes them, it never re-interprets or upgrades
/// them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginStateDecl {
    /// Content digest of the opaque state blob (`None` = no state
    /// was ever captured — distinct from an empty blob).
    pub digest: Option<String>,
    /// Observed capture quality.
    pub quality: StateCaptureQuality,
    /// Opaque-state verification status (`Known`/`Unknown`/`Missing`).
    pub status: OpaqueStateStatus,
}

/// Everything the snapshot knows about one plugin processor — the
/// authored identity, opaque state evidence, the port layout the
/// project declared, and whether automation/parameter identities are
/// complete enough for semantic editing (incomplete identity blocks
/// editing *and* substitution evidence, never guessed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginDeclaration {
    /// Authored plugin identity.
    pub identity: PluginIdentity,
    /// Opaque state evidence.
    pub state: PluginStateDecl,
    /// Authored port layout (plugin-advertised ports the project
    /// bound).
    pub declared_ports: Vec<DeclaredPort>,
    /// `true` when every automation/parameter identity the project
    /// references resolves inside the declared layout — `false`
    /// fails closed (no guessed parameters).
    pub automation_identity_complete: bool,
}

/// One declared port of a snapshot node.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPort {
    /// Snapshot-local port id.
    pub id: RcpPortId,
    /// Input or output.
    pub direction: Direction,
    /// Structural bus role (Main/Aux/Sidechain/Send/Return).
    pub role: PortRole,
    /// Payload format — its [`PortFormat::domain`] is the signal
    /// domain.
    pub format: PortFormat,
    /// Declared connection-count bounds.
    pub multiplicity: Multiplicity,
    /// Mandatory vs. optional presence.
    pub availability: Availability,
    /// Enabled vs. declared-but-inactive.
    pub activation: Activation,
}

impl ResolvedPort {
    /// Signal domain of this port.
    #[must_use]
    pub fn domain(&self) -> SignalDomain {
        self.format.domain()
    }

    /// `true` when this port's connection requirements are enforced:
    /// `Availability::Always` + `Activation::Enabled`.
    #[must_use]
    pub fn requirement_enforced(&self) -> bool {
        self.availability == Availability::Always && self.activation == Activation::Enabled
    }
}

/// One node of the resolved typed graph.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedNode {
    /// Snapshot-local node id.
    pub id: RcpNodeId,
    /// Structural class.
    pub class: NodeClass,
    /// Declared ports.
    pub ports: Vec<ResolvedPort>,
    /// Plugin declaration — `Some` iff `class == NodeClass::Plugin`.
    pub plugin: Option<PluginDeclaration>,
    /// Processing latency this node reports (frames) — contributes
    /// to boundary-edge effective latency for cutset matching.
    pub latency_frames: u64,
    /// Display-only provenance label — excluded from every digest
    /// and identity comparison (rename must never rebuild).
    pub label: Option<String>,
}

impl ResolvedNode {
    /// Looks up a declared port.
    #[must_use]
    pub fn port(&self, id: RcpPortId) -> Option<&ResolvedPort> {
        self.ports.iter().find(|p| p.id == id)
    }
}

/// Routing kind of a snapshot edge — the typed context cutset rules
/// check (send double-fire, sidechain pinning, return closure).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    /// Main signal path.
    Main,
    /// Send tap route.
    Send,
    /// Return-bus feed.
    Return,
    /// Sidechain/key input.
    Sidechain,
}

/// One enabled connection of the resolved typed graph. Disabled
/// connections are not edges — they never reach the snapshot (a
/// declared-but-disabled port keeps `Activation::Disabled` instead).
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedEdge {
    /// Snapshot-local edge id.
    pub id: RcpEdgeId,
    /// Producing endpoint (must be an `Output` port).
    pub from: PortEndpoint,
    /// Consuming endpoint (must be an `Input` port).
    pub to: PortEndpoint,
    /// Routing kind.
    pub kind: EdgeKind,
    /// Declared edge delay (zero-delay cycles follow the existing
    /// typed-graph rules upstream).
    pub delay: EdgeDelay,
    /// Explicit channel map when layouts differ.
    pub channel_map: Option<ChannelMap>,
    /// Edge-carried PDC/latency context in frames — part of the
    /// boundary contract a substitution must reproduce.
    pub pdc_frames: u64,
}

/// A port the session must be fed correctly — the snapshot's
/// declared boundary outputs (device sink inputs, audition taps,
/// record outs). Substitution cutsets must keep every required
/// output supplied; this is the "boundary covers all required
/// external output ports" contract from the spec.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BoundaryRequirement {
    /// The input port that must receive signal.
    pub port: PortEndpoint,
    /// Why it is required (evidence label, display-only).
    pub purpose: BoundaryPurpose,
}

/// Why a boundary port is required.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BoundaryPurpose {
    /// Main monitor/device output.
    MainOut,
    /// Record/write destination.
    RecordOut,
    /// Branch audition tap.
    AuditionTap,
    /// Offline render/export destination.
    RenderOut,
    /// Other declared boundary output.
    Other,
}

/// The pinned, immutable resolved-graph input of the RCP-04 resolver
/// pipeline (spec §Non-RT resolver pipeline step 1–3 input). Every
/// pin the reconciler later re-verifies lives here: project identity
/// and revision, the graph structure digest, capability/format
/// generations, the clock epoch, and the runtime instance.
#[derive(Clone, Debug)]
pub struct ResolvedGraphSnapshot {
    /// Snapshot identity for receipts (caller-assigned).
    pub snapshot_id: u64,
    /// Owning project.
    pub project_id: ProjectId,
    /// Committed project revision this snapshot resolved from.
    pub project_revision: ProjectRevision,
    /// Deterministic digest of graph-relevant content (nodes, ports,
    /// edges, plugin declarations — *excluding* display labels and
    /// control values). Same role as `RoutingFingerprint` but scoped
    /// to this typed view; computed by [`Self::structure_digest`].
    pub structure_digest: u64,
    /// Project-side port/processor capability generation.
    pub capability_generation: u64,
    /// Engine geometry generation (rate/channels/block).
    pub engine_config_generation: u64,
    /// Canonical source format generation.
    pub source_format_generation: u64,
    /// Device/engine clock epoch this snapshot targets.
    pub clock_epoch: RuntimeClockEpochRef,
    /// Runtime instance this snapshot targets.
    pub runtime_instance: RuntimeInstanceId,
    /// Typed nodes by id.
    pub nodes: BTreeMap<RcpNodeId, ResolvedNode>,
    /// Enabled edges.
    pub edges: Vec<ResolvedEdge>,
    /// Declared boundary outputs that must remain supplied.
    pub required_outputs: Vec<BoundaryRequirement>,
}

impl ResolvedGraphSnapshot {
    /// Looks up a node.
    #[must_use]
    pub fn node(&self, id: RcpNodeId) -> Option<&ResolvedNode> {
        self.nodes.get(&id)
    }

    /// Enabled edges leaving `region` for live nodes, and entering
    /// `region` from live nodes — the cut boundary of a candidate
    /// substitution. Both lists are sorted by edge id for
    /// deterministic evidence.
    #[must_use]
    pub fn cut_boundary(
        &self,
        region: &BTreeSet<RcpNodeId>,
    ) -> (Vec<&ResolvedEdge>, Vec<&ResolvedEdge>) {
        let mut outputs = Vec::new();
        let mut inputs = Vec::new();
        for edge in &self.edges {
            let from_in = region.contains(&edge.from.node);
            let to_in = region.contains(&edge.to.node);
            match (from_in, to_in) {
                (true, false) => outputs.push(edge),
                (false, true) => inputs.push(edge),
                _ => {}
            }
        }
        outputs.sort_by_key(|e| e.id);
        inputs.sort_by_key(|e| e.id);
        (outputs, inputs)
    }

    /// Live input edges into `port` plus how many artifact outputs
    /// would feed it — used by the dangling-input check.
    fn feeds_into(
        &self,
        region: &BTreeSet<RcpNodeId>,
        covered_outputs: &BTreeSet<PortEndpoint>,
        port: PortEndpoint,
    ) -> usize {
        let mut feeds = 0usize;
        for edge in &self.edges {
            if edge.to != port {
                continue;
            }
            if region.contains(&edge.from.node) {
                // Boundary output: counts only when the artifact
                // covers this source port.
                if covered_outputs.contains(&edge.from) {
                    feeds += 1;
                }
            } else {
                feeds += 1;
            }
        }
        feeds
    }

    /// `true` when `port`'s producer sits inside `region`.
    #[must_use]
    pub fn produced_inside(&self, region: &BTreeSet<RcpNodeId>, port: PortEndpoint) -> bool {
        region.contains(&port.node)
    }

    /// Feeds count for a live input port after `region` is replaced
    /// by an artifact covering `covered_outputs` (source-side
    /// endpoints). `pub(crate)` for the cutset validator.
    pub(crate) fn feeds_after_cut(
        &self,
        region: &BTreeSet<RcpNodeId>,
        covered_outputs: &BTreeSet<PortEndpoint>,
        port: PortEndpoint,
    ) -> usize {
        self.feeds_into(region, covered_outputs, port)
    }

    /// Nodes reachable upstream of `id` (exclusive), following edges
    /// backwards. Used to build substitution-region candidates.
    #[must_use]
    pub fn upstream_closure(&self, id: RcpNodeId) -> BTreeSet<RcpNodeId> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![id];
        while let Some(node) = stack.pop() {
            for edge in &self.edges {
                if edge.to.node == node && seen.insert(edge.from.node) {
                    stack.push(edge.from.node);
                }
            }
        }
        seen.remove(&id);
        seen
    }

    /// Nodes reachable downstream of `id` (exclusive).
    #[must_use]
    pub fn downstream_closure(&self, id: RcpNodeId) -> BTreeSet<RcpNodeId> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![id];
        while let Some(node) = stack.pop() {
            for edge in &self.edges {
                if edge.from.node == node && seen.insert(edge.to.node) {
                    stack.push(edge.to.node);
                }
            }
        }
        seen.remove(&id);
        seen
    }

    /// Deterministic FNV-1a digest over the graph-relevant content:
    /// every node's class/ports/plugin declaration and every edge's
    /// endpoints/kind/delay/channel-map/PDC. Display `label`s and
    /// control values are excluded — a rename cannot change this.
    /// Same contract class as [`synvelis_project::RoutingFingerprint`]:
    /// a change detector, not a cryptographic identity.
    #[must_use]
    pub fn compute_structure_digest(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut mix = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        for node in self.nodes.values() {
            mix(format!("{:?}|{:?}", node.id, node.class).as_bytes());
            for port in &node.ports {
                mix(format!(
                    "{:?}{:?}{:?}{:?}{:?}{:?}{:?}",
                    port.id,
                    port.direction,
                    port.role,
                    port.format,
                    port.multiplicity,
                    port.availability,
                    port.activation
                )
                .as_bytes());
            }
            if let Some(plugin) = &node.plugin {
                mix(format!("{plugin:?}").as_bytes());
            }
            mix(format!("{}", node.latency_frames).as_bytes());
        }
        for edge in &self.edges {
            mix(format!(
                "{:?}{:?}{:?}{:?}{:?}{:?}",
                edge.id, edge.from, edge.to, edge.kind, edge.delay, edge.pdc_frames
            )
            .as_bytes());
            if let Some(map) = &edge.channel_map {
                mix(format!("{map:?}").as_bytes());
            }
        }
        for req in &self.required_outputs {
            mix(format!("{:?}{:?}", req.port, req.purpose).as_bytes());
        }
        hash
    }
}

// ---------------------------------------------------------------------------
// Environment side: plugin capability + render candidate index
// ---------------------------------------------------------------------------

/// Host-side state save/restore support for one plugin.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StateRestoreSupport {
    /// State can be saved and restored with verified fidelity.
    Verified,
    /// The plugin does not implement state restore.
    Unsupported,
    /// Restore support cannot be determined — fail closed.
    Unknown,
}

/// Access/permission state for a plugin binary or artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessState {
    /// Granted.
    Granted,
    /// Denied (ACL/permission/sandbox) — reason is evidence, not
    /// user-facing prose contract.
    Denied {
        /// Machine-readable reason detail.
        reason: String,
    },
}

/// What the environment reports about one installed (or missing)
/// plugin — facts only; the [`super::verify`] pass turns them into
/// a typed [`super::verify::PluginVerdict`].
#[derive(Clone, Debug, PartialEq)]
pub struct PluginEnvironment {
    /// `false` = plugin binary not installed/bindable on this host.
    pub installed: bool,
    /// Authored version the environment actually resolved (`None`
    /// when not installed).
    pub resolved_version: Option<String>,
    /// Binary digest the environment resolved (`None` = unknown).
    pub resolved_binary_digest: Option<String>,
    /// Access/permission state.
    pub access: AccessState,
    /// Whether the host supports this plugin format at all.
    pub format_supported: bool,
    /// Advertised port layout when installed (`None` = layout
    /// unknown — treated as mismatch evidence, never guessed).
    pub advertised_ports: Option<Vec<DeclaredPort>>,
    /// State restore support.
    pub state_restore: StateRestoreSupport,
    /// FNV-1a uid hash helper input — see [`PluginKey::of`].
    pub uid_hash: u64,
}

impl PluginEnvironment {
    /// `true` when the environment reports a fully usable plugin:
    /// installed, access granted, format supported.
    #[must_use]
    pub fn usable(&self) -> bool {
        self.installed && self.format_supported && matches!(self.access, AccessState::Granted)
    }
}

impl PluginKey {
    /// Derives the lookup key from an authored identity. Exported so
    /// snapshot builders and tests share one hashing rule.
    #[must_use]
    pub fn of(identity: &PluginIdentity) -> Self {
        Self {
            format: identity.format,
            uid_hash: fnv1a(identity.uid.as_bytes()),
        }
    }
}

/// FNV-1a 64 — shared by [`PluginKey`] and structure digests.
#[must_use]
pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Generation-pinned environment view of plugin availability —
/// the `PluginCapabilitySnapshot(generation)` input of the Issue
/// #293 contract. `generation` participates in plan pins: any
/// change re-triggers selection (plugin missing→available or
/// capability drift) without touching render eligibility.
#[derive(Clone, Debug, Default)]
pub struct PluginCapabilitySnapshot {
    /// Capability generation — compared against the snapshot pin.
    pub generation: u64,
    /// Environment facts keyed by [`PluginKey`].
    pub entries: BTreeMap<PluginKey, PluginEnvironment>,
}

impl PluginCapabilitySnapshot {
    /// Environment facts for `identity`, if the host reported any.
    #[must_use]
    pub fn get(&self, identity: &PluginIdentity) -> Option<&PluginEnvironment> {
        self.entries.get(&PluginKey::of(identity))
    }
}

// ---------------------------------------------------------------------------
// Render candidates (verified-artifact substitution inputs)
// ---------------------------------------------------------------------------

/// Artifact eligibility state machine — the spec §Eligibility
/// vocabulary consumed as *input* by this slice (the evaluator that
/// produces it is RCP-01/02 scope). `Unknown` is unevaluated and
/// must never be treated as valid.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArtifactEligibility {
    /// No artifact exists / unreachable.
    Missing,
    /// Exists but verification incomplete.
    Unverified,
    /// Verification failed (hash/manifest/decode contradiction).
    Corrupt,
    /// Recipe/context/snapshot changed — revalidation needed.
    Stale,
    /// Recipe canonically resolvable.
    ValidRecipe,
    /// Manifest + digest + readback verified.
    VerifiedMedia,
    /// All six Exact conditions met.
    EligibleExact,
    /// Preview-relaxed conditions met (non-equivalence must be
    /// displayed).
    EligiblePreview,
    /// Capability/format technically unsupported.
    Unsupported,
    /// Unevaluated / undecidable — never valid.
    Unknown,
}

/// Reproducibility class of an artifact — spec §Reproducibility
/// Classes. No auto-promotion across classes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReproducibilityClass {
    /// Bit-identical reproduction proven.
    BitExactCertified,
    /// Verified within a defined tolerance.
    VerifiedWithinTolerance,
    /// Replay of a verified artifact (sharing path, not re-render).
    ReplayOfVerifiedArtifact,
    /// Preview use only — never promotes to Exact.
    PreviewOnly,
    /// Unclassified — never valid for Exact.
    Unknown,
}

/// One output signal an artifact reproduces, keyed by the
/// *region-side source port* it replaces. The edge's existing
/// channel map keeps applying because `format` must equal the
/// replaced port's format — substitution preserves edge semantics
/// rather than re-interpreting them.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtifactOutput {
    /// The output port inside the substituted region this signal
    /// replaces (source side of every covered boundary edge).
    pub source: PortEndpoint,
    /// Signal domain reproduced (`Audio`/`TimedEvent`/`Control`) —
    /// non-audio outputs are never fabricated from audio.
    pub domain: SignalDomain,
    /// Emitted payload format — must equal the replaced port's
    /// format for audio; unit formats compare by domain.
    pub format: PortFormat,
    /// Effective latency/PDC (frames) the artifact reproduces —
    /// must equal the covered boundary edges' expected latency.
    pub latency_frames: u64,
}

/// One verified (or candidate) render artifact eligible for
/// subgraph substitution. `node_set` is the exact subgraph the
/// artifact replaces; `pinned_inputs` lists the region-side ports
/// whose incoming boundary signals were baked into the render —
/// every boundary input of a valid cutset must appear there, which
/// is what prevents double-fire/double-add of live edges.
#[derive(Clone, Debug)]
pub struct RenderCandidate {
    /// Artifact identity (content-addressed or store-assigned).
    pub artifact_id: String,
    /// Byte-identity content hash.
    pub content_hash: String,
    /// Exact node set this artifact substitutes.
    pub node_set: BTreeSet<RcpNodeId>,
    /// Output signals reproduced, keyed by region-side source port.
    pub outputs: Vec<ArtifactOutput>,
    /// Region-side input ports whose boundary signals were pinned
    /// into the render.
    pub pinned_inputs: BTreeSet<PortEndpoint>,
    /// Eligibility state (input — produced by RCP-01/02 evaluator).
    pub eligibility: ArtifactEligibility,
    /// Reproducibility class.
    pub reproducibility: ReproducibilityClass,
    /// Project revision the artifact was rendered from.
    pub snapshot_revision: ProjectRevision,
    /// Structure digest of the snapshot it was rendered from.
    pub snapshot_digest: u64,
    /// Access/permission state (ACL/expiry evidence).
    pub access: AccessState,
}

/// Generation-pinned index of render candidates — the
/// `RenderCandidateIndex` input. `generation` changes on artifact
/// arrival and triggers re-selection (not graph rebuild).
#[derive(Clone, Debug, Default)]
pub struct RenderCandidateIndex {
    /// Index generation.
    pub generation: u64,
    /// Candidates, deterministically ordered by artifact id.
    pub candidates: Vec<RenderCandidate>,
}

impl RenderCandidateIndex {
    /// Candidates whose `node_set` contains `node`.
    pub fn covering(&self, node: RcpNodeId) -> impl Iterator<Item = &RenderCandidate> {
        self.candidates
            .iter()
            .filter(move |c| c.node_set.contains(&node))
    }
}
