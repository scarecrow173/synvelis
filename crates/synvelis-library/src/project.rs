//! `ProjectAssetsProvider` — read-only projection of the live
//! `ProjectController`'s asset registry.
//!
//! The provider never owns a second project truth: it reads through
//! the injected [`ProjectCatalog`] port (a `Mutex<ProjectController>`
//! adapter or a static snapshot for headless tests) and resolves blob
//! bytes through the injected [`ProjectBlobStore`] port. The
//! `<media_root>/blobs/<digest>` layout convention lives with the
//! caller; [`LocalBlobDir`] only maps an [`ObjectIdentity`] to a file
//! inside one directory after filename-safety checks.
//!
//! Project blobs are content-addressed and effectively immutable, so
//! `pin` produces `PinProof::ImmutableObject` keyed by the blob's
//! recorded digest. It also measures SHA-256 and returns it as the
//! `digest_hint` (`ChecksumHint` capability), and `open_pinned`
//! verifies streamed bytes against it — a tampered blob fails with
//! `IntegrityMismatch` instead of silently serving wrong bytes.
//!
//! Cursors bind to the project `ProjectRevision`: any apply that
//! advances the revision invalidates outstanding cursors.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use synvelis_project::{
    AssetId, ImportId, Project, ProjectRevision, SourceId, SourceState,
    controller::ProjectController,
};

use crate::browse::{
    BrowseEntry, BrowsePage, BrowseRequest, BrowseScope, Coverage, CursorPayload, EntryKind,
    decode_cursor, encode_cursor, fingerprint, principal_fingerprint, query_fingerprint,
    scope_fingerprint, sort_entries,
};
use crate::capability::{
    AccessMode, Capability, PinCapability, ProviderCapabilities, UnsupportedReason,
};
use crate::context::ProviderContext;
use crate::digest::{completion_slot, stream_digest};
use crate::error::{ProviderError, ProviderErrorKind};
use crate::id::{OpaqueLocator, ProviderId};
use crate::observation::{
    AssetKind, AssetObservation, AssetStat, Availability, EntryFacts, MediaFacts,
    ObservationOrigin, ProvenanceRecord, UnavailableReason,
};
use crate::provider::{
    AssetProvider, ByteRange, FetchClaims, FetchCompletion, PinnedRead, VerificationPolicy,
};
use crate::refs::{
    ContentDigest, ObjectIdentity, PinProof, PinnedAssetRef, RemoteAssetRef, RevisionKey,
};

/// The canonical serde state name for a [`SourceState`] — the same
/// lowercase tag the project model serializes.
fn source_state_name(state: SourceState) -> &'static str {
    match state {
        SourceState::Imported => "imported",
        SourceState::Verified => "verified",
        SourceState::Committed => "committed",
        SourceState::Saved => "saved",
    }
}

/// Read-only catalog port over the project's asset registry.
///
/// The single-writer model is preserved: implementations may read
/// `ProjectController::live()` under a lock but never mutate.
///
/// The bundled `Mutex<ProjectController>` / `Arc<Mutex<..>>` adapters
/// take a short-lived lock per snapshot and are **not reentrant** —
/// do not call `catalog()` while already holding that lock.
pub trait ProjectCatalog: Send + Sync {
    /// A consistent snapshot of the asset registry at one revision.
    ///
    /// # Errors
    /// [`ProviderError`] — snapshot acquisition failure (lock poisoned).
    fn catalog(&self) -> Result<ProjectCatalogSnapshot, ProviderError>;
}

/// One registered project source referencing an asset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSourceRef {
    /// The source id.
    pub source_id: SourceId,
    /// Its verification/finalization state.
    pub state: SourceState,
}

/// One project asset as projected by [`ProjectCatalog`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectAssetRecord {
    /// The project-stable asset id.
    pub asset_id: AssetId,
    /// The recorded content digest (opaque string — the blob's
    /// object identity, not a strong digest).
    pub digest: String,
    /// Sources referencing this asset.
    pub sources: Vec<ProjectSourceRef>,
    /// Imports that registered this asset.
    pub imported_via: Vec<ImportId>,
}

/// A consistent catalog snapshot at one project revision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectCatalogSnapshot {
    /// The project identity.
    pub project_id: synvelis_project::ProjectId,
    /// The revision the snapshot was taken at.
    pub revision: ProjectRevision,
    /// Registered assets.
    pub assets: Vec<ProjectAssetRecord>,
}

