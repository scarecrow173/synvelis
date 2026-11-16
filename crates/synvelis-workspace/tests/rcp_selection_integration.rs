//! RCP-04 semantic→selection integration oracle (synvelis#293
//! deferred residual slice; `docs/test-specs/sv293-semantic-selection-integration.md`).
//!
//! These tests exercise the *integration wiring* only: a semantic
//! `Project::routing` model plus caller-pinned plugin environment
//! and render-lookup evidence is resolved by
//! [`synvelis_workspace::rcp_selection::resolve_selection`] into the
//! RCP-04 pin inputs, driven through `ExecutionSelector::select`,
//! and reconciled with `PlanReconciler` + `RoutingEngine`
//! prepare/publish via `drive_selection_plan`.
//!
//! They never touch real plugins, OS endpoints, RT threads or PCM —
//! RCP-EX-06 real-machine acceptance stays `NotRun` (RCP-GATE).
//! `StaticProcessorHost` + `GainProcessor` stand in for hosted
//! plugins so the *engine plan construction* leg is real while the
//! *plugin executability* verdicts come from the pinned capability
//! evidence, exactly as the headless contract intends.

use std::collections::{BTreeMap, BTreeSet};

use ruviform_audio_core::AudioFormat;
use ruviform_graph::{Direction, PortFormat, SignalDomain};
use synvelis_player::rcp::{
    AccessState, ArtifactEligibility, EdgeKind, EditClass, EditKind, ExecutionIntent,
    ExecutionSelection, ExecutionSelector, PlanContext, PlanReconciler, PluginEnvironment,
    PluginFormat, PluginIdentity, PluginStateDecl, PluginVerdict, ReproducibilityClass,
    SelectionVerdict, StateRestoreSupport, classify_edit,
};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::routing_lower::{GainProcessor, engine_format};
use synvelis_player::{EngineConfig, StaticProcessorHost};
use synvelis_project::routing::{PortDirection, PortRole, ProcessorBinding};
use synvelis_project::snapshot::ContentDigest;
use synvelis_project::{
    BuildBudget, BuildMode, BusKind, Command, ConnectionKind, CounterIdGenerator,
    OpaqueStateStatus, PluginRef, ProcessorId, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, RoutingActivationState, RoutingBuildRequestId, RoutingCompileRequest,
    RuntimeClockEpochRef, RuntimeInstanceId, SemanticPortId, SourceId, StateCaptureQuality,
    StripId, routing_fingerprint,
};
use synvelis_render::artifact::lookup::CandidateReport;
use synvelis_render::artifact::manifest::RenderArtifactId;
use synvelis_workspace::rcp_selection::{
    PlanDrive, ProcessorPluginEvidence, SelectionBuildError, SelectionInputs, SelectionPlanDrive,
    SemanticRenderCandidate, SemanticRenderCoverage, drive_selection_plan, resolve_selection,
};

// ---------------------------------------------------------------------------
// fixture constants + builders
// ---------------------------------------------------------------------------

const ENGINE_CONFIG_GENERATION: u64 = 1;
const SOURCE_FORMAT_GENERATION: u64 = 1;
const RENDER_INDEX_GENERATION: u64 = 1;
const INSTANCE: RuntimeInstanceId = RuntimeInstanceId(1);
const EPOCH: RuntimeClockEpochRef = RuntimeClockEpochRef(1);

fn engine() -> EngineConfig {
    EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 256,
    }
}

fn audio() -> AudioFormat {
    engine_format(&engine())
}

fn project_id() -> ProjectId {
    ProjectId::new(0x293)
}

fn apply(controller: &mut ProjectController, op: ProjectOp) {
    controller
        .apply(Command {
            expected_revision: controller.live_revision(),
            op,
        })
        .expect("op must apply");
}

fn plugin_ref(descriptor: &str) -> PluginRef {
    PluginRef {
        descriptor: descriptor.to_owned(),
        version: "1.0.0".to_owned(),
        capability_revision: 1,
    }
}

/// FNV-1a 64 — same rule as `synvelis_player::rcp`'s `PluginKey::of`
/// (the helper itself is crate-private; the environment evidence
/// field is informational, never a lookup key).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Deterministic authored identity derived from the descriptor —
/// `vendor`/`uid`/`version` stay stable so evidence and environment
/// agree bit-for-bit.
fn identity_for(descriptor: &str) -> PluginIdentity {
    PluginIdentity {
        format: PluginFormat::Vst3,
        vendor: "acme".to_owned(),
        uid: descriptor.to_owned(),
        version: "1.0.0".to_owned(),
        binary_digest: Some(format!("sha256:bin-{descriptor}")),
    }
}

/// The strip's ordered chain processor ids.
fn chain_processors(project: &Project, strip: StripId) -> Vec<ProcessorId> {
    let chain_id = project.routing.strips[&strip].chain;
    project.routing.chains[&chain_id].processors.clone()
}

/// All processor ids in the project (host registration + evidence).
fn all_processors(project: &Project) -> Vec<ProcessorId> {
    project.routing.processors.keys().copied().collect()
}

/// The `(Input|Output, Main)` semantic port of `processor`.
fn processor_main_port(
    project: &Project,
    processor: ProcessorId,
    direction: PortDirection,
) -> SemanticPortId {
    let proc = &project.routing.processors[&processor];
    proc.port_ids
        .iter()
        .copied()
        .find(|id| {
            let port = &project.routing.ports[id];
            port.direction == direction && port.role == PortRole::Main
        })
        .expect("processor must have a Main port in the requested direction")
}

/// The `(direction, Main)` semantic port owned by `strip`.
fn strip_main_port(project: &Project, strip: StripId, direction: PortDirection) -> SemanticPortId {
    project.routing.strips[&strip]
        .port_ids
        .iter()
        .copied()
        .find(|id| {
            let p = &project.routing.ports[id];
            p.direction == direction && p.role == PortRole::Main
        })
        .expect("strip must have a Main port in the requested direction")
}

/// Whether `processor` owns a sidechain-role input port.
fn has_sidechain_port(project: &Project, processor: ProcessorId) -> bool {
    project.routing.processors[&processor]
        .port_ids
        .iter()
        .any(|id| project.routing.ports[id].role == PortRole::Sidechain)
}

