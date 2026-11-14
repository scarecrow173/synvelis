//! Injectable artifact store — blob CAS, manifest CAS, recipe index,
//! GC references and the external render-manifest registry
//! (synvelis#291, RCP-02).
//!
//! The trait is deliberately byte-oriented and filesystem/OS-free so
//! the whole publish/lookup/GC contract stays headless-testable. A
//! real filesystem store (RCP-02 follow-up gate) implements the same
//! contract; nothing in this module assumes paths, permissions or a
//! particular durability model — those surface as [`StoreError`]
//! variants the publish pipeline already classifies.
//!
//! Namespace separation is structural: this store only ever holds
//! *derived render artifacts* (manifests + their blobs). Raw
//! Source/Take/verified-recording bytes live in a different store
//! (`synvelis-library` blob namespace) and are unreachable from —
//! and therefore uncollectable by — artifact GC.
//!
//! Atomicity contract: [`ArtifactStore::commit_publish`] applies blob
//! + manifest + index + reference as **one** unit. Any failure leaves
//!   previously published verified artifacts untouched and never
//!   produces a partially visible artifact.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};

use synvelis_project::{BranchId, ProjectId, VariantSetId, snapshot::ContentDigest};

use crate::artifact::manifest::{RenderArtifactId, verify_manifest};
use crate::recipe::RecipeKey;

// ---------------------------------------------------------------------------
// errors / staging
// ---------------------------------------------------------------------------

/// Typed store failure — every variant maps to a fail-closed publish
/// or lookup outcome; none is silently recoverable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// Generic I/O failure.
    Io {
        /// Diagnostic detail.
        detail: String,
    },
    /// Storage exhausted — publish must abort, old artifacts kept.
    DiskFull,
    /// Staged length did not match the declared length at seal —
    /// partial write detected before any commit.
    PartialWrite {
        /// Diagnostic detail.
        detail: String,
    },
    /// Durability could not be confirmed (sync/flush unknown) —
    /// fail closed rather than assume the write survived.
    SyncUnknown,
    /// CAS precondition failed (index moved, conflicting writer,
    /// quarantined identity) — the write is rejected, prior state kept.
    Conflict {
        /// Diagnostic detail.
        detail: String,
    },
    /// Access/ACL denied by the store or trust policy.
    Denied {
        /// Machine-readable reason.
        reason: String,
    },
    /// Caller cancelled cooperatively.
    Cancelled,
    /// Store-internal integrity contradiction (digest mismatch inside
    /// the store's own records) — never served as valid.
    Integrity {
        /// Diagnostic detail.
        detail: String,
    },
    /// Referenced object absent (staging slot, manifest, …).
    NotFound {
        /// Diagnostic detail.
        detail: String,
    },
    /// Operation outside the store's declared capability.
    Unsupported {
        /// Diagnostic detail.
        detail: String,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { detail } => write!(f, "store I/O failure: {detail}"),
            Self::DiskFull => write!(f, "store disk full"),
            Self::PartialWrite { detail } => write!(f, "partial staged write: {detail}"),
            Self::SyncUnknown => write!(f, "store sync/durability unknown"),
            Self::Conflict { detail } => write!(f, "store conflict: {detail}"),
            Self::Denied { reason } => write!(f, "store access denied: {reason}"),
            Self::Cancelled => write!(f, "operation cancelled"),
            Self::Integrity { detail } => write!(f, "store integrity failure: {detail}"),
            Self::NotFound { detail } => write!(f, "store object not found: {detail}"),
            Self::Unsupported { detail } => write!(f, "store operation unsupported: {detail}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Identity of one staging slot — store-assigned, single-use.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StagingId(pub u64);

/// Fault-injection points — the in-memory store consumes one queued
/// [`StoreError`] per armed point, letting tests drive every publish
/// failure class without a real filesystem.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FaultPoint {
    /// `stage_write`.
    StageWrite,
    /// `stage_seal`.
    StageSeal,
    /// `stage_read` (readback).
    StageRead,
    /// `commit_publish` blob materialization.
    CommitBlob,
    /// `commit_publish` manifest materialization.
    CommitManifest,
    /// `commit_publish` index CAS.
    CommitIndex,
    /// `read_blob`.
    ReadBlob,
    /// `read_manifest`.
    ReadManifest,
    /// `attach_reference` / `release_reference`.
    ReferenceWrite,
    /// `attach_external_manifest` / `detach_external_manifest`.
    ExternalWrite,
    /// `delete_artifact` (GC sweep).
    DeleteArtifact,
}

// ---------------------------------------------------------------------------
// publish commit / records
// ---------------------------------------------------------------------------

/// Whether a CAS object was newly stored or already present
/// (content-identical reuse — blob dedup never implies recipe
/// identity).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Materialization {
    /// Newly written.
    Stored,
    /// Already present with identical content — idempotent reuse.
    Reused,
}

