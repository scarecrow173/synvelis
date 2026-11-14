//! Verified publish pipeline — the fixed RCP-00 order:
//! staging write → seal/finalize → hash + metadata readback →
//! independent PCM decode/frame proof → atomic manifest + blob
//! publish → index CAS (synvelis#291, RCP-02).
//!
//! Every stage failure is typed and fail-closed: the staging slot is
//! aborted, previously published verified artifacts are untouched,
//! and partial/intermediate output is never labeled `Completed` or
//! `Exact`. Blob bytes are decoded through the independent
//! symphonia/`SourceReader` path — the same reader contract
//! [`crate::verify::reopen_check`] uses for file exports — never
//! through the writer's own view.
//!
//! The blob itself is opaque to this module except for decode
//! verification: container/codec identity, canonical rate/channels,
//! frame count and per-port PCM digests are *measured* from the
//! readback bytes, then baked into the manifest. Declared expectations
//! that disagree with measurement fail the publish.

use std::io::Cursor;

use ruviform_exec::CancellationToken;
use ruviform_media_core::StreamKind;
use ruviform_media_io::{CodecId, DecodeMode, Demuxer, Error as MediaError, ErrorKind};
use ruviform_media_reader::SourceReader;
use synvelis_project::snapshot::ContentDigest;

use crate::artifact::manifest::{
    ARTIFACT_MANIFEST_VERSION, ArtifactManifest, ArtifactPort, ArtifactPortSpec,
    ArtifactProvenance, ArtifactReproducibility, ArtifactTrust, CodecDescriptor, DecodeReceipt,
    MAX_ARTIFACT_FRAMES, MAX_ARTIFACT_PORTS, RenderArtifactId, STAGE_CHUNK_BYTES,
    VerificationReceipt, codec_class_consistent, port_digest, ports_tile_channels,
};
use crate::artifact::store::{
    ArtifactReference, ArtifactStore, PublishCommit, StagingId, StoreError,
};
use crate::recipe::{RecipeKey, RecipeTarget, RendererProfile, TemporalDomain};
use crate::spec::PcmExpectRef;
use synvelis_project::SnapshotIdentity;

/// Decode scratch width — frames decoded per `read_planar` call.
const DECODE_CHUNK: usize = 8192;

/// What an artifact draft declares before measurement — everything the
/// manifest records *except* the measured facts (digests, frame count,
/// byte length, verification receipt), which publish derives.
#[derive(Clone, Debug)]
pub struct ArtifactDraft {
    /// Recipe key the artifact is filed under.
    pub recipe_key: RecipeKey,
    /// Boundary semantic reference + project identity.
    pub target: RecipeTarget,
    /// Pinned project snapshot, when available.
    pub snapshot: Option<SnapshotIdentity>,
    /// Temporal domain (interval, pre-roll, tail, PDC, format/rate).
    pub temporal: TemporalDomain,
    /// Frames cut from the head of the source interval.
    pub cut_frames: u64,
    /// Effective output latency in frames.
    pub latency_frames: u64,
    /// Ordered output port declarations (digests are measured).
    pub ports: Vec<ArtifactPortSpec>,
    /// Declared container/codec/lossless class — verified against the
    /// independent decode; a lying declaration fails the publish.
    pub codec: CodecDescriptor,
    /// Renderer capability/provenance.
    pub renderer: RendererProfile,
    /// Reproducibility class.
    pub reproducibility: ArtifactReproducibility,
    /// Producing-pipeline provenance.
    pub provenance: ArtifactProvenance,
    /// Shared/remote trust metadata — `None` for local artifacts.
    pub trust: Option<ArtifactTrust>,
}

/// What the publisher expects the blob to decode as — every field is
/// checked against the independent readback; a mismatch is a typed
/// publish failure, never a silent artifact.
#[derive(Clone, Debug)]
pub struct PublishExpectation {
    /// Expected decoded frame count.
    pub frame_count: u64,
    /// Expected canonical sample rate.
    pub sample_rate_hz: u32,
    /// Expected total decoded channel count.
    pub channels: u32,
    /// Expected codec identifier (`"pcm"`, …) — must match the
    /// independent reader's codec observation.
    pub codec: String,
    /// Expected container family name (`"wav"`, …).
    pub container: String,
}

/// Pipeline stage a failure is attributed to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PublishStage {
    /// Staging write.
    Stage,
    /// Seal/finalize.
    Seal,
    /// Hash + metadata readback.
    Readback,
    /// Independent decode/frame proof.
    Decode,
    /// Manifest construction + consistency checks.
    Verify,
    /// Atomic commit + index CAS.
    Commit,
}