impl ProjectCatalogSnapshot {
    /// Projects a `Project` aggregate into a catalog snapshot.
    #[must_use]
    pub fn from_project(project: &Project, revision: ProjectRevision) -> Self {
        let mut imported_by_asset: BTreeMap<AssetId, Vec<ImportId>> = BTreeMap::new();
        for (import_id, applied) in &project.applied_imports {
            imported_by_asset
                .entry(applied.asset_id)
                .or_default()
                .push(*import_id);
        }
        let mut sources_by_asset: BTreeMap<AssetId, Vec<ProjectSourceRef>> = BTreeMap::new();
        for source in project.sources.values() {
            sources_by_asset
                .entry(source.asset_id)
                .or_default()
                .push(ProjectSourceRef {
                    source_id: source.id,
                    state: source.state,
                });
        }
        let assets = project
            .assets
            .values()
            .map(|asset| ProjectAssetRecord {
                asset_id: asset.id,
                digest: asset.digest.clone(),
                sources: sources_by_asset.remove(&asset.id).unwrap_or_default(),
                imported_via: imported_by_asset.remove(&asset.id).unwrap_or_default(),
            })
            .collect();
        Self {
            project_id: project.project_id,
            revision,
            assets,
        }
    }
}

impl ProjectCatalog for ProjectCatalogSnapshot {
    fn catalog(&self) -> Result<ProjectCatalogSnapshot, ProviderError> {
        Ok(self.clone())
    }
}

impl ProjectCatalog for Mutex<ProjectController> {
    fn catalog(&self) -> Result<ProjectCatalogSnapshot, ProviderError> {
        let controller = self.lock().map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::Internal,
                "project controller lock poisoned",
            )
        })?;
        Ok(ProjectCatalogSnapshot::from_project(
            controller.live(),
            controller.live_revision(),
        ))
    }
}

impl ProjectCatalog for std::sync::Arc<Mutex<ProjectController>> {
    fn catalog(&self) -> Result<ProjectCatalogSnapshot, ProviderError> {
        self.as_ref().catalog()
    }
}

/// A fetched blob object.
pub struct BlobObject {
    /// Chunked byte stream.
    pub reader: Box<dyn Read + Send>,
    /// Blob byte length.
    pub byte_len: u64,
}

/// Blob store port: resolves a project [`ObjectIdentity`] to bytes.
pub trait ProjectBlobStore: Send + Sync {
    /// Opens a blob for reading.
    ///
    /// # Errors
    /// [`ProviderError`] — missing blob, I/O failure.
    fn open(&self, object: &ObjectIdentity) -> Result<BlobObject, ProviderError>;

    /// Observes a blob's byte length without opening it.
    ///
    /// # Errors
    /// [`ProviderError`] — missing blob, I/O failure.
    fn stat(&self, object: &ObjectIdentity) -> Result<u64, ProviderError>;
}

/// DOS device basenames that `CreateFile` opens as real devices on
/// Windows regardless of directory prefix — never a valid blob name.
const DOS_DEVICES: &[&str] = &[
    "NUL", "CON", "AUX", "PRN", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// A blob store over one directory of digest-named files.
///
/// The app points this at `<media_root>/blobs`. Object identities are
/// validated as filename-safe (`[0-9a-zA-Z_-]` only) before any join,
/// and resolved paths are canonicalized back inside the store, so a
/// crafted identity cannot traverse out of the directory, name a
/// device, or reach a non-regular file.
pub struct LocalBlobDir {
    dir: PathBuf,
}

impl LocalBlobDir {
    /// A blob dir bound to `dir`.
    #[must_use]
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
        }
    }

    /// Resolves an object identity to an existing canonical file path
    /// contained in the store, plus its length. Filename-safe strings
    /// still cannot escape: the resolved path is canonicalized and
    /// re-checked against the canonical store dir (defeating symlinked
    /// blob files and symlinked store dirs), device names are refused,
    /// and the target must be a regular file (FIFOs/devices rejected).
    fn resolve(&self, object: &ObjectIdentity) -> Result<(PathBuf, u64), ProviderError> {
        let name = object.as_str();
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("object identity {name:?} is not filename-safe"),
            ));
        }
        // DOS device basenames resolve to real devices on Windows
        // regardless of extension — a crafted digest must never open one.
        if DOS_DEVICES.contains(&name.to_ascii_uppercase().as_str()) {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("object identity {name:?} names a device"),
            ));
        }
        let joined = self.dir.join(name);
        let canonical = fs::canonicalize(&joined).map_err(|e| Self::io("resolve blob", &e))?;
        let canonical_dir =
            fs::canonicalize(&self.dir).map_err(|e| Self::io("resolve store", &e))?;
        if !canonical.starts_with(&canonical_dir) {
            return Err(ProviderError::new(
                ProviderErrorKind::PermissionDenied,
                format!("blob {name:?} escapes the store"),
            ));
        }
        let meta = fs::metadata(&canonical).map_err(|e| Self::io("stat blob", &e))?;
        if !meta.is_file() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("blob {name:?} is not a regular file"),
            ));
        }
        Ok((canonical, meta.len()))
    }

    fn io(op: &'static str, e: &std::io::Error) -> ProviderError {
        match e.kind() {
            std::io::ErrorKind::NotFound => {
                ProviderError::new(ProviderErrorKind::NotFound, format!("{op}: {e}"))
            }
            std::io::ErrorKind::PermissionDenied => {
                ProviderError::new(ProviderErrorKind::PermissionDenied, format!("{op}: {e}"))
            }
            _ => ProviderError::new(ProviderErrorKind::TransientIo, format!("{op}: {e}")),
        }
    }
}

