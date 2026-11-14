//! Immutable render artifacts — RCP-02 (synvelis#291).
//!
//! Render outputs are verified, content-addressed artifacts, not
//! mutable cache entries: an [`ArtifactManifest`] binds a
//! [`crate::recipe::RecipeKey`] to measured media facts (digest,
//! ports, frames, codec, temporal domain), renderer provenance,
//! reproducibility and trust metadata, plus a verification receipt.
//! Publishing is a fixed stage → seal → readback/digest/decode →
//! atomic commit pipeline ([`publish_artifact`]); lookup is a
//! deterministic eligibility evaluation ([`lookup`]); GC is
//! typed-root mark/sweep with grace and tombstones ([`sweep`]);
//! shared manifests/blobs are re-verified fail-closed before
//! registration ([`register_remote`]).

pub mod gc;
pub mod lookup;
pub mod manifest;
pub mod publish;
pub mod share;
pub mod store;

pub use gc::{GcPolicy, GcReport, sweep};
pub use lookup::{
    CacheLookup, CacheLookupResult, CandidateReport, FitRejection, IntendedUse, LookupOutcome,
    TrustScope, lookup,
};
pub use manifest::{
    ARTIFACT_HASH_ALGORITHM, ARTIFACT_ID_DOMAIN, ARTIFACT_MANIFEST_VERSION, AccessClass,
    ArtifactManifest, ArtifactPort, ArtifactPortSpec, ArtifactProvenance, ArtifactReproducibility,
    ArtifactTrust, CodecClass, CodecDescriptor, DecodeReceipt, MAX_ARTIFACT_FRAMES,
    MAX_ARTIFACT_PORTS, MAX_BLOB_BYTES, MAX_MANIFEST_BYTES, ManifestError, RenderArtifactId,
    STAGE_CHUNK_BYTES, VerificationReceipt, codec_class_consistent, decode_manifest, port_digest,
    ports_tile_channels, verify_manifest,
};
pub use publish::{
    ArtifactDraft, DecodedBlob, PublishError, PublishExpectation, PublishOutcome, PublishStage,
    decode_blob, publish_artifact,
};
pub use share::{
    ExternalManifestRef, RemoteArtifact, ShareRegistration, ShareRejection,
    attach_external_manifest, register_remote,
};
pub use store::{
    ArtifactRecord, ArtifactReference, ArtifactStore, DeleteOutcome, ExternalManifestEntry,
    FaultPoint, InMemoryArtifactStore, Materialization, PublishAck, PublishCommit, RefState,
    ReferenceRecord, StagingId, StoreError, valid_locator,
};
