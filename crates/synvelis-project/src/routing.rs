//! The Semantic Routing Model (issue #104 / SF-01): the persisted,
//! project-owned signal-flow authority.
//!
//! Buses, channel strips, processor chains, semantic ports, connections
//! and send configs live here with stable typed IDs — they survive
//! save/reload, undo/redo and VCS diffs. Runtime graph `NodeId`/`PortId`,
//! plugin-local port identifiers, process slots and node generations are
//! *different* types: a numeric match never rebinds a project ID, and an
//! unresolvable binding is kept explicitly as `Unresolved`.
//!
//! Invariants are enforced by [`RoutingModel::validate`], called from
//! [`crate::Project::validate_references`] — the same oracle the writer
//! runs on every candidate state, so a rejected command leaves the live
//! project untouched.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    control::{ControlModel, ControlOwner, ParameterRole},
    error::Error,
    id::{
        BusId, ChainId, ParameterTargetId, ProcessorId, RoutingConnectionId, SemanticPortId,
        SendId, StripId, TrackId,
    },
    project::Project,
};

/// Lower bound of the reserved raw-ID range. Values `>=` this bound are
/// *well-known constants* allocated statically (the bootstrap master
/// entities); the `IdGenerator` never reaches them, so they cannot
/// collide with issued IDs on any seed. They are excluded from the
/// `id_watermark` comparison because they are constants, not issued IDs.
pub const WELL_KNOWN_BASE: u128 = 1 << 96;

/// `true` when `raw` is an issuable (generator-domain) raw value.
#[must_use]
pub const fn is_issuable_raw(raw: u128) -> bool {
    raw < WELL_KNOWN_BASE
}

const MASTER_BUS_RAW: u128 = WELL_KNOWN_BASE + 1;
const MASTER_STRIP_RAW: u128 = WELL_KNOWN_BASE + 2;
const MASTER_CHAIN_RAW: u128 = WELL_KNOWN_BASE + 3;
const MASTER_IN_RAW: u128 = WELL_KNOWN_BASE + 4;
const MASTER_OUT_RAW: u128 = WELL_KNOWN_BASE + 5;

/// The semantic routing model — all persisted signal-flow entities.
/// Key order (`BTreeMap`) keeps serialization canonical.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingModel {
    /// The single logical master bus (exactly one).
    pub master: BusId,
    /// Buses by ID (includes the master).
    pub buses: BTreeMap<BusId, Bus>,
    /// Channel strips by ID.
    pub strips: BTreeMap<StripId, ChannelStrip>,
    /// Processors by ID.
    pub processors: BTreeMap<ProcessorId, Processor>,
    /// Processor chains by ID (each owned by exactly one strip).
    pub chains: BTreeMap<ChainId, ProcessorChain>,
    /// Semantic ports by ID.
    pub ports: BTreeMap<SemanticPortId, SemanticPort>,
    /// Connections by ID — the single authority for signal routing.
    pub connections: BTreeMap<RoutingConnectionId, RoutingConnection>,
    /// Send configs by ID (strictly 1:1 with `kind = Send` connections).
    pub sends: BTreeMap<SendId, SendConfig>,
}

impl Default for RoutingModel {
    /// Bootstrap model: the master bus, its channel strip, an empty
    /// chain and one main input + one main output port — the minimum a
    /// legal project carries so "Master exactly one" holds even for an
    /// empty project (SF-MODEL-01).
    fn default() -> Self {
        Self::bootstrap()
    }
}

