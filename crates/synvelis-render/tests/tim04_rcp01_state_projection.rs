//! TIM-04 RCP-01 persisted plugin-state projection fixtures.
//!
//! These tests exercise the persisted-manifest → probe → recipe →
//! artifact-lookup seam through public APIs. They are headless contract
//! evidence only: they are not real-plugin, real-ABI, offline-render,
//! GUI, RT, or final OS acceptance evidence.

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use serde_json::{Value, json};
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::id::{CounterIdGenerator, ProjectId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::plugin_state::{
    InMemoryPluginStateStore, PLUGIN_STATE_SCHEMA_VERSION, PluginStateFailureKind,
    PluginStateManifest, PluginStateOutcome, PluginStateRecord, PluginStateStore,
    PluginStateStoreError,
};
use synvelis_project::routing::{PluginRef, ProcessorBinding, StripOwner};
use synvelis_project::snapshot::ContentDigest;
use synvelis_project::store::SCHEMA_VERSION;
use synvelis_project::{
    ProcessorId, Project, RENDER_INPUT_SCHEMA_VERSION, StateCaptureQuality, StripId,
    render_input::resolve_render_project,
};
use synvelis_render::artifact::lookup::{
    CacheLookup, IntendedUse, LookupOutcome, TrustScope, lookup,
};
use synvelis_render::artifact::manifest::{
    ArtifactPortSpec, ArtifactReproducibility, CodecClass, CodecDescriptor, RenderArtifactId,
};
use synvelis_render::artifact::publish::{
    ArtifactDraft, PublishExpectation, PublishOutcome, publish_artifact,
};
use synvelis_render::artifact::store::{ArtifactReference, InMemoryArtifactStore};
use synvelis_render::recipe::{
    CANONICALIZATION_VERSION, ClockDependency, DenormalMode, FrameInterval, OutputContract,
    OutputPortContract, PcmFormat, RECIPE_SCHEMA_VERSION, RecipeKey, RecipeResolution,
    RenderBoundary, RenderPrecision, RenderRecipeV1, RendererProfile, ResolutionProvenance,
    TemporalDomain, UnknownCause,
};
use synvelis_render::{
    NoExternalAssets, PersistedPluginStateProjection, PluginStateProbe, PluginStateProjectionError,
    PluginStateProjectionSource, RecipeEnvironment, RecipeRequest, resolve_recipe,
};

const RATE: u32 = 48_000;
const FRAMES: u64 = 48_000;
const PROJECT_ID: u128 = 0x128;
const EVIDENCE_ENV: &str = "SYNVELIS_TIM04_RCP01_EVIDENCE_DIR";
const EVIDENCE_FILE: &str = "tim04_rcp01_state_projection.jsonl";

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(PROJECT_ID), "tim04-rcp01"),
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

