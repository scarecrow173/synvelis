//! RCP-04 headless oracle tests — `docs/test-specs/rcp-04-missing-plugin-resolver.md`.
//!
//! Every case is a deterministic fixture: pinned snapshots, fake
//! capability environments, and declared render candidates. These
//! tests prove the *decision contract* (selection verdicts, cutset
//! rejections, edit classification, stale-plan rejection, last-good
//! retention). They are explicitly NOT evidence for real-plugin,
//! OS, RT, or PCM acceptance — that remains RCP-GATE / synvelis#296.
//!
//! Port-id convention: input ports use ids 0,1,2…; output ports use
//! ids 10,11,12… — ids are unique within a node so `PortEndpoint`s
//! are unambiguous.

use std::collections::{BTreeMap, BTreeSet};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_graph::{
    Activation, Availability, Direction, EdgeDelay, Multiplicity, PortFormat, PortRole,
    SignalDomain,
};
use synvelis_player::rcp::{
    AccessState, ArtifactEligibility, ArtifactOutput, BoundaryPurpose, BoundaryRequirement,
    CutsetRejection, DeclaredPort, DeliveryDecision, EdgeKind, EditClass, EditKind,
    ExecutionIntent, ExecutionSelector, NodeClass, PlanReconciler, PluginCapabilitySnapshot,
    PluginDeclaration, PluginEnvironment, PluginFormat, PluginIdentity, PluginKey, PluginStateDecl,
    PluginVerdict, PortEndpoint, RcpEdgeId, RcpNodeId, RcpPortId, RenderCandidate,
    RenderCandidateIndex, ReproducibilityClass, ResolvedEdge, ResolvedGraphSnapshot, ResolvedNode,
    ResolvedPort, SelectionVerdict, StateRestoreSupport, classify_edit, validate_cutset,
};
use synvelis_project::{
    OpaqueStateStatus, ProjectId, ProjectRevision, RoutingActivationState, RoutingFault,
    StateCaptureQuality,
};

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

const OUT_BASE: u32 = 10;

fn audio_format() -> PortFormat {
    PortFormat::Audio(AudioFormat {
        sample_rate: SampleRate::new(48_000).expect("rate"),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    })
}

fn mkport(
    id: u32,
    direction: Direction,
    role: PortRole,
    format: PortFormat,
    min: u16,
) -> ResolvedPort {
    ResolvedPort {
        id: RcpPortId(id),
        direction,
        role,
        format,
        multiplicity: Multiplicity::new(min, Multiplicity::UNBOUNDED).expect("bounds"),
        availability: Availability::Always,
        activation: Activation::Enabled,
    }
}

fn audio_in(id: u32) -> ResolvedPort {
    mkport(id, Direction::Input, PortRole::Main, audio_format(), 0)
}

fn required_in(id: u32) -> ResolvedPort {
    mkport(id, Direction::Input, PortRole::Main, audio_format(), 1)
}

fn audio_out(id: u32) -> ResolvedPort {
    mkport(
        OUT_BASE + id,
        Direction::Output,
        PortRole::Main,
        audio_format(),
        0,
    )
}

fn node(id: u64, class: NodeClass, ports: Vec<ResolvedPort>) -> ResolvedNode {
    ResolvedNode {
        id: RcpNodeId(id),
        class,
        ports,
        plugin: None,
        latency_frames: 0,
        label: None,
    }
}

/// Source: one audio output at port `OUT_BASE`.
fn source_node(id: u64) -> ResolvedNode {
    node(id, NodeClass::Source, vec![audio_out(0)])
}

/// Builtin (gain/eq/mix): audio in port 0, audio out port `OUT_BASE`.
fn builtin_node(id: u64) -> ResolvedNode {
    node(id, NodeClass::Builtin, vec![audio_in(0), audio_out(0)])
}

/// Sink: required audio input port 0 (plus extras appended by caller).
fn sink_node(id: u64) -> ResolvedNode {
    node(id, NodeClass::Sink, vec![required_in(0)])
}

fn identity(uid: &str) -> PluginIdentity {
    PluginIdentity {
        format: PluginFormat::Vst3,
        vendor: "test".to_string(),
        uid: uid.to_string(),
        version: "1.0.0".to_string(),
        binary_digest: Some(format!("digest-{uid}")),
    }
}

fn declared_layout() -> Vec<DeclaredPort> {
    vec![
        DeclaredPort {
            name: "in".to_string(),
            direction: Direction::Input,
            domain: SignalDomain::Audio,
            format: audio_format(),
        },
        DeclaredPort {
            name: "out".to_string(),
            direction: Direction::Output,
            domain: SignalDomain::Audio,
            format: audio_format(),
        },
    ]
}

fn plugin_decl(uid: &str) -> PluginDeclaration {
    PluginDeclaration {
        identity: identity(uid),
        state: PluginStateDecl {
            digest: Some(format!("state-{uid}")),
            quality: StateCaptureQuality::Quiescent,
            status: OpaqueStateStatus::Known,
        },
        declared_ports: declared_layout(),
        automation_identity_complete: true,
    }
}

/// Plugin: audio in port 0, audio out port `OUT_BASE`.
fn plugin_node(id: u64, uid: &str) -> ResolvedNode {
    ResolvedNode {
        plugin: Some(plugin_decl(uid)),
        ..node(id, NodeClass::Plugin, vec![audio_in(0), audio_out(0)])
    }
}