/// Typed publish failure — mapped from store errors and verification
/// contradictions alike; nothing here is recoverable into `Published`.
#[derive(Clone, Debug)]
pub enum PublishError {
    /// Store-layer failure (I/O, disk full, partial write, sync
    /// unknown, CAS conflict, denial, cancellation).
    Store(StoreError),
    /// Verification contradiction — hash mismatch, decode failure,
    /// codec/format/frame/port disagreement, lying codec class.
    Verify {
        /// Stage the contradiction was detected at.
        stage: PublishStage,
        /// Machine-readable detail.
        detail: String,
    },
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "{e}"),
            Self::Verify { stage, detail } => {
                write!(f, "publish verification failed at {stage:?}: {detail}")
            }
        }
    }
}

impl std::error::Error for PublishError {}

/// Terminal publish outcome — `Published` exists only after atomic
/// commit; every other variant left no artifact behind.
#[derive(Debug)]
pub enum PublishOutcome {
    /// Artifact committed — manifest (with verification receipt)
    /// returned as evidence.
    Published {
        /// The committed artifact identity.
        artifact: RenderArtifactId,
        /// The committed manifest.
        manifest: Box<ArtifactManifest>,
    },
    /// Typed stage failure — staging aborted, nothing committed.
    Failed {
        /// Where it died.
        stage: PublishStage,
        /// The typed failure.
        error: PublishError,
    },
    /// Cooperative cancellation at a stage boundary.
    Cancelled {
        /// `"stage"`/`"seal"`/`"readback"`/`"decode"`/`"commit"`.
        phase: &'static str,
    },
}

impl PublishOutcome {
    /// `true` only for the committed outcome.
    #[must_use]
    pub const fn is_published(&self) -> bool {
        matches!(self, Self::Published { .. })
    }
}

/// Independent decode result — canonical facts plus the decoded
/// planes used for per-port digests and optional PCM comparison.
pub struct DecodedBlob {
    /// Container family name (`"wav"`, …).
    pub container: String,
    /// Codec identifier (`"pcm"`, …).
    pub codec: String,
    /// Ruviform codec classification (lossless/lossy evidence).
    pub codec_kind: CodecId,
    /// Canonical sample rate.
    pub sample_rate_hz: u32,
    /// Canonical channel count.
    pub channels: u32,
    /// Decoded frame count.
    pub frames: u64,
    /// Canonical `f32` planar samples — one `Vec` per channel.
    pub planes: Vec<Vec<f32>>,
}

fn decode_err(detail: impl Into<String>) -> MediaError {
    MediaError::with_source(
        ErrorKind::InvalidData,
        std::io::Error::new(std::io::ErrorKind::InvalidData, detail.into()),
    )
}

/// Decodes `bytes` through the independent demux + canonical decode
/// path — the same reader contract as file readback, over an owned
/// in-memory source. Exactly one audio stream is required.
///
/// # Errors
/// [`MediaError`] — undecodable, multi-stream, non-audio or truncated
/// content.
pub fn decode_blob(bytes: &[u8]) -> Result<DecodedBlob, MediaError> {
    let demuxer = ruviform_io_symphonia::open_demuxer(Cursor::new(bytes.to_vec()))?;
    let (stream_id, codec_kind) = {
        let streams = demuxer.streams();
        let audio: Vec<_> = streams
            .iter()
            .filter(|s| s.stream.kind == StreamKind::Audio)
            .collect();
        if audio.len() != 1 {
            return Err(decode_err(format!(
                "expected exactly one audio stream, found {}",
                audio.len()
            )));
        }
        let info = audio[0];
        (info.stream.id, info.codec.codec)
    };
    let mut reader = SourceReader::open(Box::new(demuxer), stream_id, DecodeMode::Canonical)?;
    // Extract the metadata projection up front — `metadata()` borrows
    // the reader, so the values the receipt needs are copied out
    // before the mutable decode loop.
    let (container, sample_rate_hz, channels) = {
        let metadata = reader.metadata();
        let canonical = metadata.canonical;
        (
            metadata.container.format.name().to_owned(),
            canonical.sample_rate.get(),
            canonical.layout.channels(),
        )
    };
    let mut planes = vec![Vec::new(); channels as usize];
    let mut scratch = vec![vec![0.0f32; DECODE_CHUNK]; planes.len()];
    let mut frames = 0u64;
    loop {
        let mut views: Vec<&mut [f32]> = scratch.iter_mut().map(Vec::as_mut_slice).collect();
        let outcome = reader.read_planar(&mut views, DECODE_CHUNK)?;
        for (plane, src) in planes.iter_mut().zip(&scratch) {
            plane.extend_from_slice(&src[..outcome.frames]);
        }
        frames += outcome.frames as u64;
        if outcome.eof || outcome.frames == 0 {
            break;
        }
    }
    Ok(DecodedBlob {
        container,
        codec: codec_kind.name().to_owned(),
        codec_kind,
        sample_rate_hz,
        channels,
        frames,
        planes,
    })
}