fn base_fx() -> (ProjectController, StripId, ProcessorId) {
    let mut c = controller();
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(&mut c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c
        .live()
        .routing
        .strips
        .values()
        .find(|s| s.owner == StripOwner::Track(track))
        .unwrap()
        .id;
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
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
    let processor = *c.live().routing.processors.keys().next_back().unwrap();
    (c, strip, processor)
}

fn temporal() -> TemporalDomain {
    TemporalDomain {
        sample_rate_hz: RATE,
        channels: 2,
        format: PcmFormat::F32,
        interval: FrameInterval::new(0, FRAMES).unwrap(),
        preroll_frames: 0,
        warmup_frames: 0,
        tail_frames: 0,
        lookahead_frames: 0,
        pdc_frames: 0,
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

fn output_for(boundary: &RenderBoundary) -> OutputContract {
    OutputContract {
        ports: boundary
            .ports
            .iter()
            .map(|p| OutputPortContract {
                port: *p,
                channels: 2,
                format: PcmFormat::F32,
            })
            .collect(),
    }
}

fn resolve_with_probe(project: &Project, plugins: &dyn PluginStateProbe) -> RecipeResolution {
    let resolved = resolve_render_project(project, None).unwrap();
    let boundary = RenderBoundary::master(&resolved.project).expect("master boundary");
    let assets = NoExternalAssets;
    let env = RecipeEnvironment {
        plugin_states: plugins,
        external_assets: &assets,
    };
    let output = output_for(&boundary);
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

fn record(processor: ProcessorId, outcome: PluginStateOutcome) -> PluginStateRecord {
    PluginStateRecord {
        processor_id: processor,
        binding: None,
        project_revision: 1,
        stored_at: "test".into(),
        outcome,
        detail: None,
    }
}

fn manifest_bytes(project: &Project, entries: Vec<PluginStateRecord>) -> Vec<u8> {
    serde_json::to_vec(&PluginStateManifest {
        schema_version: PLUGIN_STATE_SCHEMA_VERSION,
        project_id: project.project_id,
        project_revision: 1,
        project_head: None,
        entries,
    })
    .unwrap()
}

fn publish_manifest(
    entries: Vec<PluginStateRecord>,
    blobs: &[(ContentDigest, Vec<u8>)],
    project: &Project,
) -> (InMemoryPluginStateStore, String, Vec<u8>) {
    let mut store = InMemoryPluginStateStore::new();
    let bytes = manifest_bytes(project, entries);
    for (digest, blob) in blobs {
        store.put_blob(digest, blob).unwrap();
    }
    let name = store.publish_manifest(&bytes).unwrap();
    store.update_head(&name).unwrap();
    (store, name, bytes)
}

fn captured(
    quality: StateCaptureQuality,
    blob: &[u8],
) -> (PluginStateOutcome, ContentDigest, Vec<u8>) {
    let digest = ContentDigest::sha256(blob);
    (
        PluginStateOutcome::Captured {
            quality,
            blob_digest: digest,
            blob_len: blob.len() as u64,
        },
        digest,
        blob.to_vec(),
    )
}

fn captured_store(
    project: &Project,
    processor: ProcessorId,
    quality: StateCaptureQuality,
    blob: &[u8],
) -> (InMemoryPluginStateStore, String) {
    let (outcome, digest, bytes) = captured(quality, blob);
    let (store, name, _) = publish_manifest(
        vec![record(processor, outcome)],
        &[(digest, bytes)],
        project,
    );
    (store, name)
}

fn unknown_causes(resolution: &RecipeResolution) -> Vec<UnknownCause> {
    match resolution {
        RecipeResolution::Unknown { dependencies } => dependencies
            .iter()
            .map(|dependency| dependency.cause.clone())
            .collect(),
        other => panic!("expected Unknown, got {other:?}"),
    }
}

fn assert_send<T: Send>() {}

struct FaultStore {
    inner: InMemoryPluginStateStore,
    head_error: Option<PluginStateStoreError>,
    manifest_error: Option<PluginStateStoreError>,
    manifest_override: Option<Vec<u8>>,
    blob_error: Option<PluginStateStoreError>,
}

impl FaultStore {
    fn new(inner: InMemoryPluginStateStore) -> Self {
        Self {
            inner,
            head_error: None,
            manifest_error: None,
            manifest_override: None,
            blob_error: None,
        }
    }
}

impl PluginStateStore for FaultStore {
    fn publish_manifest(&mut self, bytes: &[u8]) -> Result<String, PluginStateStoreError> {
        self.inner.publish_manifest(bytes)
    }

    fn update_head(&mut self, name: &str) -> Result<(), PluginStateStoreError> {
        self.inner.update_head(name)
    }

    fn head(&self) -> Result<Option<String>, PluginStateStoreError> {
        if let Some(error) = &self.head_error {
            return Err(error.clone());
        }
        self.inner.head()
    }

    fn read_manifest(&self, name: &str) -> Result<Vec<u8>, PluginStateStoreError> {
        if let Some(error) = &self.manifest_error {
            return Err(error.clone());
        }
        if let Some(bytes) = &self.manifest_override {
            return Ok(bytes.clone());
        }
        self.inner.read_manifest(name)
    }

    fn put_blob(
        &mut self,
        digest: &ContentDigest,
        bytes: &[u8],
    ) -> Result<(), PluginStateStoreError> {
        self.inner.put_blob(digest, bytes)
    }

    fn read_blob(&self, digest: &ContentDigest) -> Result<Vec<u8>, PluginStateStoreError> {
        if let Some(error) = &self.blob_error {
            return Err(error.clone());
        }
        self.inner.read_blob(digest)
    }
}

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
    for frame in 0..frames {
        for channel in 0..2usize {
            out.extend_from_slice(&sample(frame, channel).to_le_bytes());
        }
    }
    out
}

fn artifact_blob(frames: u64) -> Vec<u8> {
    wav_i16(usize::try_from(frames).unwrap(), |frame, channel| {
        (((i32::try_from(frame).unwrap() + i32::try_from(channel).unwrap() * 7) % 97) as i16) * 100
    })
}

fn expectation(frames: u64) -> PublishExpectation {
    PublishExpectation {
        frame_count: frames,
        sample_rate_hz: RATE,
        channels: 2,
        codec: "pcm".to_owned(),
        container: "wav".to_owned(),
    }
}

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
        provenance: synvelis_render::artifact::manifest::ArtifactProvenance {
            produced_by: "test/tim04-rcp01-state-projection".to_owned(),
            job_id: None,
        },
        trust: None,
    }
}

fn publish_for(
    store: &mut InMemoryArtifactStore,
    key: RecipeKey,
    recipe: &RenderRecipeV1,
) -> RenderArtifactId {
    let draft = draft_for(key, recipe, FRAMES);
    match publish_artifact(
        store,
        &draft,
        &artifact_blob(FRAMES),
        &expectation(FRAMES),
        None,
        None,
        None::<ArtifactReference>,
    ) {
        PublishOutcome::Published { artifact, .. } => artifact,
        other => panic!("expected Published, got {other:?}"),
    }
}

fn query(key: RecipeKey, recipe: &RenderRecipeV1) -> CacheLookup {
    CacheLookup {
        recipe_key: key,
        intended_use: IntendedUse::Exact,
        requested_span: FrameInterval::new(0, FRAMES).unwrap(),
        required_ports: recipe.output_contract.ports.clone(),
        trust_scope: TrustScope::Local,
        compatibility_profile: ContentDigest::sha256(b"compat-v1"),
    }
}

fn ruviform_rev() -> String {
    let lock = include_str!("../../../Cargo.lock");
    let needle = "https://github.com/scarecrow173/ruviform.git#";
    lock.find(needle).map_or_else(
        || "unknown".to_owned(),
        |start| lock[start + needle.len()..start + needle.len() + 40].to_owned(),
    )
}

fn evidence_environment(input_source: &str, store_backend: &str) -> Value {
    json!({
        "os": env::consts::OS,
        "arch": env::consts::ARCH,
        "ruviform_resolved_rev": ruviform_rev(),
        "store_backend": store_backend,
        "projection_input_source": input_source,
        "plugin_state_schema_version": PLUGIN_STATE_SCHEMA_VERSION,
    })
}

fn write_evidence_row(
    file: &mut Option<File>,
    case: &str,
    verdict: &str,
    detail: &str,
    observed: &Value,
    input_source: &str,
    store_backend: &str,
) {
    let Some(file) = file else { return };
    let row = json!({
        "case": case,
        "driver": "crates/synvelis-render/tests/tim04_rcp01_state_projection.rs",
        "verdict": verdict,
        "detail": detail,
        "observed": observed,
        "environment": evidence_environment(input_source, store_backend),
    });
    serde_json::to_writer(&mut *file, &row).unwrap();
    file.write_all(b"\n").unwrap();
}

fn evidence_file() -> Option<File> {
    let dir = env::var_os(EVIDENCE_ENV)?;
    fs::create_dir_all(&dir).unwrap();
    Some(File::create(PathBuf::from(dir).join(EVIDENCE_FILE)).unwrap())
}

#[test]
fn exact_captured_projection_resolves_and_preserves_evidence() {
    assert_send::<PersistedPluginStateProjection>();
    let (c, _, processor) = base_fx();
    for quality in [
        StateCaptureQuality::Coordinated,
        StateCaptureQuality::Quiescent,
    ] {
        let blob = format!("state-{quality:?}").into_bytes();
        let digest = ContentDigest::sha256(&blob);
        let (store, name) = captured_store(c.live(), processor, quality, &blob);
        let projection = PersistedPluginStateProjection::from_store_head(&store).unwrap();
        assert_eq!(projection.source(), PluginStateProjectionSource::StoreHead);
        assert_eq!(projection.manifest_name(), Some(name.as_str()));
        assert_eq!(projection.project_id(), c.live().project_id);
        assert_eq!(projection.record_count(), 1);

        let resolution = resolve_with_probe(c.live(), &projection);
        let RecipeResolution::Resolved { recipe, .. } = &resolution else {
            panic!("expected Resolved, got {resolution:?}");
        };
        let evidence = recipe.closure.plugin_states.get(&processor).unwrap();
        assert_eq!(evidence.digest, digest);
        assert_eq!(evidence.quality, quality);

        let explicit = PersistedPluginStateProjection::from_store_manifest(&store, &name).unwrap();
        assert_eq!(
            explicit.source(),
            PluginStateProjectionSource::ExplicitManifest
        );
    }
}

#[test]
fn degraded_unsupported_failed_and_missing_evidence_are_typed_unknown() {
    let (c, _, processor) = base_fx();

    let (store, _) = captured_store(
        c.live(),
        processor,
        StateCaptureQuality::BestEffort,
        b"best-effort-state",
    );
    let projection = PersistedPluginStateProjection::from_store_head(&store).unwrap();
    let causes = unknown_causes(&resolve_with_probe(c.live(), &projection));
    assert!(
        causes
            .iter()
            .any(|cause| matches!(cause, UnknownCause::PluginStateDegraded { quality, .. } if quality == "BestEffort")),
        "missing BestEffort degraded cause: {causes:?}"
    );

    let unsupported_bytes = manifest_bytes(
        c.live(),
        vec![record(processor, PluginStateOutcome::Unsupported)],
    );
    let mut unsupported_store = InMemoryPluginStateStore::new();
    let unsupported_name = unsupported_store
        .publish_manifest(&unsupported_bytes)
        .unwrap();
    unsupported_store.update_head(&unsupported_name).unwrap();
    let projection = PersistedPluginStateProjection::from_store_head(&unsupported_store).unwrap();
    let causes = unknown_causes(&resolve_with_probe(c.live(), &projection));
    assert!(
        causes.iter().any(|cause| matches!(
            cause,
            UnknownCause::PluginStateDegraded { quality, failure_kind, .. }
                if quality == "Unsupported" && failure_kind.is_none()
        )),
        "missing unsupported cause: {causes:?}"
    );

    for (kind, snake) in [
        (PluginStateFailureKind::CaptureFailed, "capture_failed"),
        (PluginStateFailureKind::Timeout, "timeout"),
        (
            PluginStateFailureKind::PermissionDenied,
            "permission_denied",
        ),
        (PluginStateFailureKind::Crashed, "crashed"),
        (PluginStateFailureKind::Corrupt, "corrupt"),
        (PluginStateFailureKind::Unavailable, "unavailable"),
        (PluginStateFailureKind::Missing, "missing"),
    ] {
        let bytes = manifest_bytes(
            c.live(),
            vec![record(processor, PluginStateOutcome::Failed { kind })],
        );
        let mut store = InMemoryPluginStateStore::new();
        let name = store.publish_manifest(&bytes).unwrap();
        store.update_head(&name).unwrap();
        let projection = PersistedPluginStateProjection::from_store_head(&store).unwrap();
        let causes = unknown_causes(&resolve_with_probe(c.live(), &projection));
        assert!(
            causes.iter().any(|cause| matches!(
                cause,
                UnknownCause::PluginStateDegraded { quality, failure_kind, .. }
                    if quality == "Failed" && failure_kind.as_deref() == Some(snake)
            )),
            "missing failed kind {snake}: {causes:?}"
        );
    }

    let empty_bytes = manifest_bytes(c.live(), Vec::new());
    let mut empty_store = InMemoryPluginStateStore::new();
    let empty_name = empty_store.publish_manifest(&empty_bytes).unwrap();
    empty_store.update_head(&empty_name).unwrap();
    let projection = PersistedPluginStateProjection::from_store_head(&empty_store).unwrap();
    let causes = unknown_causes(&resolve_with_probe(c.live(), &projection));
    assert!(
        causes
            .iter()
            .any(|cause| matches!(cause, UnknownCause::PluginStateUnknown { .. })),
        "missing evidence did not become PluginStateUnknown: {causes:?}"
    );
}

#[test]
fn projection_manifest_integrity_failures_are_fail_closed() {
    let (c, _, processor) = base_fx();
    let empty = InMemoryPluginStateStore::new();
    assert!(matches!(
        PersistedPluginStateProjection::from_store_head(&empty),
        Err(PluginStateProjectionError::NoHeadManifest)
    ));
    assert!(matches!(
        PersistedPluginStateProjection::from_store_manifest(&empty, "not-a-manifest"),
        Err(PluginStateProjectionError::InvalidManifestName { .. })
    ));

    let (store, _) = captured_store(
        c.live(),
        processor,
        StateCaptureQuality::Quiescent,
        b"state",
    );
    let mut head_fault = FaultStore::new(store);
    head_fault.head_error = Some(PluginStateStoreError::Failed("head read fault".into()));
    assert!(matches!(
        PersistedPluginStateProjection::from_store_head(&head_fault),
        Err(PluginStateProjectionError::HeadRead { .. })
    ));

    let (store, _) = captured_store(
        c.live(),
        processor,
        StateCaptureQuality::Quiescent,
        b"state",
    );
    let mut manifest_fault = FaultStore::new(store);
    manifest_fault.manifest_error = Some(PluginStateStoreError::PermissionDenied {
        operation: synvelis_project::plugin_state::PluginStateStoreOperation::ManifestRead,
        effect: synvelis_project::plugin_state::PluginStateStorePublishEffect::NotApplicable,
        detail: "denied".into(),
    });
    let name = manifest_fault.inner.head().unwrap().unwrap();
    assert!(matches!(
        PersistedPluginStateProjection::from_store_manifest(&manifest_fault, &name),
        Err(PluginStateProjectionError::ManifestRead { .. })
    ));

    assert!(matches!(
        PersistedPluginStateProjection::from_manifest_bytes(b"{", &empty),
        Err(PluginStateProjectionError::MalformedManifest { .. })
    ));

    let mut wrong_schema = manifest_bytes(c.live(), Vec::new());
    wrong_schema = String::from_utf8(wrong_schema)
        .unwrap()
        .replace("\"schema_version\":1", "\"schema_version\":999")
        .into_bytes();
    assert!(matches!(
        PersistedPluginStateProjection::from_manifest_bytes(&wrong_schema, &empty),
        Err(PluginStateProjectionError::UnsupportedSchema { found: 999, .. })
    ));

    let duplicate = vec![
        record(processor, PluginStateOutcome::Unsupported),
        record(processor, PluginStateOutcome::Unsupported),
    ];
    let duplicate_bytes = manifest_bytes(c.live(), duplicate);
    assert!(matches!(
        PersistedPluginStateProjection::from_manifest_bytes(&duplicate_bytes, &empty),
        Err(PluginStateProjectionError::DuplicateProcessor { .. })
    ));
}

#[test]
fn projection_blob_integrity_failures_are_fail_closed() {
    let (c, _, processor) = base_fx();
    let empty = InMemoryPluginStateStore::new();

    let (outcome, _, _) = captured(StateCaptureQuality::Quiescent, b"missing-state");
    let missing_blob_bytes = manifest_bytes(c.live(), vec![record(processor, outcome)]);
    assert!(matches!(
        PersistedPluginStateProjection::from_manifest_bytes(&missing_blob_bytes, &empty),
        Err(PluginStateProjectionError::MissingBlob { .. })
    ));

    let (outcome, digest, _) = captured(StateCaptureQuality::Quiescent, b"expected");
    let mut corrupt_store = InMemoryPluginStateStore::new();
    corrupt_store.put_blob(&digest, b"different").unwrap();
    let corrupt_bytes = manifest_bytes(c.live(), vec![record(processor, outcome)]);
    assert!(matches!(
        PersistedPluginStateProjection::from_manifest_bytes(&corrupt_bytes, &corrupt_store),
        Err(PluginStateProjectionError::CorruptBlob { .. })
    ));

    let (store, _) = captured_store(
        c.live(),
        processor,
        StateCaptureQuality::Quiescent,
        b"state",
    );
    let mut blob_fault = FaultStore::new(store);
    blob_fault.blob_error = Some(PluginStateStoreError::PermissionDenied {
        operation: synvelis_project::plugin_state::PluginStateStoreOperation::BlobRead,
        effect: synvelis_project::plugin_state::PluginStateStorePublishEffect::NotApplicable,
        detail: "denied".into(),
    });
    assert!(matches!(
        PersistedPluginStateProjection::from_store_head(&blob_fault),
        Err(PluginStateProjectionError::BlobRead { .. })
    ));

    let (store, name) = captured_store(
        c.live(),
        processor,
        StateCaptureQuality::Quiescent,
        b"state",
    );
    let mut mismatched = FaultStore::new(store);
    mismatched.manifest_override = Some(manifest_bytes(c.live(), Vec::new()));
    assert!(matches!(
        PersistedPluginStateProjection::from_store_manifest(&mismatched, &name),
        Err(PluginStateProjectionError::ManifestDigestMismatch { .. })
    ));
}

#[test]
fn non_resolved_resolution_never_promotes_verified_artifact() {
    let (c, _, processor) = base_fx();
    let (exact_store, _) = captured_store(
        c.live(),
        processor,
        StateCaptureQuality::Quiescent,
        b"state",
    );
    let exact_projection = PersistedPluginStateProjection::from_store_head(&exact_store).unwrap();
    let exact_resolution = resolve_with_probe(c.live(), &exact_projection);
    let RecipeResolution::Resolved { recipe, key, .. } = &exact_resolution else {
        panic!("expected exact Resolved, got {exact_resolution:?}");
    };

    let mut artifacts = InMemoryArtifactStore::new();
    let artifact = publish_for(&mut artifacts, *key, recipe);
    let lookup_query = query(*key, recipe);
    let exact_lookup = lookup(&artifacts, &lookup_query, &exact_resolution).unwrap();
    assert!(matches!(
        exact_lookup.outcome,
        LookupOutcome::EligibleExact { artifact: id } if id == artifact
    ));

    let (degraded_store, _) = captured_store(
        c.live(),
        processor,
        StateCaptureQuality::BestEffort,
        b"state",
    );
    let degraded_projection =
        PersistedPluginStateProjection::from_store_head(&degraded_store).unwrap();
    let degraded_resolution = resolve_with_probe(c.live(), &degraded_projection);
    assert!(matches!(
        degraded_resolution,
        RecipeResolution::Unknown { .. }
    ));
    let unknown_lookup = lookup(&artifacts, &lookup_query, &degraded_resolution).unwrap();
    assert!(matches!(
        unknown_lookup.outcome,
        LookupOutcome::Unknown { .. }
    ));
    assert!(unknown_lookup.candidates.is_empty());
    assert!(!matches!(
        unknown_lookup.outcome,
        LookupOutcome::EligibleExact { .. } | LookupOutcome::EligiblePreview { .. }
    ));
}

#[test]
fn exact_evidence_shape_and_recipe_versions_are_unchanged() {
    assert_eq!(RECIPE_SCHEMA_VERSION, 1);
    assert_eq!(CANONICALIZATION_VERSION, 1);
    let evidence = synvelis_render::PluginStateEvidence {
        digest: ContentDigest::sha256(b"state"),
        quality: StateCaptureQuality::Quiescent,
    };
    let value = serde_json::to_value(evidence).unwrap();
    assert_eq!(
        value,
        json!({
            "digest": ContentDigest::sha256(b"state").to_string(),
            "quality": "quiescent",
        })
    );
}

fn matrix_non_claims() -> Value {
    json!([
        "not-real-plugin-offline-render",
        "not-real-abi-capability-evidence",
        "not-gui-or-final-os-acceptance",
    ])
}

fn resolution_label(resolution: &RecipeResolution) -> &'static str {
    match resolution {
        RecipeResolution::Resolved { .. } => "resolved",
        RecipeResolution::Unknown { .. } => "unknown",
        RecipeResolution::Unsupported { .. } => "unsupported",
    }
}

