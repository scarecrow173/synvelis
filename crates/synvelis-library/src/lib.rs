//! `synvelis-library` — the typed asset-provider boundary (ALIB-01,
//! Synvelis #85) and the streaming verified-import pipeline (ALIB-02,
//! Synvelis #86).
//!
//! This crate owns the common contract through which the app browses,
//! observes, pins, and fetches provider assets without depending on
//! Remote/VCS backends. Baseline providers:
//!
//! - [`LocalFolderProvider`] — a canonical-bounded filesystem root
//!   with traversal/loop protection and content-hash pinning.
//! - [`ProjectAssetsProvider`] — a read-only projection of the live
//!   `ProjectController`'s asset registry backed by a blob-store port.
//!
//! Identity is strictly separated: [`ProviderId`]/[`OpaqueLocator`] are
//! provider-layer types and never interchangeable with the project
//! aggregate's `AssetId`/`SourceId`/`ImportId`. Pins are typed
//! evidence ([`PinProof::ImmutableObject`] vs
//! [`PinProof::ConditionalStrong`]); metadata is typed observation
//! ([`AssetObservation`]) — never fabricated.
//!
//! The contract is blocking and RT-free: no Tokio, HTTP SDK, Git SDK,
//! or GUI types appear in the public API.
//!
//! ALIB-02 adds [`import_verified`]: a bounded-memory pipeline that
//! streams a [`PinnedAssetRef`] into a staging file while measuring
//! SHA-256, verifies the staged bytes by readback, publishes into the
//! `blobs/sha256/<64hex>` namespace through the [`BlobPublisher`] port
//! (the `ruviform-io-file` staged-publication primitive in
//! production), and commits asset + source + applied-import (+
//! optional clip) atomically at the manifest-pinned project revision.
//! [`ImportManifest`] is the durable job record driving crash
//! recovery; [`rebase_import`], [`inspect_imports`], and
//! [`gc_orphans`] are the explicit recovery surface. `CommittedLive`
//! never implies `SavedRevision`/`HEAD` — saving stays the caller's
//! `ProjectController::save` concern.

mod browse;
mod cache;
mod capability;
mod catalog;
mod context;
mod digest;
mod error;
mod git;
mod id;
mod import;
mod local;
mod notify;
mod observation;
mod preview;
mod project;
mod provider;
mod refs;
mod registry;
mod webdav;

pub use browse::{
    BrowseEntry, BrowsePage, BrowseRequest, BrowseScope, Coverage, EntryKind, FilterSpec,
    OpaquePageCursor, SearchQuery, SortDirection, SortField, SortSpec,
};
pub use cache::{CacheError, CachePolicy, CacheStats, LibraryCache};
pub use capability::{
    AccessMode, Capability, PinCapability, ProviderCapabilities, UnsupportedReason,
};
pub use catalog::{
    CatalogCacheKey, CatalogGeneration, CatalogQuery, IndexCompleteness, IndexPartialReason,
    IndexStatus,
};
pub use context::{
    CancellationToken, Clock, CredentialResolver, FixedClock, ProviderContext, ProviderLimits,
    SystemClock,
};
pub use error::{ProviderError, ProviderErrorKind, RetryAdvice};
pub use git::{
    GitObjectFormat, GitProviderInfo, GitRemoteConfig, GitRemoteProvider, GitRemoteScheme,
    GitRemoteUrl, GitVcsConfig, GitVcsProvider, LfsBackendKind, MirrorMode,
    UnsupportedFilterPolicy,
};
pub use id::{CredentialKey, OpaqueCredential, OpaqueLocator, PrincipalId, ProviderId};
pub use import::{
    BlobImportReceipt, BlobMaterialization, BlobPublishError, BlobPublishFailure,
    BlobPublishReceipt, BlobPublisher, BlobRetainReason, CommitReceipt, CommitVerifiedImport,
    DirectorySync, ImportError, ImportIntent, ImportJobInfo, ImportManifest, ImportOutcome,
    ImportStage, ImportStore, ImportTarget, OrphanGcReport, PublishDurability, StagedBlobPublisher,
    StagingRetainReason, gc_orphans, import_verified, inspect_imports, rebase_import,
};
pub use local::LocalFolderProvider;
pub use notify::{
    LibraryEvent, LibraryEventKind, LibraryEventSink, LibraryGeneration, RetireReason,
    RtEventQueue, SuppressingSink,
};
pub use observation::{
    AssetKind, AssetObservation, AssetStat, Availability, EntryFacts, InferenceMethod, MediaFacts,
    ObservationOrigin, ObservedAt, ProvenanceRecord, StaleReason, UnavailableReason,
};
pub use preview::{
    PreviewCacheKey, PreviewFormat, PreviewLease, PreviewMeta, PreviewPlan, PreviewRequest,
};
#[doc(hidden)]
pub use project::PreTraverseHook;
pub use project::{
    BlobObject, LocalBlobDir, ProjectAssetRecord, ProjectAssetsProvider, ProjectBlobStore,
    ProjectCatalog, ProjectCatalogSnapshot, ProjectSourceRef,
};
pub use provider::{
    AssetProvider, ByteRange, FetchClaims, FetchCompletion, FetchReceipt, FetchSlot, PinnedRead,
    VerificationPolicy,
};
pub use refs::{
    ContentDigest, DigestAlgorithm, ObjectIdentity, PinProof, PinnedAssetRef, PreconditionKind,
    RemoteAssetRef, RevisionKey, StrongRevisionToken,
};
pub use registry::{ProviderDescriptor, ProviderKind, ProviderRegistry};
pub use webdav::{WebDavConfig, WebDavProvider, WebDavScheme, WebDavUrl};