impl RoutingModel {
    /// The bootstrap model with well-known master entities.
    #[must_use]
    pub fn bootstrap() -> Self {
        let master = BusId::new(MASTER_BUS_RAW);
        let strip = StripId::new(MASTER_STRIP_RAW);
        let chain = ChainId::new(MASTER_CHAIN_RAW);
        let port_in = SemanticPortId::new(MASTER_IN_RAW);
        let port_out = SemanticPortId::new(MASTER_OUT_RAW);
        Self {
            master,
            buses: BTreeMap::from([(
                master,
                Bus {
                    id: master,
                    kind: BusKind::Master,
                    name: "Master".into(),
                    strip,
                },
            )]),
            strips: BTreeMap::from([(
                strip,
                ChannelStrip {
                    id: strip,
                    owner: StripOwner::Bus(master),
                    chain,
                    port_ids: vec![port_in, port_out],
                },
            )]),
            processors: BTreeMap::new(),
            chains: BTreeMap::from([(
                chain,
                ProcessorChain {
                    id: chain,
                    processors: Vec::new(),
                },
            )]),
            ports: BTreeMap::from([
                (
                    port_in,
                    SemanticPort {
                        id: port_in,
                        owner: PortOwner::Strip(strip),
                        direction: PortDirection::Input,
                        role: PortRole::Main,
                        domain: SignalDomain::Audio,
                        binding: PortBinding::Internal,
                        channels: DEFAULT_PORT_CHANNELS,
                    },
                ),
                (
                    port_out,
                    SemanticPort {
                        id: port_out,
                        owner: PortOwner::Strip(strip),
                        direction: PortDirection::Output,
                        role: PortRole::Main,
                        domain: SignalDomain::Audio,
                        binding: PortBinding::Internal,
                        channels: DEFAULT_PORT_CHANNELS,
                    },
                ),
            ]),
            connections: BTreeMap::new(),
            sends: BTreeMap::new(),
        }
    }

    /// The strip owned by `track`, if it has one.
    #[must_use]
    pub fn strip_of_track(&self, track_id: TrackId) -> Option<&ChannelStrip> {
        self.strips
            .values()
            .find(|s| s.owner == StripOwner::Track(track_id))
    }

