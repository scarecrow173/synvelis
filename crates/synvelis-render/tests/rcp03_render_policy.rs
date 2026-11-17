//! RCP-03 headless contract tests — Render Policy, Save/Commit/Share
//! typed trigger ports, asynchronous render-job orchestration and
//! completeness verdicts (synvelis#292;
//! `docs/test-specs/rcp-03-render-policy.md`).
//!
//! Every case maps to an acceptance id (RCP-POL-01..07). All tests
//! run against [`InMemoryArtifactStore`] plus injected fixture ports
//! (settings probe, scope-demand resolver, policy resolver, demand
//! authorizer, deterministic executor). Real OS/filesystem stores,
//! real VCS providers, real async scheduling, RT lanes, GPUI
//! projection and the workspace Gateway are later gates — headless
//! PASS here is never promoted to them.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::id::{CounterIdGenerator, ProjectId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::placement::{
    ClipExtent, ClipPlacement, ClipPlayback, PlaybackMapping, TimeAnchor,
};
use synvelis_project::project::Project;
use synvelis_project::range::SourceRange;
use synvelis_project::routing::{PluginRef, ProcessorBinding, StripOwner};
use synvelis_project::snapshot::{
    ContentDigest, FixedSnapshotLocator, SnapshotIdentity, SnapshotOrigin,
};
use synvelis_project::store::SCHEMA_VERSION;
use synvelis_project::time::{MediaDuration, MediaTimeValue, SourceFrame};
use synvelis_project::{
    LaneId, SemanticPortId, SourceId, SourceState, StripId, render_input::resolve_render_project,
};
use synvelis_render::artifact::manifest::{ArtifactReproducibility, CodecClass, CodecDescriptor};
use synvelis_render::artifact::publish::PublishExpectation;
use synvelis_render::artifact::store::{ArtifactStore, InMemoryArtifactStore};
use synvelis_render::policy::{
    CommitEvent, CommitReceipt, CompletionRequirement, DemandAuthorization, DemandAuthorizer,
    DemandProposal, InertReason, JobIntent, LocalGrants, PolicyPatch, PolicyResolver, RenderScope,
    RenderSettingsRef, RenderTrigger, ResourceBudget, SaveEvent, SaveReceipt, ScopeDemandResolver,
    ShareRequest, TriggerEvent, TriggerOutcome, TriggerProvenance, TriggerReport,
    resolve_effective_policy,
};
use synvelis_render::recipe::{
    ClockDependency, DenormalMode, FrameInterval, PcmFormat, RecipeKey, RecipeResolution,
    RenderBoundary, RenderPrecision, RendererProfile,
};
use synvelis_render::render_job::{
    BackendRequirement, CompletenessGapKind, CompletenessVerdict, DriveOutcome, EntrySurface,
    GatewayActionRef, GrantRef, JobContext, JobCostEstimate, JobFailureReason, JobOrchestrator,
    JobPurpose, JobScheduler, PinnedSnapshot, ReceiptKind, RenderDemand, RenderExecution,
    RenderExecutor, RenderJobKey, RenderJobState, RenderQualityInputs, RenderSelection,
    RenderSettingsProbe, RenderedOutput, SchedulerPorts, ShareGateVerdict, SubmitOutcome,
    TemporalQuality,
};

// ---------------------------------------------------------------------------
// project fixtures (mirrors rcp01_recipe.rs helpers)
// ---------------------------------------------------------------------------

const RATE: u32 = 48_000;
const FRAMES: u64 = 512;
const PROJECT_ID: u128 = 0x292;

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(PROJECT_ID), "rcp03"),
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
    strip: StripId,
    source: SourceId,
}

/// Track + strip (auto-wired to master) + lane + verified media
/// source + one clip — the minimal resolvable mix.
fn track_with_clip(c: &mut ProjectController) -> TrackFixture {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    let strip = c
        .live()
        .routing
        .strips
        .values()
        .find(|s| s.owner == StripOwner::Track(track))
        .unwrap()
        .id;
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
            placement: media_clip_placement(0, 48_000),
            playback: natural_playback(0, 48_000),
        },
    );
    TrackFixture {
        lane,
        strip,
        source,
    }
}

fn project_r1() -> Project {
    let mut c = controller();
    track_with_clip(&mut c);
    c.live().clone()
}

/// R2: same project plus a second clip — reachable content differs,
/// so the resolved recipe key differs from R1's.
fn project_r2() -> Project {
    let mut c = controller();
    let fx = track_with_clip(&mut c);
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: fx.lane,
            source_id: fx.source,
            placement: media_clip_placement(24_000, 48_000),
            playback: natural_playback(0, 24_000),
        },
    );
    c.live().clone()
}

/// R1 plus one resolved-binding plugin on the track strip — recipe
/// resolution fails closed (`PluginStateUnknown`) under the default
/// no-evidence probes.
fn project_with_plugin() -> Project {
    let mut c = controller();
    let fx = track_with_clip(&mut c);
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: fx.strip,
            index: 0,
            name: "fx".into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: "test-plugin".into(),
                version: "1.0".into(),
                capability_revision: 1,
            }),
            sidechain_input: false,
        },
    );
    c.live().clone()
}

// ---------------------------------------------------------------------------
// pinned snapshots + demands
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
            interval: FrameInterval::new(0, FRAMES).unwrap(),
        })
        .expect("master boundary")
}

fn selection(project: &Project, head: &str) -> RenderSelection {
    RenderSelection {
        snapshot: pinned(project, head),
        demands: vec![master_demand(project)],
    }
}

// ---------------------------------------------------------------------------
// injected ports
// ---------------------------------------------------------------------------

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

