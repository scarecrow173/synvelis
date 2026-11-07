//! `LocalFolderProvider` — the baseline local-filesystem provider.
//!
//! Locators are `/`-separated paths relative to the provider root.
//! All filesystem access goes through one Ruviform `RootedDir`
//! capability: the root directory handle is acquired once at
//! construction and every lookup below it is resolved handle-
//! relative (`openat`-family semantics). A leaf or intermediate
//! component swapped after the capability was acquired cannot
//! substitute an object outside the root — there is no path
//! re-traversal below the root for an attacker to race. Bytes,
//! metadata, and directory entries always come from the same opened
//! object (`RootedFile` + `NodeInfo`), and link following under
//! `WithinRoot` is enforced by the resolve layer's canonical-root
//! component check — a link that would resolve outside fails
//! `EscapesRoot`.
//!
//! Browsing is a bounded, deterministic walk over held directory
//! handles: children are sorted by name, real directories are
//! descended through scoped `open_dir` handles (a swap after listing
//! cannot inject out-of-scope entries), and symlinked entries are
//! probed with `metadata(WithinRoot)` — a target inside the scope
//! capability reports `Container`; one escaping it fails
//! `EscapesRoot` and is listed as observed-but-unavailable. A
//! locator scope is itself opened as a rooted capability, so the
//! scope subtree is the containment boundary exactly as the old
//! canonical-prefix check enforced. Every call re-walks the scope
//! under the configured budgets — a cursor resumes by offset, never
//! by pretending a live tree was snapshotted.
//!
//! Local files are mutable, so `pin` produces a
//! `ConditionalStrong{ContentHash}` proof: it streams the file to
//! measure SHA-256 and `open_pinned` re-verifies that digest at EOF.
//! Content pinning and root containment are separate guarantees: the
//! capability keeps *which object* is opened inside the root, while
//! the digest verifies *what bytes* the pinned content had — a
//! digest computed at EOF could never retract bytes already served,
//! so it was never an acceptable substitute for containment.