/// One atomic publish commit: staged blob bytes → content-addressed
/// blob, manifest bytes → artifact-ID key, index CAS update, optional
/// initial GC reference. All-or-nothing.
#[derive(Clone, Debug)]
pub struct PublishCommit {
    /// Sealed staging slot holding the blob bytes.
    pub staging: StagingId,
    /// `SHA-256` of the blob bytes (the publisher measured them).
    pub blob_digest: ContentDigest,
    /// Artifact ID the manifest bytes hash to.
    pub manifest_id: RenderArtifactId,
    /// Canonical manifest bytes.
    pub manifest_bytes: Vec<u8>,
    /// Recipe key the index entry is filed under.
    pub recipe_key: RecipeKey,
    /// CAS precondition: the exact current index entry list for
    /// `recipe_key` observed before publish. Any drift → `Conflict`.
    pub expected_index: Vec<RenderArtifactId>,
    /// Optional initial GC reference (e.g. the in-flight job or live
    /// branch that owns this artifact) attached atomically.
    pub reference: Option<ArtifactReference>,
}

/// Acknowledgement of a committed publish.
#[derive(Clone, Debug)]
pub struct PublishAck {
    /// Blob materialization (dedup-aware).
    pub blob: Materialization,
    /// Manifest materialization.
    pub manifest: Materialization,
    /// Resulting index entry count for the recipe.
    pub index_len: usize,
}

/// Outcome of an artifact deletion during GC.
#[derive(Clone, Debug, Default)]
pub struct DeleteOutcome {
    /// Manifest record removed.
    pub manifest_removed: bool,
    /// Index entries the artifact was removed from.
    pub index_entries_removed: usize,
    /// Blob digest freed because no remaining manifest references it.
    pub blob_freed: Option<ContentDigest>,
    /// GC references dropped with the artifact.
    pub references_dropped: usize,
    /// External-manifest entries dropped with the artifact.
    pub external_entries_dropped: usize,
}

/// A GC root's claim on an artifact — the typed reference vocabulary
/// from the RCP-00 contract. References carry no timestamps of their
/// own; release state is store-side.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ArtifactReference {
    /// A live project branch's render-manifest attachment.
    LiveBranch {
        /// Owning project.
        project_id: ProjectId,
        /// Variant set the branch belongs to.
        variant_set: VariantSetId,
        /// Branch identity.
        branch: BranchId,
    },
    /// A pinned branch-history/VCS-associated manifest attachment.
    BranchHistory {
        /// Owning project.
        project_id: ProjectId,
        /// Variant set the branch belongs to.
        variant_set: VariantSetId,
        /// Branch identity.
        branch: BranchId,
    },
    /// A published Share Package entry.
    SharePackage {
        /// Share package identity.
        package_id: String,
    },
    /// A Comparison Session pin.
    ComparisonSession {
        /// Session identity.
        session_id: String,
    },
    /// An active Playback Reader lease.
    PlaybackLease {
        /// Lease identity.
        lease_id: String,
    },
    /// An in-flight render job.
    InFlightJob {
        /// Job identity.
        job_id: String,
    },
    /// An explicit retention pin.
    RetentionPin {
        /// Pin identity.
        pin_id: String,
    },
    /// A pinned VCS snapshot the artifact was rendered against —
    /// provider-independent locator + snapshot digest binding.
    PinnedSnapshot {
        /// Opaque provider-independent locator.
        locator: String,
        /// Digest of the pinned snapshot.
        snapshot_digest: ContentDigest,
    },
}

/// Reference liveness — `Pending` models a remote/offline pin whose
/// reachability cannot be confirmed: GC treats it as live
/// (conservative keep) until resolved.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RefState {
    /// Confirmed live root.
    Active,
    /// Root released at the recorded store tick — grace runs from here.
    Released {
        /// Store tick of release.
        at: u64,
    },
    /// Remote/offline pin unresolved — conservatively live.
    Pending,
}

/// One stored reference with its liveness state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceRecord {
    /// The root reference.
    pub reference: ArtifactReference,
    /// Liveness state.
    pub state: RefState,
}

