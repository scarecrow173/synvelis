//! SF-06 (issue #109) headless slice — the Routing cross-gate Domain
//! Oracle at the project layer.
//!
//! Unlike the per-slice acceptance files (`routing_sf01.rs`, `sf_02.rs`)
//! which assert hand-picked expectations, this file keeps declarative
//! fixture topologies and re-derives every expectation with an
//! independent implementation:
//!
//! - [`AudibilityOracle`] re-implements the documented mute/solo
//!   audibility policy (SF-02 spec SF-TX-07/08; Issue #109
//!   SF-VALID-03/04) over raw persisted fields — it never calls
//!   `RoutingModel::audible_strips`/`enabled_strip_edges`/
//!   `strip_muted`/`strip_soloed`. Exhaustive mute×solo sweeps over a
//!   bus/send/sidechain topology and a chained topology (including a
//!   disabled-edge variant) compare the two field-by-field.
//! - [`routing_invariant_violations`] re-checks the persisted contract
//!   (exactly-one master, single DSP strip owner, send↔connection 1:1,
//!   direction legality, acyclicity, singleton mute/solo authority)
//!   without trusting `Project::validate_references`
//!   (Issue #109 SF-MODEL-03/04, SF-VALID-01/02).
//! - The fault matrix proves every rejected gesture — stale CAS,
//!   stale capability generation, cycle, duplicate temp IDs, illegal
//!   kinds/maps/ports, referenced deletion — leaves Live/Revision/Undo
//!   untouched (Issue #109 SF-VALID-07/08).
//! - The v5→v6 migration oracle derives the expected legacy projection
//!   from the declared fixture instead of hardcoding counts
//!   (Issue #109 SF-MODEL-05).
//!
//! Headless fixture evidence only — real OS GUI/Device/plugin and
//! consumer gates are `NotRun` (see `evidence/reports/
//! sf-06-cross-gate-ledger.md`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use synvelis_project::*;

// ---------------------------------------------------------------------------
// fixture helpers (same conventions as sf_02.rs)
// ---------------------------------------------------------------------------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn cmd(c: &ProjectController, op: ProjectOp) -> Command<ProjectOp> {
    Command {
        expected_revision: c.live_revision(),
        op,
    }
}

fn add_track(c: &mut ProjectController, name: &str) -> TrackId {
    c.apply(cmd(c, ProjectOp::AddTrack { name: name.into() }))
        .unwrap();
    *c.live().tracks.keys().next_back().unwrap()
}

fn add_track_strip(c: &mut ProjectController, track: TrackId) -> StripId {
    c.apply(cmd(c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    c.live().routing.strip_of_track(track).unwrap().id
}

fn add_bus(c: &mut ProjectController, kind: BusKind, name: &str) -> BusId {
    c.apply(cmd(
        c,
        ProjectOp::AddBus {
            kind,
            name: name.into(),
        },
    ))
    .unwrap();
    c.live()
        .routing
        .buses
        .values()
        .find(|b| b.name == name)
        .unwrap()
        .id
}

fn master_strip(p: &Project) -> StripId {
    p.routing.buses[&p.routing.master].strip
}

/// A strip's `Main`-role port in `direction`.
fn strip_main_port(p: &Project, strip: StripId, direction: PortDirection) -> SemanticPortId {
    p.routing.strips[&strip]
        .port_ids
        .iter()
        .find(|id| {
            p.routing.ports[id].direction == direction && p.routing.ports[id].role == PortRole::Main
        })
        .copied()
        .unwrap()
}

/// `PortRef` naming an existing strip's main port (intent edits).
fn strip_main(strip: StripId, direction: PortDirection) -> PortRef {
    PortRef::StripMain {
        strip: StripRef::Existing(strip),
        direction,
    }
}

/// `PortRef` naming a (possibly temporary) bus's strip main port.
fn bus_main(bus: BusRef, direction: PortDirection) -> PortRef {
    PortRef::StripMain {
        strip: StripRef::OfBus(bus),
        direction,
    }
}

fn connect_main(c: &mut ProjectController, src: StripId, dst: StripId) {
    c.apply(cmd(
        c,
        ProjectOp::AddConnection {
            source: strip_main_port(c.live(), src, PortDirection::Output),
            destination: strip_main_port(c.live(), dst, PortDirection::Input),
            kind: ConnectionKind::Main,
            channel_map: None,
        },
    ))
    .unwrap();
}

fn add_send(c: &mut ProjectController, src: StripId, dst: StripId, tap: SendTap) {
    c.apply(cmd(
        c,
        ProjectOp::AddSend {
            source: strip_main_port(c.live(), src, PortDirection::Output),
            destination: strip_main_port(c.live(), dst, PortDirection::Input),
            tap,
            gain_target: None,
        },
    ))
    .unwrap();
}

/// The strip a bus owns.
fn bus_strip(p: &Project, bus: BusId) -> StripId {
    p.routing.buses[&bus].strip
}

/// A processor's unique `(role, direction)` port.
fn processor_port(
    p: &Project,
    processor: ProcessorId,
    role: PortRole,
    direction: PortDirection,
) -> SemanticPortId {
    p.routing
        .ports
        .values()
        .find(|port| {
            port.owner == PortOwner::Processor(processor)
                && port.role == role
                && port.direction == direction
        })
        .map(|port| port.id)
        .unwrap()
}

/// Every strip ID in the model, sorted for deterministic sweeps.
fn all_strips(p: &Project) -> Vec<StripId> {
    p.routing.strips.keys().copied().collect()
}

/// Materializes the singleton mute/solo Boolean targets on every
/// strip (created on first use, base `0.0` = disengaged) so the sweep
/// can perturb persisted state directly.
fn engage_bool_targets(c: &mut ProjectController) {
    for strip in all_strips(c.live()) {
        c.apply(cmd(
            c,
            ProjectOp::SetStripMute {
                strip_id: strip,
                muted: false,
            },
        ))
        .unwrap();
        c.apply(cmd(
            c,
            ProjectOp::SetStripSolo {
                strip_id: strip,
                soloed: false,
            },
        ))
        .unwrap();
    }
}

// ---------------------------------------------------------------------------
// fixture topologies
// ---------------------------------------------------------------------------

/// The "mix" topology: four track strips, a group bus summing
/// bass+vox, a return bus fed by pre/post sends, a kick→bass
/// sidechain into a resolved compressor, and every track's auto
/// strip→master main edge retained. Seven strips total.
fn build_mix_project() -> Project {
    let mut c = controller();
    let bass = add_track(&mut c, "bass");
    let vox = add_track(&mut c, "vox");
    let keys = add_track(&mut c, "keys");
    let kick = add_track(&mut c, "kick");
    let bass_strip = add_track_strip(&mut c, bass);
    let vox_strip = add_track_strip(&mut c, vox);
    add_track_strip(&mut c, keys);
    let kick_strip = add_track_strip(&mut c, kick);
    let group = add_bus(&mut c, BusKind::Group, "drum bus");
    let ret = add_bus(&mut c, BusKind::Return, "verb");
    let group_strip = bus_strip(c.live(), group);
    let ret_strip = bus_strip(c.live(), ret);
    let master = master_strip(c.live());

    connect_main(&mut c, group_strip, master);
    connect_main(&mut c, bass_strip, group_strip);
    connect_main(&mut c, vox_strip, group_strip);
    connect_main(&mut c, ret_strip, master);
    add_send(&mut c, bass_strip, ret_strip, SendTap::PreFader);
    add_send(&mut c, vox_strip, ret_strip, SendTap::PostFader);

    // Kick → bass strip's compressor sidechain input.
    c.apply(cmd(
        &c,
        ProjectOp::InsertProcessor {
            strip_id: bass_strip,
            index: 0,
            name: "comp".into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: "vendor.comp".into(),
                version: "1.0".into(),
                capability_revision: 7,
            }),
            sidechain_input: true,
        },
    ))
    .unwrap();
    let comp = *c.live().routing.processors.keys().next_back().unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::AddConnection {
            source: strip_main_port(c.live(), kick_strip, PortDirection::Output),
            destination: processor_port(c.live(), comp, PortRole::Sidechain, PortDirection::Input),
            kind: ConnectionKind::Sidechain,
            channel_map: None,
        },
    ))
    .unwrap();

    engage_bool_targets(&mut c);
    c.live().clone()
}