fn write_captured_matrix_rows(
    file: &mut Option<File>,
    project: &Project,
    processor: ProcessorId,
    non_claims: &Value,
) {
    for quality in [
        StateCaptureQuality::Coordinated,
        StateCaptureQuality::Quiescent,
    ] {
        let (store, _) = captured_store(project, processor, quality, b"matrix-state");
        let projection = PersistedPluginStateProjection::from_store_head(&store).unwrap();
        let resolution = resolve_with_probe(project, &projection);
        assert!(matches!(resolution, RecipeResolution::Resolved { .. }));
        write_evidence_row(
            file,
            &format!("captured-{quality:?}"),
            "Pass",
            "exact-eligible captured state projects to Resolved",
            &json!({
                "processor_outcome": "captured",
                "quality": format!("{quality:?}"),
                "digest": ContentDigest::sha256(b"matrix-state").to_string(),
                "resolution_verdict": resolution_label(&resolution),
                "lookup_verdict": "not-applicable",
                "non_claims": (*non_claims).clone(),
            }),
            "store-head",
            "in-memory-plugin-state-store",
        );
    }

    let quality = StateCaptureQuality::BestEffort;
    let (store, _) = captured_store(project, processor, quality, b"matrix-state");
    let projection = PersistedPluginStateProjection::from_store_head(&store).unwrap();
    let resolution = resolve_with_probe(project, &projection);
    assert!(matches!(resolution, RecipeResolution::Unknown { .. }));
    write_evidence_row(
        file,
        "captured-BestEffort",
        "Pass",
        "BestEffort remains exact-ineligible typed Unknown",
        &json!({
            "processor_outcome": "captured",
            "quality": "BestEffort",
            "digest": ContentDigest::sha256(b"matrix-state").to_string(),
            "resolution_verdict": resolution_label(&resolution),
            "lookup_verdict": "not-applicable",
            "non_claims": (*non_claims).clone(),
        }),
        "store-head",
        "in-memory-plugin-state-store",
    );
}

