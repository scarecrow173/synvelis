//! Branch candidate content store port (synvelis#278, CB-01).
//!
//! Branch content is *referenced*, never embedded in the project
//! snapshot: [`BranchContentRef::Captured`] pins payload bytes by
//! SHA-256 digest inside a [`BranchContentStore`]. Two implementations
//! ship: [`InMemoryBranchContentStore`] for fixtures and
//! [`DirectoryBranchContentStore`] — the durable, file-backed store
//! production hosts wire through `VariantPorts` (synvelis#517).
//!
//! Orphan payloads are harmless: a failed operation may leave bytes in
//! the store, but the persisted model only references digests that a
//! successful CAS committed.

use std::collections::BTreeMap;
use std::io::Write;

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

// ---------------------------------------------------------------------------
// DirectoryBranchContentStore — durable, content-addressed payload store
// (synvelis#517)
// ---------------------------------------------------------------------------

/// File-backed [`BranchContentStore`] — one immutable file per payload
/// digest under `<root>/<sha256-hex>`.
///
/// Durability contract:
///
/// - `put` publishes through [`ruviform_io_file::StagedFile`] with
///   `FileAndDirectorySync` — a payload is either fully visible under
///   its digest name or absent; a torn write can never be read back as
///   a different digest's bytes (the name *is* the digest, and
///   [`decode_candidate`] re-verifies content digests on every read).
/// - Writes are idempotent: an existing file whose bytes hash to the
///   same digest is accepted without rewriting (content addressing
///   makes "same name, different bytes" impossible for honest writers;
///   a corrupted existing file fails closed instead of being trusted).
/// - There is no GC: orphaned payloads are harmless by contract (only
///   digests a successful project CAS references are authoritative),
///   and deletion is the host's project-lifecycle concern, not the
///   store's.
/// - The store root is the host's choice — production hosts place it
///   inside the project directory so `restore_as_copy` can fork payload
///   bytes into the restored project's own store (VCS-RESTORE-10).
#[derive(Clone, Debug)]
pub struct DirectoryBranchContentStore {
    root: std::path::PathBuf,
}

impl DirectoryBranchContentStore {
    /// Store rooted at `root` (created on first write — reads of an
    /// absent root honestly report every digest as missing).
    #[must_use]
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The root directory payloads live under.
    #[must_use]
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn path_for(&self, digest: &ContentDigest) -> std::path::PathBuf {
        self.root.join(crate::plugin_state::digest_hex(digest))
    }

    fn store_err(detail: impl Into<String>) -> VariantError {
        VariantError::Store {
            detail: detail.into(),
        }
    }
}

impl BranchContentStore for DirectoryBranchContentStore {
    fn put(&mut self, digest: &ContentDigest, bytes: &[u8]) -> Result<(), VariantError> {
        if ContentDigest::sha256(bytes) != *digest {
            return Err(Self::store_err(format!(
                "payload bytes do not match digest {digest}"
            )));
        }
        let path = self.path_for(digest);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Self::store_err(format!(
                    "create content store dir {}: {e}",
                    parent.display()
                ))
            })?;
        }
        if path.exists() {
            // Idempotent re-put: verify the existing bytes still hash
            // to the digest rather than trusting the name.
            let existing = std::fs::read(&path)
                .map_err(|e| Self::store_err(format!("readback {}: {e}", path.display())))?;
            if ContentDigest::sha256(&existing) != *digest {
                return Err(Self::store_err(format!(
                    "existing payload {} fails digest verification",
                    path.display()
                )));
            }
            return Ok(());
        }
        let mut staged = ruviform_io_file::StagedFile::create(&path)
            .map_err(|e| Self::store_err(format!("stage {}: {e}", path.display())))?;
        staged
            .write_all(bytes)
            .map_err(|e| Self::store_err(format!("write {}: {e}", path.display())))?;
        if let Err(failure) = staged.seal().publish(ruviform_io_file::PublishOptions::new(
            ruviform_io_file::PublishMode::CreateNew,
            ruviform_io_file::Durability::FileAndDirectorySync,
        )) {
            // A concurrent writer may have published the same digest
            // between the existence check and `CreateNew` — verify
            // rather than reporting a spurious failure.
            if path.exists() {
                let existing = std::fs::read(&path)
                    .map_err(|e| Self::store_err(format!("readback {}: {e}", path.display())))?;
                if ContentDigest::sha256(&existing) == *digest {
                    return Ok(());
                }
            }
            return Err(Self::store_err(format!(
                "publish {}: {failure}",
                path.display()
            )));
        }
        Ok(())
    }

    fn get(&self, digest: &ContentDigest) -> Result<Option<Vec<u8>>, VariantError> {
        let path = self.path_for(digest);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Self::store_err(format!("read {}: {e}", path.display()))),
        }
    }
}