/// Alternate quality — a different sample rate moves every
/// `RecipeKey` (RCP-POL-02's quality-affecting change).
fn quality_alt() -> RenderQualityInputs {
    RenderQualityInputs {
        sample_rate_hz: 96_000,
        ..quality()
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
    local: Option<PolicyPatch>,
}

impl PolicyResolver for StaticPolicies {
    fn declared(&self, _project_id: ProjectId) -> Option<PolicyPatch> {
        self.declared.clone()
    }
    fn local_override(&self) -> Option<PolicyPatch> {
        self.local.clone()
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

struct StaticAuthorizer(DemandAuthorization);

impl DemandAuthorizer for StaticAuthorizer {
    fn authorize(&self, _proposal: &DemandProposal) -> DemandAuthorization {
        self.0.clone()
    }
}

// ---------------------------------------------------------------------------
// executor + blob fixtures
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum ExecStep {
    Output,
    Fail(JobFailureReason, &'static str),
    CrashOnce,
    Cancelled,
}

struct FakeExecutor {
    /// Shared call counter — readable while the orchestrator holds
    /// the executor's `&mut` borrow. `Arc<AtomicUsize>` keeps the
    /// fixture `Send` so it can also back the async scheduler ports
    /// (synvelis#518).
    calls: Arc<AtomicUsize>,
    steps: Vec<ExecStep>,
    backend: BackendRequirement,
}

impl FakeExecutor {
    fn output() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            steps: vec![ExecStep::Output],
            backend: BackendRequirement::Cpu,
        }
    }

    fn next(&mut self) -> ExecStep {
        if self.steps.len() > 1 {
            self.steps.remove(0)
        } else {
            self.steps[0].clone()
        }
    }
}

fn wav_i16(frames: usize) -> Vec<u8> {
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
            let v = i16::try_from((f * 3 + c * 11) % 97).unwrap() * 100;
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

/// The fixture's verified WAV output for `frames` frames — shared by
/// the synchronous and gated executors.
fn wav_output(frames: u64) -> RenderExecution {
    RenderExecution::Output(Box::new(RenderedOutput {
        blob: wav_i16(usize::try_from(frames).unwrap()),
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

impl RenderExecutor for FakeExecutor {
    fn backend_requirement(&self) -> BackendRequirement {
        self.backend.clone()
    }

    fn render(
        &mut self,
        job: &JobContext<'_>,
        _recipe: &synvelis_render::recipe::RenderRecipeV1,
        cancel: &ruviform_exec::CancellationToken,
    ) -> RenderExecution {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if cancel.is_cancelled() {
            return RenderExecution::Cancelled;
        }
        match self.next() {
            ExecStep::Output => {
                let frames = job.demand.interval.end - job.demand.interval.start;
                wav_output(frames)
            }
            ExecStep::Fail(reason, detail) => RenderExecution::Failed {
                reason,
                detail: detail.to_owned(),
            },
            ExecStep::CrashOnce => RenderExecution::Crashed,
            ExecStep::Cancelled => RenderExecution::Cancelled,
        }
    }
}

// ---------------------------------------------------------------------------
// orchestrator assembly
// ---------------------------------------------------------------------------

struct Ports<'a> {
    store: &'a mut InMemoryArtifactStore,
    executor: &'a mut FakeExecutor,
    settings: &'a StaticSettings,
    policies: &'a StaticPolicies,
    demands: &'a MasterDemands,
    authorizer: Option<&'a dyn DemandAuthorizer>,
}

impl<'a> Ports<'a> {
    fn build(self, grants: LocalGrants) -> JobOrchestrator<'a> {
        JobOrchestrator::new(
            self.store,
            self.executor,
            self.settings,
            self.policies,
            self.demands,
            None,
            None,
            self.authorizer,
            grants,
        )
    }
}

fn cpu_grants() -> LocalGrants {
    LocalGrants::cpu_only()
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

fn save_completed(snapshot: &PinnedSnapshot, revision: u64, head: &str) -> TriggerEvent {
    TriggerEvent::Save(SaveEvent::Completed(Box::new(SaveReceipt {
        snapshot: snapshot.clone(),
        saved_revision: ProjectRevision(revision),
        head: head.to_owned(),
    })))
}

fn triggers_patch(triggers: &[RenderTrigger]) -> PolicyPatch {
    PolicyPatch {
        triggers: Some(BTreeSet::from_iter(triggers.iter().copied())),
        ..PolicyPatch::default()
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

/// Artifact count filed under one recipe key.
fn artifact_count(store: &InMemoryArtifactStore, key: &RecipeKey) -> usize {
    store.index_entries(key).map_or(0, |e| e.len())
}

/// The single resolved recipe key of a published job.
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

// ---------------------------------------------------------------------------
// RCP-POL-01 — non-exclusive triggers, coalescing, no duplicate publish
// ---------------------------------------------------------------------------

#[test]
fn pol01_mixed_triggers_coalesce_into_one_job_one_artifact() {
    let project = project_r1();
    let sel = selection(&project, "head-mix");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies {
        declared: Some(triggers_patch(&[
            RenderTrigger::OnSave,
            RenderTrigger::OnCommit,
            RenderTrigger::OnShare,
            RenderTrigger::OnDemand,
        ])),
        local: None,
    };
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    // Manual and OnSave describe the same pinned target + purpose —
    // they must coalesce, keeping both trigger provenances.
    let out1 = orch.handle(&report(manual_event(sel.clone())));
    let save_snap = sel.snapshot.clone();
    let out2 = orch.handle(&report(save_completed(&save_snap, 7, "head-mix")));

    let (
        TriggerOutcome::Job {
            key: k1,
            coalesced: c1,
        },
        TriggerOutcome::Job {
            key: k2,
            coalesced: c2,
        },
    ) = (out1, out2)
    else {
        panic!("expected job outcomes");
    };
    assert!(!c1 && c2 && k1 == k2, "mixed triggers must coalesce");
    assert_eq!(orch.job_keys().len(), 1);

    let job = orch.job(&k1).expect("job");
    assert_eq!(job.state, RenderJobState::Published);
    let triggers: Vec<RenderTrigger> = job.triggers.iter().map(|t| t.trigger).collect();
    assert!(triggers.contains(&RenderTrigger::Manual));
    assert!(triggers.contains(&RenderTrigger::OnSave));
    assert_eq!(job.saved_revision, Some(ProjectRevision(7)));

    // Exactly one render, exactly one artifact — no duplicate publish.
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let recipe_key = job_recipe_key(&orch, &k1);
    let coalesced = orch
        .job_receipts(&k1)
        .iter()
        .any(|r| matches!(r.kind, ReceiptKind::Coalesced { .. }));
    drop(orch);
    assert_eq!(artifact_count(&store, &recipe_key), 1);
    assert!(coalesced, "the coalesced duplicate is receipt-visible");
}

#[test]
fn pol01_duplicate_events_never_duplicate_work() {
    let project = project_r1();
    let sel = selection(&project, "head-dup");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    for _ in 0..3 {
        let out = orch.handle(&report(manual_event(sel.clone())));
        assert!(matches!(out, TriggerOutcome::Job { .. }));
    }
    assert_eq!(orch.job_keys().len(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let key = orch.job_keys().into_iter().next().expect("one job");
    let recipe_key = job_recipe_key(&orch, &key);
    let coalesced = orch
        .job_receipts(&key)
        .iter()
        .filter(|r| matches!(r.kind, ReceiptKind::Coalesced { .. }))
        .count();
    drop(orch);
    assert_eq!(artifact_count(&store, &recipe_key), 1);
    assert_eq!(coalesced, 2, "two duplicates coalesce with receipts");
}

// ---------------------------------------------------------------------------
// RCP-POL-02 — policy edits never move RecipeKey; quality edits do
// ---------------------------------------------------------------------------

#[test]
fn pol02_policy_edits_leave_recipe_key_quality_edits_move_it() {
    let project = project_r1();
    let sel = selection(&project, "head-policy");

    // Job A: default policy. Job B: same target/purpose but a policy
    // differing in every non-quality field — triggers, scope,
    // completion, budget, retention.
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies_a = StaticPolicies::default();
    let policies_b = StaticPolicies {
        declared: Some(PolicyPatch {
            triggers: Some(BTreeSet::from([
                RenderTrigger::OnSave,
                RenderTrigger::OnCommit,
            ])),
            scope: Some(RenderScope::CollaborativeEditing),
            completion: Some(CompletionRequirement::BestEffort),
            resource_budget: Some(ResourceBudget {
                max_cpu_ms: 60_000,
                max_wall_ms: 120_000,
                max_storage_bytes: 1 << 20,
                max_concurrent_jobs: 4,
            }),
            retention: Some(synvelis_render::policy::RetentionPolicy {
                protect_referenced: false,
                unreferenced_ttl_ticks: Some(100),
            }),
            ..PolicyPatch::default()
        }),
        local: None,
    };
    let demands = MasterDemands;

    // Job A: default policy. Job B: identical pinned target, demands
    // and purpose, but a policy differing in every non-quality field
    // — triggers, scope, completion, budget, retention. Only the
    // policy-resolution component of the job key may move.
    let rk_a;
    {
        let mut orch = Ports {
            store: &mut store,
            executor: &mut executor,
            settings: &settings,
            policies: &policies_a,
            demands: &demands,
            authorizer: None,
        }
        .build(cpu_grants());
        let key_a = match orch.submit(&intent_for(sel.clone())) {
            SubmitOutcome::Accepted {
                key,
                coalesced: false,
            } => key,
            other => panic!("submit A failed: {other:?}"),
        };
        let _ = orch.drive(&key_a);
        rk_a = job_recipe_key(&orch, &key_a);

        let mut intent_b = intent_for(sel.clone());
        intent_b.policy = resolve_effective_policy(
            policies_b.declared.as_ref(),
            policies_b.local.as_ref(),
            None,
        );
        let key_b = match orch.submit(&intent_b) {
            SubmitOutcome::Accepted {
                key,
                coalesced: false,
            } => key,
            other => panic!("submit B failed: {other:?}"),
        };
        let _ = orch.drive(&key_b);
        let rk_b = job_recipe_key(&orch, &key_b);

        assert_ne!(key_a, key_b, "policy resolution versions the job key");
        assert_eq!(rk_a, rk_b, "policy edits must not move RecipeKey");
        let renders = calls.load(Ordering::SeqCst);
        assert_eq!(renders, 1, "policy change must not re-render");
    }

    // Quality-affecting change: a different sample rate produces a
    // different recipe key.
    let settings_alt = StaticSettings(quality_alt());
    let mut executor2 = FakeExecutor::output();
    let calls2 = executor2.calls.clone();
    let rk_c;
    {
        let mut orch = Ports {
            store: &mut store,
            executor: &mut executor2,
            settings: &settings_alt,
            policies: &policies_a,
            demands: &demands,
            authorizer: None,
        }
        .build(cpu_grants());
        let key_c = match orch.submit(&intent_for(sel.clone())) {
            SubmitOutcome::Accepted { key, .. } => key,
            other @ SubmitOutcome::Rejected { .. } => panic!("submit C failed: {other:?}"),
        };
        let _ = orch.drive(&key_c);
        rk_c = job_recipe_key(&orch, &key_c);
    }
    assert_ne!(
        rk_a, rk_c,
        "quality-affecting settings change must move RecipeKey"
    );
    let renders = calls2.load(Ordering::SeqCst);
    assert_eq!(renders, 1, "quality change re-renders once");
}

// ---------------------------------------------------------------------------
// RCP-POL-03 — save failure inert; saved revision pinned; no
// mis-association across revisions
// ---------------------------------------------------------------------------

#[test]
fn pol03_save_failure_launches_no_render_job() {
    let project = project_r1();
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies {
        declared: Some(triggers_patch(&[RenderTrigger::OnSave])),
        local: None,
    };
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    let out = orch.handle(&report(TriggerEvent::Save(SaveEvent::Failed {
        project_id: project.project_id,
        revision: ProjectRevision(3),
        reason: "disk full".to_owned(),
    })));

    assert!(matches!(
        out,
        TriggerOutcome::Inert {
            reason: InertReason::ExternalFailed
        }
    ));
    assert!(orch.job_keys().is_empty(), "no job may be created");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(orch.external_receipts().iter().any(|r| matches!(
        r,
        synvelis_render::policy::ExternalReceipt::SaveFailed { .. }
    )));
}

#[test]
fn pol03_saved_revision_pinned_and_never_reassociated() {
    let project_a = project_r1();
    let project_b = project_r2();
    let snap_a = pinned(&project_a, "head-r1");
    let snap_b = pinned(&project_b, "head-r2");
    let digest_a = snap_a.identity.digest;
    let digest_b = snap_b.identity.digest;
    assert_ne!(digest_a, digest_b);

    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies {
        declared: Some(triggers_patch(&[RenderTrigger::OnSave])),
        local: None,
    };
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    let out_a = orch.handle(&report(save_completed(&snap_a, 1, "head-r1")));
    let out_b = orch.handle(&report(save_completed(&snap_b, 2, "head-r2")));
    let (TriggerOutcome::Job { key: k_a, .. }, TriggerOutcome::Job { key: k_b, .. }) =
        (out_a, out_b)
    else {
        panic!("expected job outcomes");
    };
    assert_ne!(k_a, k_b, "different pins must not share a job");

    let job_a = orch.job(&k_a).expect("job A");
    let job_b = orch.job(&k_b).expect("job B");
    assert_eq!(job_a.state, RenderJobState::Published);
    assert_eq!(job_b.state, RenderJobState::Published);
    assert_eq!(job_a.saved_revision, Some(ProjectRevision(1)));
    assert_eq!(job_b.saved_revision, Some(ProjectRevision(2)));
    assert_eq!(job_a.selection.snapshot.identity.digest, digest_a);
    assert_eq!(job_b.selection.snapshot.identity.digest, digest_b);

    // Job A's receipts reference only pin A — completion of A can
    // never be re-associated with revision 2's snapshot.
    for receipt in orch.job_receipts(&k_a) {
        if let ReceiptKind::Submitted {
            snapshot_digest, ..
        } = receipt.kind
        {
            assert_eq!(snapshot_digest, digest_a);
        }
    }

    // Distinct recipes → distinct artifacts; no cross-wiring.
    let artifacts_a = &job_a.artifacts;
    let artifacts_b = &job_b.artifacts;
    assert!(artifacts_a.is_disjoint(artifacts_b));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

// ---------------------------------------------------------------------------
// RCP-POL-04 — commit capability typed; external receipts separate
// ---------------------------------------------------------------------------

#[test]
fn pol04_commit_unsupported_is_typed_not_silent() {
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies {
        declared: Some(triggers_patch(&[RenderTrigger::OnCommit])),
        local: None,
    };
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    let out = orch.handle(&report(TriggerEvent::Commit(CommitEvent::Unsupported {
        reason: "provider has no commit API".to_owned(),
    })));

    assert!(matches!(
        out,
        TriggerOutcome::Unsupported {
            capability: "vcs-commit",
            ..
        }
    ));
    assert_eq!(orch.job_keys(), Vec::new());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(orch.receipts(), Vec::new(), "no render-job receipts");
    assert!(orch.external_receipts().iter().any(|r| matches!(
        r,
        synvelis_render::policy::ExternalReceipt::CommitUnsupported { .. }
    )));
}

#[test]
fn pol04_provider_and_push_failures_stay_off_job_stream() {
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies {
        declared: Some(triggers_patch(&[RenderTrigger::OnCommit])),
        local: None,
    };
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    let out_provider = orch.handle(&report(TriggerEvent::Commit(CommitEvent::ProviderFailed {
        detail: "hook rejected".to_owned(),
    })));
    let out_push = orch.handle(&report(TriggerEvent::Commit(CommitEvent::PushFailed {
        commit_locator: "sha256:commit9".to_owned(),
        detail: "network unreachable".to_owned(),
    })));

    assert!(matches!(
        out_provider,
        TriggerOutcome::Inert {
            reason: InertReason::ExternalFailed
        }
    ));
    assert!(matches!(
        out_push,
        TriggerOutcome::Inert {
            reason: InertReason::ExternalFailed
        }
    ));
    assert_eq!(orch.job_keys(), Vec::new());
    assert_eq!(orch.receipts(), Vec::new());
    assert!(orch.external_receipts().iter().any(|r| matches!(
        r,
        synvelis_render::policy::ExternalReceipt::CommitProviderFailed { .. }
    )));
    assert!(orch.external_receipts().iter().any(|r| matches!(
        r,
        synvelis_render::policy::ExternalReceipt::CommitPushFailed { commit_locator, .. }
            if commit_locator == "sha256:commit9"
    )));
}

#[test]
fn pol04_commit_success_pins_immutable_locator_and_attaches_manifest() {
    let project = project_r1();
    let snap = pinned(&project, "head-commit");
    let digest = snap.identity.digest;
    let locator = "sha256:0123456789abcdef".to_owned();
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies {
        declared: Some(triggers_patch(&[RenderTrigger::OnCommit])),
        local: None,
    };
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    let out = orch.handle(&report(TriggerEvent::Commit(CommitEvent::Committed(
        Box::new(CommitReceipt {
            commit_locator: locator.clone(),
            snapshot: snap,
        }),
    ))));
    let TriggerOutcome::Job { key, .. } = out else {
        panic!("expected job outcome");
    };
    assert_eq!(
        orch.job(&key).expect("job").state,
        RenderJobState::Published
    );
    let artifact_count_job = orch.job(&key).expect("job").artifacts.len();
    let artifacts: Vec<_> = orch
        .job(&key)
        .expect("job")
        .artifacts
        .iter()
        .copied()
        .collect();
    drop(orch);

    // Every published artifact is attached to the immutable commit
    // locator — the SHA is never rewritten.
    let entries = store
        .external_manifests(&locator, &digest)
        .expect("external manifests");
    assert_eq!(entries.len(), artifact_count_job);
    for artifact in &artifacts {
        assert!(entries.iter().any(|e| &e.artifact == artifact));
    }
    assert!(entries.iter().all(|e| e.locator == locator));
}

// ---------------------------------------------------------------------------
// RCP-POL-05 — BestEffort vs RequireComplete completeness gate
// ---------------------------------------------------------------------------

fn share_event(
    snapshot: &PinnedSnapshot,
    demands: Vec<RenderDemand>,
    completion: CompletionRequirement,
) -> TriggerEvent {
    TriggerEvent::Share(ShareRequest {
        package_id: "pkg-1".to_owned(),
        snapshot: snapshot.clone(),
        demands,
        policy_intent: Some(PolicyPatch {
            triggers: Some(BTreeSet::from([RenderTrigger::OnShare])),
            completion: Some(completion),
            ..PolicyPatch::default()
        }),
    })
}

fn bad_port_demand() -> RenderDemand {
    RenderDemand {
        boundary: RenderBoundary::new(vec![SemanticPortId::new(0xDEAD)]),
        interval: FrameInterval::new(0, FRAMES).unwrap(),
    }
}

#[test]
fn pol05_besteffort_share_discloses_partial_never_complete() {
    let project = project_r1();
    let snap = pinned(&project, "head-share-be");
    let demands = vec![master_demand(&project), bad_port_demand()];
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demand_port = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demand_port,
        authorizer: None,
    }
    .build(cpu_grants());

    let out = orch.handle(&report(share_event(
        &snap,
        demands,
        CompletionRequirement::BestEffort,
    )));

    let TriggerOutcome::ShareGate { job, verdict } = out else {
        panic!("expected share gate outcome");
    };
    match verdict {
        ShareGateVerdict::AllowedPartial { gaps, artifacts } => {
            assert!(!gaps.is_empty(), "partial must disclose gaps");
            assert!(!artifacts.is_empty(), "satisfied artifacts listed");
        }
        other => panic!("expected AllowedPartial, got {other:?}"),
    }
    let key = job.expect("fill job");
    let job_rec = orch.job(&key).expect("job");
    assert_eq!(job_rec.state, RenderJobState::Partial);
    assert_eq!(
        job_rec.completeness.as_ref().map(|c| c.verdict),
        Some(CompletenessVerdict::Incomplete),
        "incomplete must never be reported complete"
    );
}

#[test]
fn pol05_requirecomplete_share_blocks_with_typed_gaps() {
    let project = project_r1();
    let snap = pinned(&project, "head-share-rc");
    let demands = vec![master_demand(&project), bad_port_demand()];
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demand_port = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demand_port,
        authorizer: None,
    }
    .build(cpu_grants());

    let out = orch.handle(&report(share_event(
        &snap,
        demands,
        CompletionRequirement::RequireComplete,
    )));

    let TriggerOutcome::ShareGate { job, verdict } = out else {
        panic!("expected share gate outcome");
    };
    match verdict {
        ShareGateVerdict::Blocked { gaps, pending_jobs } => {
            assert_ne!(gaps, Vec::new());
            assert!(pending_jobs.is_empty(), "job is terminal, not pending");
        }
        other => panic!("expected Blocked, got {other:?}"),
    }
    let key = job.expect("fill job");
    assert_eq!(orch.job(&key).expect("job").state, RenderJobState::Partial);
}

#[test]
fn pol05_missing_plugin_is_a_typed_gap_never_complete() {
    let project = project_with_plugin();
    let snap = pinned(&project, "head-share-plugin");
    let demands = vec![master_demand(&project)];
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demand_port = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demand_port,
        authorizer: None,
    }
    .build(cpu_grants());

    let out = orch.handle(&report(share_event(
        &snap,
        demands,
        CompletionRequirement::RequireComplete,
    )));

    let TriggerOutcome::ShareGate { verdict, .. } = out else {
        panic!("expected share gate outcome");
    };
    match verdict {
        ShareGateVerdict::Blocked { gaps, .. } => {
            assert!(
                gaps.iter()
                    .any(|g| g.kind == CompletenessGapKind::PluginMissing),
                "missing plugin must surface as PluginMissing gap: {gaps:?}"
            );
        }
        other => panic!("expected Blocked, got {other:?}"),
    }
    // No render work was lawful — the executor was never invoked.
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn pol05_complete_package_is_allowed() {
    let project = project_r1();
    let snap = pinned(&project, "head-share-ok");
    let demands = vec![master_demand(&project)];
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demand_port = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demand_port,
        authorizer: None,
    }
    .build(cpu_grants());

    let out = orch.handle(&report(share_event(
        &snap,
        demands,
        CompletionRequirement::RequireComplete,
    )));

    let TriggerOutcome::ShareGate { verdict, .. } = out else {
        panic!("expected share gate outcome");
    };
    match verdict {
        ShareGateVerdict::Allowed { artifacts } => {
            assert_eq!(artifacts.len(), 1);
        }
        other => panic!("expected Allowed, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// RCP-POL-06 — failure matrix: budget/grant/consent/quota/cancel/
// retry/crash — no unauthorized work, no duplicate work, no false
// success, no realtime fallback
// ---------------------------------------------------------------------------

#[test]
fn pol06_budget_and_permission_denials_reject_before_any_work() {
    let project = project_r1();
    let sel = selection(&project, "head-deny");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies {
        declared: Some(PolicyPatch {
            resource_budget: Some(ResourceBudget {
                max_cpu_ms: 1,
                max_wall_ms: u64::MAX,
                max_storage_bytes: u64::MAX,
                max_concurrent_jobs: 4,
            }),
            ..PolicyPatch::default()
        }),
        local: None,
    };
    let demands = MasterDemands;

    // Budget: estimate exceeds the declared cap → typed rejection,
    // no job, no render.
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());
    let mut intent = intent_for(sel.clone());
    intent.estimate = JobCostEstimate {
        cpu_ms: 1_000,
        wall_ms: 0,
        storage_bytes: 0,
    };
    intent.policy = resolve_effective_policy(policies.declared.as_ref(), None, None);
    match orch.submit(&intent) {
        SubmitOutcome::Rejected { reason, .. } => {
            assert_eq!(reason, JobFailureReason::BudgetExceeded);
        }
        other @ SubmitOutcome::Accepted { .. } => {
            panic!("expected budget rejection, got {other:?}")
        }
    }
    assert_eq!(orch.job_keys(), Vec::new());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(orch.rejected_attempts().len(), 1);

    // Permission: CPU render not granted → typed denial.
    let policies_open = StaticPolicies::default();
    let no_grants = LocalGrants {
        cpu_render: false,
        ..LocalGrants::cpu_only()
    };
    let mut orch2 = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies_open,
        demands: &demands,
        authorizer: None,
    }
    .build(no_grants);
    match orch2.submit(&intent_for(sel.clone())) {
        SubmitOutcome::Rejected { reason, .. } => {
            assert_eq!(reason, JobFailureReason::AuthDenied);
        }
        other @ SubmitOutcome::Accepted { .. } => panic!("expected auth denial, got {other:?}"),
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn pol06_remote_backend_requires_named_consent() {
    let project = project_r1();
    let sel = selection(&project, "head-remote");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor {
        calls: Arc::new(AtomicUsize::new(0)),
        steps: vec![ExecStep::Output],
        backend: BackendRequirement::Remote {
            backend_id: "cloud-render-1".to_owned(),
        },
    };
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demands = MasterDemands;

    // No network grant → RemoteConsentDenied before any work.
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(LocalGrants::default());
    match orch.submit(&intent_for(sel.clone())) {
        SubmitOutcome::Rejected { reason, .. } => {
            assert_eq!(reason, JobFailureReason::RemoteConsentDenied);
        }
        other @ SubmitOutcome::Accepted { .. } => {
            panic!("expected remote consent denial, got {other:?}")
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    // Network granted but backend not consented → still denied.
    let grants_partial = LocalGrants {
        network: true,
        remote_backends: BTreeSet::from(["other-backend".to_owned()]),
        ..LocalGrants::cpu_only()
    };
    orch.set_grants(grants_partial);
    match orch.submit(&intent_for(sel.clone())) {
        SubmitOutcome::Rejected { reason, .. } => {
            assert_eq!(reason, JobFailureReason::RemoteConsentDenied);
        }
        other @ SubmitOutcome::Accepted { .. } => {
            panic!("expected remote consent denial, got {other:?}")
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    // Named backend + network → admitted.
    let grants_full = LocalGrants {
        network: true,
        remote_backends: BTreeSet::from(["cloud-render-1".to_owned()]),
        ..LocalGrants::cpu_only()
    };
    orch.set_grants(grants_full);
    match orch.submit(&intent_for(sel)) {
        SubmitOutcome::Accepted { .. } => {}
        other @ SubmitOutcome::Rejected { .. } => panic!("expected acceptance, got {other:?}"),
    }
    let _ = orch.run();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn pol06_concurrency_quota_pends_then_resumes() {
    let project_a = project_r1();
    let project_b = project_r2();
    let sel_a = selection(&project_a, "head-q-a");
    let sel_b = selection(&project_b, "head-q-b");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies {
        declared: Some(PolicyPatch {
            resource_budget: Some(ResourceBudget {
                max_cpu_ms: u64::MAX,
                max_wall_ms: u64::MAX,
                max_storage_bytes: u64::MAX,
                max_concurrent_jobs: 1,
            }),
            ..PolicyPatch::default()
        }),
        local: None,
    };
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    let mut i_a = intent_for(sel_a);
    i_a.policy = resolve_effective_policy(policies.declared.as_ref(), None, None);
    let mut i_b = intent_for(sel_b);
    i_b.policy = resolve_effective_policy(policies.declared.as_ref(), None, None);

    let k_a = match orch.submit(&i_a) {
        SubmitOutcome::Accepted { key, .. } => key,
        other @ SubmitOutcome::Rejected { .. } => panic!("submit A failed: {other:?}"),
    };
    let k_b = match orch.submit(&i_b) {
        SubmitOutcome::Accepted { key, .. } => key,
        other @ SubmitOutcome::Rejected { .. } => panic!("submit B failed: {other:?}"),
    };

    // Second job pends on the concurrency quota — typed Pending
    // receipt, not a failure, no work issued.
    let job_b = orch.job(&k_b).expect("job B");
    assert_eq!(job_b.pending_reason, Some(JobFailureReason::QuotaExceeded));
    assert!(orch.job_receipts(&k_b).iter().any(|r| matches!(
        r.kind,
        ReceiptKind::Pending {
            reason: JobFailureReason::QuotaExceeded,
            ..
        }
    )));

    // Drive to quiescence — both publish exactly once.
    for _ in 0..4 {
        orch.run();
    }
    assert_eq!(orch.job(&k_a).expect("A").state, RenderJobState::Published);
    assert_eq!(orch.job(&k_b).expect("B").state, RenderJobState::Published);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(
        orch.job_receipts(&k_b)
            .iter()
            .any(|r| matches!(r.kind, ReceiptKind::Resumed))
    );
}

#[test]
fn pol06_storage_quota_pends_until_headroom() {
    let project = project_r1();
    let sel = selection(&project, "head-storage");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor {
        calls: Arc::new(AtomicUsize::new(0)),
        steps: vec![ExecStep::Output],
        backend: BackendRequirement::Cpu,
    };
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demands = MasterDemands;
    let tight = LocalGrants {
        storage_quota_bytes: 1,
        ..LocalGrants::cpu_only()
    };
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(tight);

    let mut intent = intent_for(sel);
    intent.estimate.storage_bytes = 4_096;
    let key = match orch.submit(&intent) {
        SubmitOutcome::Accepted { key, .. } => key,
        other @ SubmitOutcome::Rejected { .. } => panic!("submit failed: {other:?}"),
    };
    assert_eq!(
        orch.job(&key).expect("job").pending_reason,
        Some(JobFailureReason::InsufficientStorage)
    );
    let _ = orch.run();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "no work while storage pends"
    );

    orch.set_grants(LocalGrants::cpu_only());
    let _ = orch.run();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        orch.job(&key).expect("job").state,
        RenderJobState::Published
    );
}

#[test]
fn pol06_retryable_failure_pends_and_retry_completes_once() {
    let project = project_r1();
    let sel = selection(&project, "head-retry");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor {
        calls: Arc::new(AtomicUsize::new(0)),
        steps: vec![
            ExecStep::Fail(JobFailureReason::WorkerFailure, "transient"),
            ExecStep::Output,
        ],
        backend: BackendRequirement::Cpu,
    };
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    let key = match orch.submit(&intent_for(sel)) {
        SubmitOutcome::Accepted { key, .. } => key,
        other @ SubmitOutcome::Rejected { .. } => panic!("submit failed: {other:?}"),
    };
    let outcome = orch.drive(&key);
    assert_eq!(outcome, DriveOutcome::Pending);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(orch.job_receipts(&key).iter().any(|r| matches!(
        r.kind,
        ReceiptKind::BoundaryFailed {
            failure: synvelis_render::render_job::JobFailure {
                reason: JobFailureReason::WorkerFailure,
                ..
            },
            ..
        }
    )));

    // Non-retryable failures cannot be retried; retryable ones can.
    assert!(orch.retry(&key));
    let _ = orch.run();
    let job = orch.job(&key).expect("job");
    assert_eq!(job.state, RenderJobState::Published);
    assert_eq!(job.attempt, 2);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(
        orch.job_receipts(&key)
            .iter()
            .any(|r| matches!(r.kind, ReceiptKind::Retrying { attempt: 2 }))
    );
}

#[test]
fn pol06_crash_resumes_without_duplicate_artifacts() {
    let project = project_r1();
    let sel = selection(&project, "head-crash");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor {
        calls: Arc::new(AtomicUsize::new(0)),
        steps: vec![ExecStep::CrashOnce, ExecStep::Output],
        backend: BackendRequirement::Cpu,
    };
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    let key = match orch.submit(&intent_for(sel)) {
        SubmitOutcome::Accepted { key, .. } => key,
        other @ SubmitOutcome::Rejected { .. } => panic!("submit failed: {other:?}"),
    };
    let outcome = orch.drive(&key);
    assert_eq!(outcome, DriveOutcome::Crashed);
    assert!(orch.job(&key).expect("job").crashed);
    assert!(
        orch.job_receipts(&key)
            .iter()
            .any(|r| matches!(r.kind, ReceiptKind::CrashObserved))
    );

    let outcome = orch.resume(&key);
    assert_eq!(outcome, DriveOutcome::Terminal);
    let job = orch.job(&key).expect("job");
    assert_eq!(job.state, RenderJobState::Published);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "crash + resume renders once more"
    );
    assert_eq!(job.artifacts.len(), 1, "exactly one artifact");
    let recipe_key = job_recipe_key(&orch, &key);
    drop(orch);
    assert_eq!(artifact_count(&store, &recipe_key), 1);
}

#[test]
fn pol06_cancel_before_and_during_render_is_typed() {
    let project = project_r1();
    let sel = selection(&project, "head-cancel");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor {
        calls: Arc::new(AtomicUsize::new(0)),
        steps: vec![ExecStep::Cancelled],
        backend: BackendRequirement::Cpu,
    };
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    // Queued cancel: no work was ever issued.
    let k1 = match orch.submit(&intent_for(sel.clone())) {
        SubmitOutcome::Accepted { key, .. } => key,
        other @ SubmitOutcome::Rejected { .. } => panic!("submit failed: {other:?}"),
    };
    assert!(orch.cancel(&k1));
    assert_eq!(orch.job(&k1).expect("job").state, RenderJobState::Cancelled);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(
        orch.job_receipts(&k1)
            .iter()
            .any(|r| matches!(r.kind, ReceiptKind::CancelRequested))
    );

    // Executor-observed cancellation lands as Cancelled, never as
    // failure or success.
    let sel2 = RenderSelection {
        snapshot: pinned(&project, "head-cancel-2"),
        demands: vec![master_demand(&project)],
    };
    let k2 = match orch.submit(&intent_for(sel2)) {
        SubmitOutcome::Accepted { key, .. } => key,
        other @ SubmitOutcome::Rejected { .. } => panic!("submit failed: {other:?}"),
    };
    let _ = orch.drive(&k2);
    assert_eq!(orch.job(&k2).expect("job").state, RenderJobState::Cancelled);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(orch.job(&k2).expect("job").artifacts.is_empty());
}

#[test]
fn pol06_ondemand_requires_explicit_authorization() {
    let project = project_r1();
    let sel = selection(&project, "head-demand");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demands = MasterDemands;

    let proposal = || DemandProposal {
        proposal_id: "prop-1".to_owned(),
        selection: sel.clone(),
        purpose: JobPurpose::Selection,
        rationale: "test".to_owned(),
        policy_intent: Some(triggers_patch(&[RenderTrigger::OnDemand])),
    };

    // No authorizer configured → denied, nothing consumed.
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());
    let out = orch.handle(&report(TriggerEvent::Demand(proposal())));
    assert!(matches!(out, TriggerOutcome::Denied { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    // Denying authorizer → denied with typed reason.
    let deny = StaticAuthorizer(DemandAuthorization::Denied {
        reason: "user declined".to_owned(),
    });
    let mut orch2 = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: Some(&deny),
    }
    .build(cpu_grants());
    let out = orch2.handle(&report(TriggerEvent::Demand(proposal())));
    assert!(matches!(out, TriggerOutcome::Denied { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    // Granting authorizer → job admitted under the grant identity.
    let grant = StaticAuthorizer(DemandAuthorization::Granted {
        grant: GrantRef("grant-7".to_owned()),
    });
    let mut orch3 = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: Some(&grant),
    }
    .build(cpu_grants());
    let mut rep = report(TriggerEvent::Demand(proposal()));
    rep.grant = Some(GrantRef("grant-7".to_owned()));
    let out = orch3.handle(&rep);
    let TriggerOutcome::Job { key, .. } = out else {
        panic!("expected job outcome");
    };
    let job = orch3.job(&key).expect("job");
    assert_eq!(job.state, RenderJobState::Published);
    assert_eq!(job.grant, Some(GrantRef("grant-7".to_owned())));
}

// ---------------------------------------------------------------------------
// RCP-POL-07 — one Gateway Action/Grant projection across surfaces
// ---------------------------------------------------------------------------

#[test]
fn pol07_entry_surfaces_share_one_action_and_coalesce() {
    let project = project_r1();
    let sel = selection(&project, "head-surfaces");
    let mut store = InMemoryArtifactStore::new();
    let mut executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let settings = StaticSettings(quality());
    let policies = StaticPolicies::default();
    let demands = MasterDemands;
    let mut orch = Ports {
        store: &mut store,
        executor: &mut executor,
        settings: &settings,
        policies: &policies,
        demands: &demands,
        authorizer: None,
    }
    .build(cpu_grants());

    let surfaces = [
        EntrySurface::Gui,
        EntrySurface::HeadlessCli,
        EntrySurface::Workflow,
        EntrySurface::Script,
    ];
    let mut key = None;
    for (i, surface) in surfaces.iter().enumerate() {
        let mut rep = report(manual_event(sel.clone()));
        rep.entry = *surface;
        rep.action = GatewayActionRef("action.render.shared".to_owned());
        rep.grant = Some(GrantRef("grant-shared".to_owned()));
        let out = orch.handle(&rep);
        let TriggerOutcome::Job { key: k, coalesced } = out else {
            panic!("expected job outcome for {surface:?}");
        };
        assert_eq!(coalesced, i > 0, "surface {i} must coalesce");
        match &key {
            None => key = Some(k),
            Some(prev) => assert_eq!(*prev, k, "all surfaces share one job key"),
        }
    }
    let key = key.expect("job key");

    // One job, one render, one artifact — receipts show each surface
    // entered through the same action/grant projection.
    assert_eq!(orch.job_keys().len(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let job = orch.job(&key).expect("job");
    assert_eq!(
        job.action,
        GatewayActionRef("action.render.shared".to_owned())
    );
    assert_eq!(job.grant, Some(GrantRef("grant-shared".to_owned())));
    assert_eq!(job.artifacts.len(), 1);

    let mut entries: Vec<EntrySurface> = orch
        .job_receipts(&key)
        .iter()
        .filter_map(|r| match &r.kind {
            ReceiptKind::Submitted { entry, .. } | ReceiptKind::Coalesced { entry, .. } => {
                Some(*entry)
            }
            _ => None,
        })
        .collect();
    entries.sort_by_key(|e| format!("{e:?}"));
    let mut expected = surfaces.to_vec();
    expected.sort_by_key(|e| format!("{e:?}"));
    assert_eq!(entries, expected, "every surface recorded once");
}

// ---------------------------------------------------------------------------
// async scheduler pins (synvelis#518) — the pol01/pol06 contracts under the
// non-RT `JobScheduler` worker, not the synchronous `drive()` equivalent.
// `GateExecutor` parks the worker thread inside a render so commands
// (cancel, quota-bound submits, queue fills) interleave deterministically.
// ---------------------------------------------------------------------------

/// Executor that blocks inside its first `render` call until the test
/// releases it; later calls render the fixture WAV immediately.
struct GateExecutor {
    calls: Arc<AtomicUsize>,
    entered: Sender<()>,
    release: Receiver<()>,
}

impl RenderExecutor for GateExecutor {
    fn backend_requirement(&self) -> BackendRequirement {
        BackendRequirement::Cpu
    }

    fn render(
        &mut self,
        job: &JobContext<'_>,
        _recipe: &synvelis_render::recipe::RenderRecipeV1,
        cancel: &ruviform_exec::CancellationToken,
    ) -> RenderExecution {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let _ = self.entered.send(());
        if call == 0 {
            // Park on the worker thread — the non-RT lane the
            // scheduler owns; commands keep flowing meanwhile.
            let _ = self.release.recv();
        }
        if cancel.is_cancelled() {
            return RenderExecution::Cancelled;
        }
        let frames = job.demand.interval.end - job.demand.interval.start;
        wav_output(frames)
    }
}

fn scheduler_ports(
    executor: Box<dyn RenderExecutor + Send>,
    settings: Box<dyn RenderSettingsProbe + Send>,
    policies: Box<dyn PolicyResolver + Send>,
    demands: Box<dyn ScopeDemandResolver + Send>,
) -> SchedulerPorts {
    SchedulerPorts {
        store: Box::new(InMemoryArtifactStore::new()),
        executor,
        settings,
        policies,
        demands,
        plugin_states: None,
        external_assets: None,
        authorizer: None,
        calibration: None,
    }
}

fn gated_executor() -> (GateExecutor, Receiver<()>, Sender<()>, Arc<AtomicUsize>) {
    let (entered_tx, entered_rx) = channel();
    let (release_tx, release_rx) = channel();
    let calls = Arc::new(AtomicUsize::new(0));
    (
        GateExecutor {
            calls: calls.clone(),
            entered: entered_tx,
            release: release_rx,
        },
        entered_rx,
        release_tx,
        calls,
    )
}

/// Polls `scheduler.job(key)` until `pred` holds or the timeout
/// elapses — returns the last view seen.
fn wait_job(
    scheduler: &JobScheduler,
    key: &RenderJobKey,
    timeout: Duration,
    pred: impl Fn(&synvelis_render::render_job::JobView) -> bool,
) -> Option<synvelis_render::render_job::JobView> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let view = scheduler.job(key).ok().flatten();
        if let Some(v) = &view
            && pred(v)
        {
            return view;
        }
        if std::time::Instant::now() >= deadline {
            return view;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn accept_key(outcome: SubmitOutcome) -> RenderJobKey {
    match outcome {
        SubmitOutcome::Accepted { key, .. } => key,
        other @ SubmitOutcome::Rejected { .. } => panic!("submit rejected: {other:?}"),
    }
}

#[test]
fn async_pol01_mixed_triggers_coalesce_into_one_job() {
    let project = project_r1();
    let sel = selection(&project, "head-async-mix");
    let executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let scheduler = JobScheduler::spawn(
        scheduler_ports(
            Box::new(executor),
            Box::new(StaticSettings(quality())),
            Box::new(StaticPolicies {
                declared: Some(triggers_patch(&[
                    RenderTrigger::OnSave,
                    RenderTrigger::OnCommit,
                    RenderTrigger::OnShare,
                    RenderTrigger::OnDemand,
                ])),
                local: None,
            }),
            Box::new(MasterDemands),
        ),
        cpu_grants(),
        8,
    );

    // Manual and OnSave describe the same pinned target + purpose —
    // they must coalesce into one job even though submission now
    // happens on the worker thread.
    let out1 = scheduler
        .handle_report(report(manual_event(sel.clone())))
        .expect("scheduler alive");
    let out2 = scheduler
        .handle_report(report(save_completed(&sel.snapshot, 7, "head-async-mix")))
        .expect("scheduler alive");
    let pair = (out1, out2);
    let (
        TriggerOutcome::Job {
            key: k1,
            coalesced: c1,
        },
        TriggerOutcome::Job {
            key: k2,
            coalesced: c2,
        },
    ) = (&pair.0, &pair.1)
    else {
        panic!("both reports must route to jobs: {pair:?}")
    };
    let (k1, k2) = (k1.clone(), k2.clone());
    assert_eq!(k1, k2, "same pinned target must share one job key");
    assert!(!*c1 && *c2, "the second trigger must coalesce");

    let view = scheduler
        .wait_terminal(&k1, Duration::from_secs(10))
        .expect("job reaches terminal");
    assert_eq!(view.state, RenderJobState::Published);
    assert_eq!(view.artifacts.len(), 1, "exactly one artifact");
    assert_eq!(view.saved_revision, Some(ProjectRevision(7)));
    assert_eq!(calls.load(Ordering::SeqCst), 1, "one render total");
    assert!(
        scheduler
            .job_receipts(&k1)
            .expect("receipts")
            .iter()
            .any(|r| matches!(r.kind, ReceiptKind::Coalesced { .. })),
        "coalescing stays receipt-visible under async"
    );
}

#[test]
fn async_pol01_duplicate_intent_submit_is_idempotent() {
    let project = project_r1();
    let sel = selection(&project, "head-async-dup");
    let executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let scheduler = JobScheduler::spawn(
        scheduler_ports(
            Box::new(executor),
            Box::new(StaticSettings(quality())),
            Box::new(StaticPolicies::default()),
            Box::new(MasterDemands),
        ),
        cpu_grants(),
        8,
    );

    let k1 = accept_key(
        scheduler
            .submit_intent(intent_for(sel.clone()))
            .expect("alive"),
    );
    let outcome2 = scheduler.submit_intent(intent_for(sel)).expect("alive");
    let accepted2 = match &outcome2 {
        SubmitOutcome::Accepted { key, coalesced } => (key.clone(), *coalesced),
        other @ SubmitOutcome::Rejected { .. } => {
            panic!("duplicate submit must coalesce, got {other:?}")
        }
    };
    let (k2, c2) = accepted2;
    assert_eq!(k1, k2);
    assert!(c2, "identical intent must coalesce");

    let view = scheduler
        .wait_terminal(&k1, Duration::from_secs(10))
        .expect("job reaches terminal");
    assert_eq!(view.state, RenderJobState::Published);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "coalesced duplicate never re-renders"
    );
}

#[test]
fn async_pol06_concurrency_quota_pends_then_resumes() {
    let project_a = project_r1();
    let project_b = project_r2();
    // Job A carries two demands so it stays non-terminal across
    // several scheduler quanta — B's submit is guaranteed to be
    // evaluated while A still occupies the single concurrency slot.
    let sel_a = RenderSelection {
        snapshot: pinned(&project_a, "head-async-q-a"),
        demands: vec![master_demand(&project_a), master_demand(&project_a)],
    };
    let sel_b = selection(&project_b, "head-async-q-b");
    let (executor, entered_rx, release_tx, calls) = gated_executor();
    let declared = PolicyPatch {
        resource_budget: Some(ResourceBudget {
            max_cpu_ms: u64::MAX,
            max_wall_ms: u64::MAX,
            max_storage_bytes: u64::MAX,
            max_concurrent_jobs: 1,
        }),
        ..PolicyPatch::default()
    };
    let scheduler = JobScheduler::spawn(
        scheduler_ports(
            Box::new(executor),
            Box::new(StaticSettings(quality())),
            Box::new(StaticPolicies {
                declared: Some(declared.clone()),
                local: None,
            }),
            Box::new(MasterDemands),
        ),
        cpu_grants(),
        8,
    );

    let mut i_a = intent_for(sel_a);
    i_a.policy = resolve_effective_policy(Some(&declared), None, None);
    let mut i_b = intent_for(sel_b);
    i_b.policy = resolve_effective_policy(Some(&declared), None, None);

    let k_a = accept_key(scheduler.submit_intent(i_a).expect("alive"));

    // B's submit runs on a helper thread: its reply can only arrive
    // after the worker drains it, which may be after A's parked
    // render — awaiting it on the test thread would deadlock.
    let (b_tx, b_rx) = channel();
    let sched_b = scheduler.clone();
    std::thread::spawn(move || {
        let _ = b_tx.send(sched_b.submit_intent(i_b));
    });

    // A is inside its first render — it lawfully occupies the slot.
    entered_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("A render entered");
    release_tx.send(()).expect("release render");

    let out_b = b_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("B submit answered");
    let k_b = accept_key(out_b.expect("scheduler alive"));

    let va = scheduler
        .wait_terminal(&k_a, Duration::from_secs(10))
        .expect("A terminal");
    let vb = scheduler
        .wait_terminal(&k_b, Duration::from_secs(10))
        .expect("B terminal");
    assert_eq!(va.state, RenderJobState::Published);
    assert_eq!(vb.state, RenderJobState::Published);
    assert_eq!(calls.load(Ordering::SeqCst), 2, "each job renders once");

    // The quota pend is durable receipt evidence — B resumed and
    // published only after A freed the slot.
    let receipts_b = scheduler.job_receipts(&k_b).expect("receipts");
    assert!(
        receipts_b.iter().any(|r| matches!(
            r.kind,
            ReceiptKind::Pending {
                reason: JobFailureReason::QuotaExceeded,
                ..
            }
        )),
        "quota pend stays receipt-visible under async"
    );
    assert!(
        receipts_b
            .iter()
            .any(|r| matches!(r.kind, ReceiptKind::Resumed)),
        "gate recheck emits Resumed under async"
    );
}

#[test]
fn async_pol06_cancel_queued_job_is_typed() {
    let project_a = project_r1();
    let project_b = project_r2();
    // Job A carries many demands so it stays non-terminal across
    // many scheduler quanta after the gate releases — the queued
    // cancel always lands before B's resume step.
    let sel_a = RenderSelection {
        snapshot: pinned(&project_a, "head-async-cancel-a"),
        demands: (0..16).map(|_| master_demand(&project_a)).collect(),
    };
    let sel_b = selection(&project_b, "head-async-cancel-b");
    let (executor, entered_rx, release_tx, calls) = gated_executor();
    let scheduler = JobScheduler::spawn(
        scheduler_ports(
            Box::new(executor),
            Box::new(StaticSettings(quality())),
            Box::new(StaticPolicies::default()),
            Box::new(MasterDemands),
        ),
        cpu_grants(),
        8,
    );

    // cpu_only grants allow one concurrent job — B pends queued
    // behind A's parked render.
    let k_a = accept_key(scheduler.submit_intent(intent_for(sel_a)).expect("alive"));

    // B's submit and the cancel run on helper threads: replies can
    // only arrive after the worker drains them, which may be after
    // A's parked render — awaiting on the test thread would
    // deadlock.
    let (b_tx, b_rx) = channel();
    let sched_b = scheduler.clone();
    std::thread::spawn(move || {
        let _ = b_tx.send(sched_b.submit_intent(intent_for(sel_b)));
    });
    let (kb_tx, kb_rx) = channel();
    let (c_tx, c_rx) = channel();
    let sched_c = scheduler.clone();
    std::thread::spawn(move || {
        let kb = kb_rx.recv().expect("job key channel");
        let _ = c_tx.send(sched_c.cancel(&kb));
    });

    entered_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("A render entered");

    // B's key arrives once the worker drains its submit — usually
    // before the park; otherwise right after the first release.
    let mut released = false;
    let k_b = if let Ok(out) = b_rx.recv_timeout(Duration::from_secs(2)) {
        accept_key(out.expect("scheduler alive"))
    } else {
        release_tx.send(()).expect("release render");
        released = true;
        accept_key(
            b_rx.recv_timeout(Duration::from_secs(10))
                .expect("B submit answered")
                .expect("scheduler alive"),
        )
    };
    let _ = kb_tx.send(k_b.clone());

    if !released {
        // Let the cancel helper land its try_send — it must be
        // queued before the worker's next drain.
        std::thread::sleep(Duration::from_millis(100));
        release_tx.send(()).expect("release render");
    }
    // In the fallback path the worker is already draining; A's many
    // demands leave ample iterations for the cancel to land first.

    assert!(
        c_rx.recv_timeout(Duration::from_secs(10))
            .expect("cancel answered")
            .expect("scheduler alive"),
        "cancel of a queued job is accepted"
    );

    let va = scheduler
        .wait_terminal(&k_a, Duration::from_secs(10))
        .expect("A terminal");
    let vb = scheduler
        .wait_terminal(&k_b, Duration::from_secs(10))
        .expect("B terminal");
    assert_eq!(va.state, RenderJobState::Published);
    assert_eq!(vb.state, RenderJobState::Cancelled);
    assert!(vb.artifacts.is_empty(), "cancelled job publishes nothing");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "only A rendered — B was cancelled while queued"
    );
    assert!(
        scheduler
            .job_receipts(&k_b)
            .expect("receipts")
            .iter()
            .any(|r| matches!(r.kind, ReceiptKind::CancelRequested)),
        "cancel stays receipt-visible under async"
    );
}

#[test]
fn async_pol06_retryable_failure_retries_once() {
    let project = project_r1();
    let sel = selection(&project, "head-async-retry");
    let executor = FakeExecutor {
        calls: Arc::new(AtomicUsize::new(0)),
        steps: vec![
            ExecStep::Fail(JobFailureReason::WorkerFailure, "transient"),
            ExecStep::Output,
        ],
        backend: BackendRequirement::Cpu,
    };
    let calls = executor.calls.clone();
    let scheduler = JobScheduler::spawn(
        scheduler_ports(
            Box::new(executor),
            Box::new(StaticSettings(quality())),
            Box::new(StaticPolicies::default()),
            Box::new(MasterDemands),
        ),
        cpu_grants(),
        8,
    );

    let key = accept_key(scheduler.submit_intent(intent_for(sel)).expect("alive"));
    // The retryable failure lands as a typed pending — the worker
    // rests the job until `retry` arrives.
    let pended = wait_job(&scheduler, &key, Duration::from_secs(10), |v| {
        v.pending_reason == Some(JobFailureReason::WorkerFailure)
    })
    .expect("job pends on retryable failure");
    assert_eq!(pended.state, RenderJobState::Queue);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    assert!(scheduler.retry(&key).expect("alive"), "retry accepted");
    let view = scheduler
        .wait_terminal(&key, Duration::from_secs(10))
        .expect("job reaches terminal");
    assert_eq!(view.state, RenderJobState::Published);
    assert_eq!(view.attempt, 2);
    assert_eq!(calls.load(Ordering::SeqCst), 2, "retry renders once more");
    assert!(
        scheduler
            .job_receipts(&key)
            .expect("receipts")
            .iter()
            .any(|r| matches!(r.kind, ReceiptKind::Retrying { attempt: 2 })),
        "retry stays receipt-visible under async"
    );
}

#[test]
fn async_pol06_crash_requires_explicit_resume() {
    let project = project_r1();
    let sel = selection(&project, "head-async-crash");
    let executor = FakeExecutor {
        calls: Arc::new(AtomicUsize::new(0)),
        steps: vec![ExecStep::CrashOnce, ExecStep::Output],
        backend: BackendRequirement::Cpu,
    };
    let calls = executor.calls.clone();
    let scheduler = JobScheduler::spawn(
        scheduler_ports(
            Box::new(executor),
            Box::new(StaticSettings(quality())),
            Box::new(StaticPolicies::default()),
            Box::new(MasterDemands),
        ),
        cpu_grants(),
        8,
    );

    let key = accept_key(scheduler.submit_intent(intent_for(sel)).expect("alive"));
    let crashed =
        wait_job(&scheduler, &key, Duration::from_secs(10), |v| v.crashed).expect("crash observed");
    assert!(!crashed.state.is_terminal());

    // A crashed job must NOT auto-resume — the scheduler leaves it
    // parked until the explicit `resume` command arrives.
    std::thread::sleep(Duration::from_millis(200));
    let still = scheduler.job(&key).expect("alive").expect("job");
    assert!(still.crashed, "crash must wait for explicit resume");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "no silent re-render");

    let outcome = scheduler.resume(&key).expect("alive");
    assert_eq!(
        outcome,
        DriveOutcome::Progress,
        "resume re-enters the quantum loop"
    );
    let view = scheduler
        .wait_terminal(&key, Duration::from_secs(10))
        .expect("job reaches terminal");
    assert_eq!(view.state, RenderJobState::Published);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "crash + resume renders once more"
    );
    assert_eq!(view.artifacts.len(), 1, "exactly one artifact");
    assert!(
        scheduler
            .job_receipts(&key)
            .expect("receipts")
            .iter()
            .any(|r| matches!(r.kind, ReceiptKind::CrashObserved)),
        "crash stays receipt-visible under async"
    );
}

#[test]
fn async_pol06_grant_update_wakes_gate_pending_job() {
    let project = project_r1();
    let sel = selection(&project, "head-async-storage");
    let executor = FakeExecutor::output();
    let calls = executor.calls.clone();
    let tight = LocalGrants {
        storage_quota_bytes: 1,
        ..LocalGrants::cpu_only()
    };
    let scheduler = JobScheduler::spawn(
        scheduler_ports(
            Box::new(executor),
            Box::new(StaticSettings(quality())),
            Box::new(StaticPolicies::default()),
            Box::new(MasterDemands),
        ),
        tight,
        8,
    );

    let mut intent = intent_for(sel);
    intent.estimate.storage_bytes = 4_096;
    let key = accept_key(scheduler.submit_intent(intent).expect("alive"));
    wait_job(&scheduler, &key, Duration::from_secs(10), |v| {
        v.pending_reason == Some(JobFailureReason::InsufficientStorage)
    })
    .expect("job pends on storage quota");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "no work while storage pends"
    );

    // The grant update command wakes the rested job — the gate
    // recheck runs at the next quantum and the job proceeds.
    scheduler
        .set_grants(LocalGrants::cpu_only())
        .expect("alive");
    let view = scheduler
        .wait_terminal(&key, Duration::from_secs(10))
        .expect("job reaches terminal");
    assert_eq!(view.state, RenderJobState::Published);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn async_scheduler_bounded_queue_reports_queue_full() {
    let project = project_r1();
    let sel = selection(&project, "head-async-queue");
    let (executor, entered_rx, release_tx, _calls) = gated_executor();
    // Capacity 1 — the worker is parked inside the render, so the
    // single slot admits exactly one waiting command.
    let scheduler = JobScheduler::spawn(
        scheduler_ports(
            Box::new(executor),
            Box::new(StaticSettings(quality())),
            Box::new(StaticPolicies::default()),
            Box::new(MasterDemands),
        ),
        cpu_grants(),
        1,
    );

    let key = accept_key(scheduler.submit_intent(intent_for(sel)).expect("alive"));
    entered_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("render entered");

    // Two contenders for the one queue slot while the worker is
    // parked: exactly one is admitted (and blocks on its reply),
    // the other gets the typed QueueFull — never a parked caller.
    let (done_tx, done_rx) = channel();
    for _ in 0..2 {
        let s = scheduler.clone();
        let k = key.clone();
        let tx = done_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(s.job(&k).map(|_| ()));
        });
    }
    let first = done_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("one contender answered");
    assert_eq!(
        first.expect_err("the rejected contender must see QueueFull"),
        synvelis_render::render_job::SchedulerError::QueueFull,
        "a full bounded queue fails typed, never blocks"
    );

    release_tx.send(()).expect("release render");
    scheduler
        .wait_terminal(&key, Duration::from_secs(10))
        .expect("job reaches terminal");
    let second = done_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("admitted contender answered after drain");
    assert!(second.is_ok(), "the admitted command completes normally");
}

#[test]
fn async_headless_scheduler_is_fail_closed() {
    let scheduler = JobScheduler::headless();
    let project = project_r1();
    let sel = selection(&project, "head-async-headless");

    // No settings document → typed rejection before any job exists.
    match scheduler
        .submit_intent(intent_for(sel.clone()))
        .expect("alive")
    {
        SubmitOutcome::Rejected { reason, .. } => {
            assert_eq!(reason, JobFailureReason::UnknownDependency);
        }
        other @ SubmitOutcome::Accepted { .. } => {
            panic!("headless submit must reject typed, got {other:?}")
        }
    }
    assert!(scheduler.jobs().expect("alive").is_empty());

    // The trigger path routes to the same typed denial — a render
    // success is never fabricated without a backend.
    match scheduler
        .handle_report(report(manual_event(sel)))
        .expect("alive")
    {
        TriggerOutcome::Denied { .. } => {}
        other => panic!("headless trigger must deny typed, got {other:?}"),
    }
    assert!(scheduler.jobs().expect("alive").is_empty());
    scheduler.shutdown();
}