fn write_blob_less_matrix_rows(
    file: &mut Option<File>,
    project: &Project,
    processor: ProcessorId,
    non_claims: &Value,
) {
    for (case, outcome) in [
        ("unsupported", PluginStateOutcome::Unsupported),
        (
            "failed-timeout",
            PluginStateOutcome::Failed {
                kind: PluginStateFailureKind::Timeout,
            },
        ),
    ] {
        let bytes = manifest_bytes(project, vec![record(processor, outcome)]);
        let projection = PersistedPluginStateProjection::from_manifest_bytes(
            &bytes,
            &InMemoryPluginStateStore::new(),
        )
        .unwrap();
        let resolution = resolve_with_probe(project, &projection);
        assert!(matches!(resolution, RecipeResolution::Unknown { .. }));
        write_evidence_row(
            file,
            case,
            "Pass",
            "blob-less outcome maps to typed Unknown without fake digest",
            &json!({
                "processor_outcome": case,
                "digest": Value::Null,
                "resolution_verdict": resolution_label(&resolution),
                "lookup_verdict": "not-applicable",
                "non_claims": (*non_claims).clone(),
            }),
            "manifest-bytes",
            "in-memory-plugin-state-store",
        );
    }
}

fn write_missing_matrix_rows(
    file: &mut Option<File>,
    project: &Project,
    processor: ProcessorId,
    non_claims: &Value,
) {
    let empty_bytes = manifest_bytes(project, Vec::new());
    let projection = PersistedPluginStateProjection::from_manifest_bytes(
        &empty_bytes,
        &InMemoryPluginStateStore::new(),
    )
    .unwrap();
    let missing_resolution = resolve_with_probe(project, &projection);
    assert!(matches!(
        missing_resolution,
        RecipeResolution::Unknown { .. }
    ));
    write_evidence_row(
        file,
        "missing-evidence",
        "Pass",
        "valid manifest without processor record resolves as PluginStateUnknown",
        &json!({
            "processor_outcome": "absent",
            "digest": Value::Null,
            "resolution_verdict": resolution_label(&missing_resolution),
            "lookup_verdict": "not-applicable",
            "non_claims": (*non_claims).clone(),
        }),
        "manifest-bytes",
        "in-memory-plugin-state-store",
    );

    let (outcome, _, _) = captured(StateCaptureQuality::Quiescent, b"missing-state");
    let missing_blob_bytes = manifest_bytes(project, vec![record(processor, outcome)]);
    let Err(missing_blob_error) = PersistedPluginStateProjection::from_manifest_bytes(
        &missing_blob_bytes,
        &InMemoryPluginStateStore::new(),
    ) else {
        panic!("missing blob unexpectedly projected")
    };
    write_evidence_row(
        file,
        "missing-blob",
        "Pass",
        "missing captured blob is a projection error, not exact evidence",
        &json!({
            "processor_outcome": "captured",
            "digest": ContentDigest::sha256(b"missing-state").to_string(),
            "projection_error": missing_blob_error.to_string(),
            "resolution_verdict": "not-run",
            "lookup_verdict": "not-run",
            "non_claims": (*non_claims).clone(),
        }),
        "manifest-bytes",
        "in-memory-plugin-state-store",
    );
}