impl ProjectBlobStore for LocalBlobDir {
    fn open(&self, object: &ObjectIdentity) -> Result<BlobObject, ProviderError> {
        let (path, _pre_open_len) = self.resolve(object)?;
        let file = File::open(&path).map_err(|e| Self::io("open blob", &e))?;
        // Post-open re-check: a stat→open swap to a non-file must
        // never be streamed (a blocking device read is a DoS).
        let file_meta = file.metadata().map_err(|e| Self::io("stat blob", &e))?;
        if !file_meta.is_file() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("blob {:?} is not a regular file", object.as_str()),
            ));
        }
        Ok(BlobObject {
            reader: Box::new(file),
            byte_len: file_meta.len(),
        })
    }

    fn stat(&self, object: &ObjectIdentity) -> Result<u64, ProviderError> {
        Ok(self.resolve(object)?.1)
    }
}

/// Baseline provider over the current project's asset catalog.
pub struct ProjectAssetsProvider<C, B> {
    id: ProviderId,
    catalog: C,
    blobs: B,
}

impl<C, B> ProjectAssetsProvider<C, B>
where
    C: ProjectCatalog,
    B: ProjectBlobStore,
{
    /// A provider over `catalog` + `blobs`.
    #[must_use]
    pub fn new(id: ProviderId, catalog: C, blobs: B) -> Self {
        Self { id, catalog, blobs }
    }

    fn check_owner(&self, asset: &RemoteAssetRef) -> Result<(), ProviderError> {
        if asset.provider != self.id {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!(
                    "asset belongs to provider {}, not {}",
                    asset.provider, self.id
                ),
            ));
        }
        Ok(())
    }

    fn record_for<'a>(
        snapshot: &'a ProjectCatalogSnapshot,
        asset: &RemoteAssetRef,
    ) -> Result<&'a ProjectAssetRecord, ProviderError> {
        let id = AssetId::from_tagged(asset.locator.as_str()).map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("locator {} is not a tagged asset id", asset.locator),
            )
        })?;
        snapshot
            .assets
            .iter()
            .find(|record| record.asset_id == id)
            .ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorKind::NotFound,
                    format!("asset {} not in project catalog", asset.locator),
                )
            })
    }

    /// Opens `identity` (the pinned object for `open_pinned`, the
    /// current catalog digest for previews) and wraps it in the
    /// verifying reader.
    #[allow(clippy::too_many_arguments)] // verification knobs stay flat
    fn open_blob_verified(
        &self,
        record: &ProjectAssetRecord,
        identity: ObjectIdentity,
        range: Option<ByteRange>,
        verification: VerificationPolicy,
        expected_digest: Option<ContentDigest>,
        expected_size: Option<u64>,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        let blob = self.blobs.open(&identity)?;
        if let Some(expected) = expected_size
            && blob.byte_len != expected
        {
            return Err(ProviderError::new(
                ProviderErrorKind::SourceChanged,
                format!(
                    "blob {} size {} != pinned {expected}",
                    record.digest, blob.byte_len
                ),
            ));
        }
        if blob.byte_len > cx.limits.max_bytes {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                format!("blob {} exceeds byte budget", record.digest),
            ));
        }
        let reader: Box<dyn Read + Send> = match range {
            Some(range) => {
                // Blob reads are file-backed; range is a bounded take
                // after a bounded skip implemented by the caller-side
                // reader — LocalBlobDir returns seekable files, but the
                // port is `Read`. Read-and-discard keeps the contract
                // honest for non-seekable stores too.
                let mut skip = range.offset.min(blob.byte_len);
                let mut inner = blob.reader;
                let mut buf = [0u8; 8192];
                while skip > 0 {
                    // Long skips must still honor cancel/deadline.
                    cx.check_interrupts()?;
                    let step = usize::try_from(skip.min(8192)).unwrap_or(8192);
                    let n = inner.read(&mut buf[..step]).map_err(|e| {
                        ProviderError::new(ProviderErrorKind::TransientIo, format!("skip: {e}"))
                    })?;
                    if n == 0 {
                        break;
                    }
                    skip -= n as u64;
                }
                Box::new(inner.take(range.len))
            }
            None => blob.reader,
        };
        let slot = completion_slot();
        let reader = crate::digest::VerifyingReader::new(
            reader,
            expected_digest,
            expected_size,
            cx.limits.max_bytes,
            cx.cancel.clone(),
            cx.limits.deadline,
            identity.clone(),
            slot.clone(),
        );
        Ok(PinnedRead {
            reader: Box::new(reader),
            claims: FetchClaims {
                object_identity: identity,
                expected_size,
                verification,
            },
            completion: FetchCompletion::new(slot),
        })
    }
}

