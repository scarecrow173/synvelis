//! Semantic Diff Engine (VCS-02, synvelis#93).
//!
//! Pure, non-RT comparison of two VCS-01 [`crate::snapshot::ImmutableSnapshot`]s
//! into one canonical [`ChangeSet`]. The result is deterministic, sorted,
//! and reusable by every view/provider — views must not recompute diffs
//! independently.
//!
//! The engine distinguishes project-semantic changes from Git text diffs,
//! file-tree differences, PCM/render differences, and runtime graph
//! changes. `Unknown` is a first-class outcome: missing assets, opaque
//! plugin state, migration-projected regions, and ambiguous object
//! identity are reported as [`ComparisonIssue`]s and never coerced into
//! `Equal`, zero changes, or a rename.

mod adapters;
mod engine;
mod types;

pub use engine::{ChangeSetCache, compare_snapshots};
pub use types::{
    AmbiguityReason, ChangeEvidence, ChangeId, ChangeKind, ChangeSet, ChangeSetSummary,
    ComparisonBudget, ComparisonCompleteness, ComparisonDomain, ComparisonDomains,
    ComparisonEquality, ComparisonIssue, ComparisonKey, ComparisonSpec, DIFF_POLICY_VERSION,
    DiffError, LineageRef, ObjectKind, SemanticChange, SemanticFieldDiff, SemanticObjectRef,
    SemanticTimeRange, SemanticValueRef, SnapshotSide,
};