    /// Validates the whole model against the project — the routing part
    /// of the load/apply oracle.
    ///
    /// # Errors
    /// [`Error::NotFound`]/[`Error::WrongOwner`]/[`Error::Validation`].
    #[allow(clippy::too_many_lines)] // sequential per-entity checks
    pub(crate) fn validate(&self, project: &Project) -> Result<(), Error> {
        // Map key == entity id for every map.
        for (key, bus) in &self.buses {
            if *key != bus.id {
                return Err(Error::WrongOwner(format!(
                    "bus {} stored under foreign key {key}",
                    bus.id
                )));
            }
        }
        for (key, strip) in &self.strips {
            if *key != strip.id {
                return Err(Error::WrongOwner(format!(
                    "strip {} stored under foreign key {key}",
                    strip.id
                )));
            }
        }
        for (key, proc) in &self.processors {
            if *key != proc.id {
                return Err(Error::WrongOwner(format!(
                    "processor {} stored under foreign key {key}",
                    proc.id
                )));
            }
        }
        for (key, chain) in &self.chains {
            if *key != chain.id {
                return Err(Error::WrongOwner(format!(
                    "chain {} stored under foreign key {key}",
                    chain.id
                )));
            }
        }
        for (key, port) in &self.ports {
            if *key != port.id {
                return Err(Error::WrongOwner(format!(
                    "port {} stored under foreign key {key}",
                    port.id
                )));
            }
        }
        for (key, conn) in &self.connections {
            if *key != conn.id {
                return Err(Error::WrongOwner(format!(
                    "connection {} stored under foreign key {key}",
                    conn.id
                )));
            }
        }
        for (key, send) in &self.sends {
            if *key != send.id {
                return Err(Error::WrongOwner(format!(
                    "send {} stored under foreign key {key}",
                    send.id
                )));
            }
        }

        // Master: exists, is a master, and is the only one.
        let master = self
            .buses
            .get(&self.master)
            .ok_or_else(|| Error::NotFound(format!("master bus {}", self.master)))?;
        if master.kind != BusKind::Master {
            return Err(Error::Validation(format!(
                "bus {} named master is kind {:?}",
                self.master, master.kind
            )));
        }
        if self
            .buses
            .values()
            .filter(|b| b.kind == BusKind::Master)
            .count()
            != 1
        {
            return Err(Error::Validation(
                "exactly one master bus is required".into(),
            ));
        }

        // Bus ↔ strip: the strip a bus names must be bus-owned by that
        // bus, and no other strip may claim the same bus.
        for bus in self.buses.values() {
            let strip = self
                .strips
                .get(&bus.strip)
                .ok_or_else(|| Error::NotFound(format!("strip {}", bus.strip)))?;
            if strip.owner != StripOwner::Bus(bus.id) {
                return Err(Error::WrongOwner(format!(
                    "strip {} does not belong to bus {}",
                    strip.id, bus.id
                )));
            }
            if self
                .strips
                .values()
                .filter(|s| s.owner == StripOwner::Bus(bus.id))
                .count()
                != 1
            {
                return Err(Error::Validation(format!(
                    "bus {} has more than one strip",
                    bus.id
                )));
            }
        }

        // Strips: owner exists (and for buses matches the bus's strip
        // field — checked above), chain exists, ports exist and point
        // back, exactly one main input + one main output.
        let mut track_strips: BTreeSet<TrackId> = BTreeSet::new();
        for strip in self.strips.values() {
            match strip.owner {
                StripOwner::Track(t) => {
                    if !project.tracks.contains_key(&t) {
                        return Err(Error::NotFound(format!("track {t}")));
                    }
                    if !track_strips.insert(t) {
                        return Err(Error::Validation(format!(
                            "track {t} has more than one strip"
                        )));
                    }
                }
                StripOwner::Bus(b) => {
                    if !self.buses.contains_key(&b) {
                        return Err(Error::NotFound(format!("bus {b}")));
                    }
                }
            }
            if !self.chains.contains_key(&strip.chain) {
                return Err(Error::NotFound(format!("chain {}", strip.chain)));
            }
            let mut main_in = 0usize;
            let mut main_out = 0usize;
            for port_id in &strip.port_ids {
                let port = self
                    .ports
                    .get(port_id)
                    .ok_or_else(|| Error::NotFound(format!("port {port_id}")))?;
                if port.owner != PortOwner::Strip(strip.id) {
                    return Err(Error::WrongOwner(format!(
                        "port {port_id} does not belong to strip {}",
                        strip.id
                    )));
                }
                match (port.role, port.direction) {
                    (PortRole::Main, PortDirection::Input) => main_in += 1,
                    (PortRole::Main, PortDirection::Output) => main_out += 1,
                    (PortRole::Sidechain, _) => {
                        return Err(Error::Validation(format!(
                            "strip port {port_id} has sidechain role"
                        )));
                    }
                }
            }
            if main_in != 1 || main_out != 1 {
                return Err(Error::Validation(format!(
                    "strip {} needs exactly one main input and one main output (got {main_in}/{main_out})",
                    strip.id
                )));
            }
        }

        // Chains: processors exist, are unique in the list, and each
        // processor belongs to at most one chain globally; each chain
        // has exactly one owning strip.
        let mut chain_owners: BTreeMap<ChainId, usize> = BTreeMap::new();
        for strip in self.strips.values() {
            *chain_owners.entry(strip.chain).or_insert(0) += 1;
        }
        let mut processor_owners: BTreeMap<ProcessorId, usize> = BTreeMap::new();
        for chain in self.chains.values() {
            if chain_owners.get(&chain.id).copied().unwrap_or(0) != 1 {
                return Err(Error::Validation(format!(
                    "chain {} must be owned by exactly one strip",
                    chain.id
                )));
            }
            let mut seen = BTreeSet::new();
            for proc_id in &chain.processors {
                if !self.processors.contains_key(proc_id) {
                    return Err(Error::NotFound(format!("processor {proc_id}")));
                }
                if !seen.insert(*proc_id) {
                    return Err(Error::Validation(format!(
                        "processor {proc_id} listed twice in chain {}",
                        chain.id
                    )));
                }
                *processor_owners.entry(*proc_id).or_insert(0) += 1;
            }
        }
        for proc_id in self.processors.keys() {
            if processor_owners.get(proc_id).copied().unwrap_or(0) != 1 {
                return Err(Error::Validation(format!(
                    "processor {proc_id} must belong to exactly one chain"
                )));
            }
        }

        // Processors: ports exist, point back, exactly one main input +
        // one main output, sidechain ports are inputs only.
        for proc in self.processors.values() {
            let mut main_in = 0usize;
            let mut main_out = 0usize;
            for port_id in &proc.port_ids {
                let port = self
                    .ports
                    .get(port_id)
                    .ok_or_else(|| Error::NotFound(format!("port {port_id}")))?;
                if port.owner != PortOwner::Processor(proc.id) {
                    return Err(Error::WrongOwner(format!(
                        "port {port_id} does not belong to processor {}",
                        proc.id
                    )));
                }
                match (port.role, port.direction) {
                    (PortRole::Main, PortDirection::Input) => main_in += 1,
                    (PortRole::Main, PortDirection::Output) => main_out += 1,
                    (PortRole::Sidechain, PortDirection::Input) => {}
                    (PortRole::Sidechain, PortDirection::Output) => {
                        return Err(Error::Validation(format!(
                            "sidechain port {port_id} must be an input"
                        )));
                    }
                }
            }
            if main_in != 1 || main_out != 1 {
                return Err(Error::Validation(format!(
                    "processor {} needs exactly one main input and one main output (got {main_in}/{main_out})",
                    proc.id
                )));
            }
        }

        // Ports: owner exists and lists the port exactly once.
        for port in self.ports.values() {
            let list: &Vec<SemanticPortId> = match port.owner {
                PortOwner::Strip(s) => {
                    &self
                        .strips
                        .get(&s)
                        .ok_or_else(|| Error::NotFound(format!("strip {s}")))?
                        .port_ids
                }
                PortOwner::Processor(p) => {
                    &self
                        .processors
                        .get(&p)
                        .ok_or_else(|| Error::NotFound(format!("processor {p}")))?
                        .port_ids
                }
            };
            if list.iter().filter(|id| **id == port.id).count() != 1 {
                return Err(Error::WrongOwner(format!(
                    "port {} not listed exactly once by its owner",
                    port.id
                )));
            }
        }

        // Connections: endpoints exist with legal direction/domain/
        // owner/kind, Send 1:1. One (source,destination) pair may carry
        // a main/sidechain edge at most once; send edges are excluded —
        // a send is a parallel tap path (its own level/tap), so a send
        // to a destination the strip already feeds is legal, and two
        // send slots on the same pair are distinct sends.
        let mut endpoint_pairs: BTreeSet<(SemanticPortId, SemanticPortId)> = BTreeSet::new();
        for conn in self.connections.values() {
            let src = self
                .ports
                .get(&conn.source)
                .ok_or_else(|| Error::NotFound(format!("port {}", conn.source)))?;
            let dst = self
                .ports
                .get(&conn.destination)
                .ok_or_else(|| Error::NotFound(format!("port {}", conn.destination)))?;
            if src.direction != PortDirection::Output {
                return Err(Error::Validation(format!(
                    "connection {} source {} is not an output",
                    conn.id, conn.source
                )));
            }
            if dst.direction != PortDirection::Input {
                return Err(Error::Validation(format!(
                    "connection {} destination {} is not an input",
                    conn.id, conn.destination
                )));
            }
            if src.domain != dst.domain {
                return Err(Error::Validation(format!(
                    "connection {} crosses signal domains",
                    conn.id
                )));
            }
            if src.owner == dst.owner {
                return Err(Error::Validation(format!(
                    "connection {} loops inside its own owner",
                    conn.id
                )));
            }
            if !matches!(conn.kind, ConnectionKind::Send(_))
                && !endpoint_pairs.insert((conn.source, conn.destination))
            {
                return Err(Error::Validation(format!(
                    "duplicate connection {} -> {}",
                    conn.source, conn.destination
                )));
            }
            if let Some(map) = &conn.channel_map
                && map.entries.is_empty()
            {
                return Err(Error::Validation(format!(
                    "connection {} has an empty channel map",
                    conn.id
                )));
            }
            match conn.kind {
                ConnectionKind::Main => {
                    if !(matches!(src.owner, PortOwner::Strip(_))
                        && matches!(dst.owner, PortOwner::Strip(_))
                        && src.role == PortRole::Main
                        && dst.role == PortRole::Main)
                    {
                        return Err(Error::Validation(format!(
                            "main connection {} must join strip main ports",
                            conn.id
                        )));
                    }
                }
                ConnectionKind::Sidechain => {
                    if !(matches!(src.owner, PortOwner::Strip(_))
                        && src.role == PortRole::Main
                        && matches!(dst.owner, PortOwner::Processor(_))
                        && dst.role == PortRole::Sidechain)
                    {
                        return Err(Error::Validation(format!(
                            "sidechain connection {} must join a strip main output to a processor sidechain input",
                            conn.id
                        )));
                    }
                }
                ConnectionKind::Send(send_id) => {
                    let send = self
                        .sends
                        .get(&send_id)
                        .ok_or_else(|| Error::NotFound(format!("send {send_id}")))?;
                    if send.connection != conn.id {
                        return Err(Error::Validation(format!(
                            "send {send_id} does not point back at connection {}",
                            conn.id
                        )));
                    }
                    if !(matches!(src.owner, PortOwner::Strip(_))
                        && matches!(dst.owner, PortOwner::Strip(_))
                        && src.role == PortRole::Main
                        && dst.role == PortRole::Main)
                    {
                        return Err(Error::Validation(format!(
                            "send connection {} must join strip main ports",
                            conn.id
                        )));
                    }
                }
            }
        }

        // Sends: each config points at an existing connection whose kind
        // references this send — exactly 1:1, checked both directions.
        for send in self.sends.values() {
            let conn = self
                .connections
                .get(&send.connection)
                .ok_or_else(|| Error::NotFound(format!("connection {}", send.connection)))?;
            if conn.kind != ConnectionKind::Send(send.id) {
                return Err(Error::Validation(format!(
                    "send {} points at connection {} of another kind",
                    send.id, send.connection
                )));
            }
            if let Some(target) = send.gain_target
                && !project.control.targets.contains_key(&target)
            {
                return Err(Error::NotFound(format!("param {target}")));
            }
        }

        // The strip-level graph must be acyclic: a main/send/sidechain
        // edge from strip A to an entity on strip B means B's DSP reads
        // A's output. No delayed feedback in the initial model.
        self.validate_acyclic()
    }

