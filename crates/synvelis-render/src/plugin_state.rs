//! Persisted TIM-04 plugin-state projection into RCP-01 recipe evidence.
//!
//! This module is a non-RT adapter boundary. It reads the versioned
//! manifest and content-addressed blobs owned by
//! `synvelis-project::plugin_state`, verifies their integrity at the
//! boundary, and exposes owned per-processor outcomes through the
//! RCP-01 [`PluginStateProbe`] port. It never invokes plugin capture,
//! restore, filesystem IO, or any other state operation from an RT
//! callback.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use synvelis_project::plugin_state::{
    PLUGIN_STATE_SCHEMA_VERSION, PluginStateManifest, PluginStateOutcome, PluginStateStore,
    PluginStateStoreError, is_manifest_name, plugin_state_manifest_name,
};
use synvelis_project::snapshot::ContentDigest;
use synvelis_project::{ProcessorId, ProjectId};

use crate::dependency::{PluginStateEvidence, PluginStateProbe, PluginStateProbeOutcome};

/// Input path used to construct a [`PersistedPluginStateProjection`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginStateProjectionSource {
    /// The mutable store HEAD selected the manifest.
    StoreHead,
    /// The caller supplied an explicit manifest name in the store.
    ExplicitManifest,
    /// The caller supplied manifest bytes directly.
    ManifestBytes,
}

/// Projection construction failure — persistence and integrity
/// failures stay distinct from valid blob-less plugin outcomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginStateProjectionError {
    /// HEAD does not name a manifest.
    NoHeadManifest,
    /// A manifest name is outside the content-addressed name syntax.
    InvalidManifestName {
        /// Rejected name.
        name: String,
    },
    /// Reading HEAD failed.
    HeadRead {
        /// Store error.
        source: PluginStateStoreError,
    },
    /// Reading manifest bytes failed.
    ManifestRead {
        /// Manifest name.
        name: String,
        /// Store error.
        source: PluginStateStoreError,
    },
    /// The content-derived manifest name does not match the bytes.
    ManifestDigestMismatch {
        /// Name being read.
        name: String,
        /// Name computed from the bytes.
        computed_name: String,
        /// Full digest of the bytes.
        digest: ContentDigest,
    },
    /// Manifest bytes are not valid `PluginStateManifest` JSON.
    MalformedManifest {
        /// Manifest name when one is known.
        name: Option<String>,
        /// Decode detail.
        detail: String,
    },
    /// Manifest schema version is outside the supported ladder.
    UnsupportedSchema {
        /// Manifest name when one is known.
        name: Option<String>,
        /// Schema version found.
        found: u32,
    },
    /// A captured record lacks the binding identity needed to reject
    /// stale plugin-state evidence.
    MissingCaptureBinding {
        /// Owning processor.
        processor_id: ProcessorId,
    },
    /// A processor appears more than once in one manifest.
    DuplicateProcessor {
        /// Duplicated processor.
        processor_id: ProcessorId,
    },
    /// A captured record references a blob absent from the store.
    MissingBlob {
        /// Owning processor.
        processor_id: ProcessorId,
        /// Referenced digest.
        digest: ContentDigest,
    },
    /// A captured record references a blob whose length or bytes do
    /// not match the persisted record.
    CorruptBlob {
        /// Owning processor.
        processor_id: ProcessorId,
        /// Referenced digest.
        digest: ContentDigest,
        /// Length recorded in the manifest.
        expected_len: u64,
        /// Length read from the store.
        actual_len: u64,
        /// Digest measured from the stored bytes.
        actual_digest: ContentDigest,
    },
    /// A captured record references a blob that cannot be read.
    BlobRead {
        /// Owning processor.
        processor_id: ProcessorId,
        /// Referenced digest.
        digest: ContentDigest,
        /// Store error.
        source: PluginStateStoreError,
    },
}