fn cancelled(cancel: Option<&CancellationToken>) -> bool {
    cancel.is_some_and(CancellationToken::is_cancelled)
}

fn verify_fail(stage: PublishStage, detail: impl Into<String>) -> PublishError {
    PublishError::Verify {
        stage,
        detail: detail.into(),
    }
}

fn fail(stage: PublishStage, error: StoreError) -> PublishOutcome {
    PublishOutcome::Failed {
        stage,
        error: PublishError::Store(error),
    }
}

/// Structural draft validation — runs before any store write.
/// Returns the canonically sorted port declarations on success.
fn validate_draft(draft: &ArtifactDraft) -> Result<Vec<ArtifactPortSpec>, PublishOutcome> {
    if draft.ports.is_empty()
        || draft.ports.len() > MAX_ARTIFACT_PORTS
        || draft
            .temporal
            .interval
            .end
            .saturating_sub(draft.temporal.interval.start)
            > MAX_ARTIFACT_FRAMES
        || draft.codec.container.is_empty()
        || draft.codec.codec.is_empty()
    {
        return Err(PublishOutcome::Failed {
            stage: PublishStage::Verify,
            error: verify_fail(PublishStage::Verify, "draft violates manifest bounds"),
        });
    }
    // Ports must be canonically ordered (sorted, deduplicated) and
    // tile the decoded channel space — checked again after decode.
    let mut sorted_ports = draft.ports.clone();
    sorted_ports.sort_by_key(|p| p.port);
    if sorted_ports.windows(2).any(|w| w[0].port == w[1].port) {
        return Err(PublishOutcome::Failed {
            stage: PublishStage::Verify,
            error: verify_fail(PublishStage::Verify, "duplicate output port"),
        });
    }
    Ok(sorted_ports)
}

/// Stages `blob` through seal + independent readback + whole-digest
/// verification. Returns the staging slot, the verified readback
/// bytes and their digest — or the terminal outcome with the slot
/// already aborted.
fn stage_verified_readback(
    store: &mut dyn ArtifactStore,
    blob: &[u8],
    cancel: Option<&CancellationToken>,
) -> Result<(StagingId, Vec<u8>, ContentDigest), PublishOutcome> {
    if cancelled(cancel) {
        return Err(PublishOutcome::Cancelled { phase: "stage" });
    }
    let staging = match store.stage_begin() {
        Ok(id) => id,
        Err(e) => return Err(fail(PublishStage::Stage, e)),
    };
    for chunk in blob.chunks(STAGE_CHUNK_BYTES) {
        if let Err(e) = store.stage_write(&staging, chunk) {
            store.stage_abort(&staging);
            return Err(fail(PublishStage::Stage, e));
        }
    }
    if cancelled(cancel) {
        store.stage_abort(&staging);
        return Err(PublishOutcome::Cancelled { phase: "stage" });
    }
    if let Err(e) = store.stage_seal(&staging, blob.len() as u64) {
        store.stage_abort(&staging);
        return Err(fail(PublishStage::Seal, e));
    }
    if cancelled(cancel) {
        store.stage_abort(&staging);
        return Err(PublishOutcome::Cancelled { phase: "seal" });
    }
    let readback = match store.stage_read(&staging) {
        Ok(bytes) => bytes,
        Err(e) => {
            store.stage_abort(&staging);
            return Err(fail(PublishStage::Readback, e));
        }
    };
    let readback_digest = ContentDigest::sha256(&readback);
    if readback_digest != ContentDigest::sha256(blob) || readback.len() != blob.len() {
        store.stage_abort(&staging);
        return Err(PublishOutcome::Failed {
            stage: PublishStage::Readback,
            error: verify_fail(PublishStage::Readback, "hash mismatch on readback"),
        });
    }
    if cancelled(cancel) {
        store.stage_abort(&staging);
        return Err(PublishOutcome::Cancelled { phase: "readback" });
    }
    Ok((staging, readback, readback_digest))
}

