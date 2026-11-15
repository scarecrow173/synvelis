//! RCP-07 cross-gate headless fixtures (synvelis#296;
//! `docs/test-specs/rcp-07-cross-gate.md`).
//!
//! These tests compose the merged RCP-01..05 foundations across their
//! seams — canonical recipe → artifact store → invalidation →
//! policy/job → branch pin → missing-plugin resolver — through public
//! APIs only. They pin *decision contracts* at contract level: a
//! headless PASS here is never promoted to real-plugin, real-OS,
//! real-GUI, RT, or consumer acceptance. Those lanes stay `NotRun`
//! in `evidence/sv296/` (RCP-GATE-08..10).

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_graph::{
    Activation, Availability, Direction, EdgeDelay, Multiplicity, PortFormat, PortRole,
    SignalDomain,
};
use synvelis_player::rcp::{
    AccessState, ArtifactEligibility, ArtifactOutput, BoundaryPurpose, BoundaryRequirement,
    DeclaredPort, EdgeKind, ExecutionIntent, ExecutionSelector, NodeClass,
    PluginCapabilitySnapshot, PluginDeclaration, PluginFormat, PluginIdentity, PluginStateDecl,
    PortEndpoint, RcpEdgeId, RcpNodeId, RcpPortId, RenderCandidate, RenderCandidateIndex,
    ReproducibilityClass, ResolvedEdge, ResolvedGraphSnapshot, ResolvedNode, ResolvedPort,
    SelectionVerdict,
};
use synvelis_project::apply::ApplyApproval;
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{ObjectKind, SemanticObjectRef};
use synvelis_project::id::{CounterIdGenerator, ProjectId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::placement::{
    ClipExtent, ClipPlacement, ClipPlayback, PlaybackMapping, TimeAnchor,
};
use synvelis_project::project::Project;
use synvelis_project::range::SourceRange;
use synvelis_project::snapshot::{
    ContentDigest, FixedSnapshotLocator, SnapshotCancel, SnapshotIdentity, SnapshotOrigin,
    SnapshotResolver,
};
use synvelis_project::store::SCHEMA_VERSION;
use synvelis_project::time::{MediaDuration, MediaTimeValue, SourceFrame};
use synvelis_project::variant::{
    AdoptIntent, AdoptSources, BranchEditIntent, FakeDomainAdapter, InMemoryBranchContentStore,
    RuntimePublish, RuntimePublishEvidence, RuntimePublishRequest, RuntimePublishState,
    VariantScope, adopt_branch, edit_branch, fork_variant_set, scope_digest,
};
use synvelis_project::{
    BranchId, ClipId, LaneId, OpaqueStateStatus, RENDER_INPUT_SCHEMA_VERSION, SourceId,
    SourceState, StateCaptureQuality, VariantSetId,
    render_input::{RenderBranchTarget, resolve_render_project},
};
use synvelis_render::artifact::gc::{GcPolicy, sweep};
use synvelis_render::artifact::lookup::{
    CacheLookup, IntendedUse, LookupOutcome, TrustScope, lookup,
};
use synvelis_render::artifact::manifest::{
    ArtifactManifest, ArtifactPortSpec, ArtifactProvenance, ArtifactReproducibility, CodecClass,
    CodecDescriptor, RenderArtifactId,
};
use synvelis_render::artifact::publish::{
    ArtifactDraft, PublishExpectation, PublishOutcome, publish_artifact,
};
use synvelis_render::artifact::store::{
    ArtifactReference, ArtifactStore, FaultPoint, InMemoryArtifactStore, StoreError,
};
use synvelis_render::invalidation::{ChangedObject, classify_invalidation};
use synvelis_render::policy::{
    DemandAuthorization, DemandAuthorizer, DemandProposal, JobIntent, LocalGrants, PolicyPatch,
    PolicyResolver, RenderScope, RenderSettingsRef, RenderTrigger, ScopeDemandResolver,
    TriggerEvent, TriggerOutcome, TriggerProvenance, TriggerReport, resolve_effective_policy,
};
use synvelis_render::recipe::{
    AUDIO_SEMANTICS_VERSION, CANONICALIZATION_VERSION, ClockDependency, DenormalMode,
    FrameInterval, OutputContract, OutputPortContract, PcmFormat, RECIPE_DOMAIN_TAG,
    RECIPE_HASH_ALGORITHM, RECIPE_SCHEMA_VERSION, RecipeKey, RecipeRecord, RecipeResolution,
    RecipeTarget, RenderBoundary, RenderPrecision, RenderRecipeV1, RendererProfile,
    ResolutionProvenance, TemporalDomain, Validity, evaluate_validity, recipe_record,
};
use synvelis_render::render_job::{
    BackendRequirement, EntrySurface, GatewayActionRef, JobContext, JobCostEstimate,
    JobOrchestrator, JobPurpose, PinnedSnapshot, RenderDemand, RenderExecution, RenderExecutor,
    RenderJobKey, RenderJobState, RenderQualityInputs, RenderSelection, RenderSettingsProbe,
    RenderedOutput, SubmitOutcome, TemporalQuality,
};
use synvelis_render::{
    DependencyClosure, NoExternalAssets, NoPluginStates, RecipeBranchPin, RecipeEnvironment,
    RecipeRequest, resolve_recipe,
};

// ---------------------------------------------------------------------------
// project fixtures (public-API composition of the RCP-01 helper shape)
// ---------------------------------------------------------------------------

const RATE: u32 = 48_000;
const CLIP_FRAMES: u64 = 48_000;
const JOB_FRAMES: u64 = 512;
const PROJECT_ID: u128 = 0x296;

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(PROJECT_ID), "rcp07"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    let rev = c.live_revision();
    let op_dbg = format!("{op:?}");
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap_or_else(|e| panic!("op {op_dbg} failed: {e}"))
}

fn media_clip_placement(start_frame: u64, end_frame: u64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(
            MediaTimeValue::new(i64::try_from(start_frame).unwrap(), 1, RATE).unwrap(),
        ),
        extent: ClipExtent::Absolute(
            MediaDuration::new(i64::try_from(end_frame - start_frame).unwrap(), 1, RATE).unwrap(),
        ),
    }
}

fn natural_playback(offset: i64, frames: i64) -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::new(offset).unwrap(),
            SourceFrame::new(offset + frames).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn sha(byte: u8) -> String {
    format!("sha256:{}", format!("{byte:02x}").repeat(32))
}

struct TrackFixture {
    lane: LaneId,
    source: SourceId,
    clip: ClipId,
}