    /// DFS cycle check over the strip graph (owner projection of every
    /// connection).
    fn validate_acyclic(&self) -> Result<(), Error> {
        let edges = self.enabled_strip_edges();
        let mut visiting: BTreeSet<StripId> = BTreeSet::new();
        let mut done: BTreeSet<StripId> = BTreeSet::new();
        for strip in self.strips.keys() {
            visit_strip(*strip, &edges, &mut visiting, &mut done)?;
        }
        Ok(())
    }

    /// The strip-level graph formed by *enabled* connections: a
    /// main/send edge from strip A to strip B — or a sidechain edge
    /// into a processor on strip B — means B's DSP reads A's output.
    /// Source ports owned by processors never leave the strip (their
    /// strip-internal feed is not a routing connection).
    #[must_use]
    pub fn enabled_strip_edges(&self) -> BTreeMap<StripId, BTreeSet<StripId>> {
        let mut edges: BTreeMap<StripId, BTreeSet<StripId>> = BTreeMap::new();
        for conn in self.connections.values() {
            if !conn.enabled {
                continue;
            }
            let (Some(src), Some(dst)) = (
                self.ports.get(&conn.source),
                self.ports.get(&conn.destination),
            ) else {
                continue;
            };
            let (Some(src_strip), Some(dst_strip)) =
                (self.strip_of_port(src), self.strip_of_port(dst))
            else {
                continue;
            };
            if src_strip != dst_strip {
                edges.entry(src_strip).or_default().insert(dst_strip);
            }
        }
        edges
    }