fn env_live(uid: &str) -> PluginEnvironment {
    PluginEnvironment {
        installed: true,
        resolved_version: Some("1.0.0".to_string()),
        resolved_binary_digest: Some(format!("digest-{uid}")),
        access: AccessState::Granted,
        format_supported: true,
        advertised_ports: Some(declared_layout()),
        state_restore: StateRestoreSupport::Verified,
        uid_hash: PluginKey::of(&identity(uid)).uid_hash,
    }
}

fn caps(entries: Vec<(&str, PluginEnvironment)>) -> PluginCapabilitySnapshot {
    let mut map = BTreeMap::new();
    for (uid, env) in entries {
        map.insert(PluginKey::of(&identity(uid)), env);
    }
    PluginCapabilitySnapshot {
        generation: 7,
        entries: map,
    }
}

fn all_live_caps(uids: &[&str]) -> PluginCapabilitySnapshot {
    caps(uids.iter().map(|u| (*u, env_live(u))).collect())
}

fn caps_all() -> PluginCapabilitySnapshot {
    all_live_caps(&["instr", "rev"])
}

fn edge(id: u64, from: (u64, u32), to: (u64, u32), kind: EdgeKind) -> ResolvedEdge {
    ResolvedEdge {
        id: RcpEdgeId(id),
        from: PortEndpoint {
            node: RcpNodeId(from.0),
            port: RcpPortId(from.1),
        },
        to: PortEndpoint {
            node: RcpNodeId(to.0),
            port: RcpPortId(to.1),
        },
        kind,
        delay: EdgeDelay::Zero,
        channel_map: None,
        pdc_frames: 0,
    }
}

fn snapshot(
    nodes: Vec<ResolvedNode>,
    edges: Vec<ResolvedEdge>,
    required_outputs: Vec<BoundaryRequirement>,
) -> ResolvedGraphSnapshot {
    let map: BTreeMap<_, _> = nodes.into_iter().map(|n| (n.id, n)).collect();
    let mut snap = ResolvedGraphSnapshot {
        snapshot_id: 1,
        project_id: ProjectId::new(42),
        project_revision: ProjectRevision(10),
        structure_digest: 0,
        capability_generation: 7,
        engine_config_generation: 3,
        source_format_generation: 5,
        clock_epoch: synvelis_project::RuntimeClockEpochRef(1),
        runtime_instance: synvelis_project::RuntimeInstanceId(1),
        nodes: map,
        edges,
        required_outputs,
    };
    snap.structure_digest = snap.compute_structure_digest();
    snap
}

fn main_out_req(node: u64) -> BoundaryRequirement {
    BoundaryRequirement {
        port: PortEndpoint {
            node: RcpNodeId(node),
            port: RcpPortId(0),
        },
        purpose: BoundaryPurpose::MainOut,
    }
}

/// src(1) → instr plugin(2) → eq builtin(3) → rev plugin(4) → sink(5)
fn chain_snapshot() -> ResolvedGraphSnapshot {
    let o = OUT_BASE;
    snapshot(
        vec![
            source_node(1),
            plugin_node(2, "instr"),
            builtin_node(3),
            plugin_node(4, "rev"),
            sink_node(5),
        ],
        vec![
            edge(1, (1, o), (2, 0), EdgeKind::Main),
            edge(2, (2, o), (3, 0), EdgeKind::Main),
            edge(3, (3, o), (4, 0), EdgeKind::Main),
            edge(4, (4, o), (5, 0), EdgeKind::Main),
        ],
        vec![main_out_req(5)],
    )
}

fn artifact(
    id: &str,
    nodes: &[u64],
    outputs: Vec<ArtifactOutput>,
    pinned: BTreeSet<PortEndpoint>,
    eligibility: ArtifactEligibility,
    repro: ReproducibilityClass,
) -> RenderCandidate {
    RenderCandidate {
        artifact_id: id.to_string(),
        content_hash: format!("hash-{id}"),
        node_set: nodes.iter().map(|n| RcpNodeId(*n)).collect(),
        outputs,
        pinned_inputs: pinned,
        eligibility,
        reproducibility: repro,
        snapshot_revision: ProjectRevision(0), // patched by index()
        snapshot_digest: 0,
        access: AccessState::Granted,
    }
}

/// Builds an index and pins every candidate to `snap`'s revision
/// and structure digest (the stale-pin case constructs its index
/// manually instead).
fn index(candidates: Vec<RenderCandidate>, snap: &ResolvedGraphSnapshot) -> RenderCandidateIndex {
    let mut candidates = candidates;
    for c in &mut candidates {
        c.snapshot_digest = snap.structure_digest;
        c.snapshot_revision = snap.project_revision;
    }
    candidates.sort_by(|a, b| a.artifact_id.cmp(&b.artifact_id));
    RenderCandidateIndex {
        generation: 11,
        candidates,
    }
}

fn out(source_node: u64, out_idx: u32, domain: SignalDomain, latency: u64) -> ArtifactOutput {
    ArtifactOutput {
        source: PortEndpoint {
            node: RcpNodeId(source_node),
            port: RcpPortId(OUT_BASE + out_idx),
        },
        domain,
        format: audio_format(),
        latency_frames: latency,
    }
}

fn pin_in(node: u64, in_idx: u32) -> PortEndpoint {
    PortEndpoint {
        node: RcpNodeId(node),
        port: RcpPortId(in_idx),
    }
}

// ---------------------------------------------------------------------------
// RCP-EX-01: intent-driven resolution
// ---------------------------------------------------------------------------