/// The "chain" topology: three track strips wired a→b→c on top of their
/// auto strip→master edges — alternate paths to master. When
/// `disable_middle_main` is set, b's direct main edge to master is
/// disabled so b reaches master only through c.
fn build_chain_project(disable_middle_main: bool) -> Project {
    let mut c = controller();
    let a = add_track(&mut c, "a");
    let b = add_track(&mut c, "b");
    let d = add_track(&mut c, "c");
    let a_strip = add_track_strip(&mut c, a);
    let b_strip = add_track_strip(&mut c, b);
    let c_strip = add_track_strip(&mut c, d);
    let master = master_strip(c.live());

    connect_main(&mut c, a_strip, b_strip);
    connect_main(&mut c, b_strip, c_strip);

    if disable_middle_main {
        let b_out = strip_main_port(c.live(), b_strip, PortDirection::Output);
        let m_in = strip_main_port(c.live(), master, PortDirection::Input);
        let conn = c
            .live()
            .routing
            .connections
            .values()
            .find(|conn| {
                conn.kind == ConnectionKind::Main
                    && conn.source == b_out
                    && conn.destination == m_in
            })
            .unwrap()
            .id;
        c.apply(cmd(
            &c,
            ProjectOp::SetConnectionEnabled {
                connection_id: conn,
                enabled: false,
            },
        ))
        .unwrap();
    }

    engage_bool_targets(&mut c);
    c.live().clone()
}

// ---------------------------------------------------------------------------
// independent audibility oracle (Issue #109 SF-VALID-03/04)
// ---------------------------------------------------------------------------

/// Direction of reachability traversal over the strip graph.
#[derive(Copy, Clone)]
enum Reach {
    /// Follow edges source→destination (who the strip feeds).
    Downstream,
    /// Follow edges destination→source (who feeds the strip).
    Upstream,
}

/// Independent re-derivation of the documented audibility policy —
/// SF-02 spec SF-TX-07/08 — over raw persisted fields. Never calls
/// `RoutingModel::audible_strips`/`enabled_strip_edges`/
/// `strip_muted`/`strip_soloed`; port→strip resolution, the enabled
/// edge set, mute/solo extraction, and both BFS directions are all
/// re-implemented here from the persisted model.
struct AudibilityOracle<'p> {
    project: &'p Project,
}

impl<'p> AudibilityOracle<'p> {
    fn new(project: &'p Project) -> Self {
        Self { project }
    }

    /// The strip owning `processor` — resolved by scanning chains and
    /// strips directly, not via `strip_of_processor`.
    fn strip_of_processor(&self, processor: ProcessorId) -> Option<StripId> {
        let chain = self
            .project
            .routing
            .chains
            .values()
            .find(|chain| chain.processors.contains(&processor))?;
        self.project
            .routing
            .strips
            .values()
            .find(|strip| strip.chain == chain.id)
            .map(|strip| strip.id)
    }

