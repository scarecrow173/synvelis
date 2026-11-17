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
mod arrangement;
mod audio;
pub mod compare;
pub(crate) mod engine;
mod error;
mod instrument;
mod midi_clip;
mod processing_range;
mod runtime;
pub(crate) mod scope;
mod store;
mod strip;
mod types;

pub use adapter::{BranchEditOp, DomainAdapter, FakeDomainAdapter};
pub use arrangement::{
    ArrangementAdapter, ArrangementBoundaryDependency, ArrangementBoundaryReason,
    ArrangementCapabilities,
};
pub use audio::{AudioClipAdapter, AudioClipCapabilities, ProcessingCapability, TakeUsageAdapter};
pub use compare::{
    AlignmentPolicy, BranchRevisionRef, CacheStatus, CompareError, CompareSide,
    ComparisonFreshness, ComparisonOrchestrator, ComparisonResult, ComparisonSession,
    EvaluationContext, EvidenceOrigin, EvidenceReason, LayerState, MonitoringPolicy,
    PerceptualLayerResult, SemanticLayerResult, SessionOptions, SignalAlignment,
    SignalCompareRequest, SignalComparisonProvider, SignalDomain, SignalLayerResult, SignalMetric,
    SignalProviderReport, StaleDimension,
};
pub use engine::{
    AdoptSources, BranchEditIntent, BranchEditOpIntent, FakeRuntimePublish, NoopRuntimePublish,
    add_branch, adopt_branch, adopt_branch_adapted, discard_branch, edit_branch, edit_branch_op,
    fork_variant_set,
};
pub use error::VariantError;
pub use instrument::{
    InstrumentSlotAdapter, InstrumentSlotCapabilities, InstrumentStateAssessment,
};
pub use midi_clip::{MidiClipAdapter, MidiClipCapabilities};
pub use processing_range::{
    ProcessingBoundaryDependency, ProcessingRangeAdapter, ProcessingRangeCapabilities,
};
pub use runtime::JournaledRuntimePublish;
pub use scope::scope_digest;
pub use store::{
    BranchContentStore, DirectoryBranchContentStore, InMemoryBranchContentStore, decode_candidate,
};
pub use strip::{StripBoundaryDependency, StripFieldsAdapter, StripFieldsCapabilities};
pub use types::{
    AddBranchReceipt, AdoptIntent, AdoptOutcome, AdoptReceipt, AdoptRecord, ArrangementDomainMask,
    Branch, BranchContentRef, BranchLineage, BranchRevision, DiscardReceipt, EditReceipt,
    ForkReceipt, ProcessingRangeRef, ProjectCommitEvidence, RuntimePublish, RuntimePublishEvidence,
    RuntimePublishRequest, RuntimePublishState, StripDomainMask, StripField, TakeUsageTarget,
    VariantScope, VariantSet, VariantSetRevision,
};