fn write_lookup_matrix_row(
    file: &mut Option<File>,
    project: &Project,
    processor: ProcessorId,
    non_claims: &Value,
) {
    let (exact_store, _) = captured_store(
        project,
        processor,
        StateCaptureQuality::Quiescent,
        b"lookup-state",
    );
    let exact_projection = PersistedPluginStateProjection::from_store_head(&exact_store).unwrap();
    let exact_resolution = resolve_with_probe(project, &exact_projection);
    let RecipeResolution::Resolved { recipe, key, .. } = &exact_resolution else {
        panic!("expected exact Resolved, got {exact_resolution:?}")
    };
    let mut artifacts = InMemoryArtifactStore::new();
    publish_for(&mut artifacts, *key, recipe);
    let lookup_query = query(*key, recipe);
    let exact_lookup = lookup(&artifacts, &lookup_query, &exact_resolution).unwrap();
    assert!(matches!(
        exact_lookup.outcome,
        LookupOutcome::EligibleExact { .. }
    ));

    let (degraded_store, _) = captured_store(
        project,
        processor,
        StateCaptureQuality::BestEffort,
        b"lookup-state",
    );
    let degraded_projection =
        PersistedPluginStateProjection::from_store_head(&degraded_store).unwrap();
    let degraded_resolution = resolve_with_probe(project, &degraded_projection);
    let degraded_lookup = lookup(&artifacts, &lookup_query, &degraded_resolution).unwrap();
    assert!(matches!(
        degraded_lookup.outcome,
        LookupOutcome::Unknown { .. }
    ));
    assert!(degraded_lookup.candidates.is_empty());
    write_evidence_row(
        file,
        "artifact-lookup-non-resolved",
        "Pass",
        "verified exact candidate is not promoted when current resolution is Unknown",
        &json!({
            "processor_outcome": "captured",
            "quality": "BestEffort",
            "digest": ContentDigest::sha256(b"lookup-state").to_string(),
            "resolution_verdict": resolution_label(&degraded_resolution),
            "lookup_verdict": "unknown",
            "control_lookup_verdict": "eligible-exact-with-resolved-control",
            "non_claims": (*non_claims).clone(),
        }),
        "store-head",
        "in-memory-plugin-state-store",
    );
}

#[test]
fn projection_matrix_evidence_rows() {
    let mut file = evidence_file();
    let (c, _, processor) = base_fx();
    let non_claims = matrix_non_claims();
    write_captured_matrix_rows(&mut file, c.live(), processor, &non_claims);
    write_blob_less_matrix_rows(&mut file, c.live(), processor, &non_claims);
    write_missing_matrix_rows(&mut file, c.live(), processor, &non_claims);
    write_lookup_matrix_row(&mut file, c.live(), processor, &non_claims);
    write_evidence_row(
        &mut file,
        "real-plugin-offline-render",
        "NotRun",
        "outside this slice because OfflineRenderSupport::Verified capability is unavailable",
        &json!({
            "processor_outcome": "not-run",
            "digest": Value::Null,
            "resolution_verdict": "not-run",
            "lookup_verdict": "not-run",
            "non_claims": ["no-real-plugin-offline-render-pass-claimed"],
        }),
        "not-applicable",
        "not-applicable",
    );
}