impl fmt::Display for PluginStateProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHeadManifest => write!(f, "plugin state HEAD does not name a manifest"),
            Self::InvalidManifestName { name } => {
                write!(f, "invalid plugin state manifest name {name:?}")
            }
            Self::HeadRead { source } => write!(f, "read plugin state HEAD: {source}"),
            Self::ManifestRead { name, source } => {
                write!(f, "read plugin state manifest {name}: {source}")
            }
            Self::ManifestDigestMismatch {
                name,
                computed_name,
                digest,
            } => write!(
                f,
                "plugin state manifest {name} does not match bytes digest {digest} (computed name {computed_name})"
            ),
            Self::MalformedManifest { name, detail } => {
                write!(f, "malformed plugin state manifest {name:?}: {detail}")
            }
            Self::UnsupportedSchema { name, found } => write!(
                f,
                "unsupported plugin state manifest schema {found} in {name:?} (supported {PLUGIN_STATE_SCHEMA_VERSION})"
            ),
            Self::MissingCaptureBinding { processor_id } => write!(
                f,
                "captured plugin state record for {processor_id} lacks binding identity"
            ),
            Self::DuplicateProcessor { processor_id } => {
                write!(f, "duplicate plugin state record for {processor_id}")
            }
            Self::MissingBlob {
                processor_id,
                digest,
            } => write!(
                f,
                "plugin state blob {digest} for {processor_id} is missing"
            ),
            Self::CorruptBlob {
                processor_id,
                digest,
                expected_len,
                actual_len,
                actual_digest,
            } => write!(
                f,
                "plugin state blob {digest} for {processor_id} is corrupt (len {actual_len}/{expected_len}, digest {actual_digest})"
            ),
            Self::BlobRead {
                processor_id,
                digest,
                source,
            } => write!(
                f,
                "read plugin state blob {digest} for {processor_id}: {source}"
            ),
        }
    }
}

impl std::error::Error for PluginStateProjectionError {}

/// Owned, validated projection of one persisted plugin-state manifest.
///
/// The table is immutable after construction and contains no store or
/// lifetime references, so it can move across non-RT scheduler or
/// orchestrator boundaries as `Send` evidence.
pub struct PersistedPluginStateProjection {
    outcomes: BTreeMap<ProcessorId, PluginStateProbeOutcome>,
    manifest_name: Option<String>,
    manifest_digest: ContentDigest,
    project_id: ProjectId,
    project_revision: u64,
    source: PluginStateProjectionSource,
}

impl PersistedPluginStateProjection {
    /// Reads the manifest selected by the store HEAD.
    ///
    /// # Errors
    /// [`PluginStateProjectionError`] when HEAD or the manifest/blob
    /// chain cannot be read and verified.
    pub fn from_store_head(
        store: &dyn PluginStateStore,
    ) -> Result<Self, PluginStateProjectionError> {
        let name = store
            .head()
            .map_err(|source| PluginStateProjectionError::HeadRead { source })?
            .ok_or(PluginStateProjectionError::NoHeadManifest)?;
        Self::read_named(store, &name, PluginStateProjectionSource::StoreHead)
    }

    /// Reads an explicitly named manifest from the store.
    ///
    /// # Errors
    /// [`PluginStateProjectionError`] when the name, manifest, or
    /// blob chain cannot be read and verified.
    pub fn from_store_manifest(
        store: &dyn PluginStateStore,
        name: &str,
    ) -> Result<Self, PluginStateProjectionError> {
        Self::read_named(store, name, PluginStateProjectionSource::ExplicitManifest)
    }

    /// Parses caller-supplied manifest bytes while verifying captured
    /// blob references against `store`.
    ///
    /// # Errors
    /// [`PluginStateProjectionError`] when the manifest or blob chain
    /// cannot be verified.
    pub fn from_manifest_bytes(
        bytes: &[u8],
        store: &dyn PluginStateStore,
    ) -> Result<Self, PluginStateProjectionError> {
        Self::parse(
            None,
            bytes,
            store,
            PluginStateProjectionSource::ManifestBytes,
        )
    }

    fn read_named(
        store: &dyn PluginStateStore,
        name: &str,
        source: PluginStateProjectionSource,
    ) -> Result<Self, PluginStateProjectionError> {
        if !is_manifest_name(name) {
            return Err(PluginStateProjectionError::InvalidManifestName {
                name: name.to_owned(),
            });
        }
        let bytes = store.read_manifest(name).map_err(|source_error| {
            PluginStateProjectionError::ManifestRead {
                name: name.to_owned(),
                source: source_error,
            }
        })?;
        Self::parse(Some(name), &bytes, store, source)
    }