    /// The strip a port ultimately belongs to.
    fn strip_of_port(&self, port_id: SemanticPortId) -> Option<StripId> {
        match self.project.routing.ports.get(&port_id)?.owner {
            PortOwner::Strip(strip) => Some(strip),
            PortOwner::Processor(processor) => self.strip_of_processor(processor),
        }
    }

    /// Boolean mixer authority: the `ControlOwner::Strip` + `role`
    /// target's persisted `base` — `>= 0.5` reads as engaged. Scans
    /// `control.targets` directly (first match, same as the persisted
    /// singleton contract; duplicates are a separate invariant
    /// violation).
    fn engaged(control: &ControlModel, strip: StripId, role: ParameterRole) -> bool {
        control
            .targets
            .values()
            .find(|t| t.owner == ControlOwner::Strip(strip) && t.role == role)
            .is_some_and(|t| t.base >= 0.5)
    }

    fn muted_set(&self, control: &ControlModel) -> BTreeSet<StripId> {
        self.project
            .routing
            .strips
            .keys()
            .filter(|strip| Self::engaged(control, **strip, ParameterRole::Mute))
            .copied()
            .collect()
    }

    fn soloed_set(&self, control: &ControlModel) -> BTreeSet<StripId> {
        self.project
            .routing
            .strips
            .keys()
            .filter(|strip| Self::engaged(control, **strip, ParameterRole::Solo))
            .copied()
            .collect()
    }

    /// Enabled strip-level edges with muted *sources* removed — the
    /// "mute silences every edge out of the strip, including send and
    /// sidechain supplies" rule (SF-TX-07). Disabled connections and
    /// unresolvable/self edges form no edge.
    fn edges(&self, muted: &BTreeSet<StripId>) -> BTreeMap<StripId, BTreeSet<StripId>> {
        let mut edges: BTreeMap<StripId, BTreeSet<StripId>> = BTreeMap::new();
        for conn in self.project.routing.connections.values() {
            if !conn.enabled {
                continue;
            }
            let (Some(src), Some(dst)) = (
                self.strip_of_port(conn.source),
                self.strip_of_port(conn.destination),
            ) else {
                continue;
            };
            if src == dst || muted.contains(&src) {
                continue;
            }
            edges.entry(src).or_default().insert(dst);
        }
        edges
    }

    /// BFS reachability over the strip graph; seeds are included.
    fn reach(
        edges: &BTreeMap<StripId, BTreeSet<StripId>>,
        seeds: &BTreeSet<StripId>,
        direction: Reach,
    ) -> BTreeSet<StripId> {
        // Materialize the traversed adjacency once per call — the
        // upstream direction walks the transpose.
        let adjacency: BTreeMap<StripId, BTreeSet<StripId>> = match direction {
            Reach::Downstream => edges.clone(),
            Reach::Upstream => {
                let mut transpose: BTreeMap<StripId, BTreeSet<StripId>> = BTreeMap::new();
                for (src, dsts) in edges {
                    for dst in dsts {
                        transpose.entry(*dst).or_default().insert(*src);
                    }
                }
                transpose
            }
        };
        let mut seen = seeds.clone();
        let mut queue: VecDeque<StripId> = seeds.iter().copied().collect();
        while let Some(strip) = queue.pop_front() {
            if let Some(next) = adjacency.get(&strip) {
                for target in next {
                    if seen.insert(*target) {
                        queue.push_back(*target);
                    }
                }
            }
        }
        seen
    }

    /// The documented policy, derived independently:
    ///
    /// - No solo: every non-muted strip is audible.
    /// - Solo (non-empty, union over multiple solos): audible strips are
    ///   the upstream suppliers of a soloed strip (its signal sources,
    ///   including sidechain feeders and — for a soloed bus — its
    ///   content suppliers) plus the strips downstream of a soloed
    ///   strip that still reach the master strip (return/master are
    ///   never cut unconditionally). Muted strips are removed last —
    ///   they never become audible and never relay.
    fn audible(&self, control: &ControlModel) -> BTreeSet<StripId> {
        let muted = self.muted_set(control);
        let soloed = self.soloed_set(control);
        if soloed.is_empty() {
            return self
                .project
                .routing
                .strips
                .keys()
                .filter(|strip| !muted.contains(*strip))
                .copied()
                .collect();
        }
        let edges = self.edges(&muted);
        let mut audible = Self::reach(&edges, &soloed, Reach::Upstream);
        if let Some(master) = self.project.routing.buses.get(&self.project.routing.master) {
            let to_master = Self::reach(&edges, &BTreeSet::from([master.strip]), Reach::Upstream);
            audible.extend(
                Self::reach(&edges, &soloed, Reach::Downstream)
                    .intersection(&to_master)
                    .copied(),
            );
        } else {
            audible.extend(Self::reach(&edges, &soloed, Reach::Downstream));
        }
        audible.retain(|strip| !muted.contains(strip));
        audible
    }
}