#[test]
fn ex01_missing_instrument_checkpoint_substitutes_exact() {
    let snap = chain_snapshot();
    // Plugin "instr" (node 2) absent; "rev" (node 4) live.
    let caps = caps(vec![("rev", env_live("rev"))]);
    let cand = artifact(
        "ckpt-instr",
        &[2],
        vec![out(2, 0, SignalDomain::Audio, 0)],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    let renders = index(vec![cand], &snap);
    let sel = ExecutionSelector::select(&snap, &ExecutionIntent::MainPlayback, &caps, &renders);
    assert_eq!(sel.verdict, SelectionVerdict::Exact);
    assert!(!sel.non_equivalent);
    assert_eq!(sel.substitutions.len(), 1);
    assert_eq!(sel.substitutions[0].artifact_id, "ckpt-instr");
    assert_eq!(
        sel.unresolved,
        [] as [synvelis_player::rcp::UnresolvedNode; 0]
    );
    let live: BTreeSet<_> = sel
        .live_regions
        .iter()
        .flat_map(|r| r.nodes.iter().copied())
        .collect();
    assert!(live.contains(&RcpNodeId(3)));
    assert!(live.contains(&RcpNodeId(4)));
    assert!(!live.contains(&RcpNodeId(2)));
}

#[test]
fn ex01_missing_reverb_final_checkpoint_keeps_compressor_live() {
    // src → instr(live) → eq → rev(MISSING) → comp(live) → sink
    let o = OUT_BASE;
    let snap = snapshot(
        vec![
            source_node(1),
            plugin_node(2, "instr"),
            builtin_node(3),
            plugin_node(4, "rev"),
            plugin_node(6, "comp"),
            sink_node(5),
        ],
        vec![
            edge(1, (1, o), (2, 0), EdgeKind::Main),
            edge(2, (2, o), (3, 0), EdgeKind::Main),
            edge(3, (3, o), (4, 0), EdgeKind::Main),
            edge(4, (4, o), (6, 0), EdgeKind::Main),
            edge(5, (6, o), (5, 0), EdgeKind::Main),
        ],
        vec![main_out_req(5)],
    );
    let caps = all_live_caps(&["instr", "comp"]);
    // "final checkpoint" covers instr..rev (nodes 2,3,4); comp stays live.
    let cand = artifact(
        "final-ckpt",
        &[2, 3, 4],
        vec![out(4, 0, SignalDomain::Audio, 0)],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::VerifiedWithinTolerance,
    );
    let renders = index(vec![cand], &snap);
    let sel = ExecutionSelector::select(&snap, &ExecutionIntent::MainPlayback, &caps, &renders);
    assert_eq!(sel.verdict, SelectionVerdict::Exact);
    assert_eq!(sel.substitutions.len(), 1);
    assert!(sel.substitutions[0].nodes.contains(&RcpNodeId(4)));
    let live: BTreeSet<_> = sel
        .live_regions
        .iter()
        .flat_map(|r| r.nodes.iter().copied())
        .collect();
    assert!(live.contains(&RcpNodeId(6)), "compressor must stay live");
}

#[test]
fn ex01_missing_plugin_no_artifact_blocks_main_playback() {
    let snap = chain_snapshot();
    let caps = caps(vec![("rev", env_live("rev"))]);
    let sel = ExecutionSelector::select(
        &snap,
        &ExecutionIntent::MainPlayback,
        &caps,
        &RenderCandidateIndex::default(),
    );
    assert_eq!(sel.verdict, SelectionVerdict::Blocked);
    assert_eq!(sel.unresolved.len(), 1);
    assert_eq!(sel.unresolved[0].node, RcpNodeId(2));
}

#[test]
fn ex01_preview_intent_requires_opt_in_for_preview_artifact() {
    let snap = chain_snapshot();
    let caps = caps(vec![("rev", env_live("rev"))]);
    let preview_cand = artifact(
        "ckpt-preview",
        &[2],
        vec![out(2, 0, SignalDomain::Audio, 0)],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligiblePreview,
        ReproducibilityClass::PreviewOnly,
    );
    let renders = index(vec![preview_cand], &snap);
    let no_opt = ExecutionSelector::select(
        &snap,
        &ExecutionIntent::EditingPreview {
            live_regions: vec![],
            allow_non_equivalent: false,
        },
        &caps,
        &renders,
    );
    assert_eq!(no_opt.verdict, SelectionVerdict::Blocked);
    let opted = ExecutionSelector::select(
        &snap,
        &ExecutionIntent::EditingPreview {
            live_regions: vec![],
            allow_non_equivalent: true,
        },
        &caps,
        &renders,
    );
    assert_eq!(opted.verdict, SelectionVerdict::NonEquivalent);
    assert!(opted.non_equivalent);
    assert_eq!(opted.substitutions.len(), 1);
}

#[test]
fn ex01_offline_render_and_export_require_exact() {
    let snap = chain_snapshot();
    let caps = caps(vec![("rev", env_live("rev"))]);
    let preview_cand = artifact(
        "ckpt-preview",
        &[2],
        vec![out(2, 0, SignalDomain::Audio, 0)],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligiblePreview,
        ReproducibilityClass::PreviewOnly,
    );
    let renders = index(vec![preview_cand], &snap);
    for intent in [
        ExecutionIntent::OfflineRender {
            min_reproducibility: ReproducibilityClass::VerifiedWithinTolerance,
        },
        ExecutionIntent::Export,
    ] {
        let sel = ExecutionSelector::select(&snap, &intent, &caps, &renders);
        assert_eq!(sel.verdict, SelectionVerdict::Blocked, "{intent:?}");
    }
    let exact = artifact(
        "ckpt-exact",
        &[2],
        vec![out(2, 0, SignalDomain::Audio, 0)],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    let renders = index(vec![exact], &snap);
    let sel = ExecutionSelector::select(&snap, &ExecutionIntent::Export, &caps, &renders);
    assert_eq!(sel.verdict, SelectionVerdict::Exact);
}

// ---------------------------------------------------------------------------
// RCP-EX-02: cutset boundary rules
// ---------------------------------------------------------------------------

#[test]
fn ex02_multiout_requires_every_output_port_covered() {
    // Plugin node 2: in0, out1, out2 → mixer node 3: in0, in1, out12 → sink 4.
    let o = OUT_BASE;
    let mut plug = plugin_node(2, "multi");
    plug.ports.push(audio_out(1)); // second out → port OUT_BASE+1
    let mut mix = node(
        3,
        NodeClass::Builtin,
        vec![audio_in(0), audio_in(1), audio_out(0)],
    );
    mix.label = None;
    let snap = snapshot(
        vec![source_node(1), plug, mix, sink_node(4)],
        vec![
            edge(1, (1, o), (2, 0), EdgeKind::Main),
            edge(2, (2, o), (3, 0), EdgeKind::Main),
            edge(3, (2, o + 1), (3, 1), EdgeKind::Main),
            edge(4, (3, o), (4, 0), EdgeKind::Main),
        ],
        vec![main_out_req(4)],
    );
    // Artifact covers only the first output → second uncovered.
    let partial = artifact(
        "multi-partial",
        &[2],
        vec![out(2, 0, SignalDomain::Audio, 0)],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    let report = validate_cutset(&snap, &partial);
    assert!(!report.accepted);
    assert!(
        report
            .rejections
            .iter()
            .any(|r| matches!(r, CutsetRejection::UncoveredOutput { .. }))
    );
    // Both outputs covered → accepted.
    let full = artifact(
        "multi-full",
        &[2],
        vec![
            out(2, 0, SignalDomain::Audio, 0),
            out(2, 1, SignalDomain::Audio, 0),
        ],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    let report = validate_cutset(&snap, &full);
    assert!(report.accepted, "{:?}", report.rejections);
}

#[test]
fn ex02_required_midi_output_needs_typed_artifact_support() {
    // Plugin 2: audio in0, audio out OUT_BASE, MIDI out OUT_BASE+1.
    // Sink 3: required audio in0, event in1 (required output).
    let o = OUT_BASE;
    let mut plug = plugin_node(2, "midi-fx");
    plug.ports.push(mkport(
        o + 1,
        Direction::Output,
        PortRole::Main,
        PortFormat::TimedEvent,
        0,
    ));
    let mut sink = sink_node(3);
    sink.ports.push(mkport(
        1,
        Direction::Input,
        PortRole::Main,
        PortFormat::TimedEvent,
        0,
    ));
    let snap = snapshot(
        vec![source_node(1), plug, sink],
        vec![
            edge(1, (1, o), (2, 0), EdgeKind::Main),
            edge(2, (2, o), (3, 0), EdgeKind::Main),
            edge(3, (2, o + 1), (3, 1), EdgeKind::Main),
        ],
        vec![
            main_out_req(3),
            BoundaryRequirement {
                port: PortEndpoint {
                    node: RcpNodeId(3),
                    port: RcpPortId(1),
                },
                purpose: BoundaryPurpose::Other,
            },
        ],
    );
    // Audio-domain artifact output at the MIDI source port →
    // domain mismatch (audio never fabricates event output).
    let audio_only = artifact(
        "audio-only",
        &[2],
        vec![
            out(2, 0, SignalDomain::Audio, 0),
            out(2, 1, SignalDomain::Audio, 0),
        ],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    let report = validate_cutset(&snap, &audio_only);
    assert!(!report.accepted);
    assert!(report.rejections.iter().any(|r| matches!(
        r,
        CutsetRejection::DomainMismatch { .. } | CutsetRejection::UncoveredOutput { .. }
    )));
    // Correctly typed TimedEvent output → accepted.
    let typed = RenderCandidate {
        outputs: vec![
            out(2, 0, SignalDomain::Audio, 0),
            ArtifactOutput {
                source: PortEndpoint {
                    node: RcpNodeId(2),
                    port: RcpPortId(o + 1),
                },
                domain: SignalDomain::TimedEvent,
                format: PortFormat::TimedEvent,
                latency_frames: 0,
            },
        ],
        ..audio_only
    };
    let report = validate_cutset(&snap, &typed);
    assert!(report.accepted, "{:?}", report.rejections);
}

#[test]
fn ex02_unpinned_sidechain_input_rejected() {
    // src(1) → comp(2): main in0 + sidechain in1, main out10 → sink(3)
    let o = OUT_BASE;
    let mut comp = plugin_node(2, "comp");
    comp.ports.insert(
        1,
        mkport(1, Direction::Input, PortRole::Sidechain, audio_format(), 0),
    );
    let snap = snapshot(
        vec![source_node(1), comp, sink_node(3)],
        vec![
            edge(1, (1, o), (2, 0), EdgeKind::Main),
            edge(2, (1, o), (2, 1), EdgeKind::Sidechain),
            edge(3, (2, o), (3, 0), EdgeKind::Main),
        ],
        vec![main_out_req(3)],
    );
    // Nothing pinned → sidechain + main inputs both unpinned.
    let unpinned = artifact(
        "sc-unpinned",
        &[2],
        vec![out(2, 0, SignalDomain::Audio, 0)],
        BTreeSet::new(),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    let report = validate_cutset(&snap, &unpinned);
    assert!(!report.accepted);
    assert!(
        report
            .rejections
            .iter()
            .any(|r| matches!(r, CutsetRejection::SidechainUnpinned { .. }))
    );
    // Only sidechain pinned → main input still unpinned.
    let partial = RenderCandidate {
        pinned_inputs: BTreeSet::from([pin_in(2, 1)]),
        ..unpinned.clone()
    };
    let report = validate_cutset(&snap, &partial);
    assert!(!report.accepted);
    assert!(
        report
            .rejections
            .iter()
            .any(|r| matches!(r, CutsetRejection::UnpinnedInput { .. }))
    );
    // Both inputs pinned → accepted.
    let pinned = RenderCandidate {
        pinned_inputs: BTreeSet::from([pin_in(2, 0), pin_in(2, 1)]),
        ..unpinned
    };
    let report = validate_cutset(&snap, &pinned);
    assert!(report.accepted, "{:?}", report.rejections);
}

#[test]
fn ex02_send_return_boundary_input_must_be_pinned() {
    // strip(1) --Send--> returnbus(2, missing plugin) --Return--> mixer(3) --> sink(4)
    let o = OUT_BASE;
    let snap = snapshot(
        vec![
            builtin_node(1),
            plugin_node(2, "returnbus"),
            builtin_node(3),
            sink_node(4),
        ],
        vec![
            edge(1, (1, o), (2, 0), EdgeKind::Send),
            edge(2, (2, o), (3, 0), EdgeKind::Return),
            edge(3, (3, o), (4, 0), EdgeKind::Main),
        ],
        vec![main_out_req(4)],
    );
    let unpinned = artifact(
        "sr-unpinned",
        &[2],
        vec![out(2, 0, SignalDomain::Audio, 0)],
        BTreeSet::new(),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    let report = validate_cutset(&snap, &unpinned);
    assert!(!report.accepted);
    assert!(
        report
            .rejections
            .iter()
            .any(|r| matches!(r, CutsetRejection::UnpinnedInput { .. }))
    );
    let pinned = RenderCandidate {
        pinned_inputs: BTreeSet::from([pin_in(2, 0)]),
        ..unpinned
    };
    assert!(validate_cutset(&snap, &pinned).accepted);
}

#[test]
fn ex02_latency_mismatch_rejected() {
    let snap = chain_snapshot();
    let bad = artifact(
        "lat-bad",
        &[2],
        vec![out(2, 0, SignalDomain::Audio, 7)],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    let report = validate_cutset(&snap, &bad);
    assert!(!report.accepted);
    assert!(
        report
            .rejections
            .iter()
            .any(|r| matches!(r, CutsetRejection::LatencyMismatch { .. }))
    );
}

#[test]
fn ex02_double_feed_rejected() {
    // Same source port → same dest port via two edges = double-add.
    let o = OUT_BASE;
    let snap = snapshot(
        vec![source_node(1), builtin_node(2), sink_node(3)],
        vec![
            edge(1, (1, o), (2, 0), EdgeKind::Main),
            edge(2, (2, o), (3, 0), EdgeKind::Main),
            edge(3, (2, o), (3, 0), EdgeKind::Main),
        ],
        vec![main_out_req(3)],
    );
    let cand = artifact(
        "df",
        &[2],
        vec![out(2, 0, SignalDomain::Audio, 0)],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    let report = validate_cutset(&snap, &cand);
    assert!(!report.accepted);
    assert!(
        report
            .rejections
            .iter()
            .any(|r| matches!(r, CutsetRejection::DoubleFeed { .. }))
    );
}

#[test]
fn ex02_dangling_input_when_region_feed_uncovered() {
    // Artifact output keyed at the wrong source port → boundary
    // output uncovered and required sink input loses its feed.
    let o = OUT_BASE;
    let mut plug = plugin_node(2, "p");
    plug.ports.push(audio_out(1)); // extra out port OUT_BASE+1, unused
    let snap = snapshot(
        vec![source_node(1), plug, sink_node(3)],
        vec![
            edge(1, (1, o), (2, 0), EdgeKind::Main),
            edge(2, (2, o), (3, 0), EdgeKind::Main),
        ],
        vec![main_out_req(3)],
    );
    let cand = artifact(
        "wrong-src",
        &[2],
        vec![out(2, 1, SignalDomain::Audio, 0)],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    let report = validate_cutset(&snap, &cand);
    assert!(!report.accepted);
    assert!(report.rejections.iter().any(|r| matches!(
        r,
        CutsetRejection::UncoveredOutput { .. }
            | CutsetRejection::RequiredOutputUncovered { .. }
            | CutsetRejection::DanglingInput { .. }
    )));
}

// ---------------------------------------------------------------------------
// RCP-EX-03: fail-closed artifact/plugin states
// ---------------------------------------------------------------------------

#[test]
fn ex03_bad_artifact_states_never_exact_for_main_playback() {
    let snap = chain_snapshot();
    let caps = caps(vec![("rev", env_live("rev"))]);
    for (name, eligibility) in [
        ("unsupported", ArtifactEligibility::Unsupported),
        ("corrupt", ArtifactEligibility::Corrupt),
        ("stale", ArtifactEligibility::Stale),
        ("unverified", ArtifactEligibility::Unverified),
        ("unknown", ArtifactEligibility::Unknown),
        ("missing", ArtifactEligibility::Missing),
        ("valid-recipe", ArtifactEligibility::ValidRecipe),
        ("verified-media", ArtifactEligibility::VerifiedMedia),
    ] {
        let cand = artifact(
            name,
            &[2],
            vec![out(2, 0, SignalDomain::Audio, 0)],
            BTreeSet::from([pin_in(2, 0)]),
            eligibility,
            ReproducibilityClass::BitExactCertified,
        );
        let renders = index(vec![cand], &snap);
        let sel = ExecutionSelector::select(&snap, &ExecutionIntent::MainPlayback, &caps, &renders);
        assert_eq!(
            sel.verdict,
            SelectionVerdict::Blocked,
            "{name} must not produce Exact"
        );
    }
}

#[test]
fn ex03_acl_denied_and_stale_pin_rejected() {
    let snap = chain_snapshot();
    let caps = caps(vec![("rev", env_live("rev"))]);
    let mut denied = artifact(
        "denied",
        &[2],
        vec![out(2, 0, SignalDomain::Audio, 0)],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    denied.access = AccessState::Denied {
        reason: "acl".to_string(),
    };
    denied.snapshot_digest = snap.structure_digest;
    denied.snapshot_revision = snap.project_revision;
    let mut stale = artifact(
        "stale-pin",
        &[2],
        vec![out(2, 0, SignalDomain::Audio, 0)],
        BTreeSet::from([pin_in(2, 0)]),
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
    );
    stale.snapshot_digest = snap.structure_digest;
    stale.snapshot_revision = ProjectRevision(9); // old revision pin
    let renders = RenderCandidateIndex {
        generation: 11,
        candidates: vec![denied, stale],
    };
    let sel = ExecutionSelector::select(&snap, &ExecutionIntent::MainPlayback, &caps, &renders);
    assert_eq!(sel.verdict, SelectionVerdict::Blocked);
    assert!(
        sel.receipt
            .candidate_attempts
            .iter()
            .all(|a| !matches!(a.outcome, synvelis_player::rcp::AttemptOutcome::Accepted))
    );
}

#[test]
fn ex03_unknown_plugin_state_blocks_required_path() {
    let snap = chain_snapshot();
    let mut decl_env = env_live("instr");
    decl_env.state_restore = StateRestoreSupport::Unknown;
    let caps = caps(vec![("instr", decl_env), ("rev", env_live("rev"))]);
    let sel = ExecutionSelector::select(
        &snap,
        &ExecutionIntent::MainPlayback,
        &caps,
        &RenderCandidateIndex::default(),
    );
    assert_eq!(sel.verdict, SelectionVerdict::Blocked);
    assert!(matches!(
        sel.receipt.plugin_verdicts.get(&RcpNodeId(2)),
        Some(PluginVerdict::StateUnknown)
    ));
    // Preview opt-in degrades the node explicitly: it is skipped
    // (its only downstream port is optional) and the selection is
    // marked NonEquivalent — never a silent Exact.
    let opted = ExecutionSelector::select(
        &snap,
        &ExecutionIntent::EditingPreview {
            live_regions: vec![],
            allow_non_equivalent: true,
        },
        &caps,
        &RenderCandidateIndex::default(),
    );
    assert_eq!(opted.verdict, SelectionVerdict::NonEquivalent);
    assert!(opted.non_equivalent);
    assert!(opted.skipped.iter().any(|s| s.node == RcpNodeId(2)));
}

#[test]
fn ex03_missing_plugin_verdicts_are_typed() {
    let snap = chain_snapshot();
    let caps = caps(vec![("rev", env_live("rev"))]);
    let sel = ExecutionSelector::select(
        &snap,
        &ExecutionIntent::MainPlayback,
        &caps,
        &RenderCandidateIndex::default(),
    );
    assert_eq!(sel.verdict, SelectionVerdict::Blocked);
    assert!(matches!(
        sel.receipt.plugin_verdicts.get(&RcpNodeId(2)),
        Some(PluginVerdict::Missing)
    ));
}

// ---------------------------------------------------------------------------
// RCP-EX-04: edit classification
// ---------------------------------------------------------------------------

fn plan_context(
    snap: &ResolvedGraphSnapshot,
    renders_gen: u64,
) -> synvelis_player::rcp::PlanContext {
    let sel = ExecutionSelector::select(
        snap,
        &ExecutionIntent::MainPlayback,
        &caps_all(),
        &RenderCandidateIndex {
            generation: renders_gen,
            candidates: vec![],
        },
    );
    synvelis_player::rcp::PlanContext::from_selection(&sel.receipt, renders_gen)
}

#[test]
fn ex04_rename_is_no_rebuild() {
    let snap = chain_snapshot();
    let before = plan_context(&snap, 11);
    let mut renamed = snap.clone();
    renamed.nodes.get_mut(&RcpNodeId(3)).unwrap().label = Some("EQ!".into());
    assert_eq!(renamed.compute_structure_digest(), snap.structure_digest);
    assert_eq!(
        classify_edit(&before, &renamed, 11, EditKind::Presentation),
        EditClass::NoRebuild
    );
}

#[test]
fn ex04_bound_parameter_and_automation_are_control_only() {
    let snap = chain_snapshot();
    let before = plan_context(&snap, 11);
    for kind in [EditKind::BoundParameter, EditKind::AutomationEvent] {
        let class = classify_edit(&before, &snap, 11, kind);
        assert!(matches!(class, EditClass::ControlOnly { .. }), "{kind:?}");
    }
}

#[test]
fn ex04_structural_capability_and_render_changes_rebuild() {
    let snap = chain_snapshot();
    let before = plan_context(&snap, 11);
    // Structural: add a node.
    let mut structural = snap.clone();
    structural.nodes.insert(RcpNodeId(9), builtin_node(9));
    structural.structure_digest = structural.compute_structure_digest();
    assert_eq!(
        classify_edit(&before, &structural, 11, EditKind::Structural),
        EditClass::Rebuild
    );
    // Capability generation drift.
    let mut cap = snap.clone();
    cap.capability_generation += 1;
    cap.structure_digest = cap.compute_structure_digest();
    assert_eq!(
        classify_edit(&before, &cap, 11, EditKind::PluginBinding),
        EditClass::Rebuild
    );
    // Render substitution change → Rebuild (re-selection needed).
    assert_eq!(
        classify_edit(&before, &snap, 12, EditKind::RenderSubstitution),
        EditClass::Rebuild
    );
    // Render-index move under a presentation edit still forces
    // re-selection — artifact arrival is independent of graph
    // rebuild but not of plan re-evaluation.
    assert_eq!(
        classify_edit(&before, &snap, 12, EditKind::Presentation),
        EditClass::Rebuild
    );
}

#[test]
fn ex04_source_content_edit_rebuilds() {
    let snap = chain_snapshot();
    let before = plan_context(&snap, 11);
    // Source media bytes carry no plan pin — identical snapshot
    // pins cannot prove the content is unchanged, so a declared
    // content edit must re-resolve rather than report NoRebuild.
    assert_eq!(
        classify_edit(&before, &snap, 11, EditKind::SourceContent),
        EditClass::Rebuild
    );
}

// ---------------------------------------------------------------------------
// RCP-EX-05: safe plan reconciliation
// ---------------------------------------------------------------------------

fn live_snapshot(rev: u64) -> ResolvedGraphSnapshot {
    let mut s = chain_snapshot();
    s.project_revision = ProjectRevision(rev);
    s
}

#[test]
fn ex05_rapid_edits_never_activate_stale_plan() {
    let mut rec = PlanReconciler::new();
    let snap_r1 = live_snapshot(10);
    let sel1 = ExecutionSelector::select(
        &snap_r1,
        &ExecutionIntent::MainPlayback,
        &caps_all(),
        &RenderCandidateIndex::default(),
    );
    assert_eq!(sel1.verdict, SelectionVerdict::Exact);
    let p1 = rec.prepare(&sel1.receipt);
    let r = rec.publish(&p1, &snap_r1, &caps_all(), Ok(()));
    assert!(matches!(r.state, RoutingActivationState::Published { .. }));
    assert_eq!(rec.active().unwrap().generation, p1.generation);

    // R2 prepared, but publish attempt lands after R3 committed.
    let snap_r2 = live_snapshot(11);
    let sel2 = ExecutionSelector::select(
        &snap_r2,
        &ExecutionIntent::MainPlayback,
        &caps_all(),
        &RenderCandidateIndex::default(),
    );
    let p2 = rec.prepare(&sel2.receipt);
    let snap_r3 = live_snapshot(12);
    let stale = rec.publish(&p2, &snap_r3, &caps_all(), Ok(()));
    assert!(matches!(
        stale.state,
        RoutingActivationState::Blocked {
            fault: RoutingFault::StaleProjectSnapshot,
            ..
        }
    ));
    assert_eq!(rec.active().unwrap().generation, p1.generation);

    // Correct R3 prepare publishes; gen1 demoted to last-good.
    let sel3 = ExecutionSelector::select(
        &snap_r3,
        &ExecutionIntent::MainPlayback,
        &caps_all(),
        &RenderCandidateIndex::default(),
    );
    let p3 = rec.prepare(&sel3.receipt);
    let ok = rec.publish(&p3, &snap_r3, &caps_all(), Ok(()));
    assert!(matches!(ok.state, RoutingActivationState::Published { .. }));
    assert_eq!(rec.active().unwrap().generation, p3.generation);
    assert_eq!(rec.last_good().unwrap().generation, p1.generation);
}

#[test]
fn ex05_out_of_order_publish_rejected() {
    let mut rec = PlanReconciler::new();
    let snap = live_snapshot(10);
    let sel = ExecutionSelector::select(
        &snap,
        &ExecutionIntent::MainPlayback,
        &caps_all(),
        &RenderCandidateIndex::default(),
    );
    let p_old = rec.prepare(&sel.receipt);
    let p_new = rec.prepare(&sel.receipt);
    rec.publish(&p_new, &snap, &caps_all(), Ok(()));
    let r = rec.publish(&p_old, &snap, &caps_all(), Ok(()));
    assert!(matches!(
        r.state,
        RoutingActivationState::Blocked {
            fault: RoutingFault::Superseded,
            ..
        }
    ));
    assert_eq!(rec.active().unwrap().generation, p_new.generation);
}

#[test]
fn ex05_queue_full_and_retire_failure_retain_last_good() {
    let mut rec = PlanReconciler::new();
    let snap = live_snapshot(10);
    let sel = ExecutionSelector::select(
        &snap,
        &ExecutionIntent::MainPlayback,
        &caps_all(),
        &RenderCandidateIndex::default(),
    );
    let p1 = rec.prepare(&sel.receipt);
    rec.publish(&p1, &snap, &caps_all(), Ok(()));

    let snap2 = live_snapshot(11);
    let sel2 = ExecutionSelector::select(
        &snap2,
        &ExecutionIntent::MainPlayback,
        &caps_all(),
        &RenderCandidateIndex::default(),
    );
    let p2 = rec.prepare(&sel2.receipt);
    for fault in [RoutingFault::PublishQueueFull, RoutingFault::RetireOverflow] {
        let r = rec.publish(&p2, &snap2, &caps_all(), Err(fault));
        assert!(matches!(r.state, RoutingActivationState::Failed { .. }));
        assert_eq!(r.fault, Some(fault));
        assert_eq!(rec.active().unwrap().generation, p1.generation);
        assert_eq!(
            rec.active().unwrap().context.build_key.project_revision,
            ProjectRevision(10)
        );
    }
}

#[test]
fn ex05_reconcile_detects_drift_and_gates_delivery() {
    let mut rec = PlanReconciler::new();
    let snap = live_snapshot(10);
    let sel = ExecutionSelector::select(
        &snap,
        &ExecutionIntent::MainPlayback,
        &caps_all(),
        &RenderCandidateIndex::default(),
    );
    let p1 = rec.prepare(&sel.receipt);
    rec.publish(&p1, &snap, &caps_all(), Ok(()));
    rec.reconcile(&snap, &caps_all(), 11);
    assert_eq!(rec.delivery_gate(p1.generation), DeliveryDecision::Allow);

    // Revision-only move with identical graph content → absorbed
    // (the NoRebuild evidence path), no mismatch stands.
    let rev_only = live_snapshot(11);
    let out = rec.reconcile(&rev_only, &caps_all(), 11);
    assert!(out.mismatches.is_empty(), "{:?}", out.mismatches);
    assert_eq!(rec.delivery_gate(p1.generation), DeliveryDecision::Allow);

    // Structural edit → fingerprint mismatch → delivery gate closes.
    let mut structural = live_snapshot(12);
    structural.nodes.insert(RcpNodeId(9), builtin_node(9));
    structural.structure_digest = structural.compute_structure_digest();
    let out = rec.reconcile(&structural, &caps_all(), 11);
    assert!(
        out.mismatches
            .contains(&synvelis_project::RoutingReconcileMismatch::RoutingFingerprint)
    );
    assert_eq!(
        rec.delivery_gate(p1.generation),
        DeliveryDecision::BlockStaleGeneration {
            active_generation: Some(p1.generation)
        }
    );
    assert!(rec.last_good().is_some() || rec.active().is_some());
}

#[test]
fn ex05_plugin_restore_failure_and_epoch_change_block() {
    let mut rec = PlanReconciler::new();
    let snap = live_snapshot(10);
    let sel = ExecutionSelector::select(
        &snap,
        &ExecutionIntent::MainPlayback,
        &caps_all(),
        &RenderCandidateIndex::default(),
    );
    let p1 = rec.prepare(&sel.receipt);
    rec.publish(&p1, &snap, &caps_all(), Ok(()));

    // Plugin restore failure: "instr" env now reports unsupported.
    let mut degraded = caps_all();
    let key = PluginKey::of(&identity("instr"));
    if let Some(env) = degraded.entries.get_mut(&key) {
        env.state_restore = StateRestoreSupport::Unsupported;
    }
    let out = rec.reconcile(&snap, &degraded, 11);
    assert_ne!(
        out.plugins_degraded,
        [] as [(
            synvelis_player::rcp::RcpNodeId,
            synvelis_player::rcp::PluginVerdict
        ); 0]
    );
    assert_ne!(
        out.mismatches,
        [] as [synvelis_project::RoutingReconcileMismatch; 0]
    );
    assert!(matches!(
        rec.delivery_gate(p1.generation),
        DeliveryDecision::BlockStaleGeneration { .. }
    ));

    // Clock epoch change on a fresh plan → DeviceEpoch mismatch.
    let mut rec2 = PlanReconciler::new();
    let p2 = rec2.prepare(&sel.receipt);
    rec2.publish(&p2, &snap, &caps_all(), Ok(()));
    let mut epoch_shifted = live_snapshot(10);
    epoch_shifted.clock_epoch = synvelis_project::RuntimeClockEpochRef(2);
    let out2 = rec2.reconcile(&epoch_shifted, &caps_all(), 11);
    assert!(
        out2.mismatches
            .contains(&synvelis_project::RoutingReconcileMismatch::DeviceEpoch)
    );
}

#[test]
fn ex05_active_requires_matching_generation_and_epoch() {
    let mut rec = PlanReconciler::new();
    let snap = live_snapshot(10);
    let sel = ExecutionSelector::select(
        &snap,
        &ExecutionIntent::MainPlayback,
        &caps_all(),
        &RenderCandidateIndex::default(),
    );
    let p1 = rec.prepare(&sel.receipt);
    rec.publish(&p1, &snap, &caps_all(), Ok(()));

    // RT observation on a different clock epoch is not activation
    // of this plan — the plan stays Published, never Active.
    rec.observe_active(p1.generation, synvelis_project::RuntimeClockEpochRef(9));
    assert!(matches!(
        rec.active().unwrap().state,
        RoutingActivationState::Published { .. }
    ));

    // Matching generation + pinned epoch → Active ack.
    rec.observe_active(p1.generation, synvelis_project::RuntimeClockEpochRef(1));
    assert!(matches!(
        rec.active().unwrap().state,
        RoutingActivationState::Active { .. }
    ));

    // A stale generation can never be marked active.
    let mut rec2 = PlanReconciler::new();
    let p2 = rec2.prepare(&sel.receipt);
    rec2.publish(&p2, &snap, &caps_all(), Ok(()));
    rec2.observe_active(p2.generation + 1, synvelis_project::RuntimeClockEpochRef(1));
    assert!(matches!(
        rec2.active().unwrap().state,
        RoutingActivationState::Published { .. }
    ));
}