    /// The strip a port ultimately belongs to: a strip's own port, or
    /// the strip owning the processor through its chain.
    #[must_use]
    pub fn strip_of_port(&self, port: &SemanticPort) -> Option<StripId> {
        match port.owner {
            PortOwner::Strip(s) => Some(s),
            PortOwner::Processor(p) => self.strip_of_processor(p),
        }
    }

    /// The strip that owns `processor` through its chain.
    #[must_use]
    pub fn strip_of_processor(&self, processor_id: ProcessorId) -> Option<StripId> {
        self.chains
            .values()
            .find(|c| c.processors.contains(&processor_id))
            .and_then(|chain| self.strips.values().find(|s| s.chain == chain.id))
            .map(|s| s.id)
    }

    // -- mute/solo audibility (issue #105 / SF-02) -------------------

    /// Whether `strip`'s persisted mute is on — the `ControlOwner::Strip`
    /// + `ParameterRole::Mute` Boolean target is the single authority.
    #[must_use]
    pub fn strip_muted(&self, control: &ControlModel, strip: StripId) -> bool {
        control
            .targets
            .values()
            .find(|t| t.owner == ControlOwner::Strip(strip) && t.role == ParameterRole::Mute)
            .is_some_and(|t| t.base >= 0.5)
    }

    /// Whether `strip`'s persisted solo is on — the
    /// `ControlOwner::Strip` + `ParameterRole::Solo` Boolean target is
    /// the single authority.
    #[must_use]
    pub fn strip_soloed(&self, control: &ControlModel, strip: StripId) -> bool {
        control
            .targets
            .values()
            .find(|t| t.owner == ControlOwner::Strip(strip) && t.role == ParameterRole::Solo)
            .is_some_and(|t| t.base >= 0.5)
    }