/// Post-commit external render-manifest attachment — the RCP-02 model
/// for "manifest attached after the VCS commit": a provider-independent
/// locator + snapshot digest binding that never rewrites commit
/// history (the commit object itself is never touched).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalManifestEntry {
    /// Opaque provider-independent commit/revision locator.
    pub locator: String,
    /// Digest of the snapshot the commit pinned.
    pub snapshot_digest: ContentDigest,
    /// Attached artifact.
    pub artifact: RenderArtifactId,
    /// Store tick the attachment was recorded at.
    pub attached_tick: u64,
}

/// Enumerated artifact record for GC/inspection — manifest bytes are
/// returned raw; callers verify through [`verify_manifest`].
#[derive(Clone, Debug)]
pub struct ArtifactRecord {
    /// Artifact identity (store key).
    pub id: RenderArtifactId,
    /// Canonical manifest bytes as stored.
    pub manifest_bytes: Vec<u8>,
    /// Whether the content-addressed blob is present.
    pub blob_present: bool,
    /// Store tick the artifact was committed at.
    pub published_tick: u64,
    /// Whether the artifact is quarantined (integrity-suspect — never
    /// served as valid).
    pub quarantined: bool,
}

// ---------------------------------------------------------------------------
// the trait
// ---------------------------------------------------------------------------

/// The injectable artifact store contract.
///
/// Read methods take `&self`; mutation methods take `&mut self`. All
/// methods are object-safe — headless tests use
/// [`InMemoryArtifactStore`], a filesystem store implements the same
/// surface later without contract changes.
pub trait ArtifactStore {
    /// Opens a new staging slot.
    ///
    /// # Errors
    /// [`StoreError::DiskFull`]/[`StoreError::Io`]/fault injection.
    fn stage_begin(&mut self) -> Result<StagingId, StoreError>;

    /// Appends bytes to a staging slot. Partial application is allowed
    /// — [`Self::stage_seal`] is the integrity gate.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] unknown slot,
    /// [`StoreError::DiskFull`]/[`StoreError::Io`]/fault injection.
    fn stage_write(&mut self, staging: &StagingId, bytes: &[u8]) -> Result<(), StoreError>;

    /// Seals a staging slot after verifying its length equals
    /// `expected_len` — the partial-write gate.
    ///
    /// # Errors
    /// [`StoreError::NotFound`], [`StoreError::PartialWrite`],
    /// [`StoreError::SyncUnknown`]/fault injection.
    fn stage_seal(&mut self, staging: &StagingId, expected_len: u64) -> Result<(), StoreError>;

    /// Reads back the sealed staging bytes — the independent readback
    /// surface the publish pipeline hashes and decodes.
    ///
    /// # Errors
    /// [`StoreError::NotFound`], [`StoreError::Io`]/fault injection.
    fn stage_read(&self, staging: &StagingId) -> Result<Vec<u8>, StoreError>;

    /// Aborts a staging slot, discarding partial content. Never fails
    /// the caller — cleanup is best-effort.
    fn stage_abort(&mut self, staging: &StagingId);

    /// Atomically commits blob + manifest + index CAS + optional
    /// reference. See the module docs for the all-or-nothing contract.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] (staging), [`StoreError::Integrity`]
    /// (content-addressing violation), [`StoreError::Conflict`] (CAS
    /// precondition or quarantined identity),
    /// [`StoreError::DiskFull`]/[`StoreError::SyncUnknown`]/fault
    /// injection. On any error no state changes.
    fn commit_publish(&mut self, commit: &PublishCommit) -> Result<PublishAck, StoreError>;

    /// Reads content-addressed blob bytes.
    ///
    /// # Errors
    /// [`StoreError::Io`]/fault injection.
    fn read_blob(&self, digest: &ContentDigest) -> Result<Option<Vec<u8>>, StoreError>;

    /// Reads manifest bytes by artifact ID.
    ///
    /// # Errors
    /// [`StoreError::Io`]/fault injection.
    fn read_manifest(&self, id: &RenderArtifactId) -> Result<Option<Vec<u8>>, StoreError>;

    /// Current index entries for a recipe key, canonically ordered.
    ///
    /// # Errors
    /// [`StoreError::Io`]/fault injection.
    fn index_entries(&self, recipe: &RecipeKey) -> Result<Vec<RenderArtifactId>, StoreError>;

    /// Enumerates all artifact records (published + quarantined) for
    /// GC and inspection.
    ///
    /// # Errors
    /// [`StoreError::Io`]/fault injection.
    fn artifacts(&self) -> Result<Vec<ArtifactRecord>, StoreError>;