/// `descriptor` of a processor's binding (Resolved or last-known
/// Unresolved).
fn binding_descriptor(project: &Project, processor: ProcessorId) -> String {
    match &project.routing.processors[&processor].binding {
        ProcessorBinding::Resolved(reference) => reference.descriptor.clone(),
        ProcessorBinding::Unresolved { descriptor } => descriptor.clone(),
    }
}

/// Plugin evidence for every chain processor — last-known authored
/// identity/state, automation identity complete.
fn evidence_for(project: &Project) -> BTreeMap<ProcessorId, ProcessorPluginEvidence> {
    all_processors(project)
        .into_iter()
        .map(|processor| {
            let descriptor = binding_descriptor(project, processor);
            (
                processor,
                ProcessorPluginEvidence {
                    descriptor: descriptor.clone(),
                    identity: identity_for(&descriptor),
                    state: PluginStateDecl {
                        digest: Some(format!("sha256:state-{descriptor}")),
                        quality: StateCaptureQuality::Quiescent,
                        status: OpaqueStateStatus::Known,
                    },
                    automation_identity_complete: true,
                },
            )
        })
        .collect()
}

/// A fully-usable environment entry for `identity` — installed,
/// access granted, format supported, layout matching what the
/// producer declares (`in`/`out` [+ `sidechain`]), verified state
/// restore.
fn live_environment(identity: &PluginIdentity, sidechain: bool) -> PluginEnvironment {
    let mut advertised = vec![
        synvelis_player::rcp::DeclaredPort {
            name: "in".to_owned(),
            direction: Direction::Input,
            domain: SignalDomain::Audio,
            format: PortFormat::Audio(audio()),
        },
        synvelis_player::rcp::DeclaredPort {
            name: "out".to_owned(),
            direction: Direction::Output,
            domain: SignalDomain::Audio,
            format: PortFormat::Audio(audio()),
        },
    ];
    if sidechain {
        advertised.push(synvelis_player::rcp::DeclaredPort {
            name: "sidechain".to_owned(),
            direction: Direction::Input,
            domain: SignalDomain::Audio,
            format: PortFormat::Audio(audio()),
        });
    }
    PluginEnvironment {
        installed: true,
        resolved_version: Some(identity.version.clone()),
        resolved_binary_digest: identity.binary_digest.clone(),
        access: AccessState::Granted,
        format_supported: true,
        advertised_ports: Some(advertised),
        state_restore: StateRestoreSupport::Verified,
        uid_hash: fnv1a(identity.uid.as_bytes()),
    }
}

/// Environments for every processor descriptor — `missing`
/// descriptors get *no* entry (fail-closed `PluginVerdict::Missing`,
/// never a guessed install).
fn environments_for(
    project: &Project,
    missing: &BTreeSet<String>,
) -> BTreeMap<String, PluginEnvironment> {
    all_processors(project)
        .into_iter()
        .map(|p| binding_descriptor(project, p))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|d| !missing.contains(d))
        .map(|d| {
            let identity = identity_for(&d);
            let sidechain = all_processors(project)
                .into_iter()
                .any(|p| binding_descriptor(project, p) == d && has_sidechain_port(project, p));
            (d, live_environment(&identity, sidechain))
        })
        .collect()
}

/// Builds a project: one track strip carrying `specs`
/// `(descriptor, sidechain_input)` processors in order. Returns the
/// controller and the strip id.
fn build_project(specs: &[(&str, bool)]) -> (ProjectController, StripId) {
    let mut controller = ProjectController::new(
        Project::new(project_id(), "sv293"),
        Box::new(CounterIdGenerator::new(100)),
    );
    apply(&mut controller, ProjectOp::AddTrack { name: "t".into() });
    let track = *controller.live().tracks.keys().next_back().unwrap();
    apply(
        &mut controller,
        ProjectOp::AddTrackStrip { track_id: track },
    );
    let strip = controller.live().routing.strip_of_track(track).unwrap().id;
    for (index, (descriptor, sidechain)) in specs.iter().enumerate() {
        apply(
            &mut controller,
            ProjectOp::InsertProcessor {
                strip_id: strip,
                index,
                name: (*descriptor).to_owned(),
                binding: ProcessorBinding::Resolved(plugin_ref(descriptor)),
                sidechain_input: *sidechain,
            },
        );
    }
    (controller, strip)
}

/// [`resolve_selection`] with the fixture's pinned generations.
fn resolve(
    project: &Project,
    revision: ProjectRevision,
    evidence: &BTreeMap<ProcessorId, ProcessorPluginEvidence>,
    environments: &BTreeMap<String, PluginEnvironment>,
    candidates: &[SemanticRenderCandidate],
    render_generation: u64,
    snapshot_id: u64,
) -> synvelis_workspace::rcp_selection::SelectionResolution {
    resolve_selection(&SelectionInputs {
        project,
        project_revision: revision,
        snapshot_id,
        engine: engine(),
        engine_config_generation: ENGINE_CONFIG_GENERATION,
        source_format_generation: SOURCE_FORMAT_GENERATION,
        clock_epoch: EPOCH,
        runtime_instance: INSTANCE,
        plugin_evidence: evidence,
        environments,
        render_index_generation: render_generation,
        render_candidates: candidates,
    })
    .expect("semantic model must resolve")
}

/// A render candidate substituting exactly `processor` — coverage
/// pins its main-in boundary input and reproduces its main-out
/// boundary output, matching what the chain edges require.
fn candidate_for(
    project: &Project,
    processor: ProcessorId,
    eligibility: ArtifactEligibility,
    reproducibility: ReproducibilityClass,
    artifact_byte: u8,
) -> SemanticRenderCandidate {
    SemanticRenderCandidate {
        coverage: SemanticRenderCoverage {
            processors: BTreeSet::from([processor]),
            outputs: BTreeSet::from([processor_main_port(
                project,
                processor,
                PortDirection::Output,
            )]),
            pinned_inputs: BTreeSet::from([processor_main_port(
                project,
                processor,
                PortDirection::Input,
            )]),
            ..SemanticRenderCoverage::default()
        },
        report: CandidateReport {
            artifact: RenderArtifactId(ContentDigest::sha256(&[artifact_byte; 32])),
            eligibility,
            reproducibility,
            access: AccessState::Granted,
            rejection: None,
        },
        content_hash: format!("sha256:candidate-{artifact_byte}"),
    }
}