    /// The set of strips whose signal is audible at the master under
    /// the persisted mute/solo state — a model-level evaluation over
    /// the enabled strip graph, not a runtime PCM claim (SF-03 owns
    /// the runtime projection).
    ///
    /// Policy (Issue #105): mute silences the strip's own audible
    /// output — every edge *out of* a muted strip stops carrying
    /// signal, including its send and sidechain supplies. When at
    /// least one strip is soloed, audible strips are those upstream
    /// suppliers of a soloed strip plus the strips on a path from a
    /// soloed strip to the master (return buses shared with non-solo
    /// sources stay audible; only the soloed contribution flows).
    /// Multiple solos union. A muted strip never becomes audible and
    /// never relays signal for others.
    #[must_use]
    pub fn audible_strips(&self, control: &ControlModel) -> BTreeSet<StripId> {
        let muted: BTreeSet<StripId> = self
            .strips
            .keys()
            .filter(|s| self.strip_muted(control, **s))
            .copied()
            .collect();
        let soloed: BTreeSet<StripId> = self
            .strips
            .keys()
            .filter(|s| self.strip_soloed(control, **s))
            .copied()
            .collect();
        // Edges out of a muted strip carry no audible signal.
        let edges: BTreeMap<StripId, BTreeSet<StripId>> = self
            .enabled_strip_edges()
            .into_iter()
            .filter(|(src, _)| !muted.contains(src))
            .collect();
        if soloed.is_empty() {
            return self
                .strips
                .keys()
                .filter(|s| !muted.contains(*s))
                .copied()
                .collect();
        }
        // Suppliers of a soloed strip are audible (a soloed bus plays
        // its content); strips downstream of a soloed strip are
        // audible only while they still feed the master.
        let mut audible = reach_reverse(&edges, &soloed);
        if let Some(master) = self.buses.get(&self.master) {
            let to_master = reach_reverse(&edges, &BTreeSet::from([master.strip]));
            audible.extend(
                reach_forward(&edges, &soloed)
                    .intersection(&to_master)
                    .copied(),
            );
        } else {
            audible.extend(reach_forward(&edges, &soloed));
        }
        audible.retain(|s| !muted.contains(s));
        audible
    }

    /// The channel count every semantic port declares — `Some(n)`
    /// only when the model is non-empty and uniform; `None` for an
    /// empty model or mixed declarations. Consumers use this to size
    /// the engine format a lowering runs at; a mixed or empty model
    /// has no single engine format and the lowerer fails closed
    /// either way.
    #[must_use]
    pub fn port_channel_count(&self) -> Option<u32> {
        let mut count = None;
        for port in self.ports.values() {
            match count {
                None => count = Some(port.channels),
                Some(c) if c == port.channels => {}
                Some(_) => return None,
            }
        }
        count
    }
}

/// Forward reachability over the strip graph — includes the seeds.
fn reach_forward(
    edges: &BTreeMap<StripId, BTreeSet<StripId>>,
    seeds: &BTreeSet<StripId>,
) -> BTreeSet<StripId> {
    let mut seen = seeds.clone();
    let mut queue: Vec<StripId> = seeds.iter().copied().collect();
    while let Some(strip) = queue.pop() {
        if let Some(next) = edges.get(&strip) {
            for target in next {
                if seen.insert(*target) {
                    queue.push(*target);
                }
            }
        }
    }
    seen
}

