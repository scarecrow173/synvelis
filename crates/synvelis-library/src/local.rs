//! `LocalFolderProvider` — the baseline local-filesystem provider.
//!
//! Locators are `/`-separated paths relative to the provider root.
//! Resolution always canonicalizes and re-checks containment in the
//! canonical root, so `..`, absolute paths, and symlink escapes cannot
//! traverse out. Browsing is a bounded, deterministic walk: directory
//! children are sorted by name, symlinked directories are descended
//! only when their canonical target stays under the root and was not
//! visited before (loop cut), and every call re-walks the scope under
//! the configured budgets — a cursor resumes by offset, never by
//! pretending a live tree was snapshotted.
//!
//! Local files are mutable, so `pin` produces a
//! `ConditionalStrong{ContentHash}` proof: it streams the file to
//! measure SHA-256 and `open_pinned` re-verifies that digest at EOF.
//! A mutated, replaced, or resized file fails with `SourceChanged`
//! (size precondition) or `IntegrityMismatch` (digest) — never returns
//! different bytes labelled as pinned.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::browse::{
    BrowseEntry, BrowsePage, BrowseRequest, BrowseScope, Coverage, CursorPayload, EntryKind,
    FilterSpec, OpaquePageCursor, SearchQuery, decode_cursor, encode_cursor, principal_fingerprint,
    query_fingerprint, scope_fingerprint, sort_entries,
};
use crate::capability::{AccessMode, Capability, PinCapability, ProviderCapabilities};
use crate::context::ProviderContext;
use crate::digest::{completion_slot, stream_digest};
use crate::error::{ProviderError, ProviderErrorKind};
use crate::id::{OpaqueLocator, ProviderId};
use crate::observation::{
    AssetKind, AssetObservation, AssetStat, Availability, EntryFacts, MediaFacts,
    ObservationOrigin, UnavailableReason,
};
use crate::project::PreTraverseHook;
use crate::provider::{
    AssetProvider, ByteRange, FetchClaims, FetchCompletion, PinnedRead, VerificationPolicy,
};
use crate::refs::{
    ContentDigest, ObjectIdentity, PinProof, PinnedAssetRef, RemoteAssetRef, StrongRevisionToken,
};

/// Maximum browse sessions whose cursors stay valid at once.
const MAX_LIVE_EPOCHS: usize = 512;

/// Baseline provider over one local filesystem root.
pub struct LocalFolderProvider {
    id: ProviderId,
    /// Canonical root — every resolved path must stay beneath it.
    root: PathBuf,
    /// Issued browse epochs whose cursors are still honored.
    live_epochs: Mutex<BTreeSet<u64>>,
    next_epoch: AtomicU64,
    /// Deterministic fault barrier for containment regression tests —
    /// invoked immediately before each filesystem call that
    /// re-traverses a path after the containment decision.
    pre_traverse_hook: Option<PreTraverseHook>,
}