/// Runs every decode-side expectation check — container, codec,
/// codec class, frame count, rate, channels, port tiling and the
/// optional PCM predicate.
fn check_decoded(
    decoded: &DecodedBlob,
    ports: &[ArtifactPort],
    draft: &ArtifactDraft,
    expect: &PublishExpectation,
    expect_pcm: PcmExpectRef<'_>,
) -> Result<(), PublishError> {
    let stage = PublishStage::Decode;
    if decoded.container != expect.container {
        return Err(verify_fail(
            stage,
            format!(
                "container mismatch: expected {}, decoded {}",
                expect.container, decoded.container
            ),
        ));
    }
    if decoded.codec != expect.codec || decoded.codec != draft.codec.codec {
        return Err(verify_fail(
            stage,
            format!(
                "codec mismatch: expected {}, declared {}, decoded {}",
                expect.codec, draft.codec.codec, decoded.codec
            ),
        ));
    }
    if !codec_class_consistent(draft.codec.class, &decoded.codec) {
        return Err(verify_fail(
            stage,
            format!(
                "codec class {:?} inconsistent with codec {}",
                draft.codec.class, decoded.codec
            ),
        ));
    }
    if decoded.frames != expect.frame_count {
        return Err(verify_fail(
            stage,
            format!(
                "frame count mismatch: expected {}, decoded {}",
                expect.frame_count, decoded.frames
            ),
        ));
    }
    if decoded.sample_rate_hz != expect.sample_rate_hz
        || decoded.sample_rate_hz != draft.temporal.sample_rate_hz
    {
        return Err(verify_fail(
            stage,
            format!(
                "sample rate mismatch: expected {}, decoded {}",
                expect.sample_rate_hz, decoded.sample_rate_hz
            ),
        ));
    }
    if decoded.channels != expect.channels || decoded.channels != draft.temporal.channels {
        return Err(verify_fail(
            stage,
            format!(
                "channel count mismatch: expected {}, decoded {}",
                expect.channels, decoded.channels
            ),
        ));
    }
    if !ports_tile_channels(ports, decoded.channels) {
        return Err(verify_fail(
            stage,
            "output ports do not tile decoded channels",
        ));
    }
    if let Some(expect_fn) = expect_pcm
        && !expect_fn(&decoded.planes)
    {
        return Err(verify_fail(stage, "PCM expectation refused"));
    }
    Ok(())
}

/// A fully verified manifest bundle — ID, canonical bytes and the
/// manifest itself, produced only after every measurement check.
struct VerifiedManifest {
    /// Content-addressed artifact identity.
    id: RenderArtifactId,
    /// Canonical manifest bytes (the CAS key material).
    bytes: Vec<u8>,
    /// The manifest with its verification receipt.
    manifest: ArtifactManifest,
}

/// Independent decode + expectation checks + manifest construction —
/// pure (no store access); the caller owns staging cleanup on `Err`.
fn build_verified_manifest(
    draft: &ArtifactDraft,
    sorted_ports: &[ArtifactPortSpec],
    readback: &[u8],
    readback_digest: ContentDigest,
    expect: &PublishExpectation,
    expect_pcm: PcmExpectRef<'_>,
    cancel: Option<&CancellationToken>,
) -> Result<VerifiedManifest, PublishOutcome> {
    if cancelled(cancel) {
        return Err(PublishOutcome::Cancelled { phase: "decode" });
    }
    let decoded = decode_blob(readback).map_err(|e| PublishOutcome::Failed {
        stage: PublishStage::Decode,
        error: verify_fail(PublishStage::Decode, format!("decode failed: {e}")),
    })?;
    // Per-port digests are measured from the decoded planes now — the
    // manifest records measured facts only.
    let ports: Vec<ArtifactPort> = sorted_ports
        .iter()
        .map(|spec| ArtifactPort {
            port: spec.port,
            channels: spec.channels,
            channel_offset: spec.channel_offset,
            format: spec.format,
            digest: port_digest(&decoded.planes, spec.channel_offset, spec.channels),
        })
        .collect();
    check_decoded(&decoded, &ports, draft, expect, expect_pcm).map_err(|e| {
        PublishOutcome::Failed {
            stage: PublishStage::Decode,
            error: e,
        }
    })?;
    if cancelled(cancel) {
        return Err(PublishOutcome::Cancelled { phase: "decode" });
    }
    // ---- manifest construction (all facts measured) ----
    let chunk_digests: Vec<ContentDigest> = readback
        .chunks(STAGE_CHUNK_BYTES)
        .map(ContentDigest::sha256)
        .collect();
    let manifest = ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_VERSION,
        recipe_key: draft.recipe_key,
        target: draft.target.clone(),
        snapshot: draft.snapshot.clone(),
        temporal: draft.temporal.clone(),
        cut_frames: draft.cut_frames,
        latency_frames: draft.latency_frames,
        ports,
        codec: draft.codec.clone(),
        frame_count: decoded.frames,
        byte_len: readback.len() as u64,
        content_digest: readback_digest,
        renderer: draft.renderer.clone(),
        reproducibility: draft.reproducibility,
        verification: VerificationReceipt {
            whole_digest_verified: true,
            byte_len: readback.len() as u64,
            chunk_digests,
            decode: DecodeReceipt {
                container: decoded.container.clone(),
                codec: decoded.codec.clone(),
                sample_rate_hz: decoded.sample_rate_hz,
                channels: decoded.channels,
                frames: decoded.frames,
                port_digests_verified: true,
            },
            pcm_compared: expect_pcm.is_some(),
        },
        provenance: draft.provenance.clone(),
        trust: draft.trust.clone(),
    };
    let (id, bytes) = manifest
        .artifact_id()
        .and_then(|id| manifest.canonical_bytes().map(|b| (id, b)))
        .map_err(|e| PublishOutcome::Failed {
            stage: PublishStage::Verify,
            error: verify_fail(PublishStage::Verify, format!("manifest encode: {e}")),
        })?;
    Ok(VerifiedManifest {
        id,
        bytes,
        manifest,
    })
}