impl<C, B> AssetProvider for ProjectAssetsProvider<C, B>
where
    C: ProjectCatalog,
    B: ProjectBlobStore,
{
    fn id(&self) -> ProviderId {
        self.id
    }

    fn kind(&self) -> crate::ProviderKind {
        crate::ProviderKind::ProjectAssets
    }

    fn capabilities(&self, _asset: Option<&RemoteAssetRef>) -> ProviderCapabilities {
        ProviderCapabilities {
            browse: Capability::Supported,
            stat: Capability::Supported,
            search: Capability::Unsupported {
                reason: UnsupportedReason::RequiresIndex,
            },
            pin: PinCapability::ImmutableObject,
            range_read: Capability::Supported,
            preview: Capability::Supported,
            fetch: Capability::Supported,
            checksum_hint: Capability::Supported,
            access: AccessMode::ReadOnly,
        }
    }

    #[allow(clippy::too_many_lines)] // one snapshot→entries→page pipeline kept flat
    fn browse(
        &self,
        req: &BrowseRequest,
        cx: &ProviderContext,
    ) -> Result<BrowsePage, ProviderError> {
        if req.page_limit == 0 {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "page_limit must be >= 1",
            ));
        }
        cx.check_interrupts()?;
        let snapshot = self.catalog.catalog()?;
        let epoch = snapshot.revision.0;
        // The cursor scope fingerprint also binds the project identity:
        // a reloaded or swapped catalog must invalidate outstanding
        // continuations even at the same revision counter.
        let scope_fp = fingerprint(&[
            "project",
            snapshot.project_id.to_tagged().as_str(),
            scope_fingerprint(&req.scope).as_str(),
        ]);
        let query_fp = query_fingerprint(req.sort, &req.filter);
        let principal_fp = principal_fingerprint(&cx.principal);
        let offset = match &req.cursor {
            Some(cursor) => {
                let payload =
                    decode_cursor(cursor, self.id, &scope_fp, &query_fp, &principal_fp, |e| {
                        e == epoch
                    })?;
                payload.offset
            }
            None => 0,
        };
        if !matches!(req.scope, BrowseScope::Root) {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "project catalog has a single root scope",
            ));
        }

        let mut entries: Vec<BrowseEntry> = Vec::new();
        let mut truncated = false;
        // Each record costs a blob `stat` — interruption and the entry
        // budget bound the per-asset loop just like the local walk.
        for (walked, record) in snapshot.assets.iter().enumerate() {
            cx.check_interrupts()?;
            if walked >= usize::try_from(cx.limits.max_entries).unwrap_or(usize::MAX) {
                truncated = true;
                break;
            }
            // A tagged id is always a valid locator; a record that
            // somehow is not cannot be addressed anyway.
            let Ok(locator) = OpaqueLocator::new(record.asset_id.to_tagged()) else {
                truncated = true;
                continue;
            };
            let mut facts = EntryFacts {
                byte_len: AssetObservation::unknown(UnavailableReason::RequiresFetch),
                modified: AssetObservation::unknown(UnavailableReason::NotProvided),
                kind: AssetObservation::observed_at(
                    AssetKind::Blob,
                    ObservationOrigin::ProjectModel,
                    cx.now(),
                ),
                checksum_hint: AssetObservation::unknown(UnavailableReason::Missing),
                media: MediaFacts::all_unknown(),
            };
            let availability = match ObjectIdentity::new(record.digest.clone()) {
                Err(_) => Availability::Missing {
                    detail: "asset has no blob identity".to_owned(),
                },
                Ok(identity) => {
                    facts.checksum_hint = AssetObservation::observed_at(
                        identity.clone(),
                        ObservationOrigin::ProjectModel,
                        cx.now(),
                    );
                    match self.blobs.stat(&identity) {
                        Ok(len) => {
                            facts.byte_len = AssetObservation::observed_at(
                                len,
                                ObservationOrigin::Filesystem,
                                cx.now(),
                            );
                            Availability::Available
                        }
                        Err(e) if e.kind() == ProviderErrorKind::NotFound => {
                            Availability::Missing {
                                detail: format!("blob {} not in store", record.digest),
                            }
                        }
                        Err(e) => Availability::Unavailable {
                            reason: UnavailableReason::Other {
                                detail: e.detail().to_owned(),
                            },
                        },
                    }
                }
            };
            entries.push(BrowseEntry {
                locator,
                display_name: record.digest.clone(),
                kind: EntryKind::Asset,
                availability,
                facts,
                via_symlink: false,
            });
        }
        // Apply the request filter on the assembled entries.
        let filter = &req.filter;
        entries.retain(|entry| {
            if let Some(kinds) = &filter.kinds
                && !kinds.contains(&entry.kind)
            {
                return false;
            }
            if let Some(text) = &filter.name_contains
                && !entry
                    .display_name
                    .to_ascii_lowercase()
                    .contains(&text.to_ascii_lowercase())
            {
                return false;
            }
            if filter.extensions.is_some() {
                // Blobs have no extension semantics.
                return false;
            }
            true
        });
        sort_entries(&mut entries, req.sort);

        let total = entries.len() as u64;
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(entries.len());
        let end = start
            .saturating_add(usize::try_from(req.page_limit).unwrap_or(usize::MAX))
            .min(entries.len());
        let page_entries: Vec<BrowseEntry> = entries[start..end].to_vec();
        let emitted = end as u64;
        let next_cursor = if emitted < total {
            Some(encode_cursor(&CursorPayload {
                v: 1,
                provider: self.id.to_tagged(),
                scope_fp,
                query_fp,
                principal_fp,
                epoch,
                offset: emitted,
            })?)
        } else {
            None
        };
        // A live continuation cursor always means the page is a
        // partial view of the result set.
        let coverage = if truncated || next_cursor.is_some() {
            Coverage::Partial
        } else {
            Coverage::Complete
        };
        Ok(BrowsePage {
            entries: page_entries,
            next_cursor,
            coverage,
            observed_at: cx.now(),
        })
    }

    fn stat(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<AssetStat, ProviderError> {
        self.check_owner(asset)?;
        cx.check_interrupts()?;
        let snapshot = self.catalog.catalog()?;
        let record = Self::record_for(&snapshot, asset)?;
        // Provenance is built before the identity check: an asset with
        // an unrepresentable digest still has source/import lineage to
        // report.
        let mut provenance = Vec::new();
        for source in &record.sources {
            provenance.push(ProvenanceRecord {
                kind: "source".to_owned(),
                id: source.source_id.to_tagged(),
                // Canonical serde form ("committed"), not the Debug name.
                state: Some(source_state_name(source.state).to_owned()),
            });
        }
        for import_id in &record.imported_via {
            provenance.push(ProvenanceRecord {
                kind: "import".to_owned(),
                id: import_id.to_tagged(),
                state: None,
            });
        }
        let Ok(identity) = ObjectIdentity::new(record.digest.clone()) else {
            return Ok(AssetStat {
                asset: asset.clone(),
                availability: Availability::Missing {
                    detail: "asset has no blob identity".to_owned(),
                },
                facts: EntryFacts::all_unknown(),
                provenance,
            });
        };
        let (availability, byte_len) = match self.blobs.stat(&identity) {
            Ok(len) => (
                Availability::Available,
                AssetObservation::observed_at(len, ObservationOrigin::Filesystem, cx.now()),
            ),
            Err(e) if e.kind() == ProviderErrorKind::NotFound => (
                Availability::Missing {
                    detail: format!("blob {} not in store", record.digest),
                },
                AssetObservation::unknown(UnavailableReason::Missing),
            ),
            Err(e) => (
                Availability::Unavailable {
                    reason: UnavailableReason::Other {
                        detail: e.detail().to_owned(),
                    },
                },
                AssetObservation::unknown(UnavailableReason::Other {
                    detail: e.detail().to_owned(),
                }),
            ),
        };
        Ok(AssetStat {
            asset: asset.clone(),
            availability,
            facts: EntryFacts {
                byte_len,
                modified: AssetObservation::unknown(UnavailableReason::NotProvided),
                kind: AssetObservation::observed_at(
                    AssetKind::Blob,
                    ObservationOrigin::ProjectModel,
                    cx.now(),
                ),
                checksum_hint: AssetObservation::observed_at(
                    identity,
                    ObservationOrigin::ProjectModel,
                    cx.now(),
                ),
                media: MediaFacts::all_unknown(),
            },
            provenance,
        })
    }

    fn pin(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedAssetRef, ProviderError> {
        self.check_owner(asset)?;
        cx.check_interrupts()?;
        let snapshot = self.catalog.catalog()?;
        let record = Self::record_for(&snapshot, asset)?;
        let identity = ObjectIdentity::new(record.digest.clone()).map_err(|_| {
            ProviderError::new(ProviderErrorKind::NotFound, "asset has no blob identity")
        })?;
        let mut blob = self.blobs.open(&identity)?;
        let (digest, bytes) = stream_digest(
            &mut blob.reader,
            &cx.cancel,
            cx.limits.deadline,
            cx.limits.max_bytes,
        )?;
        Ok(PinnedAssetRef {
            asset: asset.clone(),
            proof: PinProof::ImmutableObject {
                revision: RevisionKey::new(format!("blob:{}", record.digest))?,
            },
            object_identity: identity,
            expected_size: Some(bytes),
            digest_hint: Some(digest),
        })
    }

    fn open_pinned(
        &self,
        pin: &PinnedAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        self.check_owner(&pin.asset)?;
        cx.check_interrupts()?;
        match &pin.proof {
            PinProof::ImmutableObject { .. } => {}
            PinProof::ConditionalStrong { .. } => {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "project provider only opens ImmutableObject pins",
                ));
            }
        }
        let snapshot = self.catalog.catalog()?;
        let record = Self::record_for(&snapshot, &pin.asset)?;
        let expected_identity = ObjectIdentity::new(record.digest.clone()).map_err(|_| {
            ProviderError::new(ProviderErrorKind::NotFound, "asset has no blob identity")
        })?;
        if pin.object_identity != expected_identity {
            // The catalog now maps this asset to different bytes: the
            // pin did not lie — the source moved under it.
            return Err(ProviderError::new(
                ProviderErrorKind::SourceChanged,
                format!(
                    "pin object {} does not match project asset {}",
                    pin.object_identity.as_str(),
                    record.digest
                ),
            ));
        }
        let (verification, expected_digest) = match &pin.digest_hint {
            Some(digest) => (
                VerificationPolicy::StreamVerified {
                    digest: digest.clone(),
                },
                Some(digest.clone()),
            ),
            None => (
                VerificationPolicy::IdentityBound {
                    scheme: "cas-blob".to_owned(),
                },
                None,
            ),
        };
        // Resolve by the pinned object identity — the pin's claim —
        // not the (mutable) catalog digest.
        self.open_blob_verified(
            record,
            pin.object_identity.clone(),
            None,
            verification,
            expected_digest,
            pin.expected_size,
            cx,
        )
    }

    fn open_preview(
        &self,
        asset: &RemoteAssetRef,
        range: Option<ByteRange>,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        self.check_owner(asset)?;
        cx.check_interrupts()?;
        let snapshot = self.catalog.catalog()?;
        let record = Self::record_for(&snapshot, asset)?;
        let identity = ObjectIdentity::new(record.digest.clone()).map_err(|_| {
            ProviderError::new(ProviderErrorKind::NotFound, "asset has no blob identity")
        })?;
        self.open_blob_verified(
            record,
            identity,
            range,
            VerificationPolicy::Unverified,
            None,
            None,
            cx,
        )
    }
}