/// `ExecutionSelector::select` on a resolved snapshot.
fn select(
    resolution: &synvelis_workspace::rcp_selection::SelectionResolution,
    intent: &ExecutionIntent,
) -> ExecutionSelection {
    ExecutionSelector::select(
        &resolution.snapshot,
        intent,
        &resolution.capabilities,
        &resolution.renders,
    )
}

/// A `StaticProcessorHost` serving every chain processor with a
/// verified-offline `GainProcessor` stub — the engine plan leg is
/// real; plugin *executability* stays evidence-driven.
/// `connect` consumes instances, so callers re-arm before each
/// drive that may build (capability generation stays `0` — consumed
/// provisioning is part of the build, not drift).
fn static_host(project: &Project) -> StaticProcessorHost {
    let mut host = StaticProcessorHost::new();
    for processor in all_processors(project) {
        host.insert(processor, Box::new(GainProcessor::new(1.0)));
    }
    host
}

/// A `RoutingEngine` whose host can serve every chain processor.
fn engine_with_host(project: &Project) -> RoutingEngine {
    let mut engine = RoutingEngine::new(engine(), INSTANCE, EPOCH);
    engine.set_processor_host(Box::new(static_host(project)));
    engine
}

fn request(revision: ProjectRevision, mode: BuildMode, id: u64) -> RoutingCompileRequest {
    RoutingCompileRequest {
        request_id: RoutingBuildRequestId(id),
        expected_project_revision: revision,
        mode,
        resource_budget: BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        runtime_instance: INSTANCE,
        device_clock_epoch: EPOCH,
    }
}

/// One `drive_selection_plan` call with fresh reconciler state
/// carried by the caller.
fn drive(
    engine: &mut RoutingEngine,
    reconciler: &mut PlanReconciler,
    project: &Project,
    revision: ProjectRevision,
    resolution: &synvelis_workspace::rcp_selection::SelectionResolution,
    selection: &ExecutionSelection,
    mode: BuildMode,
) -> SelectionPlanDrive {
    // Re-arm consumed host instances for this drive's project —
    // `StaticProcessorHost::connect` takes ownership, and a failed
    // or equivalent prepare must not silently starve the next build.
    engine.set_processor_host(Box::new(static_host(project)));
    let canonical_of = |_: SourceId| -> Option<AudioFormat> { None };
    let req = request(revision, mode, 1);
    drive_selection_plan(PlanDrive {
        engine,
        reconciler,
        project,
        revision,
        live: &resolution.snapshot,
        capabilities: &resolution.capabilities,
        selection,
        request: &req,
        deps: RoutingDeps {
            canonical_of: &canonical_of,
            clips: Vec::new(),
            engine_config_generation: ENGINE_CONFIG_GENERATION,
            source_format_generation: SOURCE_FORMAT_GENERATION,
        },
    })
}

/// The mode `ExecutionSelector` pins into `receipt.build_key` for
/// `intent` — the engine request must match it so `semantic_eq`
/// equivalence holds across drives.
fn mode_of(intent: &ExecutionIntent) -> BuildMode {
    match intent {
        ExecutionIntent::OfflineRender { .. } | ExecutionIntent::Export => BuildMode::Offline,
        _ => BuildMode::Realtime,
    }
}

/// The `RcpNodeId` the producer assigned to `processor` — found via
/// the display label the producer stamps (`processor:{id}`).
fn node_of_processor(
    resolution: &synvelis_workspace::rcp_selection::SelectionResolution,
    processor: ProcessorId,
) -> synvelis_player::rcp::RcpNodeId {
    let label = format!("processor:{processor}");
    resolution
        .snapshot
        .nodes
        .values()
        .find(|n| n.label.as_deref() == Some(label.as_str()))
        .map(|n| n.id)
        .expect("processor node must exist in snapshot")
}

// ---------------------------------------------------------------------------
// RCP-EX-01 — per-intent Plan/Blocked/Preview + substitution
// ---------------------------------------------------------------------------

/// RCP-EX-01: all plugins Live → `MainPlayback` selects `Exact` and
/// the selection drives a real engine publish; the receipt build key
/// is field-identical to the engine's `checked_key` inputs
/// (`routing_fingerprint` is the shared digest).
#[test]
fn rcp_ex01_main_playback_all_live_publishes() {
    let (controller, _strip) = build_project(&[("acme.eq", false), ("acme.comp", false)]);
    let revision = controller.live_revision();
    let project = controller.live().clone();
    let evidence = evidence_for(&project);
    let environments = environments_for(&project, &BTreeSet::new());
    let resolution = resolve(
        &project,
        revision,
        &evidence,
        &environments,
        &[],
        RENDER_INDEX_GENERATION,
        1,
    );

    // Snapshot invariants: strips + processors + sink, chain order
    // preserved as edges, master out feeds the device sink.
    assert!(
        resolution
            .snapshot
            .nodes
            .values()
            .any(|n| n.class == synvelis_player::rcp::NodeClass::Sink)
    );
    assert_eq!(
        resolution.snapshot.structure_digest,
        routing_fingerprint(&project).0,
        "structure digest must be the routing fingerprint so the \
         selection build key matches the engine checked_key"
    );

    let selection = select(&resolution, &ExecutionIntent::MainPlayback);
    assert_eq!(selection.verdict, SelectionVerdict::Exact);
    assert_eq!(selection.substitutions.len(), 0);
    assert_eq!(selection.unresolved.len(), 0);
    // Build key pins: receipt carries the same fingerprint the
    // engine recomputes — the integration seam this slice owns.
    assert_eq!(
        selection.receipt.build_key.routing_fingerprint.0,
        routing_fingerprint(&project).0
    );
    assert_eq!(
        selection.receipt.build_key.capability_generation,
        resolution.snapshot.capability_generation
    );

    let mut engine = engine_with_host(&project);
    let mut reconciler = PlanReconciler::new();
    let driven = drive(
        &mut engine,
        &mut reconciler,
        &project,
        revision,
        &resolution,
        &selection,
        mode_of(&selection.intent),
    );
    let SelectionPlanDrive::Published {
        engine_receipt,
        reconcile_receipt,
        ..
    } = driven
    else {
        panic!("expected Published");
    };
    assert!(matches!(
        engine_receipt.state,
        RoutingActivationState::Published { .. }
    ));
    assert!(matches!(
        reconcile_receipt.state,
        RoutingActivationState::Published { .. }
    ));
    assert!(reconciler.active().is_some());
}