    /// Attaches (or re-activates) a GC reference on an artifact.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] unknown artifact,
    /// [`StoreError::Conflict`] quarantined artifact,
    /// fault injection.
    fn attach_reference(
        &mut self,
        artifact: &RenderArtifactId,
        reference: ArtifactReference,
    ) -> Result<(), StoreError>;

    /// Releases a GC reference at the current store tick.
    ///
    /// # Errors
    /// [`StoreError::NotFound`]/fault injection.
    fn release_reference(
        &mut self,
        artifact: &RenderArtifactId,
        reference: &ArtifactReference,
    ) -> Result<(), StoreError>;

    /// Marks a reference `Pending` — a remote/offline pin whose
    /// reachability cannot be confirmed; GC keeps the artifact
    /// conservatively until the pin resolves.
    ///
    /// # Errors
    /// [`StoreError::NotFound`]/fault injection.
    fn pend_reference(
        &mut self,
        artifact: &RenderArtifactId,
        reference: &ArtifactReference,
    ) -> Result<(), StoreError>;

    /// Lists an artifact's references with liveness state.
    ///
    /// # Errors
    /// [`StoreError::Io`]/fault injection.
    fn references(&self, artifact: &RenderArtifactId) -> Result<Vec<ReferenceRecord>, StoreError>;

    /// Records a post-commit external render-manifest attachment —
    /// idempotent; the commit locator/digest are inputs and are never
    /// mutated (commit SHA invariance by construction).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] unknown artifact,
    /// [`StoreError::Conflict`] quarantined artifact or invalid
    /// locator, fault injection.
    fn attach_external_manifest(&mut self, entry: ExternalManifestEntry) -> Result<(), StoreError>;

    /// Removes an external-manifest attachment.
    ///
    /// # Errors
    /// [`StoreError::NotFound`]/fault injection.
    fn detach_external_manifest(
        &mut self,
        locator: &str,
        snapshot_digest: &ContentDigest,
        artifact: &RenderArtifactId,
    ) -> Result<(), StoreError>;

    /// Lists external-manifest attachments for a commit locator +
    /// snapshot digest pair.
    ///
    /// # Errors
    /// [`StoreError::Io`]/fault injection.
    fn external_manifests(
        &self,
        locator: &str,
        snapshot_digest: &ContentDigest,
    ) -> Result<Vec<ExternalManifestEntry>, StoreError>;

    /// Whether any external-manifest attachment references the
    /// artifact — the GC reachability probe for commit-associated
    /// roots.
    ///
    /// # Errors
    /// [`StoreError::Io`]/fault injection.
    fn has_external_manifest(&self, artifact: &RenderArtifactId) -> Result<bool, StoreError>;

    /// Whether an artifact ID is quarantined — quarantined artifacts
    /// are never served as valid by lookup.
    fn is_quarantined(&self, id: &RenderArtifactId) -> bool;

    /// Deletes an artifact unconditionally — GC sweep's primitive.
    /// Removes manifest, index entries, references, external entries
    /// and the blob when no remaining manifest references its digest;
    /// writes a tombstone. Does not check reachability — that is the
    /// sweeper's job.
    ///
    /// # Errors
    /// [`StoreError::NotFound`]/fault injection.
    fn delete_artifact(&mut self, id: &RenderArtifactId) -> Result<DeleteOutcome, StoreError>;

    /// Tombstone tick for a deleted artifact, when present.
    fn tombstone(&self, id: &RenderArtifactId) -> Option<u64>;

    /// Current store logical clock (monotonic ticks — GC grace,
    /// publish/attach stamps). Never wall time.
    fn now(&self) -> u64;

    /// Advances the logical clock — test/driver-controlled.
    fn advance_clock(&mut self, ticks: u64);
}

// ---------------------------------------------------------------------------
// in-memory implementation
// ---------------------------------------------------------------------------

struct StagingSlot {
    bytes: Vec<u8>,
    sealed: bool,
}