/// Exhaustive mute×solo sweep: every persisted Boolean combination is
/// applied to a cloned [`ControlModel`] and the SUT's
/// `audible_strips` is compared against the oracle's independent
/// derivation.
fn sweep_audibility(project: &Project, label: &str) {
    let strips = all_strips(project);
    let n = strips.len();
    assert!(n <= 8, "{label}: sweep width {n} exceeds u64 mask budget");

    // Locate the singleton authority targets once; the invariant
    // sweep separately proves there is at most one per (strip, role).
    let mut mute_ids: BTreeMap<StripId, ParameterTargetId> = BTreeMap::new();
    let mut solo_ids: BTreeMap<StripId, ParameterTargetId> = BTreeMap::new();
    for target in project.control.targets.values() {
        if let ControlOwner::Strip(strip) = target.owner {
            match target.role {
                ParameterRole::Mute => {
                    assert!(
                        mute_ids.insert(strip, target.id).is_none(),
                        "{label}: duplicate mute authority on {strip}"
                    );
                }
                ParameterRole::Solo => {
                    assert!(
                        solo_ids.insert(strip, target.id).is_none(),
                        "{label}: duplicate solo authority on {strip}"
                    );
                }
                _ => {}
            }
        }
    }
    for strip in &strips {
        assert!(
            mute_ids.contains_key(strip) && solo_ids.contains_key(strip),
            "{label}: strip {strip} lacks materialized mute/solo targets"
        );
    }

    let oracle = AudibilityOracle::new(project);
    let combos = 1u64 << (2 * n);
    for mask in 0..combos {
        let mut control = project.control.clone();
        for (i, strip) in strips.iter().enumerate() {
            control.targets.get_mut(&mute_ids[strip]).unwrap().base =
                if (mask >> i) & 1 == 1 { 1.0 } else { 0.0 };
            control.targets.get_mut(&solo_ids[strip]).unwrap().base =
                if (mask >> (n + i)) & 1 == 1 { 1.0 } else { 0.0 };
        }
        let observed = project.routing.audible_strips(&control);
        let expected = oracle.audible(&control);
        assert_eq!(
            observed, expected,
            "{label}: audibility diverged at mute/solo mask {mask:016b}"
        );
    }
}

// ---------------------------------------------------------------------------
// persisted-model invariant oracle (Issue #109 SF-MODEL-03/04,
// SF-VALID-01/02)
// ---------------------------------------------------------------------------

/// DFS cycle detector for the strip-level enabled-edge graph used by
/// [`routing_invariant_violations`]. `state`: 0=unvisited, 1=open,
/// 2=done; revisiting an open node reports a cycle.
fn visit_for_cycle(
    strip: StripId,
    edges: &BTreeMap<StripId, BTreeSet<StripId>>,
    state: &mut BTreeMap<StripId, u8>,
    violations: &mut Vec<String>,
) {
    match state.get(&strip).copied().unwrap_or(0) {
        2 => return,
        1 => {
            violations.push(format!("routing cycle at strip {strip}"));
            return;
        }
        _ => {}
    }
    state.insert(strip, 1);
    if let Some(next) = edges.get(&strip) {
        for target in next {
            visit_for_cycle(*target, edges, state, violations);
        }
    }
    state.insert(strip, 2);
}