/// Reverse reachability over the strip graph — includes the seeds.
fn reach_reverse(
    edges: &BTreeMap<StripId, BTreeSet<StripId>>,
    seeds: &BTreeSet<StripId>,
) -> BTreeSet<StripId> {
    let mut reverse: BTreeMap<StripId, BTreeSet<StripId>> = BTreeMap::new();
    for (src, targets) in edges {
        for target in targets {
            reverse.entry(*target).or_default().insert(*src);
        }
    }
    reach_forward(&reverse, seeds)
}

fn visit_strip(
    strip: StripId,
    edges: &BTreeMap<StripId, BTreeSet<StripId>>,
    visiting: &mut BTreeSet<StripId>,
    done: &mut BTreeSet<StripId>,
) -> Result<(), Error> {
    if done.contains(&strip) {
        return Ok(());
    }
    if !visiting.insert(strip) {
        return Err(Error::Validation(format!("routing cycle at strip {strip}")));
    }
    if let Some(next) = edges.get(&strip) {
        for target in next {
            visit_strip(*target, edges, visiting, done)?;
        }
    }
    visiting.remove(&strip);
    done.insert(strip);
    Ok(())
}

/// A logical bus — a named route, not a device endpoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bus {
    /// Stable ID.
    pub id: BusId,
    /// Bus role.
    pub kind: BusKind,
    /// Display name.
    pub name: String,
    /// The single channel strip that carries this bus's DSP.
    pub strip: StripId,
}

/// The role of a [`Bus`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BusKind {
    /// A user-created group bus.
    Group,
    /// A return bus (send/FX return).
    Return,
    /// The single logical master bus.
    Master,
}

/// A channel strip — the DSP owner (fader/pan/insert chain/sends).
/// `Track` is the production container (lanes/clips/display); the
/// strip is the audio entity the track maps to. Bus/Return/Master shown
/// through a track template still reference this same strip — no
/// duplicate DSP is created.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelStrip {
    /// Stable ID.
    pub id: StripId,
    /// What this strip is bound to.
    pub owner: StripOwner,
    /// The strip's ordered insert chain.
    pub chain: ChainId,
    /// Owned ports, in order.
    pub port_ids: Vec<SemanticPortId>,
}

/// The entity a [`ChannelStrip`] is bound to.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "owner", content = "id", deny_unknown_fields)]
pub enum StripOwner {
    /// A production track's strip.
    Track(TrackId),
    /// A bus's strip (group/return/master).
    Bus(BusId),
}

/// An ordered processor list owned by one strip — order is meaningful
/// (insert position in the DSP chain).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessorChain {
    /// Stable ID.
    pub id: ChainId,
    /// Processors in chain order.
    pub processors: Vec<ProcessorId>,
}

/// A processor in an insert chain — a hosted plugin instance or an
/// unresolved placeholder. `name` is display-only; identity lives in
/// `binding`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Processor {
    /// Stable ID.
    pub id: ProcessorId,
    /// Display name.
    pub name: String,
    /// Plugin instance identity / binding state.
    pub binding: ProcessorBinding,
    /// Owned ports, in order.
    pub port_ids: Vec<SemanticPortId>,
}

/// Identity of a resolved plugin instance as authored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginRef {
    /// Plugin descriptor identity.
    pub descriptor: String,
    /// Plugin version as authored.
    pub version: String,
    /// Capability revision observed when bound.
    pub capability_revision: u64,
}

/// A processor's plugin binding — `Resolved` carries the authored
/// plugin identity, `Unresolved` keeps a missing plugin explicitly
/// (never silently rebound to a numerically matching plugin).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "binding", deny_unknown_fields)]
pub enum ProcessorBinding {
    /// A resolved plugin instance.
    Resolved(PluginRef),
    /// The plugin is not resolvable — kept explicitly.
    Unresolved {
        /// Last known plugin descriptor identity.
        descriptor: String,
    },
}

/// Default channel count of a [`SemanticPort`] (stereo) — the
/// `#[serde(default)]` value snapshots written before SF-02 derive.
pub const DEFAULT_PORT_CHANNELS: u32 = 2;

fn default_port_channels() -> u32 {
    DEFAULT_PORT_CHANNELS
}