fn track_with_clip(c: &mut ProjectController) -> TrackFixture {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    apply(c, ProjectOp::RegisterAsset { digest: sha(0xA1) });
    let asset = *c.live().assets.keys().next_back().unwrap();
    apply(
        c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: sha(0xA1),
        },
    );
    let source = *c.live().sources.keys().next_back().unwrap();
    apply(
        c,
        ProjectOp::SetSourceState {
            source_id: source,
            state: SourceState::Verified,
        },
    );
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_clip_placement(0, CLIP_FRAMES),
            playback: natural_playback(0, i64::try_from(CLIP_FRAMES).unwrap()),
        },
    );
    let clip = *c.live().clips.keys().next_back().unwrap();
    TrackFixture { lane, source, clip }
}

fn base() -> (ProjectController, TrackFixture) {
    let mut c = controller();
    let fx = track_with_clip(&mut c);
    (c, fx)
}

// ---------------------------------------------------------------------------
// recipe resolution (RCP-01 public surface)
// ---------------------------------------------------------------------------

fn temporal() -> TemporalDomain {
    TemporalDomain {
        sample_rate_hz: RATE,
        channels: 2,
        format: PcmFormat::F32,
        interval: FrameInterval::new(0, CLIP_FRAMES).unwrap(),
        preroll_frames: 0,
        warmup_frames: 0,
        tail_frames: 480,
        lookahead_frames: 64,
        pdc_frames: 32,
        clock: ClockDependency::None,
        loop_region: None,
        seek_boundaries: Vec::new(),
    }
}

fn renderer() -> RendererProfile {
    RendererProfile {
        renderer_id: "test-render".into(),
        renderer_version: "0".into(),
        compatibility_profile: ContentDigest::sha256(b"compat-v1"),
        precision: RenderPrecision::F32,
        backend: "offline".into(),
        seed: 0,
        denormal_mode: DenormalMode::Preserve,
    }
}

fn resolve_live(project: &Project) -> RecipeResolution {
    let resolved = resolve_render_project(project, None).unwrap();
    let boundary = RenderBoundary::master(&resolved.project).expect("master boundary");
    let plugins = NoPluginStates;
    let assets = NoExternalAssets;
    let env = RecipeEnvironment {
        plugin_states: &plugins,
        external_assets: &assets,
    };
    let output = OutputContract {
        ports: boundary
            .ports
            .iter()
            .map(|p| OutputPortContract {
                port: *p,
                channels: 2,
                format: PcmFormat::F32,
            })
            .collect(),
    };
    resolve_recipe(&RecipeRequest {
        project: &resolved.project,
        boundary,
        temporal: temporal(),
        renderer: renderer(),
        output,
        environment: &env,
        branch: None,
        provenance: ResolutionProvenance::default(),
        project_schema_version: SCHEMA_VERSION,
        render_input_schema_version: RENDER_INPUT_SCHEMA_VERSION,
    })
}

fn resolved_key(resolution: &RecipeResolution) -> RecipeKey {
    match resolution {
        RecipeResolution::Resolved { key, .. } => *key,
        other => panic!("expected Resolved, got {other:?}"),
    }
}

fn object_ref(project: &Project, kind: ObjectKind, tagged_id: String) -> SemanticObjectRef {
    SemanticObjectRef {
        project_id: project.project_id,
        kind,
        stable_id: tagged_id,
        lineage: None,
    }
}

// ---------------------------------------------------------------------------
// artifact store fixtures (RCP-02 public surface)
// ---------------------------------------------------------------------------

fn wav_i16(frames: usize, sample: impl Fn(usize, usize) -> i16) -> Vec<u8> {
    let channels = 2u16;
    let block_align = channels * 2;
    let byte_rate = RATE * u32::from(block_align);
    let data_len = u32::try_from(frames * 2 * 2).expect("fixture fits u32");
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for f in 0..frames {
        for c in 0..2usize {
            out.extend_from_slice(&sample(f, c).to_le_bytes());
        }
    }
    out
}

fn blob(frames: u64) -> Vec<u8> {
    let frames_usize = usize::try_from(frames).unwrap();
    wav_i16(frames_usize, |f, c| {
        (((i32::try_from(f).unwrap() + i32::try_from(c).unwrap() * 7) % 97) as i16) * 100
    })
}

fn expect(frames: u64) -> PublishExpectation {
    PublishExpectation {
        frame_count: frames,
        sample_rate_hz: RATE,
        channels: 2,
        codec: "pcm".to_owned(),
        container: "wav".to_owned(),
    }
}

/// Draft whose identity fields come from a *real* canonical recipe —
/// the recipe→cache seam under test, not a synthetic key.
fn draft_for(key: RecipeKey, recipe: &RenderRecipeV1, frames: u64) -> ArtifactDraft {
    ArtifactDraft {
        recipe_key: key,
        target: recipe.target.clone(),
        snapshot: None,
        temporal: TemporalDomain {
            interval: FrameInterval::new(0, frames).unwrap(),
            ..temporal()
        },
        cut_frames: 0,
        latency_frames: 0,
        ports: recipe
            .output_contract
            .ports
            .iter()
            .map(|p| ArtifactPortSpec {
                port: p.port,
                channels: p.channels,
                channel_offset: 0,
                format: p.format,
            })
            .collect(),
        codec: CodecDescriptor {
            container: "wav".to_owned(),
            codec: "pcm".to_owned(),
            class: CodecClass::Lossless,
        },
        renderer: renderer(),
        reproducibility: ArtifactReproducibility::BitExactCertified,
        provenance: ArtifactProvenance {
            produced_by: "test/rcp07-gate".to_owned(),
            job_id: None,
        },
        trust: None,
    }
}

fn publish_for(
    store: &mut InMemoryArtifactStore,
    key: RecipeKey,
    recipe: &RenderRecipeV1,
    frames: u64,
    reference: Option<ArtifactReference>,
) -> (RenderArtifactId, ArtifactManifest) {
    let d = draft_for(key, recipe, frames);
    match publish_artifact(
        store,
        &d,
        &blob(frames),
        &expect(frames),
        None,
        None,
        reference,
    ) {
        PublishOutcome::Published { artifact, manifest } => (artifact, *manifest),
        other => panic!("expected Published, got {other:?}"),
    }
}

fn query(key: RecipeKey, ports: Vec<OutputPortContract>, span: FrameInterval) -> CacheLookup {
    CacheLookup {
        recipe_key: key,
        intended_use: IntendedUse::Exact,
        requested_span: span,
        required_ports: ports,
        trust_scope: TrustScope::Local,
        compatibility_profile: ContentDigest::sha256(b"compat-v1"),
    }
}