/// RCP-EX-01: a missing plugin with no render evidence →
/// `MainPlayback` is `Blocked` (never a fake exact), the unresolved
/// node carries the typed `Missing` verdict.
#[test]
fn rcp_ex01_missing_plugin_blocks_main_playback() {
    let (controller, _strip) = build_project(&[("acme.eq", false), ("acme.comp", false)]);
    let revision = controller.live_revision();
    let project = controller.live().clone();
    let missing_proc = *project
        .routing
        .processors
        .keys()
        .find(|p| binding_descriptor(&project, **p) == "acme.comp")
        .unwrap();
    let evidence = evidence_for(&project);
    // "acme.comp" has no environment entry → PluginVerdict::Missing.
    let environments = environments_for(&project, &BTreeSet::from(["acme.comp".to_owned()]));
    let resolution = resolve(
        &project,
        revision,
        &evidence,
        &environments,
        &[],
        RENDER_INDEX_GENERATION,
        1,
    );

    let selection = select(&resolution, &ExecutionIntent::MainPlayback);
    assert_eq!(selection.verdict, SelectionVerdict::Blocked);
    assert_eq!(selection.substitutions.len(), 0);
    let missing_node = node_of_processor(&resolution, missing_proc);
    assert!(selection.unresolved.iter().any(|u| u.node == missing_node));
    assert_eq!(
        selection.receipt.plugin_verdicts.get(&missing_node),
        Some(&PluginVerdict::Missing)
    );
}

/// RCP-EX-01: missing plugin + `EligibleExact`/`BitExactCertified`
/// render candidate covering exactly that processor → substitution
/// accepted, verdict stays `Exact`, plan publishes.
#[test]
fn rcp_ex01_missing_plugin_exact_substitution_publishes() {
    let (controller, _strip) = build_project(&[("acme.eq", false), ("acme.comp", false)]);
    let revision = controller.live_revision();
    let project = controller.live().clone();
    let missing_proc = *project
        .routing
        .processors
        .keys()
        .find(|p| binding_descriptor(&project, **p) == "acme.comp")
        .unwrap();
    let evidence = evidence_for(&project);
    let environments = environments_for(&project, &BTreeSet::from(["acme.comp".to_owned()]));
    let candidates = vec![candidate_for(
        &project,
        missing_proc,
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
        0xC0,
    )];
    let resolution = resolve(
        &project,
        revision,
        &evidence,
        &environments,
        &candidates,
        RENDER_INDEX_GENERATION,
        1,
    );

    let selection = select(&resolution, &ExecutionIntent::MainPlayback);
    assert_eq!(selection.verdict, SelectionVerdict::Exact);
    assert_eq!(selection.substitutions.len(), 1);
    assert_eq!(
        selection.substitutions[0].nodes,
        BTreeSet::from([node_of_processor(&resolution, missing_proc)])
    );
    assert!(!selection.substitutions[0].preview_class);

    let mut engine = engine_with_host(&project);
    let mut reconciler = PlanReconciler::new();
    let driven = drive(
        &mut engine,
        &mut reconciler,
        &project,
        revision,
        &resolution,
        &selection,
        mode_of(&selection.intent),
    );
    assert!(matches!(driven, SelectionPlanDrive::Published { .. }));
}

/// RCP-EX-01: `OfflineRender` enforces `min_reproducibility` — a
/// `VerifiedWithinTolerance` artifact is below
/// `BitExactCertified` and cannot substitute; the same artifact at
/// the required class substitutes and publishes under
/// `BuildMode::Offline`.
#[test]
fn rcp_ex01_offline_render_enforces_min_reproducibility() {
    let (controller, _strip) = build_project(&[("acme.eq", false), ("acme.comp", false)]);
    let revision = controller.live_revision();
    let project = controller.live().clone();
    let missing_proc = *project
        .routing
        .processors
        .keys()
        .find(|p| binding_descriptor(&project, **p) == "acme.comp")
        .unwrap();
    let evidence = evidence_for(&project);
    let environments = environments_for(&project, &BTreeSet::from(["acme.comp".to_owned()]));

    // Below-minimum reproducibility → gate rejects → Blocked.
    let weak = vec![candidate_for(
        &project,
        missing_proc,
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::VerifiedWithinTolerance,
        0xC1,
    )];
    let resolution = resolve(
        &project,
        revision,
        &evidence,
        &environments,
        &weak,
        RENDER_INDEX_GENERATION,
        1,
    );
    let intent = ExecutionIntent::OfflineRender {
        min_reproducibility: ReproducibilityClass::BitExactCertified,
    };
    let selection = select(&resolution, &intent);
    assert_eq!(selection.verdict, SelectionVerdict::Blocked);
    assert_eq!(selection.substitutions.len(), 0);
    assert!(
        selection
            .receipt
            .candidate_attempts
            .iter()
            .any(|a| !matches!(a.outcome, synvelis_player::rcp::AttemptOutcome::Accepted))
    );

    // Certified reproducibility → substitution → Exact → publish.
    let strong = vec![candidate_for(
        &project,
        missing_proc,
        ArtifactEligibility::EligibleExact,
        ReproducibilityClass::BitExactCertified,
        0xC2,
    )];
    let resolution = resolve(
        &project,
        revision,
        &evidence,
        &environments,
        &strong,
        RENDER_INDEX_GENERATION,
        2,
    );
    let selection = select(&resolution, &intent);
    assert_eq!(selection.verdict, SelectionVerdict::Exact);
    assert_eq!(selection.receipt.build_key.build_mode, BuildMode::Offline);

    let mut engine = engine_with_host(&project);
    let mut reconciler = PlanReconciler::new();
    let driven = drive(
        &mut engine,
        &mut reconciler,
        &project,
        revision,
        &resolution,
        &selection,
        mode_of(&selection.intent),
    );
    assert!(matches!(driven, SelectionPlanDrive::Published { .. }));
}