/// Headless in-memory [`ArtifactStore`] — the reference
/// implementation for the RCP-02 contract tests. All state is
/// byte-exact; fault injection via [`Self::arm_fault`] drives every
/// publish failure class.
#[derive(Default)]
pub struct InMemoryArtifactStore {
    clock: u64,
    next_staging: u64,
    staging: BTreeMap<StagingId, StagingSlot>,
    blobs: BTreeMap<ContentDigest, Vec<u8>>,
    /// blob digest → artifacts referencing it (CAS refcount).
    blob_refs: BTreeMap<ContentDigest, BTreeSet<RenderArtifactId>>,
    manifests: BTreeMap<RenderArtifactId, Vec<u8>>,
    index: BTreeMap<RecipeKey, BTreeSet<RenderArtifactId>>,
    references: BTreeMap<RenderArtifactId, BTreeMap<ArtifactReference, RefState>>,
    published: BTreeMap<RenderArtifactId, u64>,
    tombstones: BTreeMap<RenderArtifactId, u64>,
    quarantine: BTreeMap<RenderArtifactId, (String, u64)>,
    external: BTreeMap<(String, ContentDigest), BTreeSet<RenderArtifactId>>,
    external_entries: BTreeMap<(String, ContentDigest), Vec<ExternalManifestEntry>>,
    // Interior mutability lets `&self` read paths consume armed faults
    // and the one-shot readback-corruption flag without changing the
    // trait's read/write receiver split.
    faults: RefCell<BTreeMap<FaultPoint, VecDeque<StoreError>>>,
    corrupt_next_readback: Cell<bool>,
}

impl InMemoryArtifactStore {
    /// A fresh empty store at logical tick 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues a one-shot fault consumed by the next operation at
    /// `point` — fault-injection seam for failure-class tests.
    pub fn arm_fault(&mut self, point: FaultPoint, error: StoreError) {
        self.faults
            .borrow_mut()
            .entry(point)
            .or_default()
            .push_back(error);
    }

    /// Test hook: the next [`ArtifactStore::stage_read`] flips one byte
    /// of the staged content — simulates a corrupt readback so the
    /// publish pipeline's hash check must fail closed.
    pub fn corrupt_next_readback(&mut self) {
        self.corrupt_next_readback.set(true);
    }

    /// Quarantines an artifact identity — integrity-suspect records are
    /// never served by lookup. Used when a conflicting/corrupt write
    /// is detected.
    pub fn quarantine(&mut self, id: &RenderArtifactId, reason: &str) {
        self.quarantine.insert(*id, (reason.to_owned(), self.clock));
    }

    /// Test/diagnostic: number of distinct blob objects stored.
    #[must_use]
    pub fn blob_count(&self) -> usize {
        self.blobs.len()
    }

    /// Test/diagnostic: staged slots currently open.
    #[must_use]
    pub fn staging_count(&self) -> usize {
        self.staging.len()
    }

    fn take_fault(&self, point: FaultPoint) -> Result<(), StoreError> {
        let mut faults = self.faults.borrow_mut();
        if let Some(queue) = faults.get_mut(&point)
            && let Some(error) = queue.pop_front()
        {
            return Err(error);
        }
        Ok(())
    }
}

impl ArtifactStore for InMemoryArtifactStore {
    fn stage_begin(&mut self) -> Result<StagingId, StoreError> {
        self.take_fault(FaultPoint::StageWrite)?;
        let id = StagingId(self.next_staging);
        self.next_staging += 1;
        self.staging.insert(
            id,
            StagingSlot {
                bytes: Vec::new(),
                sealed: false,
            },
        );
        Ok(id)
    }