impl LocalFolderProvider {
    /// Opens `root` as a provider. `root` must exist and be a
    /// directory; it is canonicalized once here.
    ///
    /// # Errors
    /// [`ProviderErrorKind::NotFound`] / [`ProviderErrorKind::InvalidRequest`]
    /// when the root does not exist or is not a directory.
    pub fn new(id: ProviderId, root: impl AsRef<Path>) -> Result<Self, ProviderError> {
        let root = root.as_ref();
        let canonical = fs::canonicalize(root).map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::NotFound,
                format!("local root {}: {e}", root.display()),
            )
        })?;
        if !canonical.is_dir() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("local root {} is not a directory", canonical.display()),
            ));
        }
        Ok(Self {
            id,
            root: canonical,
            live_epochs: Mutex::new(BTreeSet::new()),
            next_epoch: AtomicU64::new(1),
            pre_traverse_hook: None,
        })
    }

    /// Installs the deterministic pre-traversal fault barrier used by
    /// containment regression tests. `None` restores normal behavior.
    /// Op tags: `"stat"`/`"open"`/`"read_dir"`. Not a stable contract.
    #[doc(hidden)]
    pub fn set_pre_traverse_hook(&mut self, hook: Option<PreTraverseHook>) {
        self.pre_traverse_hook = hook;
    }

    fn pre_traverse(&self, path: &Path, op: &'static str) {
        if let Some(hook) = &self.pre_traverse_hook {
            hook(path, op);
        }
    }

    /// The canonical root this provider serves.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Splits a `/`-separated relative locator into components,
    /// rejecting traversal, absolute, and empty forms.
    fn parse_locator(locator: &OpaqueLocator) -> Result<Vec<String>, ProviderError> {
        let text = locator.as_str();
        let invalid = |detail: &str| {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("invalid locator {text:?}: {detail}"),
            )
        };
        if text.contains('\\') {
            return Err(invalid("backslash separators are not portable"));
        }
        if text.starts_with('/') || text.contains(':') {
            return Err(invalid("absolute paths are not locators"));
        }
        let mut components = Vec::new();
        for part in text.split('/') {
            match part {
                "" | "." => return Err(invalid("empty or dot component")),
                ".." => return Err(invalid("parent traversal is rejected")),
                _ => components.push(part.to_owned()),
            }
        }
        Ok(components)
    }

    /// Joins a validated locator onto the root without resolving
    /// links — used to inspect the link entry itself.
    fn join_locator(&self, locator: &OpaqueLocator) -> Result<PathBuf, ProviderError> {
        let mut path = self.root.clone();
        for part in Self::parse_locator(locator)? {
            path.push(part);
        }
        Ok(path)
    }

    /// Resolves a locator to a canonical path under the root.
    fn resolve(&self, locator: &OpaqueLocator) -> Result<PathBuf, ProviderError> {
        let path = self.join_locator(locator)?;
        let canonical = fs::canonicalize(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ProviderError::new(
                ProviderErrorKind::NotFound,
                format!("locator {locator} not found"),
            ),
            std::io::ErrorKind::PermissionDenied => ProviderError::new(
                ProviderErrorKind::PermissionDenied,
                format!("locator {locator}: {e}"),
            ),
            _ => ProviderError::new(
                ProviderErrorKind::TransientIo,
                format!("resolve {locator}: {e}"),
            ),
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(ProviderError::new(
                ProviderErrorKind::PermissionDenied,
                format!("locator {locator} escapes the provider root"),
            ));
        }
        Ok(canonical)
    }

    /// The canonical target of a directory entry, when it stays under
    /// the root and exists.
    fn contained_target(path: &Path, root: &Path) -> Option<PathBuf> {
        fs::canonicalize(path).ok().filter(|c| c.starts_with(root))
    }

    /// Builds the relative `/`-joined locator for a path under root.
    /// `None` when the name cannot be represented (over the opaque
    /// bound) — unaddressable entries are skipped, never aliased.
    fn locator_for(&self, path: &Path) -> Option<OpaqueLocator> {
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        let text = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        OpaqueLocator::new(text).ok()
    }

    fn entry_facts(meta: &fs::Metadata, kind: AssetKind, at: crate::ObservedAt) -> EntryFacts {
        let origin = ObservationOrigin::Filesystem;
        EntryFacts {
            byte_len: AssetObservation::observed_at(meta.len(), origin, at),
            modified: match meta.modified() {
                Ok(t) => AssetObservation::observed_at(crate::ObservedAt(t), origin, at),
                Err(_) => AssetObservation::unknown(UnavailableReason::Other {
                    detail: "mtime unavailable".into(),
                }),
            },
            kind: AssetObservation::observed_at(kind, origin, at),
            checksum_hint: AssetObservation::unknown(UnavailableReason::RequiresFetch),
            media: MediaFacts::all_unknown(),
        }
    }

    /// Walks the scope and collects entries, bounded by `cx.limits`.
    /// Returns `(entries, truncated)`.
    #[allow(clippy::too_many_lines)] // one bounded stack-walk kept flat
    fn collect_entries(
        &self,
        scope_dir: &Path,
        filter: &FilterSpec,
        cx: &ProviderContext,
    ) -> Result<(Vec<BrowseEntry>, bool), ProviderError> {
        let mut entries = Vec::new();
        let mut seen_locators: BTreeSet<String> = BTreeSet::new();
        // The scope root is pre-seeded: a symlink targeting it (or an
        // ancestor) can never re-enter the scope and re-emit entries.
        let mut visited: BTreeSet<PathBuf> = BTreeSet::from([scope_dir.to_path_buf()]);
        let mut truncated = false;
        let mut walked = 0u32;
        let mut stack = vec![(scope_dir.to_path_buf(), 0u32)];

        'walk: while let Some((dir, depth)) = stack.pop() {
            cx.check_interrupts()?;
            self.pre_traverse(&dir, "read_dir");
            let read = match fs::read_dir(&dir) {
                Ok(read) => read,
                Err(e) => {
                    if dir == scope_dir {
                        return Err(io_error("read scope", &e));
                    }
                    // Unreadable subdirectory: skip it and mark the
                    // page partial instead of faking completeness.
                    truncated = true;
                    continue;
                }
            };
            // Bounded by the remaining entry budget so a pathological
            // directory cannot defeat `max_entries`' memory bound.
            let take = usize::try_from(
                cx.limits
                    .max_entries
                    .saturating_sub(walked)
                    .saturating_add(1),
            )
            .unwrap_or(usize::MAX);
            let mut children: Vec<fs::DirEntry> = read.filter_map(Result::ok).take(take).collect();
            children.sort_by_key(fs::DirEntry::file_name);
            for child in children {
                if walked >= cx.limits.max_entries {
                    truncated = true;
                    break 'walk;
                }
                walked += 1;
                let path = child.path();
                self.pre_traverse(&path, "stat");
                let Ok(meta) = child.metadata() else {
                    continue;
                };
                // `DirEntry::file_type` reports the enumeration-time
                // type without traversing the link — `metadata()` may
                // resolve reparse points on some platforms.
                let Ok(ft) = child.file_type() else {
                    continue;
                };
                let via_symlink = ft.is_symlink();
                let mut kind = EntryKind::Asset;
                let asset_kind = if ft.is_dir() {
                    AssetKind::Directory
                } else if via_symlink {
                    AssetKind::Symlink
                } else {
                    AssetKind::File
                };
                let mut availability = Availability::Available;
                // The directory to descend into: plain dirs descend
                // into their own path (canonical since the parent is),
                // symlinked dirs into their canonical root-contained
                // target.
                let mut descend: Option<PathBuf> = None;

                if ft.is_dir() {
                    kind = EntryKind::Container;
                    descend = Some(path.clone());
                } else if via_symlink {
                    // Links are followed only while their target stays
                    // inside the *scope* subtree — an ancestor- or
                    // sibling-bound link never leaks out-of-scope
                    // entries.
                    match Self::contained_target(&path, scope_dir) {
                        Some(target)
                            if {
                                self.pre_traverse(&target, "stat");
                                target.is_dir()
                            } =>
                        {
                            kind = EntryKind::Container;
                            descend = Some(target);
                        }
                        Some(_) => {
                            // Link to a file inside the scope: an asset.
                        }
                        None => {
                            // Escapes the scope or dangling: listed as an
                            // observed entry, but never navigated or
                            // opened through the link.
                            availability = if fs::canonicalize(&path).is_ok() {
                                Availability::Unavailable {
                                    reason: UnavailableReason::Other {
                                        detail: "symlink target outside browse scope".into(),
                                    },
                                }
                            } else {
                                Availability::Missing {
                                    detail: "dangling symlink".into(),
                                }
                            };
                        }
                    }
                }

                if Self::matches_filter(&child.file_name().to_string_lossy(), &path, kind, filter) {
                    // Entries that cannot produce a unique locator are
                    // unaddressable — skipped and reported as partial,
                    // never aliased to a bogus identity.
                    match self.locator_for(&path) {
                        Some(locator) if seen_locators.insert(locator.as_str().to_owned()) => {
                            entries.push(BrowseEntry {
                                locator,
                                display_name: child.file_name().to_string_lossy().into_owned(),
                                kind,
                                availability,
                                facts: Self::entry_facts(&meta, asset_kind, cx.now()),
                                via_symlink,
                            });
                        }
                        _ => truncated = true,
                    }
                }

                if let Some(target) = descend {
                    if depth + 1 >= cx.limits.max_depth {
                        truncated = true;
                    } else if visited.insert(target.clone()) {
                        stack.push((target, depth + 1));
                    }
                    // else: already visited — loop cut.
                }
            }
        }
        Ok((entries, truncated))
    }

    fn matches_filter(name: &str, path: &Path, kind: EntryKind, filter: &FilterSpec) -> bool {
        if let Some(kinds) = &filter.kinds
            && !kinds.contains(&kind)
        {
            return false;
        }
        if let Some(text) = &filter.name_contains
            && !name
                .to_ascii_lowercase()
                .contains(&text.to_ascii_lowercase())
        {
            return false;
        }
        if let Some(exts) = &filter.extensions {
            if kind != EntryKind::Asset {
                return false;
            }
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if !exts.iter().any(|want| want.eq_ignore_ascii_case(&ext)) {
                return false;
            }
        }
        true
    }

    fn issue_epoch(&self) -> u64 {
        let epoch = self.next_epoch.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut epochs) = self.live_epochs.lock() {
            while epochs.len() >= MAX_LIVE_EPOCHS {
                if let Some(oldest) = epochs.iter().next().copied() {
                    epochs.remove(&oldest);
                } else {
                    break;
                }
            }
            epochs.insert(epoch);
        }
        epoch
    }

    fn epoch_valid(&self, epoch: u64) -> bool {
        self.live_epochs
            .lock()
            .is_ok_and(|epochs| epochs.contains(&epoch))
    }

    /// Shared browse/search pipeline: resolve scope, collect entries,
    /// sort, page, emit cursor.
    fn list_page(
        &self,
        scope: &BrowseScope,
        filter: &FilterSpec,
        sort: crate::SortSpec,
        page_limit: u32,
        cursor: Option<&OpaquePageCursor>,
        cx: &ProviderContext,
    ) -> Result<BrowsePage, ProviderError> {
        if page_limit == 0 {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "page_limit must be >= 1",
            ));
        }
        let scope_fp = scope_fingerprint(scope);
        let query_fp = query_fingerprint(sort, filter);
        let principal_fp = principal_fingerprint(&cx.principal);
        // Epochs are minted only when a page actually emits a
        // continuation cursor — failed browses and single-page results
        // must not burn live-epoch slots.
        let (offset, epoch) = match cursor {
            Some(cursor) => {
                let payload = decode_cursor(
                    cursor,
                    self.id,
                    &scope_fp,
                    &query_fp,
                    &principal_fp,
                    |epoch| self.epoch_valid(epoch),
                )?;
                (payload.offset, Some(payload.epoch))
            }
            None => (0, None),
        };

        let scope_dir = match scope {
            BrowseScope::Root => self.root.clone(),
            BrowseScope::Locator { locator } => {
                let dir = self.resolve(locator)?;
                self.pre_traverse(&dir, "stat");
                if !dir.is_dir() {
                    return Err(ProviderError::new(
                        ProviderErrorKind::InvalidRequest,
                        format!("scope {locator} is not a container"),
                    ));
                }
                dir
            }
        };

        let (mut entries, truncated) = self.collect_entries(&scope_dir, filter, cx)?;
        sort_entries(&mut entries, sort);
        let total = entries.len() as u64;
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(entries.len());
        let end = start
            .saturating_add(usize::try_from(page_limit).unwrap_or(usize::MAX))
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
                epoch: epoch.unwrap_or_else(|| self.issue_epoch()),
                offset: emitted,
            })?)
        } else {
            None
        };
        let coverage = if next_cursor.is_none() && !truncated {
            Coverage::Complete
        } else {
            Coverage::Partial
        };
        Ok(BrowsePage {
            entries: page_entries,
            next_cursor,
            coverage,
            observed_at: cx.now(),
        })
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

    #[allow(clippy::too_many_arguments)] // one bundle per verification knob stays flat
    fn open_file_verified(
        &self,
        asset: &RemoteAssetRef,
        range: Option<ByteRange>,
        claims_verification: VerificationPolicy,
        expected_digest: Option<ContentDigest>,
        expected_size: Option<u64>,
        object_identity: ObjectIdentity,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        let path = self.resolve(&asset.locator)?;
        self.pre_traverse(&path, "open");
        let mut file = File::open(&path).map_err(|e| io_error("open", &e))?;
        let file_meta = file.metadata().map_err(|e| io_error("stat", &e))?;
        // Post-open type check: a regular file swapped for a non-file
        // between resolve and open must never be streamed.
        if !file_meta.is_file() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("{} is not a regular file", asset.locator),
            ));
        }
        let file_len = file_meta.len();
        if let Some(expected) = expected_size
            && file_len != expected
        {
            return Err(ProviderError::new(
                ProviderErrorKind::SourceChanged,
                format!(
                    "{} changed since pin: size {file_len} != {expected}",
                    asset.locator
                ),
            ));
        }
        if file_len > cx.limits.max_bytes {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                format!(
                    "{} exceeds byte budget {}",
                    asset.locator, cx.limits.max_bytes
                ),
            ));
        }
        let reader: Box<dyn Read + Send> = match range {
            Some(range) => {
                use std::io::Seek;
                file.seek(std::io::SeekFrom::Start(range.offset))
                    .map_err(|e| io_error("seek", &e))?;
                // +1 so crossing the budget surfaces as BudgetExceeded
                // rather than a silent truncation.
                let cap = range.len.min(cx.limits.max_bytes.saturating_add(1));
                Box::new(file.take(cap))
            }
            None => Box::new(file.take(cx.limits.max_bytes.saturating_add(1))),
        };
        let slot = completion_slot();
        let reader = crate::digest::VerifyingReader::new(
            reader,
            expected_digest,
            expected_size,
            cx.limits.max_bytes,
            cx.cancel.clone(),
            cx.limits.deadline,
            object_identity.clone(),
            slot.clone(),
        );
        Ok(PinnedRead {
            reader: Box::new(reader),
            claims: FetchClaims {
                object_identity,
                expected_size,
                verification: claims_verification,
            },
            completion: FetchCompletion::new(slot),
        })
    }
}