/// Minimal `Resolved` resolution — lookup reads `key` only (RCP-02
/// contract); used where the authentic resolution is not at hand
/// (orchestrator-published artifacts).
fn synthetic_resolved(key: RecipeKey) -> RecipeResolution {
    RecipeResolution::Resolved {
        recipe: Box::new(RenderRecipeV1 {
            schema_version: RECIPE_SCHEMA_VERSION,
            canonicalization_version: CANONICALIZATION_VERSION,
            audio_semantics_version: AUDIO_SEMANTICS_VERSION,
            hash_algorithm: RECIPE_HASH_ALGORITHM.to_owned(),
            domain_separator: RECIPE_DOMAIN_TAG.to_owned(),
            target: RecipeTarget {
                boundary: RenderBoundary::new(Vec::new()),
                project_id: ProjectId::new(PROJECT_ID),
                project_schema_version: 1,
                render_input_schema_version: 1,
                branch: None,
            },
            inputs_digest: ContentDigest::sha256(b"inputs"),
            processor_digest: ContentDigest::sha256(b"processors"),
            temporal_digest: ContentDigest::sha256(b"temporal"),
            temporal: temporal(),
            renderer_digest: ContentDigest::sha256(b"renderer"),
            output_contract: OutputContract { ports: Vec::new() },
            closure: DependencyClosure {
                nodes: Vec::new(),
                edges: Vec::new(),
                plugin_states: BTreeMap::new(),
                external_pins: BTreeMap::new(),
            },
        }),
        key,
        provenance: Box::new(ResolutionProvenance::default()),
    }
}

fn lookup_outcome(
    store: &InMemoryArtifactStore,
    q: &CacheLookup,
    current: &RecipeResolution,
) -> LookupOutcome {
    lookup(store, q, current).expect("lookup").outcome
}

fn assert_not_exact(outcome: &LookupOutcome) {
    assert!(
        !matches!(outcome, LookupOutcome::EligibleExact { .. }),
        "quarantined/corrupt artifact must never stay Exact-eligible, got {outcome:?}"
    );
}

// ---------------------------------------------------------------------------
// branch fixtures (RCP-01 variant plumbing, public API)
// ---------------------------------------------------------------------------

struct BranchFixture {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    adapter: FakeDomainAdapter,
    resolver: SnapshotResolver,
}

impl BranchFixture {
    fn new(c: ProjectController) -> Self {
        Self {
            controller: c,
            store: InMemoryBranchContentStore::default(),
            adapter: FakeDomainAdapter::new(),
            resolver: SnapshotResolver::new(),
        }
    }

    fn fork(&mut self, scope: VariantScope) -> synvelis_project::variant::ForkReceipt {
        fork_variant_set(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &self.resolver,
            scope,
            &SnapshotCancel::new(),
        )
        .unwrap()
    }

    fn edit(&mut self, set: VariantSetId, branch: BranchId, op: ProjectOp) {
        let expected = self.controller.live().variant_sets[&set].branches[&branch]
            .content
            .revision();
        edit_branch(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &BranchEditIntent {
                variant_set: set,
                branch,
                expected_branch_revision: expected,
                op,
            },
        )
        .unwrap();
    }

    fn resolve_branch(&self, set: VariantSetId, branch: BranchId) -> RecipeResolution {
        let live = self.controller.live();
        let target = RenderBranchTarget {
            variant_set: set,
            branch,
            store: &self.store,
        };
        let resolved = resolve_render_project(live, Some(target)).unwrap();
        let boundary = RenderBoundary::master(&resolved.project).unwrap();
        let plugins = NoPluginStates;
        let assets = NoExternalAssets;
        let env = RecipeEnvironment {
            plugin_states: &plugins,
            external_assets: &assets,
        };
        let out = OutputContract {
            ports: boundary
                .ports
                .iter()
                .map(|p| OutputPortContract {
                    port: *p,
                    channels: 2,
                    format: PcmFormat::F32,
                })
                .collect(),
        };
        let pin = resolved.branch.as_ref().unwrap();
        let branch_pin = RecipeBranchPin {
            scope_digest: pin.scope_digest,
            shared_context_digest: pin.shared_context_digest,
        };
        let provenance = ResolutionProvenance {
            live_revision: Some(self.controller.live_revision().0),
            variant_set: Some(set),
            branch: Some(branch),
            branch_revision: Some(pin.branch_revision.raw()),
            payload_digest: pin.payload_digest,
            ..ResolutionProvenance::default()
        };
        resolve_recipe(&RecipeRequest {
            project: &resolved.project,
            boundary,
            temporal: temporal(),
            renderer: renderer(),
            output: out,
            environment: &env,
            branch: Some(branch_pin),
            provenance,
            project_schema_version: SCHEMA_VERSION,
            render_input_schema_version: RENDER_INPUT_SCHEMA_VERSION,
        })
    }
}

struct NoopRuntime;

impl RuntimePublish for NoopRuntime {
    fn publish(&self, _request: &RuntimePublishRequest) -> RuntimePublishEvidence {
        RuntimePublishEvidence {
            state: RuntimePublishState::NotRequested,
            detail: None,
        }
    }
}

// ---------------------------------------------------------------------------
// orchestrator fixtures (RCP-03 public surface)
// ---------------------------------------------------------------------------

fn pinned(project: &Project, head: &str) -> PinnedSnapshot {
    PinnedSnapshot {
        identity: SnapshotIdentity {
            project_id: project.project_id,
            source: SnapshotOrigin::Saved,
            locator: FixedSnapshotLocator::Saved {
                project_id: project.project_id,
                head: head.to_owned(),
            },
            digest: ContentDigest::sha256(
                format!("{}:{}", head, project.project_id.raw()).as_bytes(),
            ),
            schema_version: SCHEMA_VERSION,
            projection_version: SCHEMA_VERSION,
            capture_generation: 1,
        },
        project: project.clone(),
    }
}

fn master_demand(project: &Project) -> RenderDemand {
    let resolved = resolve_render_project(project, None).expect("render input resolves");
    RenderBoundary::master(&resolved.project)
        .map(|boundary| RenderDemand {
            boundary,
            interval: FrameInterval::new(0, JOB_FRAMES).unwrap(),
        })
        .expect("master boundary")
}

fn master_ports(project: &Project) -> Vec<OutputPortContract> {
    let resolved = resolve_render_project(project, None).expect("render input resolves");
    RenderBoundary::master(&resolved.project)
        .expect("master boundary")
        .ports
        .iter()
        .map(|p| OutputPortContract {
            port: *p,
            channels: 2,
            format: PcmFormat::F32,
        })
        .collect()
}

