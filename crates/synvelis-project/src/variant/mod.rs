//! Creative Branch variant model, engine and adapter boundary
//! (synvelis#278, CB-01 headless slice).
//!
//! [`VariantSet`]/[`Branch`] are the Project's versioned,
//! project-authoritative production candidates. All mutations go
//! through [`ProjectController`] as single-writer CAS operations;
//! candidate payloads are immutable, digest-pinned references held by a
//! [`BranchContentStore`] — artifact blobs are never embedded in the
//! project snapshot.
//!
//! This slice ships the typed model, schema v14 persistence, the
//! [`DomainAdapter`] contract with a [`FakeDomainAdapter`] fixture, and
//! the Fork/Edit/Adopt/Discard engine. Real domain adapters, the
//! comparison orchestrator, GUI wiring and runtime integration are
//! later issues' scope.

mod adapter;
pub(crate) mod engine;
mod error;
pub(crate) mod scope;
mod store;
mod types;

pub use adapter::{DomainAdapter, FakeDomainAdapter};
pub use engine::{
    AdoptSources, BranchEditIntent, FakeRuntimePublish, NoopRuntimePublish, adopt_branch,
    discard_branch, edit_branch, fork_variant_set,
};
pub use error::VariantError;
pub use scope::scope_digest;
pub use store::{BranchContentStore, InMemoryBranchContentStore, decode_candidate};
pub use types::{
    AdoptIntent, AdoptOutcome, AdoptReceipt, AdoptRecord, ArrangementDomainMask, Branch,
    BranchContentRef, BranchLineage, BranchRevision, DiscardReceipt, EditReceipt, ForkReceipt,
    ProcessingRangeRef, ProjectCommitEvidence, RuntimePublish, RuntimePublishEvidence,
    RuntimePublishRequest, RuntimePublishState, StripDomainMask, StripField, TakeUsageTarget,
    VariantScope, VariantSet, VariantSetRevision,
};
