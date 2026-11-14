//! RCP-02 headless contract tests — immutable artifact manifest,
//! verified publish, CAS references/GC and shared verification
//! (synvelis#291; `docs/test-specs/rcp-02-artifact-manifest.md`).
//!
//! All tests run against [`InMemoryArtifactStore`] — the injectable,
//! filesystem-free store contract. Blob fixtures are hand-built
//! 16-bit PCM WAV bytes decoded through the real symphonia/
//! `SourceReader` path, so publish verification exercises the same
//! independent reader contract as file export readback. Real
//! OS/filesystem store behavior is a later gate (`NotRun` here).

use std::collections::BTreeMap;

use ruviform_exec::CancellationToken;
use synvelis_player::rcp::model::ArtifactEligibility;
use synvelis_project::snapshot::ContentDigest;
use synvelis_project::{BranchId, ProjectId, SemanticPortId, VariantSetId};
use synvelis_render::DenormalMode;
use synvelis_render::DependencyClosure;
use synvelis_render::artifact::FitRejection;
use synvelis_render::artifact::gc::{GcPolicy, sweep};
use synvelis_render::artifact::lookup::{
    CacheLookup, IntendedUse, LookupOutcome, TrustScope, lookup,
};
use synvelis_render::artifact::manifest::{
    AccessClass, ArtifactManifest, ArtifactPortSpec, ArtifactProvenance, ArtifactReproducibility,
    ArtifactTrust, CodecClass, CodecDescriptor, RenderArtifactId, VerificationReceipt,
    verify_manifest,
};
use synvelis_render::artifact::publish::{
    ArtifactDraft, PublishExpectation, PublishOutcome, PublishStage, publish_artifact,
};
use synvelis_render::artifact::share::{
    ExternalManifestRef, RemoteArtifact, ShareRegistration, ShareRejection,
    attach_external_manifest, register_remote,
};
use synvelis_render::artifact::store::{
    ArtifactReference, ArtifactStore, FaultPoint, InMemoryArtifactStore, PublishCommit, RefState,
    StoreError,
};
use synvelis_render::recipe::{
    AUDIO_SEMANTICS_VERSION, CANONICALIZATION_VERSION, ClockDependency, FrameInterval,
    OutputContract, OutputPortContract, PcmFormat, RECIPE_DOMAIN_TAG, RECIPE_HASH_ALGORITHM,
    RECIPE_SCHEMA_VERSION, RecipeKey, RecipeResolution, RecipeTarget, RenderBoundary,
    RenderPrecision, RenderRecipeV1, RendererProfile, ResolutionProvenance, TemporalDomain,
};

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

const RATE: u32 = 48_000;
const FRAMES: u64 = 512;
const CHANNELS: u32 = 2;

fn port() -> SemanticPortId {
    SemanticPortId::new(0xA11)
}

fn recipe_key(seed: u8) -> RecipeKey {
    RecipeKey(ContentDigest::sha256(&[seed]))
}