/// Atomic commit + index CAS — the final stage; aborts the staging
/// slot on any failure.
fn commit_verified(
    store: &mut dyn ArtifactStore,
    staging: StagingId,
    draft: &ArtifactDraft,
    readback_digest: ContentDigest,
    verified: VerifiedManifest,
    reference: Option<ArtifactReference>,
    cancel: Option<&CancellationToken>,
) -> PublishOutcome {
    if cancelled(cancel) {
        store.stage_abort(&staging);
        return PublishOutcome::Cancelled { phase: "commit" };
    }
    let expected_index = match store.index_entries(&draft.recipe_key) {
        Ok(entries) => entries,
        Err(e) => {
            store.stage_abort(&staging);
            return fail(PublishStage::Commit, e);
        }
    };
    let commit = PublishCommit {
        staging,
        blob_digest: readback_digest,
        manifest_id: verified.id,
        manifest_bytes: verified.bytes,
        recipe_key: draft.recipe_key,
        expected_index,
        reference,
    };
    match store.commit_publish(&commit) {
        Ok(_ack) => PublishOutcome::Published {
            artifact: verified.id,
            manifest: Box::new(verified.manifest),
        },
        Err(e) => {
            // Commit consumed or rejected the slot; abort is a no-op
            // when the slot is already gone — best-effort either way.
            store.stage_abort(&staging);
            fail(PublishStage::Commit, e)
        }
    }
}

/// Runs the fixed verified-publish pipeline against `store`.
///
/// `blob` is the rendered output bytes; `expect` declares what they
/// must decode as; `expect_pcm` optionally compares decoded planes;
/// `cancel` is honored at every stage boundary; `reference` is attached
/// atomically with the commit when supplied.
///
/// On any failure the staging slot is aborted and the outcome is
/// `Failed`/`Cancelled` — old verified artifacts are never touched.
pub fn publish_artifact(
    store: &mut dyn ArtifactStore,
    draft: &ArtifactDraft,
    blob: &[u8],
    expect: &PublishExpectation,
    expect_pcm: PcmExpectRef<'_>,
    cancel: Option<&CancellationToken>,
    reference: Option<ArtifactReference>,
) -> PublishOutcome {
    let sorted_ports = match validate_draft(draft) {
        Ok(ports) => ports,
        Err(outcome) => return outcome,
    };
    let (staging, readback, readback_digest) = match stage_verified_readback(store, blob, cancel) {
        Ok(parts) => parts,
        Err(outcome) => return outcome,
    };
    let verified = match build_verified_manifest(
        draft,
        &sorted_ports,
        &readback,
        readback_digest,
        expect,
        expect_pcm,
        cancel,
    ) {
        Ok(verified) => verified,
        Err(outcome) => {
            store.stage_abort(&staging);
            return outcome;
        }
    };
    commit_verified(
        store,
        staging,
        draft,
        readback_digest,
        verified,
        reference,
        cancel,
    )
}