// ---------------------------------------------------------------------------
// RCP-EX-03 — no fake Exact from non-exact artifacts; preview opt-in
// ---------------------------------------------------------------------------

/// RCP-EX-03: `Export` (strictest intent) rejects a
/// `EligiblePreview`/`PreviewOnly` artifact outright — no
/// substitution, `Blocked`, no fake exact. The same artifact under
/// `EditingPreview` *with* explicit opt-in substitutes as
/// preview-class (`NonEquivalent`); without opt-in it blocks.
#[test]
fn rcp_ex03_export_rejects_preview_only_artifact() {
    let (controller, _strip) = build_project(&[("acme.eq", false), ("acme.comp", false)]);
    let revision = controller.live_revision();
    let project = controller.live().clone();
    let missing_proc = *project
        .routing
        .processors
        .keys()
        .find(|p| binding_descriptor(&project, **p) == "acme.comp")
        .unwrap();
    let evidence = evidence_for(&project);
    let environments = environments_for(&project, &BTreeSet::from(["acme.comp".to_owned()]));
    let candidates = vec![candidate_for(
        &project,
        missing_proc,
        ArtifactEligibility::EligiblePreview,
        ReproducibilityClass::PreviewOnly,
        0xE0,
    )];
    let resolution = resolve(
        &project,
        revision,
        &evidence,
        &environments,
        &candidates,
        RENDER_INDEX_GENERATION,
        1,
    );

    // Export: preview-class artifact can never satisfy the gate.
    let export = select(&resolution, &ExecutionIntent::Export);
    assert_eq!(export.verdict, SelectionVerdict::Blocked);
    assert_eq!(export.substitutions.len(), 0);
    assert!(
        export
            .unresolved
            .iter()
            .any(|u| { u.node == node_of_processor(&resolution, missing_proc) })
    );

    // MainPlayback: same — EligiblePreview is not EligibleExact.
    let playback = select(&resolution, &ExecutionIntent::MainPlayback);
    assert_eq!(playback.verdict, SelectionVerdict::Blocked);
    assert_eq!(playback.substitutions.len(), 0);

    // EditingPreview WITH opt-in: preview-class substitution is
    // accepted and honestly marked non-equivalent.
    let preview_opt_in = select(
        &resolution,
        &ExecutionIntent::EditingPreview {
            live_regions: Vec::new(),
            allow_non_equivalent: true,
        },
    );
    assert_eq!(preview_opt_in.verdict, SelectionVerdict::NonEquivalent);
    assert_eq!(preview_opt_in.substitutions.len(), 1);
    assert!(preview_opt_in.substitutions[0].preview_class);
    assert!(preview_opt_in.non_equivalent);

    // EditingPreview WITHOUT opt-in: the preview-class artifact is
    // gated out → unresolved → Blocked (fail closed, no silent
    // degradation).
    let preview_strict = select(
        &resolution,
        &ExecutionIntent::EditingPreview {
            live_regions: Vec::new(),
            allow_non_equivalent: false,
        },
    );
    assert_eq!(preview_strict.verdict, SelectionVerdict::Blocked);
}

// ---------------------------------------------------------------------------
// RCP-EX-04 — edit classification + NoRebuild/Equivalent lifecycle
// ---------------------------------------------------------------------------

/// 公開済み plan を持つ fixture — project/controller・engine・
/// reconciler・直近の resolution/selection をまとめて返す
/// （ex04/ex05 系の lifecycle test 共通の起点）。
struct PublishedFixture {
    controller: ProjectController,
    revision: ProjectRevision,
    resolution: synvelis_workspace::rcp_selection::SelectionResolution,
    selection: ExecutionSelection,
    engine: RoutingEngine,
    reconciler: PlanReconciler,
}

/// 単一 strip project を構築し、MainPlayback selection を
/// `drive_selection_plan` で publish まで進めた fixture を返す。
fn published_fixture(specs: &[(&str, bool)]) -> PublishedFixture {
    let (controller, _strip) = build_project(specs);
    let revision = controller.live_revision();
    let project = controller.live().clone();
    let evidence = evidence_for(&project);
    let environments = environments_for(&project, &BTreeSet::new());
    let resolution = resolve(
        &project,
        revision,
        &evidence,
        &environments,
        &[],
        RENDER_INDEX_GENERATION,
        1,
    );
    let intent = ExecutionIntent::MainPlayback;
    let selection = select(&resolution, &intent);
    assert_eq!(selection.verdict, SelectionVerdict::Exact);
    let mut engine = engine_with_host(&project);
    let mut reconciler = PlanReconciler::new();
    let driven = drive(
        &mut engine,
        &mut reconciler,
        &project,
        revision,
        &resolution,
        &selection,
        mode_of(&intent),
    );
    assert!(matches!(driven, SelectionPlanDrive::Published { .. }));
    PublishedFixture {
        controller,
        revision,
        resolution,
        selection,
        engine,
        reconciler,
    }
}