fn temporal(frames: u64) -> TemporalDomain {
    TemporalDomain {
        sample_rate_hz: RATE,
        channels: CHANNELS,
        format: PcmFormat::F32,
        interval: FrameInterval::new(0, frames).unwrap(),
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

fn target() -> RecipeTarget {
    RecipeTarget {
        boundary: RenderBoundary::new(vec![port()]),
        project_id: ProjectId::new(0x291),
        project_schema_version: 1,
        render_input_schema_version: 1,
        branch: None,
    }
}

fn renderer(seed: u8) -> RendererProfile {
    RendererProfile {
        renderer_id: "test-renderer".to_owned(),
        renderer_version: "0".to_owned(),
        compatibility_profile: ContentDigest::sha256(&[seed]),
        precision: RenderPrecision::F32,
        backend: "test-backend".to_owned(),
        seed: 0,
        denormal_mode: DenormalMode::Preserve,
    }
}

fn draft(key: RecipeKey, codec_class: CodecClass, codec: &str) -> ArtifactDraft {
    ArtifactDraft {
        recipe_key: key,
        target: target(),
        snapshot: None,
        temporal: temporal(FRAMES),
        cut_frames: 0,
        latency_frames: 0,
        ports: vec![ArtifactPortSpec {
            port: port(),
            channels: CHANNELS,
            channel_offset: 0,
            format: PcmFormat::F32,
        }],
        codec: CodecDescriptor {
            container: "wav".to_owned(),
            codec: codec.to_owned(),
            class: codec_class,
        },
        renderer: renderer(0x51),
        reproducibility: ArtifactReproducibility::BitExactCertified,
        provenance: ArtifactProvenance {
            produced_by: "test/publish".to_owned(),
            job_id: None,
        },
        trust: None,
    }
}

fn expect(frames: u64) -> PublishExpectation {
    PublishExpectation {
        frame_count: frames,
        sample_rate_hz: RATE,
        channels: CHANNELS,
        codec: "pcm".to_owned(),
        container: "wav".to_owned(),
    }
}

/// Minimal `Resolved` resolution — lookup only reads `key`.
fn resolved(key: RecipeKey) -> RecipeResolution {
    RecipeResolution::Resolved {
        recipe: Box::new(RenderRecipeV1 {
            schema_version: RECIPE_SCHEMA_VERSION,
            canonicalization_version: CANONICALIZATION_VERSION,
            audio_semantics_version: AUDIO_SEMANTICS_VERSION,
            hash_algorithm: RECIPE_HASH_ALGORITHM.to_owned(),
            domain_separator: RECIPE_DOMAIN_TAG.to_owned(),
            target: target(),
            inputs_digest: ContentDigest::sha256(b"inputs"),
            processor_digest: ContentDigest::sha256(b"processors"),
            temporal_digest: ContentDigest::sha256(b"temporal"),
            temporal: temporal(FRAMES),
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

fn query(key: RecipeKey) -> CacheLookup {
    CacheLookup {
        recipe_key: key,
        intended_use: IntendedUse::Exact,
        requested_span: FrameInterval::new(0, FRAMES).unwrap(),
        required_ports: vec![OutputPortContract {
            port: port(),
            channels: CHANNELS,
            format: PcmFormat::F32,
        }],
        trust_scope: TrustScope::Local,
        compatibility_profile: ContentDigest::sha256(&[0x51]),
    }
}

/// Hand-built 16-bit PCM WAV — interleaved frame-major samples.
fn wav_i16(frames: usize, sample: impl Fn(usize, usize) -> i16) -> Vec<u8> {
    let channels = u16::try_from(CHANNELS).expect("channels fits u16");
    let block_align = channels * 2;
    let byte_rate = RATE * u32::from(block_align);
    let data_len = u32::try_from(frames * usize::try_from(CHANNELS).unwrap() * 2)
        .expect("fixture data length fits u32");
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
    let channels_usize = usize::try_from(CHANNELS).unwrap();
    for f in 0..frames {
        for c in 0..channels_usize {
            out.extend_from_slice(&sample(f, c).to_le_bytes());
        }
    }
    out
}

fn sample_i32(value: usize) -> i32 {
    i32::try_from(value).expect("fixture index fits i32")
}

fn sine_blob() -> Vec<u8> {
    let frames = usize::try_from(FRAMES).unwrap();
    wav_i16(frames, |f, c| {
        (((sample_i32(f) + sample_i32(c) * 7) % 97) as i16) * 100
    })
}

fn other_blob() -> Vec<u8> {
    let frames = usize::try_from(FRAMES).unwrap();
    wav_i16(frames, |f, c| {
        -(((sample_i32(f) * 3 + sample_i32(c)) % 89) as i16) * 200
    })
}

fn publish_ok(
    store: &mut InMemoryArtifactStore,
    key: RecipeKey,
    blob: &[u8],
) -> (RenderArtifactId, ArtifactManifest) {
    let d = draft(key, CodecClass::Lossless, "pcm");
    match publish_artifact(store, &d, blob, &expect(FRAMES), None, None, None) {
        PublishOutcome::Published { artifact, manifest } => (artifact, *manifest),
        other => panic!("expected Published, got {other:?}"),
    }
}

/// Commits a caller-mutated manifest + its blob through the same CAS
/// path — used to construct eligibility edge cases publish cannot
/// produce (lossy class, incomplete receipt, foreign trust).
fn commit_manifest(
    store: &mut InMemoryArtifactStore,
    manifest: &ArtifactManifest,
    blob: &[u8],
) -> RenderArtifactId {
    let bytes = manifest.canonical_bytes().unwrap();
    let id = RenderArtifactId::of_manifest_bytes(&bytes);
    let staging = store.stage_begin().unwrap();
    store.stage_write(&staging, blob).unwrap();
    store.stage_seal(&staging, blob.len() as u64).unwrap();
    let expected = store.index_entries(&manifest.recipe_key).unwrap();
    store
        .commit_publish(&PublishCommit {
            staging,
            blob_digest: manifest.content_digest,
            manifest_id: id,
            manifest_bytes: bytes,
            recipe_key: manifest.recipe_key,
            expected_index: expected,
            reference: None,
        })
        .unwrap();
    id
}

fn published_manifest(store: &InMemoryArtifactStore, id: &RenderArtifactId) -> ArtifactManifest {
    let bytes = store.read_manifest(id).unwrap().unwrap();
    verify_manifest(id, &bytes).unwrap()
}

// ---------------------------------------------------------------------------
// RCP-STORE-01 — manifest roundtrip
// ---------------------------------------------------------------------------

#[test]
fn rcp_store_01_manifest_roundtrip_preserves_contract_fields() {
    let mut store = InMemoryArtifactStore::new();
    let key = recipe_key(0x01);
    let blob = sine_blob();
    let (id, manifest) = publish_ok(&mut store, key, &blob);

    // Stored bytes verify against the content-addressed ID.
    let stored = store.read_manifest(&id).unwrap().unwrap();
    let decoded = verify_manifest(&id, &stored).unwrap();
    assert_eq!(decoded, manifest);

    // Roundtrip through canonical bytes is deterministic.
    let bytes1 = manifest.canonical_bytes().unwrap();
    let bytes2 = decoded.canonical_bytes().unwrap();
    assert_eq!(bytes1, bytes2);
    assert_eq!(RenderArtifactId::of_manifest_bytes(&bytes1), id);

    // Contract fields preserved.
    assert_eq!(
        manifest.schema_version,
        synvelis_render::artifact::ARTIFACT_MANIFEST_VERSION
    );
    assert_eq!(manifest.recipe_key, key);
    assert_eq!(manifest.frame_count, FRAMES);
    assert_eq!(
        manifest.temporal.interval,
        FrameInterval::new(0, FRAMES).unwrap()
    );
    assert_eq!(manifest.temporal.sample_rate_hz, RATE);
    assert_eq!(manifest.temporal.channels, CHANNELS);
    assert_eq!(manifest.codec.container, "wav");
    assert_eq!(manifest.codec.codec, "pcm");
    assert_eq!(manifest.codec.class, CodecClass::Lossless);
    assert_eq!(manifest.content_digest, ContentDigest::sha256(&blob));
    assert_eq!(manifest.byte_len, blob.len() as u64);
    assert_eq!(manifest.provenance.produced_by, "test/publish");
    assert_eq!(
        manifest.reproducibility,
        ArtifactReproducibility::BitExactCertified
    );
    assert_eq!(manifest.ports.len(), 1);
    assert_eq!(manifest.ports[0].port, port());
    assert_eq!(manifest.ports[0].channels, CHANNELS);
    assert_eq!(manifest.ports[0].channel_offset, 0);
    assert_eq!(manifest.ports[0].format, PcmFormat::F32);
    assert_ne!(manifest.ports[0].digest, ContentDigest::sha256(b""));
    assert!(manifest.verification.whole_digest_verified);
    assert_eq!(manifest.verification.byte_len, blob.len() as u64);
    assert_ne!(manifest.verification.chunk_digests.len(), 0);
    assert_eq!(manifest.verification.decode.container, "wav");
    assert_eq!(manifest.verification.decode.codec, "pcm");
    assert_eq!(manifest.verification.decode.frames, FRAMES);
    assert_eq!(manifest.verification.decode.sample_rate_hz, RATE);
    assert_eq!(manifest.verification.decode.channels, CHANNELS);
    assert!(manifest.verification.decode.port_digests_verified);

    // Index + blob are reachable.
    assert_eq!(store.index_entries(&key).unwrap(), vec![id]);
    assert_eq!(
        store.read_blob(&manifest.content_digest).unwrap().unwrap(),
        blob
    );
    assert_eq!(store.staging_count(), 0);
}

// ---------------------------------------------------------------------------
// RCP-STORE-02 — verified publish failure classes keep prior state
// ---------------------------------------------------------------------------

#[test]
fn rcp_store_02_publish_success_then_failures_preserve_prior_artifact() {
    let mut store = InMemoryArtifactStore::new();
    let key = recipe_key(0x02);
    let blob_a = sine_blob();
    let (id_a, _) = publish_ok(&mut store, key, &blob_a);

    // Corrupt readback — hash mismatch must fail closed at Readback.
    store.corrupt_next_readback();
    let blob_b = other_blob();
    let d = draft(key, CodecClass::Lossless, "pcm");
    let outcome = publish_artifact(&mut store, &d, &blob_b, &expect(FRAMES), None, None, None);
    assert!(matches!(
        outcome,
        PublishOutcome::Failed {
            stage: PublishStage::Readback,
            ..
        }
    ));
    assert_eq!(store.staging_count(), 0);
    // Prior verified artifact untouched and still indexed.
    assert_eq!(store.index_entries(&key).unwrap(), vec![id_a]);
    assert!(store.read_manifest(&id_a).unwrap().is_some());

    // Decode failure — undecodable bytes never publish.
    let outcome = publish_artifact(
        &mut store,
        &d,
        b"this is not audio",
        &expect(FRAMES),
        None,
        None,
        None,
    );
    assert!(matches!(
        outcome,
        PublishOutcome::Failed {
            stage: PublishStage::Decode,
            ..
        }
    ));

    // Expectation mismatch — declared frame count disagrees with
    // measurement.
    let bad_expect = PublishExpectation {
        frame_count: FRAMES + 1,
        ..expect(FRAMES)
    };
    let outcome = publish_artifact(&mut store, &d, &blob_b, &bad_expect, None, None, None);
    assert!(matches!(
        outcome,
        PublishOutcome::Failed {
            stage: PublishStage::Decode,
            ..
        }
    ));
    assert_eq!(store.index_entries(&key).unwrap(), vec![id_a]);
}

#[test]
fn rcp_store_02_fault_injection_every_stage_fails_closed() {
    let cases: &[(FaultPoint, PublishStage)] = &[
        (FaultPoint::StageWrite, PublishStage::Stage),
        (FaultPoint::StageSeal, PublishStage::Seal),
        (FaultPoint::StageRead, PublishStage::Readback),
        (FaultPoint::CommitIndex, PublishStage::Commit),
        (FaultPoint::CommitBlob, PublishStage::Commit),
        (FaultPoint::CommitManifest, PublishStage::Commit),
    ];
    for (point, stage) in cases {
        let mut store = InMemoryArtifactStore::new();
        store.arm_fault(*point, StoreError::DiskFull);
        let d = draft(recipe_key(0x03), CodecClass::Lossless, "pcm");
        let outcome = publish_artifact(
            &mut store,
            &d,
            &sine_blob(),
            &expect(FRAMES),
            None,
            None,
            None,
        );
        assert!(
            matches!(outcome, PublishOutcome::Failed { stage: s, .. } if s == *stage),
            "fault {point:?} should fail at {stage:?}, got {outcome:?}"
        );
        assert_eq!(store.artifacts().unwrap().len(), 0, "fault {point:?}");
        assert_eq!(store.staging_count(), 0, "fault {point:?} leaked staging");
    }
}

#[test]
fn rcp_store_02_partial_write_and_cas_conflict_rejected() {
    // Partial write: staged length != declared length at seal.
    let mut store = InMemoryArtifactStore::new();
    let staging = store.stage_begin().unwrap();
    store.stage_write(&staging, b"partial").unwrap();
    let err = store.stage_seal(&staging, 1_000_000).unwrap_err();
    assert!(matches!(err, StoreError::PartialWrite { .. }));
    store.stage_abort(&staging);

    // CAS conflict: a commit whose expected_index is stale is
    // rejected and changes nothing.
    let mut store = InMemoryArtifactStore::new();
    let key = recipe_key(0x04);
    let (id_a, manifest_a) = publish_ok(&mut store, key, &sine_blob());
    let blob_b = other_blob();
    // Publish B normally first so the index moves.
    let (id_b, _) = publish_ok(&mut store, key, &blob_b);
    assert_ne!(id_a, id_b);
    // Now hand a commit with the *old* single-entry expectation.
    let staging = store.stage_begin().unwrap();
    store.stage_write(&staging, &sine_blob()).unwrap();
    store
        .stage_seal(&staging, sine_blob().len() as u64)
        .unwrap();
    let stale_index = vec![id_b]; // not the real current [id_a, id_b]
    let bytes_a = manifest_a.canonical_bytes().unwrap();
    let err = store
        .commit_publish(&PublishCommit {
            staging,
            blob_digest: manifest_a.content_digest,
            manifest_id: id_a,
            manifest_bytes: bytes_a,
            recipe_key: key,
            expected_index: stale_index,
            reference: None,
        })
        .unwrap_err();
    assert!(matches!(err, StoreError::Conflict { .. }));
    // State unchanged: both artifacts still indexed.
    let mut entries = store.index_entries(&key).unwrap();
    entries.sort();
    let mut want = vec![id_a, id_b];
    want.sort();
    assert_eq!(entries, want);
}

#[test]
fn rcp_store_02_cancellation_at_stage_boundaries() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let mut store = InMemoryArtifactStore::new();
    let d = draft(recipe_key(0x05), CodecClass::Lossless, "pcm");
    let outcome = publish_artifact(
        &mut store,
        &d,
        &sine_blob(),
        &expect(FRAMES),
        None,
        Some(&cancel),
        None,
    );
    assert!(matches!(outcome, PublishOutcome::Cancelled { .. }));
    assert_eq!(store.artifacts().unwrap().len(), 0);
    assert_eq!(store.staging_count(), 0);
}

// ---------------------------------------------------------------------------
// RCP-STORE-03 — same recipe, different PCM → distinct artifacts;
// blob dedup never implies recipe identity
// ---------------------------------------------------------------------------

#[test]
fn rcp_store_03_same_recipe_distinct_pcm_distinct_artifacts() {
    let mut store = InMemoryArtifactStore::new();
    let key = recipe_key(0x06);
    let blob_a = sine_blob();
    let blob_b = other_blob();
    let (id_a, ma) = publish_ok(&mut store, key, &blob_a);
    let (id_b, mb) = publish_ok(&mut store, key, &blob_b);

    assert_ne!(id_a, id_b);
    assert_ne!(ma.content_digest, mb.content_digest);
    // Both coexist under one recipe — no "latest wins".
    let mut entries = store.index_entries(&key).unwrap();
    entries.sort();
    let mut want = vec![id_a, id_b];
    want.sort();
    assert_eq!(entries, want);
    // Provenance differs per publish — recorded, not hidden.
    let mut md_a = ma.clone();
    md_a.provenance.job_id = Some("job-a".to_owned());
    let mut md_b = mb.clone();
    md_b.provenance.job_id = Some("job-b".to_owned());
    assert_ne!(md_a, md_b);
}

#[test]
fn rcp_store_03_blob_dedup_does_not_merge_recipe_identity() {
    let mut store = InMemoryArtifactStore::new();
    let key_a = recipe_key(0x07);
    let key_b = recipe_key(0x08);
    let blob = sine_blob();
    let (id_a, ma) = publish_ok(&mut store, key_a, &blob);
    let blobs_before = store.blob_count();
    let (id_b, mb) = publish_ok(&mut store, key_b, &blob);

    // Same bytes → one CAS blob, two manifests, two artifact IDs.
    assert_eq!(store.blob_count(), blobs_before);
    assert_ne!(id_a, id_b);
    assert_eq!(ma.content_digest, mb.content_digest);
    assert_ne!(ma.recipe_key, mb.recipe_key);
    assert_eq!(store.index_entries(&key_a).unwrap(), vec![id_a]);
    assert_eq!(store.index_entries(&key_b).unwrap(), vec![id_b]);
    // Recipe identity is never inferred from blob identity: each
    // manifest carries its own recipe key.
    assert_eq!(published_manifest(&store, &id_a).recipe_key, key_a);
    assert_eq!(published_manifest(&store, &id_b).recipe_key, key_b);
}

// ---------------------------------------------------------------------------
// RCP-STORE-04 — eligibility vocabulary, unconverted
// ---------------------------------------------------------------------------

#[test]
fn rcp_store_04_lookup_states_roundtrip() {
    let mut store = InMemoryArtifactStore::new();
    let key = recipe_key(0x09);

    // Missing — empty index.
    let result = lookup(&store, &query(key), &resolved(key)).unwrap();
    assert!(matches!(result.outcome, LookupOutcome::Missing));

    // Non-resolved current resolution → Unknown, never valid.
    let unknown_current = RecipeResolution::Unknown {
        dependencies: Vec::new(),
    };
    let result = lookup(&store, &query(key), &unknown_current).unwrap();
    assert!(matches!(result.outcome, LookupOutcome::Unknown { .. }));

    // Publish → EligibleExact under matching request.
    let blob = sine_blob();
    let (id, manifest) = publish_ok(&mut store, key, &blob);
    let result = lookup(&store, &query(key), &resolved(key)).unwrap();
    assert!(
        matches!(result.outcome, LookupOutcome::EligibleExact { artifact } if artifact == id),
        "got {:?}",
        result.outcome
    );
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(
        result.candidates[0].eligibility,
        ArtifactEligibility::EligibleExact
    );

    // Stale — artifact's recipe is no longer the current resolution.
    let other_key = recipe_key(0x0A);
    let result = lookup(&store, &query(key), &resolved(other_key)).unwrap();
    assert!(matches!(result.outcome, LookupOutcome::Stale));

    // Quarantined → Corrupt.
    store.quarantine(&id, "test");
    let result = lookup(&store, &query(key), &resolved(key)).unwrap();
    assert!(matches!(result.outcome, LookupOutcome::Corrupt));
    store.delete_artifact(&id).unwrap();
    let _ = manifest;
}

/// Manifest variants exercising each eligibility gate, plus the id
/// of the lossy variant (the only id asserted by value).
struct FitnessVariants {
    lossy: ArtifactManifest,
    preview: ArtifactManifest,
    unknown: ArtifactManifest,
    unverified: ArtifactManifest,
    denied: ArtifactManifest,
    lossy_id: RenderArtifactId,
}

/// Commits manifest variants publish cannot produce (lossy class,
/// preview-only/unknown reproducibility, incomplete receipt, denied
/// trust) through the same CAS path.
fn fitness_variants(
    store: &mut InMemoryArtifactStore,
    manifest: &ArtifactManifest,
    blob: &[u8],
) -> FitnessVariants {
    // Lossy class + foreign codec string: media receipt stays
    // self-consistent, but Exact gates refuse → Preview only.
    let mut lossy = manifest.clone();
    lossy.codec.class = CodecClass::Lossy;
    "mp3".clone_into(&mut lossy.codec.codec);
    "mp3".clone_into(&mut lossy.verification.decode.codec);
    let lossy_id = commit_manifest(store, &lossy, blob);

    // PreviewOnly reproducibility → Preview only.
    let mut preview = manifest.clone();
    preview.reproducibility = ArtifactReproducibility::PreviewOnly;
    // provenance differs → distinct manifest bytes/id
    preview.provenance.job_id = Some("preview".to_owned());
    commit_manifest(store, &preview, blob);

    // Unknown reproducibility → never valid, not even Preview.
    let mut unknown = manifest.clone();
    unknown.reproducibility = ArtifactReproducibility::Unknown;
    unknown.provenance.job_id = Some("unknown".to_owned());
    commit_manifest(store, &unknown, blob);

    // Incomplete receipt → Unverified.
    let mut unverified = manifest.clone();
    unverified.verification.whole_digest_verified = false;
    unverified.provenance.job_id = Some("unverified".to_owned());
    commit_manifest(store, &unverified, blob);

    // Trusted-but-denied ACL → Denied under Shared scope.
    let mut denied = manifest.clone();
    denied.trust = Some(ArtifactTrust {
        principal: "peer".to_owned(),
        acl: AccessClass::None,
        expires_at: None,
        revoked: false,
        share_verified: true,
    });
    denied.provenance.job_id = Some("denied".to_owned());
    commit_manifest(store, &denied, blob);

    FitnessVariants {
        lossy,
        preview,
        unknown,
        unverified,
        denied,
        lossy_id,
    }
}

/// Outcome of a lookup where `manifest` is the only candidate.
fn solo_outcome(
    manifest: &ArtifactManifest,
    blob: &[u8],
    q: &CacheLookup,
    current: &RecipeResolution,
) -> LookupOutcome {
    let mut store = InMemoryArtifactStore::new();
    commit_manifest(&mut store, manifest, blob);
    lookup(&store, q, current).unwrap().outcome
}

#[test]
fn rcp_store_04_fitness_gates_never_promote_to_exact() {
    let mut store = InMemoryArtifactStore::new();
    let key = recipe_key(0x0B);
    let blob = sine_blob();
    let (id, manifest) = publish_ok(&mut store, key, &blob);
    let v = fitness_variants(&mut store, &manifest, &blob);
    let local_id = id;
    let current = resolved(key);
    let q = query(key);

    // Deterministic best pick: Exact artifact wins over Preview.
    let result = lookup(&store, &q, &current).unwrap();
    assert!(
        matches!(result.outcome, LookupOutcome::EligibleExact { artifact } if artifact == local_id),
        "got {:?}",
        result.outcome
    );
    // Candidates are reported in deterministic rank order.
    let els: Vec<_> = result
        .candidates
        .iter()
        .map(|c| (c.artifact, c.eligibility))
        .collect();
    assert_eq!(els.first().unwrap().0, local_id);

    // Lossy artifact alone → EligiblePreview (never Exact).
    let outcome = solo_outcome(&v.lossy, &blob, &q, &current);
    assert!(
        matches!(outcome, LookupOutcome::EligiblePreview { artifact } if artifact == v.lossy_id),
        "got {outcome:?}"
    );

    // PreviewOnly alone → EligiblePreview.
    let outcome = solo_outcome(&v.preview, &blob, &q, &current);
    assert!(matches!(outcome, LookupOutcome::EligiblePreview { .. }));

    // Unknown repro alone → Unknown outcome (verified but unfit).
    let outcome = solo_outcome(&v.unknown, &blob, &q, &current);
    assert!(matches!(outcome, LookupOutcome::Unknown { .. }));

    // Unverified receipt alone → Unverified.
    let outcome = solo_outcome(&v.unverified, &blob, &q, &current);
    assert!(matches!(outcome, LookupOutcome::Unverified));

    // Denied ACL under Shared scope → Denied (all candidates denied).
    let mut shared = q.clone();
    shared.trust_scope = TrustScope::Shared;
    let outcome = solo_outcome(&v.denied, &blob, &shared, &current);
    assert!(matches!(outcome, LookupOutcome::Denied { .. }));

    // Local artifact (trust=None) under Shared scope → Denied.
    let outcome = solo_outcome(&manifest, &blob, &shared, &current);
    assert!(matches!(outcome, LookupOutcome::Denied { .. }));
}

#[test]
fn rcp_store_04_request_fit_gates_never_select_exact() {
    let mut store = InMemoryArtifactStore::new();
    let key = recipe_key(0x10);
    let blob = sine_blob();
    let (_, manifest) = publish_ok(&mut store, key, &blob);
    let current = resolved(key);

    // Insufficient span → verified media but request-unfit.
    let mut wide = query(key);
    wide.requested_span = FrameInterval::new(0, FRAMES + 100).unwrap();

    // Missing required port → request-unfit.
    let mut extra_port = query(key);
    extra_port.required_ports.push(OutputPortContract {
        port: SemanticPortId::new(0xB00),
        channels: 1,
        format: PcmFormat::F32,
    });

    // Incompatible renderer profile → request-unfit.
    let mut wrong_profile = query(key);
    wrong_profile.compatibility_profile = ContentDigest::sha256(b"other-profile");

    for (q, rejection) in [
        (wide, FitRejection::InsufficientSpan),
        (extra_port, FitRejection::PortContract),
        (wrong_profile, FitRejection::IncompatibleProfile),
    ] {
        let result = lookup(&store, &q, &current).unwrap();
        assert!(
            !matches!(result.outcome, LookupOutcome::EligibleExact { .. }),
            "fit gate promoted to Exact: {:?}",
            result.outcome
        );
        assert!(
            result
                .candidates
                .iter()
                .any(|c| c.rejection == Some(rejection)),
            "missing {rejection:?} evidence: {:?}",
            result.candidates
        );
    }

    // Preview intent on a lossy-only artifact still yields Preview.
    let mut lossy = manifest.clone();
    lossy.codec.class = CodecClass::Lossy;
    "mp3".clone_into(&mut lossy.codec.codec);
    "mp3".clone_into(&mut lossy.verification.decode.codec);
    let mut lossy_store = InMemoryArtifactStore::new();
    commit_manifest(&mut lossy_store, &lossy, &blob);
    let mut preview_use = query(key);
    preview_use.intended_use = IntendedUse::Preview;
    let result = lookup(&lossy_store, &preview_use, &current).unwrap();
    assert!(matches!(
        result.outcome,
        LookupOutcome::EligiblePreview { .. }
    ));
}

#[test]
fn rcp_store_04_semantic_contradiction_is_corrupt() {
    let key = recipe_key(0x11);
    let blob = sine_blob();
    let current = resolved(key);
    let q = query(key);

    // Baseline manifest to mutate — committed through the same CAS
    // path a store bypass would use (publish never produces these).
    let mut origin = InMemoryArtifactStore::new();
    let (_, manifest) = publish_ok(&mut origin, key, &blob);

    // Codec class contradicts codec identity — `Lossless` declared
    // for a lossy codec string (the shared-path exploit shape).
    let mut store = InMemoryArtifactStore::new();
    let mut bad = manifest.clone();
    bad.codec.class = CodecClass::Lossless;
    "mp3".clone_into(&mut bad.codec.codec);
    "mp3".clone_into(&mut bad.verification.decode.codec);
    commit_manifest(&mut store, &bad, &blob);
    let result = lookup(&store, &q, &current).unwrap();
    assert!(
        matches!(result.outcome, LookupOutcome::Corrupt),
        "codec class/codec contradiction promoted: {:?}",
        result.outcome
    );

    // Inverse lie — `Lossy` declared for lossless `pcm`.
    let mut store = InMemoryArtifactStore::new();
    let mut bad = manifest.clone();
    bad.codec.class = CodecClass::Lossy;
    commit_manifest(&mut store, &bad, &blob);
    let result = lookup(&store, &q, &current).unwrap();
    assert!(
        matches!(result.outcome, LookupOutcome::Corrupt),
        "inverse codec contradiction promoted: {:?}",
        result.outcome
    );

    // Ports do not tile the declared channel count.
    let mut store = InMemoryArtifactStore::new();
    let mut bad = manifest.clone();
    bad.ports[0].channels = 1;
    commit_manifest(&mut store, &bad, &blob);
    let result = lookup(&store, &q, &current).unwrap();
    assert!(
        matches!(result.outcome, LookupOutcome::Corrupt),
        "non-tiling ports promoted: {:?}",
        result.outcome
    );
}

// ---------------------------------------------------------------------------
// RCP-STORE-05 — GC roots, grace, tombstone, conservative retention
// ---------------------------------------------------------------------------

#[test]
fn rcp_store_05_gc_grace_and_tombstone() {
    let mut store = InMemoryArtifactStore::new();
    let key = recipe_key(0x0C);
    let (id, manifest) = publish_ok(&mut store, key, &sine_blob());
    let digest = manifest.content_digest;
    let policy = GcPolicy { grace_ticks: 10 };

    // Never referenced: grace runs from publish tick.
    let report = sweep(&mut store, &policy).unwrap();
    assert_eq!(report.kept_grace, vec![id]);
    store.advance_clock(9);
    let report = sweep(&mut store, &policy).unwrap();
    assert_eq!(report.kept_grace, vec![id]);
    store.advance_clock(1);
    let report = sweep(&mut store, &policy).unwrap();
    assert_eq!(report.collected, vec![id]);
    assert_eq!(store.tombstone(&id), Some(store.now()));
    assert!(store.read_manifest(&id).unwrap().is_none());
    assert!(store.read_blob(&digest).unwrap().is_none());
    assert!(report.blobs_freed.contains(&digest));
}

#[test]
fn rcp_store_05_gc_reference_lifecycle() {
    let mut store = InMemoryArtifactStore::new();
    let key = recipe_key(0x0D);
    let branch = ArtifactReference::LiveBranch {
        project_id: ProjectId::new(0x291),
        variant_set: VariantSetId::new(0x1),
        branch: BranchId::new(0x2),
    };
    let d = draft(key, CodecClass::Lossless, "pcm");
    let (id, _) = match publish_artifact(
        &mut store,
        &d,
        &sine_blob(),
        &expect(FRAMES),
        None,
        None,
        Some(branch.clone()),
    ) {
        PublishOutcome::Published { artifact, manifest } => (artifact, *manifest),
        other => panic!("expected Published, got {other:?}"),
    };
    let policy = GcPolicy { grace_ticks: 5 };

    // Active reference → reachable regardless of clock.
    store.advance_clock(100);
    let report = sweep(&mut store, &policy).unwrap();
    assert_eq!(report.kept_reachable, vec![id]);

    // Release → grace window from release tick.
    store.release_reference(&id, &branch).unwrap();
    let report = sweep(&mut store, &policy).unwrap();
    assert_eq!(report.kept_grace, vec![id]);
    store.advance_clock(4);
    let report = sweep(&mut store, &policy).unwrap();
    assert_eq!(report.kept_grace, vec![id]);
    store.advance_clock(1);
    let report = sweep(&mut store, &policy).unwrap();
    assert_eq!(report.collected, vec![id]);
}

#[test]
fn rcp_store_05_gc_pending_and_external_roots_conservative() {
    let mut store = InMemoryArtifactStore::new();
    let key = recipe_key(0x0E);
    let pending_ref = ArtifactReference::PinnedSnapshot {
        locator: "remote-pin-1".to_owned(),
        snapshot_digest: ContentDigest::sha256(b"snap"),
    };
    let d = draft(key, CodecClass::Lossless, "pcm");
    let (id_pending, _) = match publish_artifact(
        &mut store,
        &d,
        &sine_blob(),
        &expect(FRAMES),
        None,
        None,
        Some(pending_ref.clone()),
    ) {
        PublishOutcome::Published { artifact, manifest } => (artifact, *manifest),
        other => panic!("expected Published, got {other:?}"),
    };
    // Second artifact, externally attached (commit-associated root).
    let blob_b = other_blob();
    let d2 = draft(recipe_key(0x0F), CodecClass::Lossless, "pcm");
    let (id_ext, _) =
        match publish_artifact(&mut store, &d2, &blob_b, &expect(FRAMES), None, None, None) {
            PublishOutcome::Published { artifact, manifest } => (artifact, *manifest),
            other => panic!("expected Published, got {other:?}"),
        };
    attach_external_manifest(
        &mut store,
        &ExternalManifestRef {
            commit_locator: "commit-deadbeef".to_owned(),
            snapshot_digest: ContentDigest::sha256(b"snap2"),
            artifact: id_ext,
        },
    )
    .unwrap();

    let policy = GcPolicy { grace_ticks: 0 };
    store.advance_clock(10_000);
    let report = sweep(&mut store, &policy).unwrap();
    // Pending = unresolved remote reachability → conservative keep.
    assert!(report.kept_reachable.contains(&id_pending));
    // External manifest attachment = commit-associated root → keep.
    assert!(report.kept_reachable.contains(&id_ext));
    assert_eq!(report.collected.len(), 0);

    // Detach the external root → grace applies (grace_ticks=0 → now).
    store
        .detach_external_manifest("commit-deadbeef", &ContentDigest::sha256(b"snap2"), &id_ext)
        .unwrap();
    let report = sweep(&mut store, &policy).unwrap();
    assert!(report.collected.contains(&id_ext));
    assert!(report.kept_reachable.contains(&id_pending));
}

#[test]
fn rcp_store_05_gc_shared_blob_refcount_and_quarantine() {
    let mut store = InMemoryArtifactStore::new();
    let key_a = recipe_key(0x10);
    let key_b = recipe_key(0x11);
    let blob = sine_blob();
    let (id_a, ma) = publish_ok(&mut store, key_a, &blob);
    let (id_b, mb) = publish_ok(&mut store, key_b, &blob);
    assert_eq!(ma.content_digest, mb.content_digest);
    let digest = ma.content_digest;

    // Keep B alive with an active job reference; A is unreferenced.
    let job = ArtifactReference::InFlightJob {
        job_id: "job-1".to_owned(),
    };
    store.attach_reference(&id_b, job).unwrap();

    let policy = GcPolicy { grace_ticks: 0 };
    store.advance_clock(1);
    let report = sweep(&mut store, &policy).unwrap();
    assert!(report.collected.contains(&id_a));
    // Blob survives — B still references the same digest.
    assert!(!report.blobs_freed.contains(&digest));
    assert!(store.read_blob(&digest).unwrap().is_some());

    // Quarantine C → collected on its own grace like any record.
    let key_c = recipe_key(0x12);
    let (id_c, _) = publish_ok(&mut store, key_c, &other_blob());
    store.quarantine(&id_c, "integrity");
    store.advance_clock(1);
    let report = sweep(&mut store, &policy).unwrap();
    assert!(report.quarantined_collected.contains(&id_c));
    assert!(store.tombstone(&id_c).is_some());

    // Release B's job ref → next sweep collects B and frees the blob.
    let job_ref = ArtifactReference::InFlightJob {
        job_id: "job-1".to_owned(),
    };
    store.release_reference(&id_b, &job_ref).unwrap();
    let report = sweep(&mut store, &policy).unwrap();
    assert!(report.collected.contains(&id_b));
    assert!(report.blobs_freed.contains(&digest));
    assert!(store.read_blob(&digest).unwrap().is_none());
}

// ---------------------------------------------------------------------------
// RCP-STORE-06 — shared verification + external manifest attach
// ---------------------------------------------------------------------------

fn shared_draft(key: RecipeKey) -> ArtifactDraft {
    let mut d = draft(key, CodecClass::Lossless, "pcm");
    d.trust = Some(ArtifactTrust {
        principal: "peer-node".to_owned(),
        acl: AccessClass::Read,
        expires_at: Some(1_000_000),
        revoked: false,
        share_verified: true,
    });
    d
}

fn remote_of(store: &InMemoryArtifactStore, id: &RenderArtifactId, blob: &[u8]) -> RemoteArtifact {
    RemoteArtifact {
        artifact: *id,
        manifest_bytes: store.read_manifest(id).unwrap().unwrap(),
        blob: Some(blob.to_vec()),
        principal: "peer-node".to_owned(),
        expires_at: None,
    }
}

#[test]
fn rcp_store_06_remote_registration_verified_and_idempotent() {
    let mut origin = InMemoryArtifactStore::new();
    let key = recipe_key(0x13);
    let blob = sine_blob();
    let d = shared_draft(key);
    let (id, _) = match publish_artifact(&mut origin, &d, &blob, &expect(FRAMES), None, None, None)
    {
        PublishOutcome::Published { artifact, manifest } => (artifact, *manifest),
        other => panic!("expected Published, got {other:?}"),
    };

    let mut sink = InMemoryArtifactStore::new();
    let remote = remote_of(&origin, &id, &blob);
    let result = register_remote(&mut sink, &remote).unwrap();
    assert!(matches!(result, ShareRegistration::Registered { artifact } if artifact == id));
    // Bytes + manifest + index all landed.
    assert_eq!(
        sink.read_blob(&ContentDigest::sha256(&blob))
            .unwrap()
            .unwrap(),
        blob
    );
    assert!(sink.read_manifest(&id).unwrap().is_some());
    assert_eq!(sink.index_entries(&key).unwrap(), vec![id]);

    // Idempotent re-registration.
    let result = register_remote(&mut sink, &remote).unwrap();
    assert!(matches!(result, ShareRegistration::AlreadyPresent { .. }));
}

/// `RemoteArtifact` carrying explicit manifest bytes + blob.
fn remote_bytes(id: RenderArtifactId, bytes: Vec<u8>, blob: &[u8]) -> RemoteArtifact {
    RemoteArtifact {
        artifact: id,
        manifest_bytes: bytes,
        blob: Some(blob.to_vec()),
        principal: "p".to_owned(),
        expires_at: None,
    }
}

/// Mutated manifest → (id, canonical bytes) for remote submission.
fn remanifest(
    manifest: &ArtifactManifest,
    mutate: impl FnOnce(&mut ArtifactManifest),
) -> (RenderArtifactId, Vec<u8>) {
    let mut m = manifest.clone();
    mutate(&mut m);
    let bytes = m.canonical_bytes().unwrap();
    (RenderArtifactId::of_manifest_bytes(&bytes), bytes)
}

fn assert_share_rejected(
    sink: &mut InMemoryArtifactStore,
    remote: &RemoteArtifact,
    reason: &ShareRejection,
) {
    let result = register_remote(sink, remote).unwrap();
    assert!(
        matches!(&result, ShareRegistration::Rejected { reason: r } if r == reason),
        "expected {reason:?}, got {result:?}"
    );
}

#[test]
fn rcp_store_06_remote_fail_closed_blob_and_manifest() {
    let mut origin = InMemoryArtifactStore::new();
    let key = recipe_key(0x14);
    let blob = sine_blob();
    let d = shared_draft(key);
    let (id, _) = match publish_artifact(&mut origin, &d, &blob, &expect(FRAMES), None, None, None)
    {
        PublishOutcome::Published { artifact, manifest } => (artifact, *manifest),
        other => panic!("expected Published, got {other:?}"),
    };
    let manifest_bytes = origin.read_manifest(&id).unwrap().unwrap();
    let mut sink = InMemoryArtifactStore::new();

    // Missing blob → BlobMissing.
    let remote = RemoteArtifact {
        blob: None,
        ..remote_of(&origin, &id, &blob)
    };
    assert_share_rejected(&mut sink, &remote, &ShareRejection::BlobMissing);

    // Digest mismatch (truncated download) → BlobMismatch.
    let remote = RemoteArtifact {
        blob: Some(blob[..blob.len() / 2].to_vec()),
        ..remote.clone()
    };
    assert_share_rejected(&mut sink, &remote, &ShareRejection::BlobMismatch);

    // Tampered manifest bytes → ManifestInvalid (id no longer matches).
    let mut bad_manifest = manifest_bytes.clone();
    let mid = bad_manifest.len() / 2;
    bad_manifest[mid] ^= 0xFF;
    let remote = RemoteArtifact {
        manifest_bytes: bad_manifest,
        ..remote.clone()
    };
    assert_share_rejected(&mut sink, &remote, &ShareRejection::ManifestInvalid);

    // Nothing was registered by any rejection.
    assert_eq!(sink.artifacts().unwrap().len(), 0);
}

#[test]
fn rcp_store_06_remote_fail_closed_trust_claim_and_receipt() {
    let mut origin = InMemoryArtifactStore::new();
    let key = recipe_key(0x16);
    let blob = sine_blob();
    let d = shared_draft(key);
    let (id, manifest) =
        match publish_artifact(&mut origin, &d, &blob, &expect(FRAMES), None, None, None) {
            PublishOutcome::Published { artifact, manifest } => (artifact, *manifest),
            other => panic!("expected Published, got {other:?}"),
        };
    let mut sink = InMemoryArtifactStore::new();

    // Trust rejections — each trust mutation fails closed.
    for mutate in [
        |t: &mut ArtifactTrust| t.share_verified = false,
        |t: &mut ArtifactTrust| t.revoked = true,
        |t: &mut ArtifactTrust| t.acl = AccessClass::None,
        |t: &mut ArtifactTrust| t.expires_at = Some(0),
    ] {
        let (bad_id, bad_bytes) = remanifest(&manifest, |m| {
            let mut trust = m.trust.clone().unwrap();
            mutate(&mut trust);
            m.trust = Some(trust);
        });
        let remote = remote_bytes(bad_id, bad_bytes, &blob);
        assert_share_rejected(&mut sink, &remote, &ShareRejection::AccessDenied);
    }

    // No trust block at all → AccessDenied for shared path.
    let (local_id, local_bytes) = remanifest(&manifest, |m| m.trust = None);
    let remote = remote_bytes(local_id, local_bytes, &blob);
    assert_share_rejected(&mut sink, &remote, &ShareRejection::AccessDenied);

    // Transport claims longer validity than the manifest → ClaimMismatch.
    let remote = RemoteArtifact {
        expires_at: Some(2_000_000),
        ..remote_of(&origin, &id, &blob)
    };
    assert_share_rejected(&mut sink, &remote, &ShareRejection::ClaimMismatch);

    // Receipt contradicting actual decode → BlobInvalid.
    let (lying_id, lying_bytes) = remanifest(&manifest, |m| {
        m.verification.decode.frames += 1;
    });
    let remote = remote_bytes(lying_id, lying_bytes, &blob);
    assert_share_rejected(&mut sink, &remote, &ShareRejection::BlobInvalid);

    // Nothing was registered by any rejection.
    assert_eq!(sink.artifacts().unwrap().len(), 0);
}

#[test]
fn rcp_store_06_remote_media_contradiction_rejected() {
    let mut origin = InMemoryArtifactStore::new();
    let key = recipe_key(0x17);
    let blob = sine_blob();
    let d = shared_draft(key);
    let (_, manifest) =
        match publish_artifact(&mut origin, &d, &blob, &expect(FRAMES), None, None, None) {
            PublishOutcome::Published { artifact, manifest } => (artifact, *manifest),
            other => panic!("expected Published, got {other:?}"),
        };
    let mut sink = InMemoryArtifactStore::new();

    // Each mutation forges a media fact the receipt claims but the
    // blob never measured — remote re-verification must reject all.
    for mutate in [
        // Codec class contradicts codec identity.
        (|m: &mut ArtifactManifest| m.codec.class = CodecClass::Lossy) as fn(&mut ArtifactManifest),
        // Declared frame count contradicts decoded frames.
        (|m: &mut ArtifactManifest| m.frame_count += 1) as fn(&mut ArtifactManifest),
        // Port layout does not tile decoded channels.
        (|m: &mut ArtifactManifest| m.ports[0].channels = 1) as fn(&mut ArtifactManifest),
        // Per-port PCM digest was never measured.
        (|m: &mut ArtifactManifest| {
            m.ports[0].digest = ContentDigest::sha256(b"forged");
        }) as fn(&mut ArtifactManifest),
        // Chunk digest evidence forged.
        (|m: &mut ArtifactManifest| {
            m.verification.chunk_digests[0] = ContentDigest::sha256(b"forged");
        }) as fn(&mut ArtifactManifest),
        // Receipt admits incomplete verification.
        (|m: &mut ArtifactManifest| {
            m.verification.whole_digest_verified = false;
        }) as fn(&mut ArtifactManifest),
    ] {
        let (bad_id, bad_bytes) = remanifest(&manifest, mutate);
        let remote = remote_bytes(bad_id, bad_bytes, &blob);
        assert_share_rejected(&mut sink, &remote, &ShareRejection::BlobInvalid);
    }

    // Nothing was registered by any rejection.
    assert_eq!(sink.artifacts().unwrap().len(), 0);
}

#[test]
fn rcp_store_06_external_manifest_attach_commit_sha_invariant() {
    let mut store = InMemoryArtifactStore::new();
    let key = recipe_key(0x15);
    let blob = sine_blob();
    let (id, _) = publish_ok(&mut store, key, &blob);
    let manifest_before = store.read_manifest(&id).unwrap().unwrap();
    let locator = "feedfacecafebabe".to_owned();
    let snap = ContentDigest::sha256(b"snapshot");

    attach_external_manifest(
        &mut store,
        &ExternalManifestRef {
            commit_locator: locator.clone(),
            snapshot_digest: snap,
            artifact: id,
        },
    )
    .unwrap();

    // Commit SHA/manifest bytes are untouched — attach is registry-only.
    assert_eq!(store.read_manifest(&id).unwrap().unwrap(), manifest_before);
    let entries = store.external_manifests(&locator, &snap).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].artifact, id);
    assert!(store.has_external_manifest(&id).unwrap());

    // Unsafe locators and unknown artifacts fail closed.
    for bad in ["../escape", "a/b", "with space", ""] {
        let err = attach_external_manifest(
            &mut store,
            &ExternalManifestRef {
                commit_locator: bad.to_owned(),
                snapshot_digest: snap,
                artifact: id,
            },
        )
        .unwrap_err();
        assert!(matches!(
            err,
            StoreError::Conflict { .. } | StoreError::NotFound { .. }
        ));
    }
    let err = attach_external_manifest(
        &mut store,
        &ExternalManifestRef {
            commit_locator: "ok-locator".to_owned(),
            snapshot_digest: snap,
            artifact: RenderArtifactId(ContentDigest::sha256(b"ghost")),
        },
    )
    .unwrap_err();
    assert!(matches!(err, StoreError::NotFound { .. }));

    // Reference records expose the typed vocabulary for GC roots.
    let _ = RefState::Pending; // vocabulary exercised in GC tests
    let _ = VerificationReceipt {
        whole_digest_verified: true,
        byte_len: 0,
        chunk_digests: Vec::new(),
        decode: synvelis_render::artifact::manifest::DecodeReceipt {
            container: String::new(),
            codec: String::new(),
            sample_rate_hz: 0,
            channels: 0,
            frames: 0,
            port_digests_verified: false,
        },
        pcm_compared: false,
    };
}