fn selection(project: &Project, head: &str) -> RenderSelection {
    RenderSelection {
        snapshot: pinned(project, head),
        demands: vec![master_demand(project)],
    }
}

fn project_r1() -> Project {
    let mut c = controller();
    track_with_clip(&mut c);
    c.live().clone()
}

fn project_r2() -> Project {
    let mut c = controller();
    let fx = track_with_clip(&mut c);
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: fx.lane,
            source_id: fx.source,
            placement: media_clip_placement(24_000, CLIP_FRAMES),
            playback: natural_playback(0, 24_000),
        },
    );
    c.live().clone()
}

fn quality() -> RenderQualityInputs {
    RenderQualityInputs {
        sample_rate_hz: RATE,
        channels: 2,
        format: PcmFormat::F32,
        temporal: TemporalQuality {
            preroll_frames: 0,
            warmup_frames: 0,
            tail_frames: 0,
            lookahead_frames: 0,
            pdc_frames: 0,
            clock: ClockDependency::None,
            loop_region: None,
            seek_boundaries: Vec::new(),
        },
        renderer: renderer(),
    }
}

struct StaticSettings(RenderQualityInputs);

impl RenderSettingsProbe for StaticSettings {
    fn resolve(&self, _reference: &RenderSettingsRef) -> Option<RenderQualityInputs> {
        Some(self.0.clone())
    }
}

#[derive(Default)]
struct StaticPolicies {
    declared: Option<PolicyPatch>,
}

impl PolicyResolver for StaticPolicies {
    fn declared(&self, _project_id: ProjectId) -> Option<PolicyPatch> {
        self.declared.clone()
    }
    fn local_override(&self) -> Option<PolicyPatch> {
        None
    }
}

struct MasterDemands;

impl ScopeDemandResolver for MasterDemands {
    fn demands(
        &self,
        project: &synvelis_project::Project,
        _purpose: &JobPurpose,
    ) -> Vec<RenderDemand> {
        vec![master_demand(project)]
    }
}

struct FakeExecutor {
    calls: Rc<Cell<usize>>,
}

impl FakeExecutor {
    fn output() -> Self {
        Self {
            calls: Rc::new(Cell::new(0)),
        }
    }
}

impl RenderExecutor for FakeExecutor {
    fn backend_requirement(&self) -> BackendRequirement {
        BackendRequirement::Cpu
    }

    fn render(
        &mut self,
        job: &JobContext<'_>,
        _recipe: &RenderRecipeV1,
        cancel: &ruviform_exec::CancellationToken,
    ) -> RenderExecution {
        self.calls.set(self.calls.get() + 1);
        if cancel.is_cancelled() {
            return RenderExecution::Cancelled;
        }
        let frames = job.demand.interval.end - job.demand.interval.start;
        RenderExecution::Output(Box::new(RenderedOutput {
            blob: wav_i16(usize::try_from(frames).unwrap(), |f, c| {
                (((i32::try_from(f).unwrap() + i32::try_from(c).unwrap() * 7) % 97) as i16) * 100
            }),
            codec: CodecDescriptor {
                container: "wav".to_owned(),
                codec: "pcm".to_owned(),
                class: CodecClass::Lossless,
            },
            expect: PublishExpectation {
                frame_count: frames,
                sample_rate_hz: RATE,
                channels: 2,
                codec: "pcm".to_owned(),
                container: "wav".to_owned(),
            },
            expect_pcm: None,
            cut_frames: 0,
            latency_frames: 0,
            reproducibility: ArtifactReproducibility::BitExactCertified,
            trust: None,
        }))
    }
}

struct Ports<'a> {
    store: &'a mut InMemoryArtifactStore,
    executor: &'a mut FakeExecutor,
    settings: &'a StaticSettings,
    policies: &'a StaticPolicies,
    demands: &'a MasterDemands,
    authorizer: Option<&'a dyn DemandAuthorizer>,
}

impl<'a> Ports<'a> {
    fn build(self) -> JobOrchestrator<'a> {
        JobOrchestrator::new(
            self.store,
            self.executor,
            self.settings,
            self.policies,
            self.demands,
            None,
            None,
            self.authorizer,
            LocalGrants::cpu_only(),
        )
    }
}

fn report(event: TriggerEvent) -> TriggerReport {
    TriggerReport {
        event,
        entry: EntrySurface::HeadlessCli,
        action: GatewayActionRef("action.render".to_owned()),
        grant: None,
    }
}

fn manual_event(selection: RenderSelection) -> TriggerEvent {
    TriggerEvent::Manual {
        selection,
        purpose: JobPurpose::Scoped {
            scope: RenderScope::PlaybackCompatibility,
        },
        policy_intent: None,
    }
}

fn intent_for(selection: RenderSelection) -> JobIntent {
    JobIntent {
        selection,
        purpose: JobPurpose::Scoped {
            scope: RenderScope::PlaybackCompatibility,
        },
        policy: resolve_effective_policy(None, None, None),
        triggers: vec![TriggerProvenance {
            trigger: RenderTrigger::Manual,
            detail: "test".to_owned(),
        }],
        external_manifests: Vec::new(),
        estimate: JobCostEstimate::default(),
        saved_revision: None,
        entry: EntrySurface::HeadlessCli,
        action: GatewayActionRef("action.render".to_owned()),
        grant: None,
    }
}

fn demand_proposal(selection: RenderSelection) -> DemandProposal {
    DemandProposal {
        proposal_id: "prop-1".to_owned(),
        selection,
        purpose: JobPurpose::Scoped {
            scope: RenderScope::PlaybackCompatibility,
        },
        rationale: "rcp07 cross-gate".to_owned(),
        policy_intent: None,
    }
}

fn triggers_patch(triggers: &[RenderTrigger]) -> PolicyPatch {
    PolicyPatch {
        triggers: Some(BTreeSet::from_iter(triggers.iter().copied())),
        ..PolicyPatch::default()
    }
}

fn job_recipe_key(orch: &JobOrchestrator<'_>, job_key: &RenderJobKey) -> RecipeKey {
    let job = orch.job(job_key).expect("job exists");
    job.boundaries
        .iter()
        .find_map(|b| match b {
            synvelis_render::render_job::BoundaryResolution::Resolved { resolution, .. } => {
                match resolution.as_ref() {
                    RecipeResolution::Resolved { key, .. } => Some(*key),
                    _ => None,
                }
            }
            _ => None,
        })
        .expect("resolved boundary")
}