/// RCP-EX-04 (rename leg): after a published plan, a rename
/// (presentation edit) classifies `NoRebuild`, bound-parameter
/// edits classify `ControlOnly`, and the re-selection drives
/// engine `Equivalent` (no publish) — render-index movement stays
/// independent of plan rebuild.
#[test]
fn rcp_ex04_rename_norebuild_controlonly() {
    let mut fx = published_fixture(&[("acme.eq", false)]);
    let plan_context = PlanContext::from_selection(&fx.selection.receipt, RENDER_INDEX_GENERATION);
    let intent = ExecutionIntent::MainPlayback;

    // --- rename: presentation-only edit --------------------------
    apply(
        &mut fx.controller,
        ProjectOp::SetProjectName {
            name: "sv293-renamed".to_owned(),
        },
    );
    let rev2 = fx.controller.live_revision();
    assert_ne!(rev2, fx.revision);
    let renamed = fx.controller.live().clone();
    // Fingerprint must be unchanged — display names are excluded.
    assert_eq!(
        fx.resolution.snapshot.structure_digest,
        routing_fingerprint(&renamed).0
    );
    let renamed_resolution = resolve(
        &renamed,
        rev2,
        &evidence_for(&renamed),
        &environments_for(&renamed, &BTreeSet::new()),
        &[],
        RENDER_INDEX_GENERATION,
        2,
    );
    let renamed_selection = select(&renamed_resolution, &intent);

    // classify_edit: presentation edit on identical graph → NoRebuild.
    assert_eq!(
        classify_edit(
            &plan_context,
            &renamed_resolution.snapshot,
            RENDER_INDEX_GENERATION,
            EditKind::Presentation,
        ),
        EditClass::NoRebuild
    );
    // Bound-parameter edit on identical graph → ControlOnly.
    match classify_edit(
        &plan_context,
        &renamed_resolution.snapshot,
        RENDER_INDEX_GENERATION,
        EditKind::BoundParameter,
    ) {
        EditClass::ControlOnly { .. } => {}
        _ => panic!("expected ControlOnly"),
    }

    // drive: engine equivalence — same semantic build key except
    // revision → NoRebuild evidence, nothing published.
    let driven = drive(
        &mut fx.engine,
        &mut fx.reconciler,
        &renamed,
        rev2,
        &renamed_resolution,
        &renamed_selection,
        mode_of(&intent),
    );
    let SelectionPlanDrive::Equivalent { equivalence, .. } = driven else {
        panic!("expected Equivalent");
    };
    assert_eq!(
        equivalence.configured_project_revision, rev2,
        "equivalence must name the newly configured revision"
    );
}

/// RCP-EX-04 (structural leg): adding a track strip moves the
/// routing fingerprint → `classify_edit` reports `Rebuild` and the
/// re-selection drives a real rebuild → new publish on a newer
/// generation.
#[test]
fn rcp_ex04_structural_edit_rebuilds_and_republishes() {
    let mut fx = published_fixture(&[("acme.eq", false)]);
    let intent = ExecutionIntent::MainPlayback;
    let before_context =
        PlanContext::from_selection(&fx.selection.receipt, RENDER_INDEX_GENERATION);

    // --- structural edit: add a second track strip ---------------
    apply(&mut fx.controller, ProjectOp::AddTrack { name: "u".into() });
    let track2 = *fx.controller.live().tracks.keys().next_back().unwrap();
    apply(
        &mut fx.controller,
        ProjectOp::AddTrackStrip { track_id: track2 },
    );
    let rev2 = fx.controller.live_revision();
    let grown = fx.controller.live().clone();
    assert_ne!(
        fx.resolution.snapshot.structure_digest,
        routing_fingerprint(&grown).0,
        "structural edit must move the routing fingerprint"
    );
    let grown_resolution = resolve(
        &grown,
        rev2,
        &evidence_for(&grown),
        &environments_for(&grown, &BTreeSet::new()),
        &[],
        RENDER_INDEX_GENERATION,
        2,
    );
    let grown_selection = select(&grown_resolution, &intent);
    assert_eq!(grown_selection.verdict, SelectionVerdict::Exact);

    // classify_edit against the pre-edit context: the structure
    // moved → Rebuild.
    assert_eq!(
        classify_edit(
            &before_context,
            &grown_resolution.snapshot,
            RENDER_INDEX_GENERATION,
            EditKind::Structural,
        ),
        EditClass::Rebuild
    );

    // drive: real rebuild → new publish on a newer generation.
    let driven = drive(
        &mut fx.engine,
        &mut fx.reconciler,
        &grown,
        rev2,
        &grown_resolution,
        &grown_selection,
        mode_of(&intent),
    );
    match driven {
        SelectionPlanDrive::Published {
            reconcile_receipt, ..
        } => {
            assert!(matches!(
                reconcile_receipt.state,
                RoutingActivationState::Published { .. }
            ));
        }
        SelectionPlanDrive::Failed { fault, .. } => {
            panic!("expected Published after structural edit, got fault {fault:?}")
        }
        SelectionPlanDrive::Equivalent { .. } => {
            panic!("expected Published after structural edit")
        }
    }
}

// ---------------------------------------------------------------------------
// RCP-EX-05 — stale selection never publishes; last-good retained
// ---------------------------------------------------------------------------