fn io_error(op: &'static str, e: &std::io::Error) -> ProviderError {
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

impl AssetProvider for LocalFolderProvider {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn kind(&self) -> crate::ProviderKind {
        crate::ProviderKind::LocalFolder
    }

    fn capabilities(&self, _asset: Option<&RemoteAssetRef>) -> ProviderCapabilities {
        ProviderCapabilities {
            browse: Capability::Supported,
            stat: Capability::Supported,
            search: Capability::Supported,
            pin: PinCapability::ConditionalStrong,
            range_read: Capability::Supported,
            preview: Capability::Supported,
            fetch: Capability::Supported,
            checksum_hint: Capability::Supported,
            access: AccessMode::ReadOnly,
        }
    }

    fn browse(
        &self,
        req: &BrowseRequest,
        cx: &ProviderContext,
    ) -> Result<BrowsePage, ProviderError> {
        self.list_page(
            &req.scope,
            &req.filter,
            req.sort,
            req.page_limit,
            req.cursor.as_ref(),
            cx,
        )
    }

    fn stat(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<AssetStat, ProviderError> {
        self.check_owner(asset)?;
        cx.check_interrupts()?;
        // Inspect the link entry itself for the kind, then resolve the
        // canonical target for facts — a symlinked locator reports
        // `Symlink`, never silently the target's kind.
        let joined = self.join_locator(&asset.locator)?;
        let link_meta = fs::symlink_metadata(&joined).map_err(|e| io_error("stat", &e))?;
        let kind = if link_meta.file_type().is_symlink() {
            AssetKind::Symlink
        } else if link_meta.is_dir() {
            AssetKind::Directory
        } else {
            AssetKind::File
        };
        let path = self.resolve(&asset.locator)?;
        self.pre_traverse(&path, "stat");
        let meta = fs::metadata(&path).map_err(|e| io_error("stat", &e))?;
        Ok(AssetStat {
            asset: asset.clone(),
            availability: Availability::Available,
            facts: Self::entry_facts(&meta, kind, cx.now()),
            provenance: Vec::new(),
        })
    }

    fn pin(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedAssetRef, ProviderError> {
        self.check_owner(asset)?;
        cx.check_interrupts()?;
        let path = self.resolve(&asset.locator)?;
        self.pre_traverse(&path, "stat");
        let meta = fs::metadata(&path).map_err(|e| io_error("stat", &e))?;
        if !meta.is_file() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("cannot pin a container locator {}", asset.locator),
            ));
        }
        if meta.len() > cx.limits.max_bytes {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                format!(
                    "{} exceeds byte budget {}",
                    asset.locator, cx.limits.max_bytes
                ),
            ));
        }
        self.pre_traverse(&path, "open");
        let mut file = File::open(&path).map_err(|e| io_error("open", &e))?;
        // Post-open re-check: a stat→open swap to a non-file must
        // never be digested as pinned content.
        if !file.metadata().map_err(|e| io_error("stat", &e))?.is_file() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("{} is not a regular file", asset.locator),
            ));
        }
        let (digest, bytes) = stream_digest(
            &mut file,
            &cx.cancel,
            cx.limits.deadline,
            cx.limits.max_bytes,
        )?;
        Ok(PinnedAssetRef {
            asset: asset.clone(),
            proof: PinProof::ConditionalStrong {
                token: StrongRevisionToken::ContentHash {
                    digest: digest.clone(),
                },
            },
            object_identity: ObjectIdentity::new(digest.to_string())
                .expect("digest display is non-empty"),
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
        let digest = match &pin.proof {
            PinProof::ConditionalStrong {
                token: StrongRevisionToken::ContentHash { digest },
            } => digest.clone(),
            _ => {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "local provider only opens ConditionalStrong{ContentHash} pins",
                ));
            }
        };
        self.open_file_verified(
            &pin.asset,
            None,
            VerificationPolicy::StreamVerified {
                digest: digest.clone(),
            },
            Some(digest),
            pin.expected_size,
            pin.object_identity.clone(),
            cx,
        )
    }

    fn search(
        &self,
        query: &SearchQuery,
        cx: &ProviderContext,
    ) -> Result<BrowsePage, ProviderError> {
        let filter = FilterSpec {
            name_contains: Some(query.text.clone()),
            ..FilterSpec::default()
        };
        self.list_page(
            &query.scope,
            &filter,
            crate::SortSpec::default(),
            query.page_limit,
            query.cursor.as_ref(),
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
        let path = self.resolve(&asset.locator)?;
        self.pre_traverse(&path, "stat");
        if !path.is_file() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("cannot preview container {}", asset.locator),
            ));
        }
        self.open_file_verified(
            asset,
            range,
            VerificationPolicy::Unverified,
            None,
            None,
            ObjectIdentity::new(asset.locator.as_str())?,
            cx,
        )
    }
}
