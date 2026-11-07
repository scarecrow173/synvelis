//! `synvelis-library` — the typed asset-provider boundary (ALIB-01,
//! Synvelis #85).
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

mod browse;
mod capability;
mod context;
mod digest;
mod error;
mod id;
mod local;
mod observation;
mod project;
mod provider;
mod refs;
mod registry;

pub use browse::{
    BrowseEntry, BrowsePage, BrowseRequest, BrowseScope, Coverage, EntryKind, FilterSpec,
    OpaquePageCursor, SearchQuery, SortDirection, SortField, SortSpec,
};
pub use capability::{
    AccessMode, Capability, PinCapability, ProviderCapabilities, UnsupportedReason,
};
pub use context::{
    CancellationToken, Clock, CredentialResolver, FixedClock, ProviderContext, ProviderLimits,
    SystemClock,
};
pub use error::{ProviderError, ProviderErrorKind, RetryAdvice};
pub use id::{CredentialKey, OpaqueCredential, OpaqueLocator, PrincipalId, ProviderId};
pub use local::LocalFolderProvider;
pub use observation::{
    AssetKind, AssetObservation, AssetStat, Availability, EntryFacts, InferenceMethod, MediaFacts,
    ObservationOrigin, ObservedAt, ProvenanceRecord, StaleReason, UnavailableReason,
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