/// RCP-EX-05: a selection resolved at revision N driven against a
/// live snapshot at N+1 is rejected by the pre-publish stale gate —
/// the engine is never invoked, the reconciler records `Blocked`,
/// and a subsequent fresh selection still publishes (last-good /
/// current path intact).
#[test]
fn rcp_ex05_stale_selection_never_publishes() {
    let (mut controller, _strip) = build_project(&[("acme.eq", false)]);
    let revision = controller.live_revision();
    let project = controller.live().clone();
    let evidence = evidence_for(&project);
    let environments = environments_for(&project, &BTreeSet::new());
    let resolution = resolve(
        &project,
        revision,
        &evidence,
        &environments,
        &[],
        RENDER_INDEX_GENERATION,
        1,
    );
    let intent = ExecutionIntent::MainPlayback;
    let selection = select(&resolution, &intent);
    assert_eq!(selection.verdict, SelectionVerdict::Exact);

    // Publish a good plan first — last-good exists.
    let mut engine = engine_with_host(&project);
    let mut reconciler = PlanReconciler::new();
    let published = drive(
        &mut engine,
        &mut reconciler,
        &project,
        revision,
        &resolution,
        &selection,
        mode_of(&intent),
    );
    assert!(matches!(published, SelectionPlanDrive::Published { .. }));

    // Rename commits → revision moves, graph identical.
    apply(
        &mut controller,
        ProjectOp::SetProjectName {
            name: "sv293-stale".to_owned(),
        },
    );
    let rev2 = controller.live_revision();
    let renamed = controller.live().clone();
    let live_resolution = resolve(
        &renamed,
        rev2,
        &evidence,
        &environments,
        &[],
        RENDER_INDEX_GENERATION,
        2,
    );

    // Drive the *old* selection against the *new* live snapshot —
    // the pre-publish stale gate must refuse before the engine runs.
    // (Engine-side inputs stay consistent at rev2, so a Published
    // result here would prove the gate failed to fire.)
    let stale_drive = drive(
        &mut engine,
        &mut reconciler,
        &renamed,
        rev2,
        &live_resolution,
        &selection,
        mode_of(&intent),
    );
    let SelectionPlanDrive::Failed {
        fault,
        reconcile_receipt,
        ..
    } = stale_drive
    else {
        panic!("expected Failed for stale selection");
    };
    assert_eq!(fault, synvelis_project::RoutingFault::StaleProjectSnapshot);
    assert!(matches!(
        reconcile_receipt.state,
        RoutingActivationState::Blocked {
            fault: synvelis_project::RoutingFault::StaleProjectSnapshot,
            ..
        }
    ));

    // Fresh selection at rev2 → publishes normally; last-good chain
    // intact (active plan advances, previous becomes last-good).
    let fresh_selection = select(&live_resolution, &intent);
    assert_eq!(fresh_selection.verdict, SelectionVerdict::Exact);
    let fresh_drive = drive(
        &mut engine,
        &mut reconciler,
        &renamed,
        rev2,
        &live_resolution,
        &fresh_selection,
        mode_of(&intent),
    );
    match fresh_drive {
        SelectionPlanDrive::Published {
            reconcile_receipt, ..
        } => {
            assert!(matches!(
                reconcile_receipt.state,
                RoutingActivationState::Published { .. }
            ));
            // The superseded plan is retained as last-good evidence.
            assert!(
                reconcile_receipt.last_good_generation.is_some()
                    || reconciler.last_good().is_some()
            );
        }
        // Equivalent is also acceptable here only if the engine saw
        // identical semantics — but revision moved so a fresh
        // selection at rev2 still builds (revision is part of the
        // key's semantic_eq-exempt field; equivalence requires the
        // *live published* revision to differ — it does, so
        // Equivalent is the honest NoRebuild outcome).
        SelectionPlanDrive::Equivalent { .. } => {}
        SelectionPlanDrive::Failed { .. } => {
            panic!("expected Published or Equivalent")
        }
    }
}

// ---------------------------------------------------------------------------
// RCP-EX-02 — topology: send/return/sidechain edge kinds survive
// ---------------------------------------------------------------------------

/// RCP-EX-02 topology fixture: strip A carries eq→comp (comp has a
/// sidechain input fed by strip B's output), strip B carries a
/// gain, and a return bus receives a post-fader send from A whose
/// output feeds the master. Returns
/// `(controller, strip_a, strip_b, return_strip, comp_proc)`.
fn topology_project() -> (ProjectController, StripId, StripId, StripId, ProcessorId) {
    let mut controller = ProjectController::new(
        Project::new(project_id(), "sv293-topo"),
        Box::new(CounterIdGenerator::new(200)),
    );
    let add_track_strip = |controller: &mut ProjectController, name: &str| {
        apply(controller, ProjectOp::AddTrack { name: name.into() });
        let track = *controller.live().tracks.keys().next_back().unwrap();
        apply(controller, ProjectOp::AddTrackStrip { track_id: track });
        controller.live().routing.strip_of_track(track).unwrap().id
    };
    let strip_a = add_track_strip(&mut controller, "a");
    let strip_b = add_track_strip(&mut controller, "b");

    // Return bus for the send.
    apply(
        &mut controller,
        ProjectOp::AddBus {
            kind: BusKind::Return,
            name: "rev".into(),
        },
    );
    let return_bus = *controller
        .live()
        .routing
        .buses
        .keys()
        .find(|b| controller.live().routing.buses[b].kind == BusKind::Return)
        .unwrap();
    let return_strip = controller.live().routing.buses[&return_bus].strip;

    // Processors: A gets eq→comp(sidechain); B gets a gain.
    let insert = |controller: &mut ProjectController,
                  strip: StripId,
                  index: usize,
                  descriptor: &str,
                  sidechain: bool| {
        apply(
            controller,
            ProjectOp::InsertProcessor {
                strip_id: strip,
                index,
                name: descriptor.to_owned(),
                binding: ProcessorBinding::Resolved(plugin_ref(descriptor)),
                sidechain_input: sidechain,
            },
        );
    };
    insert(&mut controller, strip_a, 0, "acme.eq", false);
    insert(&mut controller, strip_a, 1, "acme.comp", true);
    insert(&mut controller, strip_b, 0, "acme.gain", false);

    // Semantic port lookups — `controller.live()` borrows end at
    // each call so `apply` can take `&mut controller` between them.
    let comp_proc = chain_processors(controller.live(), strip_a)[1];
    let comp_sidechain_in = {
        let project = controller.live();
        project.routing.processors[&comp_proc]
            .port_ids
            .iter()
            .copied()
            .find(|id| project.routing.ports[id].role == PortRole::Sidechain)
            .unwrap()
    };
    let master_strip = controller.live().routing.buses[&controller.live().routing.master].strip;

    // B output → A's comp sidechain input.
    let sidechain_src = strip_main_port(controller.live(), strip_b, PortDirection::Output);
    apply(
        &mut controller,
        ProjectOp::AddConnection {
            source: sidechain_src,
            destination: comp_sidechain_in,
            kind: ConnectionKind::Sidechain,
            channel_map: None,
        },
    );
    // Send: A main out → return strip main in (post-fader).
    let send_src = strip_main_port(controller.live(), strip_a, PortDirection::Output);
    let send_dst = strip_main_port(controller.live(), return_strip, PortDirection::Input);
    apply(
        &mut controller,
        ProjectOp::AddSend {
            source: send_src,
            destination: send_dst,
            tap: synvelis_project::SendTap::PostFader,
            gain_target: None,
        },
    );
    // Return strip output → master input (the return feed).
    let return_src = strip_main_port(controller.live(), return_strip, PortDirection::Output);
    let return_dst = strip_main_port(controller.live(), master_strip, PortDirection::Input);
    apply(
        &mut controller,
        ProjectOp::AddConnection {
            source: return_src,
            destination: return_dst,
            kind: ConnectionKind::Main,
            channel_map: None,
        },
    );
    (controller, strip_a, strip_b, return_strip, comp_proc)
}

