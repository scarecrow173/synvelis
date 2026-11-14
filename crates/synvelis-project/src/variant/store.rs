//! Branch candidate content store port (synvelis#278, CB-01).
//!
//! Branch content is *referenced*, never embedded in the project
//! snapshot: [`BranchContentRef::Captured`] pins payload bytes by
//! SHA-256 digest inside a [`BranchContentStore`]. The project crate
//! ships only the in-memory implementation — real content-addressed
//! blob storage is a later slice's concern; the port boundary is what
//! the model needs now.
//!
//! Orphan payloads are harmless: a failed operation may leave bytes in
//! the store, but the persisted model only references digests that a
//! successful CAS committed.

use std::collections::BTreeMap;

use crate::snapshot::ContentDigest;
use crate::variant::VariantError;
use crate::variant::types::BranchContentRef;

/// Content-addressed store for immutable branch candidate payloads.
pub trait BranchContentStore {
    /// Stores `bytes` under `digest` (idempotent — same digest, same
    /// bytes by construction of the digest).
    ///
    /// # Errors
    /// [`VariantError::Store`] on backend failure.
    fn put(&mut self, digest: &ContentDigest, bytes: &[u8]) -> Result<(), VariantError>;

    /// Returns the payload stored under `digest`, or `None` when absent.
    ///
    /// # Errors
    /// [`VariantError::Store`] on backend failure.
    fn get(&self, digest: &ContentDigest) -> Result<Option<Vec<u8>>, VariantError>;
}

/// Serialize a candidate [`crate::Project`] for storage. The variant
/// model itself is stripped — candidate payloads are domain state only,
/// never recursive variant declarations.
pub(crate) fn encode_candidate(project: &crate::Project) -> Result<Vec<u8>, VariantError> {
    serde_json::to_vec(project).map_err(|e| VariantError::Store {
        detail: format!("candidate encode: {e}"),
    })
}

/// Decode + integrity-check a stored candidate payload against its
/// declared [`BranchContentRef::Captured`] digests.
///
/// # Errors
/// [`VariantError::CandidateUnavailable`] on missing bytes, digest
/// mismatch, undecodable payload, or schema-version drift.
pub fn decode_candidate(
    store: &dyn BranchContentStore,
    reference: &BranchContentRef,
) -> Result<crate::Project, VariantError> {
    let BranchContentRef::Captured {
        payload_digest,
        scope_digest: _,
        schema_version,
        revision: _,
    } = reference
    else {
        return Err(VariantError::CandidateUnavailable {
            detail: "branch content is not a captured payload".into(),
        });
    };
    if *schema_version != crate::store::SCHEMA_VERSION {
        return Err(VariantError::CandidateUnavailable {
            detail: format!(
                "candidate schema {schema_version} != current {}",
                crate::store::SCHEMA_VERSION
            ),
        });
    }
    let bytes = store
        .get(payload_digest)?
        .ok_or_else(|| VariantError::CandidateUnavailable {
            detail: format!("payload {payload_digest} absent from content store"),
        })?;
    if ContentDigest::sha256(&bytes) != *payload_digest {
        return Err(VariantError::CandidateUnavailable {
            detail: format!("payload digest mismatch for {payload_digest}"),
        });
    }
    let project: crate::Project =
        serde_json::from_slice(&bytes).map_err(|e| VariantError::CandidateUnavailable {
            detail: format!("candidate decode: {e}"),
        })?;
    project.validate_references()?;
    Ok(project)
}

/// In-memory [`BranchContentStore`] — the headless fixture store.
#[derive(Clone, Debug, Default)]
pub struct InMemoryBranchContentStore {
    objects: BTreeMap<ContentDigest, Vec<u8>>,
}

impl InMemoryBranchContentStore {
    /// Number of stored payloads (test observability).
    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// `true` when the store holds no payloads.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

impl BranchContentStore for InMemoryBranchContentStore {
    fn put(&mut self, digest: &ContentDigest, bytes: &[u8]) -> Result<(), VariantError> {
        self.objects.insert(*digest, bytes.to_vec());
        Ok(())
    }

    fn get(&self, digest: &ContentDigest) -> Result<Option<Vec<u8>>, VariantError> {
        Ok(self.objects.get(digest).cloned())
    }
}