fn artifact_count(store: &InMemoryArtifactStore, key: &RecipeKey) -> usize {
    store.index_entries(key).map_or(0, |e| e.len())
}

// ---------------------------------------------------------------------------
// missing-plugin resolver fixtures (RCP-04 public surface)
// ---------------------------------------------------------------------------

const OUT_BASE: u32 = 10;

fn audio_format() -> PortFormat {
    PortFormat::Audio(AudioFormat {
        sample_rate: SampleRate::new(RATE).expect("rate"),
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

fn source_node(id: u64) -> ResolvedNode {
    node(id, NodeClass::Source, vec![audio_out(0)])
}

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

fn plugin_node(id: u64, uid: &str) -> ResolvedNode {
    ResolvedNode {
        plugin: Some(plugin_decl(uid)),
        ..node(id, NodeClass::Plugin, vec![audio_in(0), audio_out(0)])
    }
}

fn empty_caps() -> PluginCapabilitySnapshot {
    PluginCapabilitySnapshot {
        generation: 7,
        entries: BTreeMap::new(),
    }
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

/// src(1) → instr plugin(2, absent) → sink(3).
fn missing_plugin_snapshot() -> ResolvedGraphSnapshot {
    let o = OUT_BASE;
    let nodes = vec![source_node(1), plugin_node(2, "instr"), sink_node(3)];
    let map: BTreeMap<_, _> = nodes.into_iter().map(|n| (n.id, n)).collect();
    let mut snap = ResolvedGraphSnapshot {
        snapshot_id: 1,
        project_id: ProjectId::new(PROJECT_ID),
        project_revision: ProjectRevision(10),
        structure_digest: 0,
        capability_generation: 7,
        engine_config_generation: 3,
        source_format_generation: 5,
        clock_epoch: synvelis_project::RuntimeClockEpochRef(1),
        runtime_instance: synvelis_project::RuntimeInstanceId(1),
        nodes: map,
        edges: vec![
            edge(1, (1, o), (2, 0), EdgeKind::Main),
            edge(2, (2, o), (3, 0), EdgeKind::Main),
        ],
        required_outputs: vec![BoundaryRequirement {
            port: PortEndpoint {
                node: RcpNodeId(3),
                port: RcpPortId(0),
            },
            purpose: BoundaryPurpose::MainOut,
        }],
    };
    snap.structure_digest = snap.compute_structure_digest();
    snap
}

fn artifact_output(node_id: u64) -> ArtifactOutput {
    ArtifactOutput {
        source: PortEndpoint {
            node: RcpNodeId(node_id),
            port: RcpPortId(OUT_BASE),
        },
        domain: SignalDomain::Audio,
        format: audio_format(),
        latency_frames: 0,
    }
}

fn pin_in(node_id: u64) -> PortEndpoint {
    PortEndpoint {
        node: RcpNodeId(node_id),
        port: RcpPortId(0),
    }
}

fn render_index(
    candidates: Vec<RenderCandidate>,
    snap: &ResolvedGraphSnapshot,
) -> RenderCandidateIndex {
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

fn candidate_from_report(
    artifact_id: &str,
    content_hash: &str,
    report: &synvelis_render::artifact::lookup::CandidateReport,
) -> RenderCandidate {
    RenderCandidate {
        artifact_id: artifact_id.to_owned(),
        content_hash: content_hash.to_owned(),
        node_set: BTreeSet::from([RcpNodeId(2)]),
        outputs: vec![artifact_output(2)],
        pinned_inputs: BTreeSet::from([pin_in(2)]),
        eligibility: report.eligibility,
        reproducibility: report.reproducibility,
        snapshot_revision: ProjectRevision(0),
        snapshot_digest: 0,
        access: report.access.clone(),
    }
}

// ---------------------------------------------------------------------------
// RCP-GATE-01 — recipe → cache identity, false-valid = 0
// ---------------------------------------------------------------------------

#[test]
fn rcp07_gate01_recipe_key_is_cache_identity_and_false_valid_zero() {
    let (c, _fx) = base();
    let resolution = resolve_live(c.live());
    let RecipeResolution::Resolved { recipe, key, .. } = &resolution else {
        panic!("expected Resolved");
    };

    // Determinism: identical project → identical canonical key.
    let again = resolve_live(c.live());
    assert_eq!(resolved_key(&again), *key);

    // recipe_record → evaluate_validity round-trip: Match, never a
    // self-referential pass — the record is rebuilt from canonical
    // bytes and re-verified against a fresh resolution.
    let record: RecipeRecord = recipe_record(recipe, *key).unwrap();
    assert_eq!(record.verify(), Ok(()));
    assert_eq!(
        evaluate_validity(&record, &resolve_live(c.live())),
        Validity::Match
    );

    // Acoustic change → Stale (keys differ), never Match.
    let (mut c2, _fx2) = base();
    apply(
        &mut c2,
        ProjectOp::AddTempoEvent {
            tick: 3_840,
            bpm_milli: 120_000,
        },
    );
    assert_eq!(
        evaluate_validity(&record, &resolve_live(c2.live())),
        Validity::Stale
    );

    // Hash-collision injection: record claims the right key over
    // different bytes → verify fails → evaluate is Unknown, never Match.
    let mut forged = record.clone();
    let mut other_recipe = recipe.as_ref().clone();
    other_recipe.renderer_digest = ContentDigest::sha256(b"other");
    forged.canonical_bytes = recipe_record(&other_recipe, *key).unwrap().canonical_bytes;
    assert!(forged.verify().is_err());
    assert_eq!(
        evaluate_validity(&forged, &resolve_live(c.live())),
        Validity::Unknown
    );

    // Cache identity: an empty store reports Missing for the real
    // key — nothing is fabricated for an unresolved lookup.
    let store = InMemoryArtifactStore::new();
    let ports = recipe
        .output_contract
        .ports
        .iter()
        .map(|p| OutputPortContract {
            port: p.port,
            channels: p.channels,
            format: p.format,
        })
        .collect();
    let outcome = lookup_outcome(
        &store,
        &query(*key, ports, FrameInterval::new(0, CLIP_FRAMES).unwrap()),
        &resolution,
    );
    assert!(matches!(outcome, LookupOutcome::Missing), "got {outcome:?}");
}

// ---------------------------------------------------------------------------
// RCP-GATE-02 — closure-scoped invalidation, under-approximation = 0
// ---------------------------------------------------------------------------

#[test]
fn rcp07_gate02_closure_invalidation_is_sound_and_complete() {
    let (c, fx) = base();
    let resolution = resolve_live(c.live());
    let RecipeResolution::Resolved { recipe, .. } = &resolution else {
        panic!("expected Resolved");
    };

    let clip_obj = object_ref(c.live(), ObjectKind::Clip, fx.clip.to_tagged());

    // In-closure change with a known interval → partial dirty
    // interval, never full, and the affected object is recorded.
    let plan = classify_invalidation(
        recipe,
        &[ChangedObject {
            object: clip_obj.clone(),
            interval: Some(FrameInterval::new(1_000, 2_000).unwrap()),
            temporal: false,
        }],
    );
    assert!(!plan.full);
    assert!(plan.affected_objects.contains(&clip_obj));
    assert_eq!(plan.dirty_intervals.len(), 1);

    // Unknown extent → fail-closed full invalidation (under-
    // approximation is impossible: None means "everything").
    let plan = classify_invalidation(
        recipe,
        &[ChangedObject {
            object: clip_obj.clone(),
            interval: None,
            temporal: false,
        }],
    );
    assert!(plan.full);

    // Temporal-domain change → whole interval invalidates.
    let plan = classify_invalidation(
        recipe,
        &[ChangedObject {
            object: clip_obj.clone(),
            interval: Some(FrameInterval::new(1_000, 2_000).unwrap()),
            temporal: true,
        }],
    );
    assert!(plan.full);

    // Out-of-closure object → zero effect on this boundary.
    let foreign = object_ref(c.live(), ObjectKind::Track, "track-foreign".to_owned());
    let plan = classify_invalidation(
        recipe,
        &[ChangedObject {
            object: foreign,
            interval: Some(FrameInterval::new(0, 100).unwrap()),
            temporal: false,
        }],
    );
    assert_eq!(plan.dirty_intervals, Vec::<FrameInterval>::new());
    assert!(!plan.full);
}

// ---------------------------------------------------------------------------
// RCP-GATE-03 — verified publish → fault/GC → old-Verified protection
// ---------------------------------------------------------------------------

#[test]
fn rcp07_gate03_verified_artifact_survives_faults_and_gc_roots() {
    let (c, fx) = base();
    let resolution = resolve_live(c.live());
    let RecipeResolution::Resolved { recipe, key, .. } = &resolution else {
        panic!("expected Resolved");
    };
    let ports = recipe
        .output_contract
        .ports
        .iter()
        .map(|p| OutputPortContract {
            port: p.port,
            channels: p.channels,
            format: p.format,
        })
        .collect();
    let q = query(*key, ports, FrameInterval::new(0, CLIP_FRAMES).unwrap());

    let mut store = InMemoryArtifactStore::new();
    let branch_ref = ArtifactReference::LiveBranch {
        project_id: ProjectId::new(PROJECT_ID),
        variant_set: VariantSetId::new(0x01),
        branch: BranchId::new(0x02),
    };
    let (id, manifest) = publish_for(
        &mut store,
        *key,
        recipe,
        CLIP_FRAMES,
        Some(branch_ref.clone()),
    );
    let digest = manifest.content_digest;

    // Verified publish → Exact-eligible through the same lookup the
    // resolver consumes.
    let outcome = lookup_outcome(&store, &q, &resolution);
    assert!(
        matches!(outcome, LookupOutcome::EligibleExact { artifact } if artifact == id),
        "got {outcome:?}"
    );

    // Publish-path fault: a failed commit must not disturb the
    // previously verified artifact (old-Verified protection).
    store.arm_fault(
        FaultPoint::CommitIndex,
        StoreError::Conflict {
            detail: "injected".to_owned(),
        },
    );
    let other_key = RecipeKey(ContentDigest::sha256(b"other-key"));
    let failed = publish_artifact(
        &mut store,
        &draft_for(other_key, recipe, CLIP_FRAMES),
        &blob(CLIP_FRAMES),
        &expect(CLIP_FRAMES),
        None,
        None,
        None,
    );
    assert!(
        !matches!(failed, PublishOutcome::Published { .. }),
        "faulted publish must not succeed"
    );
    let outcome = lookup_outcome(&store, &q, &resolution);
    assert!(
        matches!(outcome, LookupOutcome::EligibleExact { artifact } if artifact == id),
        "old verified artifact disturbed by publish fault: {outcome:?}"
    );

    // GC: active branch reference keeps the artifact reachable
    // regardless of clock; raw project media is a different store and
    // is never touched by artifact GC.
    store.advance_clock(100_000);
    let report = sweep(&mut store, &GcPolicy { grace_ticks: 0 }).unwrap();
    assert!(report.kept_reachable.contains(&id));
    assert_eq!(
        c.live().sources[&fx.source].content_digest,
        sha(0xA1),
        "artifact GC reached into project raw source state"
    );

    // Release → grace → collected; manifest, blob and index entry all
    // go together (CAS/GC consistency).
    store.release_reference(&id, &branch_ref).unwrap();
    let report = sweep(&mut store, &GcPolicy { grace_ticks: 0 }).unwrap();
    assert!(report.collected.contains(&id));
    assert!(store.tombstone(&id).is_some());
    assert!(store.read_manifest(&id).unwrap().is_none());
    assert!(store.read_blob(&digest).unwrap().is_none());
    assert!(report.blobs_freed.contains(&digest));

    // Post-collection lookup can never resurrect the artifact.
    let outcome = lookup_outcome(&store, &q, &resolution);
    assert_not_exact(&outcome);

    // Quarantine → fail-closed lookup (integrity fault surfaces as a
    // non-Exact state, never silently reused).
    let (id2, _) = publish_for(&mut store, *key, recipe, CLIP_FRAMES, None);
    store.quarantine(&id2, "integrity");
    let outcome = lookup_outcome(&store, &q, &resolution);
    assert_not_exact(&outcome);
}

// ---------------------------------------------------------------------------
// RCP-GATE-04 — policy authorization, idempotence, cancel, no
// unauthorized side effects
// ---------------------------------------------------------------------------

struct StaticAuthorizer(DemandAuthorization);

impl DemandAuthorizer for StaticAuthorizer {
    fn authorize(&self, _proposal: &DemandProposal) -> DemandAuthorization {
        self.0.clone()
    }
}

#[test]
fn rcp07_gate04_ondemand_denied_manual_idempotent_cancel_clean() {
    let project = project_r1();
    let sel = selection(&project, "head-r1");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let settings = StaticSettings(quality());
    let demands = MasterDemands;

    // OnDemand without consent → typed Denied, zero jobs, zero
    // executor calls (unauthorized side effects = 0).
    let deny = StaticAuthorizer(DemandAuthorization::Denied {
        reason: "no consent".to_owned(),
    });
    let policies = StaticPolicies {
        declared: Some(triggers_patch(&[RenderTrigger::OnDemand])),
    };
    {
        let mut orch = Ports {
            store: &mut store,
            executor: &mut executor,
            settings: &settings,
            policies: &policies,
            demands: &demands,
            authorizer: Some(&deny),
        }
        .build();
        let outcome = orch.handle(&report(TriggerEvent::Demand(demand_proposal(sel.clone()))));
        assert!(
            matches!(outcome, TriggerOutcome::Denied { .. }),
            "got {outcome:?}"
        );
        assert!(orch.job_keys().is_empty(), "denied demand made a job");
    }
    assert_eq!(executor.calls.get(), 0);

    // Manual trigger: duplicate events coalesce into one job and one
    // artifact. A queued job cancelled before issue publishes
    // nothing — `handle` drives synchronously, so the cancel case
    // goes through `submit` (the non-driving entry point).
    let policies = StaticPolicies::default();
    let rk_manual;
    {
        let mut orch = Ports {
            store: &mut store,
            executor: &mut executor,
            settings: &settings,
            policies: &policies,
            demands: &demands,
            authorizer: None,
        }
        .build();
        let key1 = match orch.handle(&report(manual_event(sel.clone()))) {
            TriggerOutcome::Job { key, coalesced } => {
                assert!(!coalesced);
                key
            }
            other => panic!("expected Job, got {other:?}"),
        };
        match orch.handle(&report(manual_event(sel.clone()))) {
            TriggerOutcome::Job { coalesced, .. } => assert!(coalesced),
            other => panic!("expected coalesced Job, got {other:?}"),
        }
        assert_eq!(orch.job_keys().len(), 1);

        let cancel_key = match orch.submit(&intent_for(selection(&project_r2(), "head-cancel"))) {
            SubmitOutcome::Accepted { key, .. } => key,
            other @ SubmitOutcome::Rejected { .. } => panic!("submit failed: {other:?}"),
        };
        assert!(orch.cancel(&cancel_key), "cancel rejected");
        assert_eq!(
            orch.job(&cancel_key).expect("job").state,
            RenderJobState::Cancelled
        );
        assert!(orch.job(&cancel_key).expect("job").artifacts.is_empty());

        rk_manual = job_recipe_key(&orch, &key1);
    }
    assert_eq!(executor.calls.get(), 1, "cancelled/duplicate work rendered");
    assert_eq!(artifact_count(&store, &rk_manual), 1);
}

// ---------------------------------------------------------------------------
// RCP-GATE-05 — branch pin isolates cache; adopt is one CAS; old pins hold
// ---------------------------------------------------------------------------

fn audio_scope(clip: ClipId) -> VariantScope {
    VariantScope::AudioClip { clip }
}

#[test]
fn rcp07_gate05_branch_edit_moves_key_adopt_pins_live() {
    let (c, fx) = base();
    let mut fixture = BranchFixture::new(c);
    let receipt = fixture.fork(audio_scope(fx.clip));
    let set = receipt.variant_set;
    let candidate = receipt.candidate;

    let k_live = resolved_key(&resolve_live(fixture.controller.live()));

    // Branch-internal acoustic edit (mute scoped clip) moves the
    // candidate recipe key away from live — cache identity follows
    // content, not branch labels.
    fixture.edit(
        set,
        candidate,
        ProjectOp::SetClipMuted {
            clip_id: fx.clip,
            muted: true,
        },
    );
    let res_cand = fixture.resolve_branch(set, candidate);
    let k_cand = resolved_key(&res_cand);
    assert_ne!(k_cand, k_live, "branch edit did not move recipe key");

    // Publish a verified artifact under the *candidate* key with a
    // LiveBranch GC root — the branch→cache pin.
    let mut store = InMemoryArtifactStore::new();
    let RecipeResolution::Resolved { recipe, .. } = &res_cand else {
        panic!("expected Resolved");
    };
    let ports = recipe
        .output_contract
        .ports
        .iter()
        .map(|p| OutputPortContract {
            port: p.port,
            channels: p.channels,
            format: p.format,
        })
        .collect();
    let q = query(k_cand, ports, FrameInterval::new(0, CLIP_FRAMES).unwrap());
    let branch_ref = ArtifactReference::LiveBranch {
        project_id: ProjectId::new(PROJECT_ID),
        variant_set: set,
        branch: candidate,
    };
    let (id, _m) = publish_for(&mut store, k_cand, recipe, CLIP_FRAMES, Some(branch_ref));
    let outcome = lookup_outcome(&store, &q, &res_cand);
    assert!(
        matches!(outcome, LookupOutcome::EligibleExact { artifact } if artifact == id),
        "got {outcome:?}"
    );

    // Adopt: one project CAS, live content becomes the candidate's
    // acoustics — the live recipe key converges to an independent
    // muted-live resolution (the hashed `RecipeBranchPin` keeps the
    // branch-pinned key distinct from live identity).
    let noop = NoopRuntime;
    let intent = AdoptIntent {
        variant_set: set,
        branch: candidate,
        expected_project_revision: fixture.controller.live_revision(),
        expected_branch_revision: fixture.controller.live().variant_sets[&set].branches[&candidate]
            .content
            .revision(),
        expected_scope_digest: scope_digest(fixture.controller.live(), &audio_scope(fx.clip))
            .unwrap(),
        approval: ApplyApproval {
            ticket: "approve".into(),
        },
    };
    let adopt = adopt_branch(
        &mut fixture.controller,
        &mut fixture.store,
        &AdoptSources {
            assets: None,
            plugins: None,
            runtime: &noop,
        },
        &intent,
    )
    .unwrap();
    assert!(adopt.project.committed, "adopt did not commit");
    assert!(
        adopt.after_revision.0 > adopt.before_revision.0,
        "adopt was not a single forward CAS"
    );

    // Post-adopt live key must equal an *independent* live project
    // carrying the same acoustic edit — the oracle is a fresh
    // resolution, not the branch resolution (the hashed
    // `RecipeBranchPin` deliberately separates branch cache identity
    // from live identity even for identical acoustics).
    let k_live_after = resolved_key(&resolve_live(fixture.controller.live()));
    let (mut c3, fx3) = base();
    apply(
        &mut c3,
        ProjectOp::SetClipMuted {
            clip_id: fx3.clip,
            muted: true,
        },
    );
    let k_muted_live = resolved_key(&resolve_live(c3.live()));
    assert_eq!(
        k_live_after, k_muted_live,
        "adopt did not materialize the candidate's acoustics into live"
    );
    assert_ne!(
        k_cand, k_live_after,
        "branch pin failed to separate cache identity"
    );

    // The branch-pinned artifact remains verified and reachable —
    // old-commit protection across adopt.
    store.advance_clock(100_000);
    let report = sweep(&mut store, &GcPolicy { grace_ticks: 0 }).unwrap();
    assert!(report.kept_reachable.contains(&id));
    let outcome = lookup_outcome(&store, &q, &res_cand);
    assert!(
        matches!(outcome, LookupOutcome::EligibleExact { artifact } if artifact == id),
        "got {outcome:?}"
    );
}

// ---------------------------------------------------------------------------
// RCP-GATE-06/07 (headless half) — store eligibility drives the
// missing-plugin resolver; job keys separate stale from current
// ---------------------------------------------------------------------------

#[test]
fn rcp07_gate06_store_eligibility_drives_resolver_selection() {
    let (c, _fx) = base();
    let resolution = resolve_live(c.live());
    let RecipeResolution::Resolved { recipe, key, .. } = &resolution else {
        panic!("expected Resolved");
    };
    let ports = recipe
        .output_contract
        .ports
        .iter()
        .map(|p| OutputPortContract {
            port: p.port,
            channels: p.channels,
            format: p.format,
        })
        .collect();
    let q = query(*key, ports, FrameInterval::new(0, CLIP_FRAMES).unwrap());

    // Publish a verified artifact for the real recipe key, then feed
    // the *lookup verdict* — not a hand-declared eligibility — into
    // the resolver candidate index.
    let mut store = InMemoryArtifactStore::new();
    let (id, manifest) = publish_for(&mut store, *key, recipe, CLIP_FRAMES, None);
    let result = lookup(&store, &q, &resolution).expect("lookup");
    let report = result
        .candidates
        .iter()
        .find(|r| r.artifact == id)
        .expect("published artifact reported");
    assert!(matches!(
        report.eligibility,
        ArtifactEligibility::EligibleExact
    ));

    let snap = missing_plugin_snapshot();
    let caps = empty_caps();
    let cand = candidate_from_report(
        &id.to_string(),
        &manifest.content_digest.to_string(),
        report,
    );
    let renders = render_index(vec![cand], &snap);
    let sel = ExecutionSelector::select(&snap, &ExecutionIntent::MainPlayback, &caps, &renders);

    // Missing plugin + verified artifact → Exact selection whose
    // substitution names the published artifact identity.
    assert_eq!(sel.verdict, SelectionVerdict::Exact);
    assert!(!sel.non_equivalent);
    assert_eq!(sel.substitutions.len(), 1);
    assert_eq!(sel.substitutions[0].artifact_id, id.to_string());
    assert_eq!(
        sel.substitutions[0].content_hash,
        manifest.content_digest.to_string()
    );

    // Integrity fault → lookup no longer Exact → resolver must fail
    // closed (no substitution, no Exact claim).
    store.quarantine(&id, "integrity");
    let result = lookup(&store, &q, &resolution).expect("lookup");
    let cand = result
        .candidates
        .iter()
        .find(|r| r.artifact == id)
        .map(|r| candidate_from_report(&id.to_string(), &manifest.content_digest.to_string(), r))
        .unwrap_or(RenderCandidate {
            artifact_id: id.to_string(),
            content_hash: manifest.content_digest.to_string(),
            node_set: BTreeSet::from([RcpNodeId(2)]),
            outputs: vec![artifact_output(2)],
            pinned_inputs: BTreeSet::from([pin_in(2)]),
            eligibility: ArtifactEligibility::Missing,
            reproducibility: ReproducibilityClass::Unknown,
            snapshot_revision: ProjectRevision(0),
            snapshot_digest: 0,
            access: AccessState::Granted,
        });
    let renders = render_index(vec![cand], &snap);
    let sel = ExecutionSelector::select(&snap, &ExecutionIntent::MainPlayback, &caps, &renders);
    assert_ne!(sel.verdict, SelectionVerdict::Exact);
    assert!(sel.substitutions.is_empty());
}

#[test]
fn rcp07_gate07_job_keys_separate_stale_and_current_recipes() {
    let project_r1 = project_r1();
    let project_r2 = project_r2();
    let sel1 = selection(&project_r1, "head-r1");
    let sel2 = selection(&project_r2, "head-r2");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demands = MasterDemands;

    // Two different project contents → two distinct job keys → two
    // artifacts under two distinct recipe keys. The R1 artifact stays
    // verified under its own key (old-commit protection); nothing
    // cross-activates under R2.
    let (rk1, rk2);
    {
        let mut orch = Ports {
            store: &mut store,
            executor: &mut executor,
            settings: &settings,
            policies: &policies,
            demands: &demands,
            authorizer: None,
        }
        .build();
        let mut job_keys = Vec::new();
        for sel in [sel1, sel2] {
            match orch.handle(&report(manual_event(sel))) {
                TriggerOutcome::Job { key, .. } => job_keys.push(key),
                other => panic!("expected Job, got {other:?}"),
            }
        }
        assert_eq!(orch.job_keys().len(), 2);
        orch.run();
        rk1 = job_recipe_key(&orch, &job_keys[0]);
        rk2 = job_recipe_key(&orch, &job_keys[1]);
    }
    assert_ne!(rk1, rk2, "distinct recipes collapsed to one key");
    assert_eq!(artifact_count(&store, &rk1), 1);
    assert_eq!(artifact_count(&store, &rk2), 1);

    // Old-commit pin: R1's artifact remains Exact-eligible under R1's
    // key even after R2 rendered — staleness is per-recipe-key, never
    // a global invalidation.
    let span = FrameInterval::new(0, JOB_FRAMES).unwrap();
    let cur1 = synthetic_resolved(rk1);
    let out1 = lookup_outcome(&store, &query(rk1, master_ports(&project_r1), span), &cur1);
    assert!(
        matches!(out1, LookupOutcome::EligibleExact { .. }),
        "stale R1 artifact lost verification: {out1:?}"
    );
    let cur2 = synthetic_resolved(rk2);
    let out2 = lookup_outcome(&store, &query(rk2, master_ports(&project_r2), span), &cur2);
    assert!(
        matches!(out2, LookupOutcome::EligibleExact { .. }),
        "current R2 artifact not eligible: {out2:?}"
    );
}