    fn parse(
        name: Option<&str>,
        bytes: &[u8],
        store: &dyn PluginStateStore,
        source: PluginStateProjectionSource,
    ) -> Result<Self, PluginStateProjectionError> {
        let manifest_digest = ContentDigest::sha256(bytes);
        if let Some(name) = name {
            let computed_name = plugin_state_manifest_name(bytes);
            if computed_name != name {
                return Err(PluginStateProjectionError::ManifestDigestMismatch {
                    name: name.to_owned(),
                    computed_name,
                    digest: manifest_digest,
                });
            }
        }
        let name_owned = name.map(str::to_owned);
        let manifest: PluginStateManifest = serde_json::from_slice(bytes).map_err(|error| {
            PluginStateProjectionError::MalformedManifest {
                name: name_owned.clone(),
                detail: error.to_string(),
            }
        })?;
        if manifest.schema_version != PLUGIN_STATE_SCHEMA_VERSION {
            return Err(PluginStateProjectionError::UnsupportedSchema {
                name: name_owned,
                found: manifest.schema_version,
            });
        }

        let project_id = manifest.project_id;
        let project_revision = manifest.project_revision;
        let mut outcomes = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for entry in manifest.entries {
            if !seen.insert(entry.processor_id) {
                return Err(PluginStateProjectionError::DuplicateProcessor {
                    processor_id: entry.processor_id,
                });
            }
            let outcome = match entry.outcome {
                PluginStateOutcome::Captured {
                    quality,
                    blob_digest,
                    blob_len,
                } => {
                    validate_blob(store, entry.processor_id, blob_digest, blob_len)?;
                    let Some(binding) = entry.binding.clone() else {
                        return Err(PluginStateProjectionError::MissingCaptureBinding {
                            processor_id: entry.processor_id,
                        });
                    };
                    PluginStateProbeOutcome::Captured {
                        evidence: PluginStateEvidence {
                            digest: blob_digest,
                            quality,
                        },
                        binding: Some(binding),
                    }
                }
                PluginStateOutcome::Unsupported => PluginStateProbeOutcome::Unsupported,
                PluginStateOutcome::Failed { kind } => PluginStateProbeOutcome::Failed { kind },
            };
            outcomes.insert(entry.processor_id, outcome);
        }

        Ok(Self {
            outcomes,
            manifest_name: name.map(str::to_owned),
            manifest_digest,
            project_id,
            project_revision,
            source,
        })
    }

    /// Manifest name when the projection came from a named store object.
    #[must_use]
    pub fn manifest_name(&self) -> Option<&str> {
        self.manifest_name.as_deref()
    }

    /// SHA-256 digest of the exact manifest bytes.
    #[must_use]
    pub const fn manifest_digest(&self) -> ContentDigest {
        self.manifest_digest
    }

    /// Project identity recorded in the manifest.
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }

    /// Project revision recorded in the manifest.
    #[must_use]
    pub const fn project_revision(&self) -> u64 {
        self.project_revision
    }

    /// Input path used to construct the projection.
    #[must_use]
    pub const fn source(&self) -> PluginStateProjectionSource {
        self.source
    }

    /// Number of processor records in the validated manifest.
    #[must_use]
    pub fn record_count(&self) -> usize {
        self.outcomes.len()
    }
}

impl PluginStateProbe for PersistedPluginStateProjection {
    fn outcome(&self, processor: ProcessorId) -> Option<PluginStateProbeOutcome> {
        self.outcomes.get(&processor).cloned()
    }
}

fn validate_blob(
    store: &dyn PluginStateStore,
    processor_id: ProcessorId,
    digest: ContentDigest,
    expected_len: u64,
) -> Result<(), PluginStateProjectionError> {
    let blob = match store.read_blob(&digest) {
        Ok(blob) => blob,
        Err(PluginStateStoreError::NotFound) => {
            return Err(PluginStateProjectionError::MissingBlob {
                processor_id,
                digest,
            });
        }
        Err(source) => {
            return Err(PluginStateProjectionError::BlobRead {
                processor_id,
                digest,
                source,
            });
        }
    };
    let actual_len = blob.len() as u64;
    let actual_digest = ContentDigest::sha256(&blob);
    if actual_len != expected_len || actual_digest != digest {
        return Err(PluginStateProjectionError::CorruptBlob {
            processor_id,
            digest,
            expected_len,
            actual_len,
            actual_digest,
        });
    }
    Ok(())
}