    fn stage_write(&mut self, staging: &StagingId, bytes: &[u8]) -> Result<(), StoreError> {
        self.take_fault(FaultPoint::StageWrite)?;
        let slot = self
            .staging
            .get_mut(staging)
            .ok_or_else(|| StoreError::NotFound {
                detail: format!("staging slot {}", staging.0),
            })?;
        if slot.sealed {
            return Err(StoreError::Conflict {
                detail: "write to sealed staging".to_owned(),
            });
        }
        slot.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn stage_seal(&mut self, staging: &StagingId, expected_len: u64) -> Result<(), StoreError> {
        self.take_fault(FaultPoint::StageSeal)?;
        let slot = self
            .staging
            .get_mut(staging)
            .ok_or_else(|| StoreError::NotFound {
                detail: format!("staging slot {}", staging.0),
            })?;
        let actual = slot.bytes.len() as u64;
        if actual != expected_len {
            return Err(StoreError::PartialWrite {
                detail: format!("staged {actual} bytes, expected {expected_len}"),
            });
        }
        slot.sealed = true;
        Ok(())
    }

    fn stage_read(&self, staging: &StagingId) -> Result<Vec<u8>, StoreError> {
        self.take_fault(FaultPoint::StageRead)?;
        let slot = self
            .staging
            .get(staging)
            .ok_or_else(|| StoreError::NotFound {
                detail: format!("staging slot {}", staging.0),
            })?;
        if !slot.sealed {
            return Err(StoreError::Integrity {
                detail: "readback of unsealed staging".to_owned(),
            });
        }
        let mut bytes = slot.bytes.clone();
        if self.corrupt_next_readback.replace(false)
            && let Some(first) = bytes.first_mut()
        {
            *first ^= 0xFF;
        }
        Ok(bytes)
    }

    fn stage_abort(&mut self, staging: &StagingId) {
        self.staging.remove(staging);
    }

    fn commit_publish(&mut self, commit: &PublishCommit) -> Result<PublishAck, StoreError> {
        // ---- validation phase: everything checked before mutation ----
        let slot = self
            .staging
            .get(&commit.staging)
            .ok_or_else(|| StoreError::NotFound {
                detail: format!("staging slot {}", commit.staging.0),
            })?;
        if !slot.sealed {
            return Err(StoreError::Integrity {
                detail: "commit of unsealed staging".to_owned(),
            });
        }
        if ContentDigest::sha256(&slot.bytes) != commit.blob_digest {
            return Err(StoreError::Integrity {
                detail: "staged bytes do not match declared blob digest".to_owned(),
            });
        }
        // Content-addressing invariant: the store key must be the hash
        // of the stored manifest bytes.
        if RenderArtifactId::of_manifest_bytes(&commit.manifest_bytes) != commit.manifest_id {
            return Err(StoreError::Integrity {
                detail: "manifest id does not match manifest bytes".to_owned(),
            });
        }
        // Manifest decode must succeed for the store to index it —
        // the store never files unparseable manifests.
        let manifest =
            verify_manifest(&commit.manifest_id, &commit.manifest_bytes).map_err(|e| {
                StoreError::Integrity {
                    detail: format!("manifest verification failed: {e}"),
                }
            })?;
        if manifest.recipe_key != commit.recipe_key {
            return Err(StoreError::Integrity {
                detail: "index recipe key disagrees with manifest".to_owned(),
            });
        }
        let recipe_key = commit.recipe_key;
        if self.quarantine.contains_key(&commit.manifest_id) {
            return Err(StoreError::Conflict {
                detail: "artifact identity is quarantined".to_owned(),
            });
        }
        // CAS precondition on the index.
        let current: Vec<RenderArtifactId> = self
            .index
            .get(&recipe_key)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();
        if current != commit.expected_index {
            return Err(StoreError::Conflict {
                detail: "index entries changed since lookup".to_owned(),
            });
        }
        // Existing-object consistency (dedup / conflict detection).
        let blob_materialization = match self.blobs.get(&commit.blob_digest) {
            Some(existing) => {
                if ContentDigest::sha256(existing) != commit.blob_digest {
                    return Err(StoreError::Integrity {
                        detail: "stored blob fails its own digest".to_owned(),
                    });
                }
                Materialization::Reused
            }
            None => Materialization::Stored,
        };
        let manifest_materialization = match self.manifests.get(&commit.manifest_id) {
            Some(existing) if existing == &commit.manifest_bytes => Materialization::Reused,
            Some(_) => {
                return Err(StoreError::Integrity {
                    detail: "manifest id collision with different bytes".to_owned(),
                });
            }
            None => Materialization::Stored,
        };
        // ---- commit faults fire here: any failure aborts before ----
        // ---- mutation, preserving atomicity ------------------------
        self.take_fault(FaultPoint::CommitBlob)?;
        self.take_fault(FaultPoint::CommitManifest)?;
        self.take_fault(FaultPoint::CommitIndex)?;
        // ---- mutation phase ----
        let blob_bytes = slot.bytes.clone();
        self.blobs
            .entry(commit.blob_digest)
            .or_insert_with(|| blob_bytes.clone());
        self.blob_refs
            .entry(commit.blob_digest)
            .or_default()
            .insert(commit.manifest_id);
        self.manifests
            .insert(commit.manifest_id, commit.manifest_bytes.clone());
        self.published.insert(commit.manifest_id, self.clock);
        self.tombstones.remove(&commit.manifest_id);
        self.index
            .entry(recipe_key)
            .or_default()
            .insert(commit.manifest_id);
        self.staging.remove(&commit.staging);
        if let Some(reference) = &commit.reference {
            self.references
                .entry(commit.manifest_id)
                .or_default()
                .insert(reference.clone(), RefState::Active);
        }
        let index_len = self.index.get(&recipe_key).map_or(0, BTreeSet::len);
        Ok(PublishAck {
            blob: blob_materialization,
            manifest: manifest_materialization,
            index_len,
        })
    }

    fn read_blob(&self, digest: &ContentDigest) -> Result<Option<Vec<u8>>, StoreError> {
        self.take_fault(FaultPoint::ReadBlob)?;
        Ok(self.blobs.get(digest).cloned())
    }

    fn read_manifest(&self, id: &RenderArtifactId) -> Result<Option<Vec<u8>>, StoreError> {
        self.take_fault(FaultPoint::ReadManifest)?;
        Ok(self.manifests.get(id).cloned())
    }

    fn index_entries(&self, recipe: &RecipeKey) -> Result<Vec<RenderArtifactId>, StoreError> {
        Ok(self
            .index
            .get(recipe)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default())
    }

    fn artifacts(&self) -> Result<Vec<ArtifactRecord>, StoreError> {
        Ok(self
            .manifests
            .keys()
            .map(|id| ArtifactRecord {
                id: *id,
                manifest_bytes: self.manifests[id].clone(),
                blob_present: verify_manifest(id, &self.manifests[id])
                    .is_ok_and(|m| self.blobs.contains_key(&m.content_digest)),
                published_tick: self.published.get(id).copied().unwrap_or(0),
                quarantined: self.quarantine.contains_key(id),
            })
            .collect())
    }

    fn attach_reference(
        &mut self,
        artifact: &RenderArtifactId,
        reference: ArtifactReference,
    ) -> Result<(), StoreError> {
        self.take_fault(FaultPoint::ReferenceWrite)?;
        if self.quarantine.contains_key(artifact) {
            return Err(StoreError::Conflict {
                detail: "cannot reference quarantined artifact".to_owned(),
            });
        }
        if !self.manifests.contains_key(artifact) {
            return Err(StoreError::NotFound {
                detail: format!("artifact {artifact}"),
            });
        }
        self.references
            .entry(*artifact)
            .or_default()
            .insert(reference, RefState::Active);
        Ok(())
    }

    fn release_reference(
        &mut self,
        artifact: &RenderArtifactId,
        reference: &ArtifactReference,
    ) -> Result<(), StoreError> {
        self.take_fault(FaultPoint::ReferenceWrite)?;
        let refs = self
            .references
            .get_mut(artifact)
            .ok_or_else(|| StoreError::NotFound {
                detail: format!("artifact {artifact}"),
            })?;
        let state = refs
            .get_mut(reference)
            .ok_or_else(|| StoreError::NotFound {
                detail: "reference not attached".to_owned(),
            })?;
        *state = RefState::Released { at: self.clock };
        Ok(())
    }

    fn pend_reference(
        &mut self,
        artifact: &RenderArtifactId,
        reference: &ArtifactReference,
    ) -> Result<(), StoreError> {
        self.take_fault(FaultPoint::ReferenceWrite)?;
        let refs = self
            .references
            .get_mut(artifact)
            .ok_or_else(|| StoreError::NotFound {
                detail: format!("artifact {artifact}"),
            })?;
        let state = refs
            .get_mut(reference)
            .ok_or_else(|| StoreError::NotFound {
                detail: "reference not attached".to_owned(),
            })?;
        *state = RefState::Pending;
        Ok(())
    }

    fn references(&self, artifact: &RenderArtifactId) -> Result<Vec<ReferenceRecord>, StoreError> {
        Ok(self
            .references
            .get(artifact)
            .map(|refs| {
                refs.iter()
                    .map(|(reference, state)| ReferenceRecord {
                        reference: reference.clone(),
                        state: *state,
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn attach_external_manifest(&mut self, entry: ExternalManifestEntry) -> Result<(), StoreError> {
        self.take_fault(FaultPoint::ExternalWrite)?;
        if self.quarantine.contains_key(&entry.artifact) {
            return Err(StoreError::Conflict {
                detail: "cannot attach manifest to quarantined artifact".to_owned(),
            });
        }
        if !self.manifests.contains_key(&entry.artifact) {
            return Err(StoreError::NotFound {
                detail: format!("artifact {}", entry.artifact),
            });
        }
        if !valid_locator(&entry.locator) {
            return Err(StoreError::Conflict {
                detail: "invalid external manifest locator".to_owned(),
            });
        }
        let key = (entry.locator.clone(), entry.snapshot_digest);
        let entries = self.external_entries.entry(key.clone()).or_default();
        if !entries.contains(&entry) {
            entries.push(entry.clone());
            entries.sort();
        }
        self.external.entry(key).or_default().insert(entry.artifact);
        Ok(())
    }

    fn detach_external_manifest(
        &mut self,
        locator: &str,
        snapshot_digest: &ContentDigest,
        artifact: &RenderArtifactId,
    ) -> Result<(), StoreError> {
        self.take_fault(FaultPoint::ExternalWrite)?;
        let key = (locator.to_owned(), *snapshot_digest);
        if let Some(artifacts) = self.external.get_mut(&key) {
            artifacts.remove(artifact);
            if artifacts.is_empty() {
                self.external.remove(&key);
            }
        }
        if let Some(entries) = self.external_entries.get_mut(&key) {
            entries.retain(|e| e.artifact != *artifact);
            if entries.is_empty() {
                self.external_entries.remove(&key);
            }
        }
        Ok(())
    }

    fn external_manifests(
        &self,
        locator: &str,
        snapshot_digest: &ContentDigest,
    ) -> Result<Vec<ExternalManifestEntry>, StoreError> {
        Ok(self
            .external_entries
            .get(&(locator.to_owned(), *snapshot_digest))
            .cloned()
            .unwrap_or_default())
    }

    fn has_external_manifest(&self, artifact: &RenderArtifactId) -> Result<bool, StoreError> {
        Ok(self
            .external_entries
            .values()
            .any(|entries| entries.iter().any(|e| e.artifact == *artifact)))
    }

    fn is_quarantined(&self, id: &RenderArtifactId) -> bool {
        self.quarantine.contains_key(id)
    }

    fn delete_artifact(&mut self, id: &RenderArtifactId) -> Result<DeleteOutcome, StoreError> {
        self.take_fault(FaultPoint::DeleteArtifact)?;
        if !self.manifests.contains_key(id) {
            return Err(StoreError::NotFound {
                detail: format!("artifact {id}"),
            });
        }
        let mut outcome = DeleteOutcome::default();
        // Resolve the blob digest before dropping the manifest.
        let blob_digest = verify_manifest(id, &self.manifests[id])
            .ok()
            .map(|m| m.content_digest);
        self.manifests.remove(id);
        outcome.manifest_removed = true;
        self.published.remove(id);
        // Index entries.
        let mut emptied = Vec::new();
        for (recipe, set) in &mut self.index {
            if set.remove(id) {
                outcome.index_entries_removed += 1;
            }
            if set.is_empty() {
                emptied.push(*recipe);
            }
        }
        for recipe in emptied {
            self.index.remove(&recipe);
        }
        // References.
        outcome.references_dropped = self.references.remove(id).map_or(0, |r| r.len());
        // External entries pointing at this artifact.
        let mut external_keys: Vec<(String, ContentDigest)> = self
            .external_entries
            .iter()
            .filter(|(_, entries)| entries.iter().any(|e| e.artifact == *id))
            .map(|(key, _)| key.clone())
            .collect();
        for key in external_keys.drain(..) {
            if let Some(entries) = self.external_entries.get_mut(&key) {
                entries.retain(|e| e.artifact != *id);
                if entries.is_empty() {
                    self.external_entries.remove(&key);
                }
            }
            if let Some(set) = self.external.get_mut(&key) {
                set.remove(id);
                if set.is_empty() {
                    self.external.remove(&key);
                }
            }
            outcome.external_entries_dropped += 1;
        }
        // Blob refcount — freed only when no manifest references it.
        if let Some(digest) = blob_digest
            && let Some(refs) = self.blob_refs.get_mut(&digest)
        {
            refs.remove(id);
            if refs.is_empty() {
                self.blob_refs.remove(&digest);
                self.blobs.remove(&digest);
                outcome.blob_freed = Some(digest);
            }
        }
        self.quarantine.remove(id);
        self.tombstones.insert(*id, self.clock);
        Ok(outcome)
    }

    fn tombstone(&self, id: &RenderArtifactId) -> Option<u64> {
        self.tombstones.get(id).copied()
    }

    fn now(&self) -> u64 {
        self.clock
    }

    fn advance_clock(&mut self, ticks: u64) {
        self.clock = self.clock.saturating_add(ticks);
    }
}

/// Locator hygiene for untrusted remote/external identifiers — opaque
/// strings only: no separators, no traversal, bounded length.
#[must_use]
pub fn valid_locator(locator: &str) -> bool {
    !locator.is_empty()
        && locator.len() <= 512
        && !locator.contains('/')
        && !locator.contains('\\')
        && !locator.contains("..")
        && !locator.chars().any(|c| c.is_control() || c.is_whitespace())
}