/// RCP-EX-02 (topology leg): sends, return-bus feeds, sidechain
/// inputs and chain order all survive semantic→snapshot mapping as
/// typed edges — the cutset layer downstream can distinguish them.
#[test]
fn rcp_ex02_topology_edge_kinds_preserved() {
    let (controller, strip_a, _strip_b, _return_strip, comp_proc) = topology_project();
    let revision = controller.live_revision();
    let project = controller.live().clone();
    let evidence = evidence_for(&project);
    let environments = environments_for(&project, &BTreeSet::new());
    let resolution = resolve(
        &project,
        revision,
        &evidence,
        &environments,
        &[],
        RENDER_INDEX_GENERATION,
        1,
    );

    for kind in [
        EdgeKind::Send,
        EdgeKind::Sidechain,
        EdgeKind::Return,
        EdgeKind::Main,
    ] {
        assert!(
            resolution.snapshot.edges.iter().any(|e| e.kind == kind),
            "edge kind {kind:?} missing"
        );
    }

    // Chain order is structural: A's strip → eq → comp → chain_out
    // must appear as consecutive Main edges.
    let eq_proc = chain_processors(&project, strip_a)[0];
    let eq_node = node_of_processor(&resolution, eq_proc);
    let comp_node = node_of_processor(&resolution, comp_proc);
    let chain_edges: Vec<_> = resolution
        .snapshot
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Main)
        .filter(|e| {
            [eq_node, comp_node].contains(&e.from.node) || [eq_node, comp_node].contains(&e.to.node)
        })
        .collect();
    assert!(
        chain_edges
            .iter()
            .any(|e| e.from.node == eq_node && e.to.node == comp_node),
        "eq→comp chain edge missing"
    );
    // The comp sidechain input is fed by B's output — a Sidechain
    // edge into the comp node's sidechain port.
    assert!(
        resolution
            .snapshot
            .edges
            .iter()
            .any(|e| { e.kind == EdgeKind::Sidechain && e.to.node == comp_node })
    );

    // Full selection still resolves Exact with everything Live.
    let selection = select(&resolution, &ExecutionIntent::MainPlayback);
    assert_eq!(selection.verdict, SelectionVerdict::Exact);
}

// ---------------------------------------------------------------------------
// producer fail-closed contract
// ---------------------------------------------------------------------------

/// The producer refuses to guess: missing plugin evidence, evidence
/// whose descriptor disagrees with the binding, and render coverage
/// referencing unknown semantic entities are all typed errors.
#[test]
fn rcp_producer_fails_closed_on_bad_evidence() {
    let (controller, _strip) = build_project(&[("acme.eq", false)]);
    let revision = controller.live_revision();
    let project = controller.live().clone();
    let processor = all_processors(&project)[0];

    // Missing evidence → MissingPluginEvidence.
    let err = resolve_selection(&SelectionInputs {
        project: &project,
        project_revision: revision,
        snapshot_id: 1,
        engine: engine(),
        engine_config_generation: ENGINE_CONFIG_GENERATION,
        source_format_generation: SOURCE_FORMAT_GENERATION,
        clock_epoch: EPOCH,
        runtime_instance: INSTANCE,
        plugin_evidence: &BTreeMap::new(),
        environments: &BTreeMap::new(),
        render_index_generation: RENDER_INDEX_GENERATION,
        render_candidates: &[],
    })
    .err()
    .expect("missing evidence must fail closed");
    assert_eq!(
        err,
        SelectionBuildError::MissingPluginEvidence { processor }
    );

    // Descriptor mismatch → DescriptorMismatch.
    let mut bad_evidence = evidence_for(&project);
    bad_evidence.get_mut(&processor).unwrap().descriptor = "acme.other".to_owned();
    let err = resolve_selection(&SelectionInputs {
        project: &project,
        project_revision: revision,
        snapshot_id: 1,
        engine: engine(),
        engine_config_generation: ENGINE_CONFIG_GENERATION,
        source_format_generation: SOURCE_FORMAT_GENERATION,
        clock_epoch: EPOCH,
        runtime_instance: INSTANCE,
        plugin_evidence: &bad_evidence,
        environments: &BTreeMap::new(),
        render_index_generation: RENDER_INDEX_GENERATION,
        render_candidates: &[],
    })
    .err()
    .expect("descriptor mismatch must fail closed");
    assert!(matches!(
        err,
        SelectionBuildError::DescriptorMismatch { .. }
    ));

    // Coverage naming an unknown processor → UnknownCoverage.
    let evidence = evidence_for(&project);
    let environments = environments_for(&project, &BTreeSet::new());
    let ghost = SemanticRenderCandidate {
        coverage: SemanticRenderCoverage {
            processors: BTreeSet::from([ProcessorId::new(999_999)]),
            ..SemanticRenderCoverage::default()
        },
        report: CandidateReport {
            artifact: RenderArtifactId(ContentDigest::sha256(&[0xFF; 32])),
            eligibility: ArtifactEligibility::EligibleExact,
            reproducibility: ReproducibilityClass::BitExactCertified,
            access: AccessState::Granted,
            rejection: None,
        },
        content_hash: "sha256:ghost".to_owned(),
    };
    let err = resolve_selection(&SelectionInputs {
        project: &project,
        project_revision: revision,
        snapshot_id: 1,
        engine: engine(),
        engine_config_generation: ENGINE_CONFIG_GENERATION,
        source_format_generation: SOURCE_FORMAT_GENERATION,
        clock_epoch: EPOCH,
        runtime_instance: INSTANCE,
        plugin_evidence: &evidence,
        environments: &environments,
        render_index_generation: RENDER_INDEX_GENERATION,
        render_candidates: &[ghost],
    })
    .err()
    .expect("unknown coverage must fail closed");
    assert!(matches!(err, SelectionBuildError::UnknownCoverage { .. }));
}