/// Re-checks the documented persisted routing contract from raw
/// fields — the same invariants `Project::validate_references`
/// enforces, re-derived independently so a validator blind spot
/// cannot hide behind the oracle that shares it. Returns human-readable
/// violations; empty means the model satisfies the contract.
#[allow(clippy::too_many_lines)] // sequential contract checks read top-to-bottom
fn routing_invariant_violations(p: &Project) -> Vec<String> {
    let mut violations = Vec::new();
    let r = &p.routing;

    // Map key == entity id for every persisted map.
    for (key, bus) in &r.buses {
        if *key != bus.id {
            violations.push(format!("bus {} stored under foreign key {key}", bus.id));
        }
    }
    for (key, strip) in &r.strips {
        if *key != strip.id {
            violations.push(format!("strip {} stored under foreign key {key}", strip.id));
        }
    }
    for (key, proc) in &r.processors {
        if *key != proc.id {
            violations.push(format!(
                "processor {} stored under foreign key {key}",
                proc.id
            ));
        }
    }
    for (key, chain) in &r.chains {
        if *key != chain.id {
            violations.push(format!("chain {} stored under foreign key {key}", chain.id));
        }
    }
    for (key, port) in &r.ports {
        if *key != port.id {
            violations.push(format!("port {} stored under foreign key {key}", port.id));
        }
    }
    for (key, conn) in &r.connections {
        if *key != conn.id {
            violations.push(format!(
                "connection {} stored under foreign key {key}",
                conn.id
            ));
        }
    }
    for (key, send) in &r.sends {
        if *key != send.id {
            violations.push(format!("send {} stored under foreign key {key}", send.id));
        }
    }

    // Exactly one master bus, and `routing.master` names it.
    let master_count = r
        .buses
        .values()
        .filter(|b| b.kind == BusKind::Master)
        .count();
    if master_count != 1 {
        violations.push(format!(
            "expected exactly one master bus, found {master_count}"
        ));
    }
    match r.buses.get(&r.master) {
        Some(bus) if bus.kind == BusKind::Master => {}
        _ => violations.push(format!("routing.master {} is not a master bus", r.master)),
    }

    // Bus ↔ strip: the named strip exists, is bus-owned by this bus,
    // and no other strip claims the same bus (single DSP owner).
    for bus in r.buses.values() {
        match r.strips.get(&bus.strip) {
            Some(strip) if strip.owner == StripOwner::Bus(bus.id) => {}
            _ => violations.push(format!(
                "bus {} strip {} missing or foreign-owned",
                bus.id, bus.strip
            )),
        }
        let owned = r
            .strips
            .values()
            .filter(|s| s.owner == StripOwner::Bus(bus.id))
            .count();
        if owned != 1 {
            violations.push(format!(
                "bus {} has {owned} owned strips, expected 1",
                bus.id
            ));
        }
    }

    // Strips: owner resolves, chain exists, ports exist and
    // back-reference the strip, exactly one Main input + one Main
    // output per strip.
    for strip in r.strips.values() {
        match strip.owner {
            StripOwner::Track(track) => {
                if !p.tracks.contains_key(&track) {
                    violations.push(format!("strip {} owned by missing track {track}", strip.id));
                }
            }
            StripOwner::Bus(bus) => {
                if !r.buses.contains_key(&bus) {
                    violations.push(format!("strip {} owned by missing bus {bus}", strip.id));
                }
            }
        }
        if !r.chains.contains_key(&strip.chain) {
            violations.push(format!("strip {} references missing chain", strip.id));
        }
        let mut main_in = 0usize;
        let mut main_out = 0usize;
        for port_id in &strip.port_ids {
            match r.ports.get(port_id) {
                Some(port) if port.owner == PortOwner::Strip(strip.id) => {
                    if port.role == PortRole::Main {
                        match port.direction {
                            PortDirection::Input => main_in += 1,
                            PortDirection::Output => main_out += 1,
                        }
                    }
                }
                _ => violations.push(format!(
                    "strip {} port {port_id} missing or foreign-owned",
                    strip.id
                )),
            }
        }
        if main_in != 1 || main_out != 1 {
            violations.push(format!(
                "strip {} has {main_in} main inputs / {main_out} main outputs, expected 1/1",
                strip.id
            ));
        }
    }

    // Chains/processors: chain members exist, each processor lives in
    // at most one chain, processor ports back-reference their owner.
    let mut processor_chain: BTreeMap<ProcessorId, ChainId> = BTreeMap::new();
    for chain in r.chains.values() {
        for processor in &chain.processors {
            if !r.processors.contains_key(processor) {
                violations.push(format!(
                    "chain {} references missing processor {processor}",
                    chain.id
                ));
            }
            if processor_chain.insert(*processor, chain.id).is_some() {
                violations.push(format!("processor {processor} appears in two chains"));
            }
        }
    }
    for proc in r.processors.values() {
        for port_id in &proc.port_ids {
            match r.ports.get(port_id) {
                Some(port) if port.owner == PortOwner::Processor(proc.id) => {}
                _ => violations.push(format!(
                    "processor {} port {port_id} missing or foreign-owned",
                    proc.id
                )),
            }
        }
    }

    // Ports: owner resolves; sidechain ports are inputs only.
    for port in r.ports.values() {
        let owner_ok = match port.owner {
            PortOwner::Strip(strip) => r.strips.contains_key(&strip),
            PortOwner::Processor(proc) => r.processors.contains_key(&proc),
        };
        if !owner_ok {
            violations.push(format!("port {} has dangling owner", port.id));
        }
        if port.role == PortRole::Sidechain && port.direction != PortDirection::Input {
            violations.push(format!("sidechain port {} is not an input", port.id));
        }
    }

    // Connections: endpoints exist, direction is output→input, and a
    // non-send edge is unique per (source, destination) pair.
    let mut send_edges: BTreeMap<SendId, RoutingConnectionId> = BTreeMap::new();
    let mut plain_edges: BTreeSet<(SemanticPortId, SemanticPortId)> = BTreeSet::new();
    for conn in r.connections.values() {
        let endpoints = match (r.ports.get(&conn.source), r.ports.get(&conn.destination)) {
            (Some(src), Some(dst)) => Some((src.direction, dst.direction)),
            _ => None,
        };
        match endpoints {
            Some((PortDirection::Output, PortDirection::Input)) => {}
            Some(_) => violations.push(format!("connection {} is not output→input", conn.id)),
            None => violations.push(format!("connection {} has a dangling endpoint", conn.id)),
        }
        match conn.kind {
            ConnectionKind::Send(send_id) => {
                send_edges.insert(send_id, conn.id);
                match r.sends.get(&send_id) {
                    Some(send) if send.connection == conn.id => {}
                    _ => violations.push(format!(
                        "send-kind connection {} lacks its SendConfig back-reference",
                        conn.id
                    )),
                }
            }
            _ => {
                if !plain_edges.insert((conn.source, conn.destination)) {
                    violations.push(format!(
                        "duplicate main/sidechain edge {}→{}",
                        conn.source, conn.destination
                    ));
                }
            }
        }
    }

    // Send ↔ connection strict 1:1 in both directions.
    if r.sends.len() != send_edges.len() {
        violations.push(format!(
            "{} sends vs {} send-kind connections",
            r.sends.len(),
            send_edges.len()
        ));
    }
    for send in r.sends.values() {
        match r.connections.get(&send.connection) {
            Some(conn) if conn.kind == ConnectionKind::Send(send.id) => {}
            _ => violations.push(format!(
                "send {} points at a non-send or missing connection",
                send.id
            )),
        }
        if let Some(target) = send.gain_target
            && !p.control.targets.contains_key(&target)
        {
            violations.push(format!("send {} gain_target {target} is dangling", send.id));
        }
    }

    // The enabled strip graph must be acyclic (own DFS, independent of
    // `validate_acyclic`).
    let oracle = AudibilityOracle::new(p);
    let mut edges: BTreeMap<StripId, BTreeSet<StripId>> = BTreeMap::new();
    for conn in r.connections.values() {
        if !conn.enabled {
            continue;
        }
        if let (Some(src), Some(dst)) = (
            oracle.strip_of_port(conn.source),
            oracle.strip_of_port(conn.destination),
        ) && src != dst
        {
            edges.entry(src).or_default().insert(dst);
        }
    }
    let mut state: BTreeMap<StripId, u8> = BTreeMap::new(); // 0=unvisited 1=open 2=done
    for strip in r.strips.keys() {
        visit_for_cycle(*strip, &edges, &mut state, &mut violations);
    }

    // Singleton mixer authority: at most one Mute and one Solo
    // Boolean target per strip (the documented "one per strip" role
    // contract `strip_muted`/`strip_soloed` rely on).
    let mut bool_roles: BTreeMap<(StripId, &'static str), usize> = BTreeMap::new();
    for target in p.control.targets.values() {
        if let ControlOwner::Strip(strip) = target.owner {
            let role_name = match target.role {
                ParameterRole::Mute => "Mute",
                ParameterRole::Solo => "Solo",
                _ => continue,
            };
            *bool_roles.entry((strip, role_name)).or_default() += 1;
        }
    }
    for ((strip, role), count) in bool_roles {
        if count > 1 {
            violations.push(format!("strip {strip} has {count} {role} targets"));
        }
    }

    violations
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

/// Issue #109 SF-VALID-03/04 — audibility oracle sweep over the mix
/// topology (group bus, return bus, pre/post sends, sidechain,
/// muted relays, shared return, master strip edge cases).
#[test]
fn sf_valid_03_04_audibility_oracle_mix_topology() {
    let project = build_mix_project();
    assert_eq!(all_strips(&project).len(), 7);
    sweep_audibility(&project, "mix");
}

/// Issue #109 SF-VALID-03/04 — chain topology with alternate paths to
/// master, run once with all edges enabled and once with the middle
/// strip's direct main edge disabled.
#[test]
fn sf_valid_03_04_audibility_oracle_chain_topology() {
    sweep_audibility(&build_chain_project(false), "chain");
    sweep_audibility(&build_chain_project(true), "chain-disabled");
}

/// Issue #109 SF-MODEL-03/04 + SF-VALID-01/02 — the persisted model
/// satisfies every documented invariant, checked independently of
/// `validate_references`.
#[test]
fn sf_model_valid_persisted_invariants_hold() {
    for (label, project) in [
        ("fresh", Project::new(ProjectId::new(0), "p")),
        ("mix", build_mix_project()),
        ("chain", build_chain_project(false)),
        ("chain-disabled", build_chain_project(true)),
    ] {
        assert_oracle_clean(&project, label);
        assert!(project.validate_references().is_ok(), "{label}");
    }
}

/// The invariant oracle must actually detect corruption — an oracle
/// that cannot fail proves nothing.
#[test]
fn sf_model_valid_oracle_detects_corruption() {
    let project = build_mix_project();
    assert_oracle_clean(&project, "clean");

    // Orphaned send: drop the send-kind connection, keep the config.
    let mut bad = project.clone();
    let send_id = *bad.routing.sends.keys().next().unwrap();
    let conn_id = bad.routing.sends[&send_id].connection;
    bad.routing.connections.remove(&conn_id);
    assert_oracle_dirty(&bad, "orphaned-send");

    // Second master-kind bus.
    let mut bad = project.clone();
    let extra = BusId::new(4_242_424);
    let strip = *bad.routing.strips.keys().next().unwrap();
    bad.routing.buses.insert(
        extra,
        Bus {
            id: extra,
            kind: BusKind::Master,
            name: "impostor".into(),
            strip,
        },
    );
    assert_oracle_dirty(&bad, "second-master");

    // Duplicate main/sidechain edge under a fresh ID.
    let mut bad = project.clone();
    let conn = bad
        .routing
        .connections
        .values()
        .find(|conn| matches!(conn.kind, ConnectionKind::Main))
        .unwrap()
        .clone();
    let new_id = RoutingConnectionId::new(9_999_999);
    bad.routing
        .connections
        .insert(new_id, RoutingConnection { id: new_id, ..conn });
    assert!(
        routing_invariant_violations(&bad)
            .iter()
            .any(|v| v.contains("duplicate"))
    );

    // Duplicate mute authority on one strip.
    let mut bad = project.clone();
    let strip = *bad.routing.strips.keys().next().unwrap();
    let dup = ParameterTargetId::new(8_888_888);
    bad.control.targets.insert(
        dup,
        ParameterTarget {
            id: dup,
            owner: ControlOwner::Strip(strip),
            role: ParameterRole::Mute,
            time_scope: AutomationTimeScope::ProjectMusical,
            name: "mute-2".into(),
            kind: ValueKind::Boolean,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.0,
        },
    );
    assert!(
        routing_invariant_violations(&bad)
            .iter()
            .any(|v| v.contains("Mute"))
    );

    // Dangling connection endpoint.
    let mut bad = project.clone();
    let conn_id = *bad.routing.connections.keys().next().unwrap();
    bad.routing.connections.get_mut(&conn_id).unwrap().source = SemanticPortId::new(7_777_777);
    assert_oracle_dirty(&bad, "dangling-endpoint");
}

/// Asserts the invariant oracle reports no violations, printing them
/// on failure.
fn assert_oracle_clean(project: &Project, label: &str) {
    let violations = routing_invariant_violations(project);
    assert!(violations.is_empty(), "{label}: {violations:?}");
}

/// Asserts the invariant oracle reports at least one violation —
/// corruption the oracle must not miss.
fn assert_oracle_dirty(project: &Project, label: &str) {
    let violations = routing_invariant_violations(project);
    assert!(!violations.is_empty(), "{label}: oracle missed corruption");
}

/// (live revision, serialized live project) — the observable state a
/// rejected gesture must leave untouched.
fn state_snapshot(c: &ProjectController) -> (ProjectRevision, serde_json::Value) {
    (c.live_revision(), serde_json::to_value(c.live()).unwrap())
}

/// Asserts `result` is an error and the controller state is
/// byte-identical to `before` — the "failure leaves Live/Revision
/// invariant" half of SF-VALID-07/08.
fn expect_reject(
    c: &ProjectController,
    result: &Result<ProjectRevision, Error>,
    before: &(ProjectRevision, serde_json::Value),
    label: &str,
) {
    assert!(result.is_err(), "{label}: expected rejection");
    assert_eq!(
        &state_snapshot(c),
        before,
        "{label}: state mutated on reject"
    );
}

/// Issue #109 SF-VALID-07/08 — every rejected gesture leaves the live
/// project, revision, and undo history untouched.
#[test]
#[allow(clippy::too_many_lines)] // sequential fault rows read top-to-bottom
fn sf_valid_07_08_fault_matrix_preserves_state() {
    let mut c = controller();
    let bass = add_track(&mut c, "bass");
    let vox = add_track(&mut c, "vox");
    let bass_strip = add_track_strip(&mut c, bass);
    let vox_strip = add_track_strip(&mut c, vox);
    let group = add_bus(&mut c, BusKind::Group, "bus");
    let group_strip = bus_strip(c.live(), group);
    let master = master_strip(c.live());
    connect_main(&mut c, bass_strip, group_strip);
    connect_main(&mut c, group_strip, master);
    add_send(&mut c, vox_strip, master, SendTap::PostFader);
    let existing_send = *c.live().routing.sends.keys().next().unwrap();

    let before = state_snapshot(&c);

    // Stale CAS on a plain command.
    let result = c.apply(Command {
        expected_revision: ProjectRevision(before.0.0 + 99),
        op: ProjectOp::SetProjectName {
            name: "stale".into(),
        },
    });
    // (expected_revision only needs to differ — any stale value rejects)
    expect_reject(&c, &result, &before, "stale-revision");

    // Stale port-capability generation on an intent.
    let stale_cap = RoutingEditIntent {
        operation_id: "op-stale-cap".into(),
        expected_port_capability_generation: port_capability_generation(c.live()) + 1,
        edits: vec![RoutingEdit::SetMute {
            strip: StripRef::Existing(bass_strip),
            muted: true,
        }],
        approved_conversion: ExplicitConversionPolicy::None,
    };
    let result = c.apply(cmd(&c, ProjectOp::Routing { intent: stale_cap }));
    expect_reject(&c, &result, &before, "stale-capability");

    // Feedback cycle through the group bus (typed rejection).
    let cycle = RoutingEditIntent {
        operation_id: "op-cycle".into(),
        expected_port_capability_generation: port_capability_generation(c.live()),
        edits: vec![RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(1),
            source: bus_main(BusRef::Existing(group), PortDirection::Output),
            destination: strip_main(bass_strip, PortDirection::Input),
            kind: ConnectionKind::Main,
            channel_map: None,
        }],
        approved_conversion: ExplicitConversionPolicy::None,
    };
    let err = c
        .apply(cmd(&c, ProjectOp::Routing { intent: cycle }))
        .unwrap_err();
    assert!(err.to_string().contains("routing-cycle"), "{err}");
    assert_eq!(
        &state_snapshot(&c),
        &before,
        "cycle: state mutated on reject"
    );

    // Duplicate temporary IDs inside one intent.
    let dup_temp = RoutingEditIntent {
        operation_id: "op-dup".into(),
        expected_port_capability_generation: port_capability_generation(c.live()),
        edits: vec![
            RoutingEdit::CreateBus {
                temp_id: TempBusId(1),
                kind: BusKind::Group,
                name: "x".into(),
            },
            RoutingEdit::CreateBus {
                temp_id: TempBusId(1),
                kind: BusKind::Group,
                name: "y".into(),
            },
        ],
        approved_conversion: ExplicitConversionPolicy::None,
    };
    let err = c
        .apply(cmd(&c, ProjectOp::Routing { intent: dup_temp }))
        .unwrap_err();
    assert!(err.to_string().contains("routing-"), "{err}");
    assert_eq!(
        &state_snapshot(&c),
        &before,
        "dup-temp: state mutated on reject"
    );

    // A `Send`-kind connection cannot be minted outside `AddSend`.
    let result = c.apply(cmd(
        &c,
        ProjectOp::AddConnection {
            source: strip_main_port(c.live(), vox_strip, PortDirection::Output),
            destination: strip_main_port(c.live(), master, PortDirection::Input),
            kind: ConnectionKind::Send(existing_send),
            channel_map: None,
        },
    ));
    expect_reject(&c, &result, &before, "send-via-addconnection");

    // An empty explicit channel map is malformed — target a fresh
    // edge (vox input) so the rejection is the map, not a duplicate.
    let empty_map = RoutingEditIntent {
        operation_id: "op-map".into(),
        expected_port_capability_generation: port_capability_generation(c.live()),
        edits: vec![RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(2),
            source: strip_main(bass_strip, PortDirection::Output),
            destination: strip_main(vox_strip, PortDirection::Input),
            kind: ConnectionKind::Main,
            channel_map: Some(ExplicitChannelMap { entries: vec![] }),
        }],
        approved_conversion: ExplicitConversionPolicy::None,
    };
    let result = c.apply(cmd(&c, ProjectOp::Routing { intent: empty_map }));
    expect_reject(&c, &result, &before, "empty-channel-map");

    // A second master bus cannot be created.
    let result = c.apply(cmd(
        &c,
        ProjectOp::AddBus {
            kind: BusKind::Master,
            name: "master2".into(),
        },
    ));
    expect_reject(&c, &result, &before, "second-master");

    // Track deletion with live references rejects.
    let result = c.apply(cmd(&c, ProjectOp::RemoveTrack { track_id: bass }));
    expect_reject(&c, &result, &before, "referenced-delete");

    // A successful op after the fault battery still undoes to the
    // pre-fault project content — no phantom undo entries were
    // recorded. `undo` issues a new live revision by design, so the
    // revision advances by exactly the apply+undo pair while the
    // serialized project returns to the pre-fault bytes.
    c.apply(cmd(
        &c,
        ProjectOp::SetProjectName {
            name: "post-fault".into(),
        },
    ))
    .unwrap();
    c.undo().unwrap();
    let after = state_snapshot(&c);
    assert_eq!(after.0, ProjectRevision(before.0.0 + 2));
    assert_eq!(after.1, before.1, "undo did not restore pre-fault project");
}