/// A project semantic port — owned by a strip or a processor. Not the
/// runtime graph `PortId`: runtime IDs are reissued on every lowering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPort {
    /// Stable ID.
    pub id: SemanticPortId,
    /// Owning strip or processor.
    pub owner: PortOwner,
    /// Signal direction.
    pub direction: PortDirection,
    /// Port role within its owner.
    pub role: PortRole,
    /// Signal domain.
    pub domain: SignalDomain,
    /// Resolution/binding state.
    pub binding: PortBinding,
    /// Channel count — the persisted layout authority a connection's
    /// two ends must match unless an [`ExplicitChannelMap`] plus an
    /// approved conversion policy records the user's consent (SF-02).
    /// `#[serde(default)]`: absent in pre-SF-02 snapshots, derived as
    /// [`DEFAULT_PORT_CHANNELS`].
    #[serde(default = "default_port_channels")]
    pub channels: u32,
}

/// The entity a [`SemanticPort`] belongs to.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "owner", content = "id", deny_unknown_fields)]
pub enum PortOwner {
    /// A channel strip's port.
    Strip(StripId),
    /// A processor's port.
    Processor(ProcessorId),
}

/// Signal direction of a [`SemanticPort`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PortDirection {
    /// Consumes signal.
    Input,
    /// Produces signal.
    Output,
}

/// The role of a [`SemanticPort`] within its owner.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PortRole {
    /// The main signal path (exactly one input + one output per owner).
    Main,
    /// A sidechain key input (processors only, input only).
    Sidechain,
}

/// The signal domain a port carries.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignalDomain {
    /// Audio-rate signal. (Event/MIDI domains arrive with instrument
    /// routing — a control relation is the PC-C2 model, not a port.)
    Audio,
}

/// Resolution state of a [`SemanticPort`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "binding", deny_unknown_fields)]
pub enum PortBinding {
    /// Built-in project port (strip main I/O).
    Internal,
    /// Bound to a plugin-advertised port identity.
    PluginDeclared {
        /// Plugin instance identity (descriptor as authored).
        plugin: String,
        /// Plugin-advertised port identifier.
        advertised: String,
        /// Capability revision observed at bind time.
        capability_revision: u64,
    },
    /// The plugin or port is not resolvable — kept explicitly and never
    /// auto-rebound to a numerically matching port.
    Unresolved {
        /// Last known plugin instance identity.
        plugin: String,
        /// Last known advertised port identifier.
        advertised: String,
    },
}

/// A routing connection — the single authority for where signal goes.
/// `SendConfig` holds only the send-level settings (tap, gain binding);
/// it never carries a second destination or edge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingConnection {
    /// Stable ID.
    pub id: RoutingConnectionId,
    /// Source output port.
    pub source: SemanticPortId,
    /// Destination input port.
    pub destination: SemanticPortId,
    /// What kind of route this is.
    pub kind: ConnectionKind,
    /// Disabled connections carry no signal and form no graph edge.
    pub enabled: bool,
    /// Explicit channel mapping (required when port layouts differ).
    pub channel_map: Option<ExplicitChannelMap>,
}

/// The kind of a [`RoutingConnection`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", deny_unknown_fields)]
pub enum ConnectionKind {
    /// The strip's main output feeds a destination strip's main input.
    Main,
    /// A send tap route — strictly 1:1 with [`SendConfig`].
    Send(SendId),
    /// A key/sidechain input into a processor.
    Sidechain,
}

/// An explicit channel map: destination channel `i` reads source
/// channel `entries[i]`. Layout identity is validated by the generic
/// engine, not persisted — this records only the approved mapping.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplicitChannelMap {
    /// Destination→source channel indices (non-empty).
    pub entries: Vec<u32>,
}

/// A send's level/tap settings. Exactly 1:1 with its `kind = Send`
/// [`RoutingConnection`]. `gain_target` is a *reference* into
/// `Project::control` — the `ParameterTargetId` is the single persisted
/// authority for the send level; no second gain value lives here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendConfig {
    /// Stable ID.
    pub id: SendId,
    /// The connection carrying this send (points back with
    /// `kind = Send(id)`).
    pub connection: RoutingConnectionId,
    /// Where in the strip the send taps.
    pub tap: SendTap,
    /// Send level control target (control model is the authority).
    #[serde(default)]
    pub gain_target: Option<ParameterTargetId>,
}

/// Where a send taps the source strip.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SendTap {
    /// Pre-fader tap.
    PreFader,
    /// Post-fader tap.
    PostFader,
}