use std::collections::{BTreeSet, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_io_file::rooted::{
    FollowMode, IdentityStrength, NodeId, NodeInfo, NodeKind, RelativePath, RootedDir, RootedFile,
    RootedOptions,
};
use ruviform_io_file::{FileError, FileErrorKind};

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

/// Resolve budgets for every rooted capability this crate opens.
/// The defaults bound locator depth at 40 components — tighter than
/// the historical canonicalize-based resolver's practical
/// acceptance — so depth/total/open-dirs are raised to stay well
/// above existing trees while remaining finite.
pub(crate) fn rooted_options() -> RootedOptions {
    let mut options = RootedOptions::default();
    options.max_depth = 256;
    options.max_components_total = 2048;
    options.max_open_dirs = 256;
    options
}

/// Baseline provider over one local filesystem root.
pub struct LocalFolderProvider {
    id: ProviderId,
    /// Rooted capability — every lookup below it is handle-relative.
    root: RootedDir,
    /// Issued browse epochs whose cursors are still honored.
    live_epochs: Mutex<BTreeSet<u64>>,
    next_epoch: AtomicU64,
    /// Deterministic fault barrier for containment regression tests —
    /// invoked immediately before each rooted operation that touches
    /// the filesystem (`"stat"`/`"open"`/`"read_dir"`). Not a stable
    /// contract.
    pre_traverse_hook: Option<PreTraverseHook>,
}

/// Maps a [`FileError`] onto the provider error surface. `Io` errors
/// keep the raw `io::ErrorKind` through `source` so `NotFound` /
/// `PermissionDenied` stay precise; resolve-layer kinds map onto the
/// honest typed surface (`EscapesRoot` → `PermissionDenied`,
/// `Unsupported` → `UnsupportedCapability`, `IdentityMismatch` →
/// `SourceChanged`, `LimitExceeded` → `BudgetExceeded`). Loops and
/// budget rejections are deterministic — never `TransientIo`, which
/// would advise a pointless retry.
pub(crate) fn rooted_error(op: &'static str, e: &FileError) -> ProviderError {
    let kind = match e.kind() {
        FileErrorKind::NotFound => ProviderErrorKind::NotFound,
        FileErrorKind::EscapesRoot => ProviderErrorKind::PermissionDenied,
        FileErrorKind::Unsupported => ProviderErrorKind::UnsupportedCapability,
        FileErrorKind::InvalidInput
        | FileErrorKind::NotRegularFile
        | FileErrorKind::NotDirectory
        | FileErrorKind::LoopDetected => ProviderErrorKind::InvalidRequest,
        FileErrorKind::LimitExceeded => ProviderErrorKind::BudgetExceeded,
        FileErrorKind::IdentityMismatch => ProviderErrorKind::SourceChanged,
        FileErrorKind::Io => match io_source_kind(e) {
            Some(std::io::ErrorKind::NotFound) => ProviderErrorKind::NotFound,
            Some(std::io::ErrorKind::PermissionDenied) => ProviderErrorKind::PermissionDenied,
            _ => ProviderErrorKind::TransientIo,
        },
        _ => ProviderErrorKind::Internal,
    };
    ProviderError::new(kind, format!("{op}: {e}"))
}

/// The raw `io::ErrorKind` behind a [`FileError::Io`], when the
/// backend attached one as the diagnostic source.
fn io_source_kind(e: &FileError) -> Option<std::io::ErrorKind> {
    std::error::Error::source(e)
        .and_then(|s| s.downcast_ref::<std::io::Error>())
        .map(std::io::Error::kind)
}

impl LocalFolderProvider {
    /// Opens `root` as a provider. `root` must exist and be a
    /// directory; its handle becomes the containment authority for
    /// every later lookup.
    ///
    /// # Errors
    /// [`ProviderErrorKind::NotFound`] / [`ProviderErrorKind::InvalidRequest`]
    /// when the root does not exist or is not a directory;
    /// [`ProviderErrorKind::UnsupportedCapability`] on platforms
    /// without a handle-relative backend.
    pub fn new(id: ProviderId, root: impl AsRef<Path>) -> Result<Self, ProviderError> {
        let root = root.as_ref();
        let rooted = RootedDir::open_with(root, rooted_options()).map_err(|e| match e.kind() {
            FileErrorKind::NotFound => ProviderError::new(
                ProviderErrorKind::NotFound,
                format!("local root {}: {e}", root.display()),
            ),
            FileErrorKind::NotDirectory | FileErrorKind::NotRegularFile => ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("local root {} is not a directory", root.display()),
            ),
            _ => rooted_error("open local root", &e),
        })?;
        Ok(Self {
            id,
            root: rooted,
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
        self.root.canonical_root()
    }

    /// Validates a caller locator into a [`RelativePath`]. The
    /// grammar rejects `..`, absolute/prefixed forms, `\`, `:`, NUL,
    /// control chars, Windows-reserved names, and >255-byte
    /// components — the lexical layer of containment.
    fn locator_rel(locator: &OpaqueLocator) -> Result<RelativePath, ProviderError> {
        RelativePath::new(locator.as_str()).map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("invalid locator {:?}: {e}", locator.as_str()),
            )
        })
    }

    /// Diagnostic lexical path under the capability root for a rel —
    /// used only as the fault-barrier argument, never for I/O.
    fn lexical(root: &Path, rel: &str) -> PathBuf {
        let mut path = root.to_path_buf();
        for comp in rel.split('/') {
            if !comp.is_empty() {
                path.push(comp);
            }
        }
        path
    }

    /// The `AssetKind` a resolved object reports: a leaf link always
    /// reports `Symlink` — the target's kind never leaks through.
    fn asset_kind(info: &NodeInfo) -> AssetKind {
        if info.leaf_kind == NodeKind::Symlink {
            return AssetKind::Symlink;
        }
        match info.kind {
            NodeKind::File => AssetKind::File,
            NodeKind::Dir => AssetKind::Directory,
            NodeKind::Symlink => AssetKind::Symlink,
            _ => AssetKind::Other {
                label: "other".into(),
            },
        }
    }

    fn node_facts(info: &NodeInfo, kind: AssetKind, at: crate::ObservedAt) -> EntryFacts {
        let origin = ObservationOrigin::Filesystem;
        EntryFacts {
            byte_len: match info.byte_len {
                Some(len) => AssetObservation::observed_at(len, origin, at),
                // Directories and other non-length kinds carry no
                // byte length — reported honestly, not faked.
                None => AssetObservation::unknown(UnavailableReason::NotApplicable),
            },
            modified: match info.modified {
                Some(t) => AssetObservation::observed_at(crate::ObservedAt(t), origin, at),
                None => AssetObservation::unknown(UnavailableReason::Other {
                    detail: "mtime unavailable".into(),
                }),
            },
            kind: AssetObservation::observed_at(kind, origin, at),
            checksum_hint: AssetObservation::unknown(UnavailableReason::RequiresFetch),
            media: MediaFacts::all_unknown(),
        }
    }

    /// Facts for a listed-but-unreachable link: the leaf is known to
    /// be a symlink (the enumeration reports it) but nothing about
    /// the target was served.
    fn unavailable_link_facts(at: crate::ObservedAt) -> EntryFacts {
        let origin = ObservationOrigin::Filesystem;
        EntryFacts {
            byte_len: AssetObservation::unknown(UnavailableReason::NotProvided),
            modified: AssetObservation::unknown(UnavailableReason::NotProvided),
            kind: AssetObservation::observed_at(AssetKind::Symlink, origin, at),
            checksum_hint: AssetObservation::unknown(UnavailableReason::RequiresFetch),
            media: MediaFacts::all_unknown(),
        }
    }

    /// Opens a scoped rooted capability for a locator scope. The
    /// scope subtree is itself the capability root, so `WithinRoot`
    /// enforces exactly the old canonical-prefix scope containment —
    /// an ancestor- or sibling-bound link inside the scope fails
    /// `EscapesRoot`.
    ///
    /// Authority comes from the held provider capability, never the
    /// root pathname alone: the scope is first resolved relative to
    /// `self.root` (held handles — the root may have been renamed
    /// since construction), which also enforces link containment on
    /// the locator itself. `RootedDir::open_dir` cannot be used for
    /// the scoped capability because its `WithinRoot` boundary stays
    /// the provider root — a scope-internal link to an in-root
    /// sibling would then resolve instead of failing `EscapesRoot`.
    /// `open_with` on the canonical path therefore acquires the
    /// scope-subtree capability, and the opened object is verified
    /// against the held-capability resolution by `NodeId`: a
    /// renamed/replaced root pathname that now names a different
    /// tree fails `SourceChanged` instead of rebinding the scope.
    /// The two filesystem touch points carry separate fault-barrier
    /// tags — `"stat"` before the held-resolution `metadata`,
    /// `"open"` before the `open_with` acquisition.
    fn open_scope(&self, locator: &OpaqueLocator) -> Result<(RootedDir, String), ProviderError> {
        let rel = Self::locator_rel(locator)?;
        let mut lexical = self.root.canonical_root().to_path_buf();
        for comp in rel.components() {
            lexical.push(comp);
        }
        self.pre_traverse(&lexical, "stat");
        let scope_info = self
            .root
            .metadata(&rel, FollowMode::WithinRoot)
            .map_err(|e| rooted_error("browse scope", &e))?;
        if scope_info.kind != NodeKind::Dir {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("scope {locator} is not a container"),
            ));
        }
        // The `NodeId` equality check below is only as strong as the
        // discriminator behind it: a weak-identity filesystem cannot
        // tell a same-tick delete+recreate apart, so the rebind
        // window it is meant to close would stay open silently.
        // Mirror RF's `open_pinned` gate and refuse rather than
        // pretend the check held (RF rooted.rs `Unsupported` on
        // non-`Strong` strength).
        if scope_info.identity_strength != IdentityStrength::Strong {
            return Err(ProviderError::new(
                ProviderErrorKind::UnsupportedCapability,
                format!("scope {locator} cannot be safely acquired: filesystem identity is weak"),
            ));
        }
        self.pre_traverse(&lexical, "open");
        let cap = RootedDir::open_with(&lexical, rooted_options()).map_err(|e| {
            if matches!(
                e.kind(),
                FileErrorKind::NotDirectory | FileErrorKind::NotRegularFile
            ) || io_source_kind(&e) == Some(std::io::ErrorKind::NotADirectory)
            {
                ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    format!("scope {locator} is not a container"),
                )
            } else {
                rooted_error("browse scope", &e)
            }
        })?;
        if !cap.canonical_root().starts_with(self.root.canonical_root()) {
            return Err(ProviderError::new(
                ProviderErrorKind::PermissionDenied,
                format!("scope {locator} escapes the provider root"),
            ));
        }
        // Same-object check across the acquisition boundary: the
        // pathname may have been repointed at a different tree since
        // the held-capability resolution above. Comparing `NodeId`s
        // — not paths — is what makes the opened capability's
        // authority match the held root's. The `Strong`-strength
        // gate above keeps this meaningful on weak-identity
        // filesystems; a backend whose `identity()` call itself
        // fails fails closed via `rooted_error`.
        if cap
            .dir_identity()
            .map_err(|e| rooted_error("browse scope", &e))?
            != scope_info.identity
        {
            return Err(ProviderError::new(
                ProviderErrorKind::SourceChanged,
                format!(
                    "scope {locator} resolved to a different object under the provider root pathname"
                ),
            ));
        }
        let base = cap
            .canonical_root()
            .strip_prefix(self.root.canonical_root())
            .map(|rel| {
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default();
        Ok((cap, base))
    }

    /// Walks the scope and collects entries, bounded by `cx.limits`.
    /// Descent is over held scoped handles — directories are opened
    /// once through `open_dir` and enumerated from the acquired
    /// handle, so an intermediate swap cannot redirect the listing.
    /// Returns `(entries, truncated)`.
    #[allow(clippy::too_many_lines)] // one bounded stack-walk kept flat
    fn collect_entries(
        &self,
        scope: &RootedDir,
        base: &str,
        filter: &FilterSpec,
        cx: &ProviderContext,
    ) -> Result<(Vec<BrowseEntry>, bool), ProviderError> {
        let self_rel = RelativePath::new("").expect("the empty locator is the valid self form");
        let scope_root = scope.canonical_root().to_path_buf();
        let mut entries = Vec::new();
        let mut seen_locators: BTreeSet<String> = BTreeSet::new();
        // Loop guard on directory identity — real dirs cannot cycle
        // by pathname, but mount/bind points can alias a subtree.
        let mut visited: HashSet<NodeId> = HashSet::new();
        if let Ok(id) = scope.dir_identity() {
            visited.insert(id);
        }
        let mut truncated = false;
        let mut walked = 0u32;
        let scope_clone = scope
            .open_dir(&self_rel, FollowMode::NoFollow)
            .map_err(|e| rooted_error("open scope", &e))?;
        // Pending dirs keep their scoped handles open for the whole
        // walk (depth-first, siblings deferred). Handle count grows
        // with discovered-but-unvisited dirs — bounded by `max_entries`
        // and `max_open_dirs`, and exhaustion surfaces as `truncated`
        // rather than a hidden skip. A pathname re-resolve on pop
        // would reintroduce the traversal window, so held handles are
        // the honest option.
        let mut stack = vec![(scope_clone, String::new(), 0u32)];

        'walk: while let Some((dir_cap, prefix, depth)) = stack.pop() {
            cx.check_interrupts()?;
            self.pre_traverse(&Self::lexical(&scope_root, &prefix), "read_dir");
            let mut children = match dir_cap.read_dir(&self_rel, FollowMode::NoFollow) {
                Ok(children) => children,
                Err(e) => {
                    if prefix.is_empty() {
                        return Err(rooted_error("read scope", &e));
                    }
                    // Unreadable subdirectory: skip it and mark the
                    // page partial instead of faking completeness.
                    truncated = true;
                    continue;
                }
            };
            // Bound the kept window the way the old
            // `take(remaining + 1)` did — enumeration-order prefix,
            // then sorted. The returned page is never a
            // globally-smallest subset (that was true before too);
            // the contract is `max_entries` + honest `truncated`.
            // Residual: `read_dir` materializes the full directory
            // inside Ruviform before this bound applies — bounded
            // enumeration needs an RF streaming/cap API (tracked in
            // docs/tasks/alib-01-task5-rooted-cutover.md), so the
            // pre-sort memory bound is the ownable part, not a
            // hidden fix.
            let remaining =
                usize::try_from(cx.limits.max_entries.saturating_sub(walked)).unwrap_or(usize::MAX);
            children.truncate(remaining.saturating_add(1));
            children.sort_by(|a, b| a.name.cmp(&b.name));
            for child in children {
                if walked >= cx.limits.max_entries {
                    truncated = true;
                    break 'walk;
                }
                walked += 1;
                // Non-UTF-8 / grammar-invalid names can never become
                // locators — skipped as partial, never aliased.
                let Some(name) = child.name.to_str() else {
                    truncated = true;
                    continue;
                };
                let Ok(comp) = RelativePath::from_component(&child.name) else {
                    truncated = true;
                    continue;
                };
                let entry_rel = if prefix.is_empty() {
                    name.to_owned()
                } else {
                    format!("{prefix}/{name}")
                };
                self.pre_traverse(&Self::lexical(&scope_root, &entry_rel), "stat");

                let via_symlink = child.kind == NodeKind::Symlink;
                let mut kind = EntryKind::Asset;
                let mut availability = Availability::Available;
                // `None` for links that cannot be served — the leaf is
                // still reported (as `Symlink`) but no target facts.
                let mut facts = None;

                match dir_cap.metadata(&comp, FollowMode::WithinRoot) {
                    Ok(info) => {
                        if info.kind == NodeKind::Dir {
                            kind = EntryKind::Container;
                        }
                        facts = Some(Self::node_facts(&info, Self::asset_kind(&info), cx.now()));
                    }
                    Err(e) if via_symlink => match e.kind() {
                        FileErrorKind::EscapesRoot => {
                            availability = Availability::Unavailable {
                                reason: UnavailableReason::Other {
                                    detail: "symlink target outside browse scope".into(),
                                },
                            };
                        }
                        FileErrorKind::NotFound => {
                            availability = Availability::Missing {
                                detail: "dangling symlink".into(),
                            };
                        }
                        FileErrorKind::LoopDetected => {
                            availability = Availability::Missing {
                                detail: "unresolvable symlink".into(),
                            };
                        }
                        _ => {
                            availability = Availability::Unavailable {
                                reason: UnavailableReason::Other {
                                    detail: format!("symlink target unreadable: {e}"),
                                },
                            };
                        }
                    },
                    // A vanished non-link entry mid-walk: skip it.
                    Err(e) if e.kind() == FileErrorKind::NotFound => continue,
                    // Existed but unobservable — the page is partial.
                    Err(_) => {
                        truncated = true;
                        continue;
                    }
                }

                if Self::matches_filter(name, Path::new(&entry_rel), kind, filter) {
                    let locator_text = if base.is_empty() {
                        entry_rel.clone()
                    } else {
                        format!("{base}/{entry_rel}")
                    };
                    match OpaqueLocator::new(locator_text) {
                        Ok(locator) if seen_locators.insert(locator.as_str().to_owned()) => {
                            entries.push(BrowseEntry {
                                locator,
                                display_name: name.to_owned(),
                                kind,
                                availability,
                                facts: facts
                                    .unwrap_or_else(|| Self::unavailable_link_facts(cx.now())),
                                via_symlink,
                            });
                        }
                        _ => truncated = true,
                    }
                }

                // Only real directories are descended — through the
                // held scoped handle. Link targets inside the scope
                // appear under their own canonical locators anyway,
                // so link descent would only duplicate entries.
                if child.kind == NodeKind::Dir {
                    if depth + 1 >= cx.limits.max_depth {
                        truncated = true;
                    } else {
                        match dir_cap.open_dir(&comp, FollowMode::NoFollow) {
                            Ok(sub_cap) => {
                                let fresh = match sub_cap.dir_identity() {
                                    Ok(id) => visited.insert(id),
                                    Err(_) => true,
                                };
                                if fresh {
                                    stack.push((sub_cap, entry_rel, depth + 1));
                                }
                            }
                            Err(_) => truncated = true,
                        }
                    }
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

    /// Shared browse/search pipeline: open the scope capability,
    /// collect entries, sort, page, emit cursor.
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

        let (scope_cap, base) = match scope {
            BrowseScope::Root => (
                self.root
                    .open_dir(
                        &RelativePath::new("").expect("empty locator is valid"),
                        FollowMode::NoFollow,
                    )
                    .map_err(|e| rooted_error("open scope", &e))?,
                String::new(),
            ),
            BrowseScope::Locator { locator } => self.open_scope(locator)?,
        };

        let (mut entries, truncated) = self.collect_entries(&scope_cap, &base, filter, cx)?;
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

    /// Verified open of the regular file `locator` resolves to under
    /// `WithinRoot` — the returned [`RootedFile`] pairs the reader
    /// with facts of the same opened object.
    fn open_asset(
        &self,
        locator: &OpaqueLocator,
        not_regular: impl FnOnce() -> String,
    ) -> Result<RootedFile, ProviderError> {
        let rel = Self::locator_rel(locator)?;
        let mut lexical = self.root.canonical_root().to_path_buf();
        for comp in rel.components() {
            lexical.push(comp);
        }
        self.pre_traverse(&lexical, "open");
        self.root
            .open_regular(&rel, FollowMode::WithinRoot)
            .map_err(|e| match e.kind() {
                FileErrorKind::NotRegularFile => {
                    ProviderError::new(ProviderErrorKind::InvalidRequest, not_regular())
                }
                _ => rooted_error("open", &e),
            })
    }

    #[allow(clippy::too_many_arguments)] // one bundle per verification knob stays flat
    fn verified_read(
        asset: &RemoteAssetRef,
        mut file: RootedFile,
        range: Option<ByteRange>,
        claims_verification: VerificationPolicy,
        expected_digest: Option<ContentDigest>,
        expected_size: Option<u64>,
        object_identity: ObjectIdentity,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        // `info()` is the same opened object's probe — a regular file
        // always carries a byte length.
        let file_len = file.info().byte_len.ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::TransientIo,
                format!("{} opened object reports no length", asset.locator),
            )
        })?;
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
                file.seek(SeekFrom::Start(range.offset))
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
        // One resolve gives the leaf's own kind and the resolved
        // object's facts — a symlinked locator reports `Symlink`,
        // never silently the target's kind.
        let rel = Self::locator_rel(&asset.locator)?;
        let mut lexical = self.root.canonical_root().to_path_buf();
        for comp in rel.components() {
            lexical.push(comp);
        }
        self.pre_traverse(&lexical, "stat");
        let info = self
            .root
            .metadata(&rel, FollowMode::WithinRoot)
            .map_err(|e| rooted_error("stat", &e))?;
        Ok(AssetStat {
            asset: asset.clone(),
            availability: Availability::Available,
            facts: Self::node_facts(&info, Self::asset_kind(&info), cx.now()),
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
        // Verified open then digest the same opened object — a
        // stat→open swap cannot redirect the digest because there is
        // no path between the two.
        let mut file = self.open_asset(&asset.locator, || {
            format!("cannot pin a container locator {}", asset.locator)
        })?;
        if let Some(len) = file.info().byte_len
            && len > cx.limits.max_bytes
        {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                format!(
                    "{} exceeds byte budget {}",
                    asset.locator, cx.limits.max_bytes
                ),
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
        let file = self.open_asset(&pin.asset.locator, || {
            format!("{} is not a regular file", pin.asset.locator)
        })?;
        Self::verified_read(
            &pin.asset,
            file,
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
        let file = self.open_asset(&asset.locator, || {
            format!("cannot preview container {}", asset.locator)
        })?;
        Self::verified_read(
            asset,
            file,
            range,
            VerificationPolicy::Unverified,
            None,
            None,
            ObjectIdentity::new(asset.locator.as_str())?,
            cx,
        )
    }
}