/// Issue #109 SF-MODEL-05 — the v5→v6 migration oracle derives the
/// expected legacy projection (one strip per track + master, direct
/// Main edges to master, no fabricated processors) from the declared
/// fixture, and re-migration is deterministic.
#[test]
fn sf_model_05_v5_migration_projection_matches_declaration() {
    /// Serializes `project` as a schema-v5 envelope — the `routing`
    /// field and every `Strip`-owned control target (the bootstrap
    /// master fader plus the CTL-02 strip channel targets) did not
    /// exist in v5, so they are omitted here and re-seeded by the
    /// later migration steps on load (same construction as
    /// `routing_sf01.rs`).
    fn as_v5_envelope(c: &ProjectController) -> serde_json::Value {
        let mut env = serde_json::json!({
            "schema_version": 5,
            "project_id": c.live().project_id,
            "revision": c.live_revision(),
            "project": serde_json::to_value(c.live()).unwrap(),
        });
        env["project"].as_object_mut().unwrap().remove("routing");
        let targets = env["project"]["control"]["targets"]
            .as_object_mut()
            .unwrap();
        targets.remove(&ParameterTargetId::new(MASTER_FADER_RAW).to_tagged());
        let strip_owned: Vec<String> = targets
            .iter()
            .filter(|(_, t)| t["owner"]["owner"] == "Strip")
            .map(|(k, _)| k.clone())
            .collect();
        for key in &strip_owned {
            targets.remove(key);
        }
        if !strip_owned.is_empty() {
            let lanes = env["project"]["control"]["lanes"].as_object_mut().unwrap();
            lanes.retain(|_, l| {
                l["target"]
                    .as_str()
                    .is_none_or(|t| !strip_owned.iter().any(|k| k == t))
            });
            let assignments = env["project"]["control"]["assignments"]
                .as_object_mut()
                .unwrap();
            assignments.retain(|_, a| {
                [a["target"].as_str(), a["depth_source"].as_str()]
                    .into_iter()
                    .flatten()
                    .all(|t| !strip_owned.iter().any(|k| k == t))
            });
        }
        env
    }

    const TRACKS: usize = 3;
    let mut c = controller();
    for name in ["a", "b", "c"] {
        add_track(&mut c, name);
    }
    assert_eq!(c.live().tracks.len(), TRACKS);
    let saved_rev = c.live_revision();
    let mut store = InMemoryStore::new();
    store
        .save(
            "p",
            saved_rev,
            &serde_json::to_vec(&as_v5_envelope(&c)).unwrap(),
        )
        .unwrap();

    let mut c2 = controller();
    c2.load(&store).unwrap();
    let r = &c2.live().routing;

    // Oracle-derived expectations: exactly-one master, one strip per
    // declared track plus the master strip, one Main edge per track
    // into the master input, nothing fabricated.
    assert_eq!(
        r.buses
            .values()
            .filter(|b| b.kind == BusKind::Master)
            .count(),
        1
    );
    assert_eq!(r.strips.len(), TRACKS + 1);
    assert_eq!(r.connections.len(), TRACKS);
    assert!(r.processors.is_empty() && r.sends.is_empty());
    let master = master_strip(c2.live());
    let master_in = strip_main_port(c2.live(), master, PortDirection::Input);
    for track in c2.live().tracks.keys() {
        let strip = r.strip_of_track(*track).unwrap();
        let out = strip_main_port(c2.live(), strip.id, PortDirection::Output);
        assert!(
            r.connections.values().any(|conn| {
                conn.kind == ConnectionKind::Main
                    && conn.source == out
                    && conn.destination == master_in
            }),
            "track {track} lacks its direct-to-master Main edge"
        );
    }
    // The migrated load is one implicit revision step so a save can
    // never overwrite the v5 body.
    assert_eq!(c2.live_revision(), ProjectRevision(saved_rev.0 + 1));
    assert!(c2.live().validate_references().is_ok());

    // Deterministic re-migration: the same v5 body always projects
    // identically, and saving publishes a new snapshot head.
    let mut c3 = controller();
    c3.load(&store).unwrap();
    assert_eq!(c2.live(), c3.live());
    let head_before = store.head().unwrap().unwrap();
    let saved = c2.save(&mut store).unwrap();
    assert_ne!(saved.head, head_before);
}
