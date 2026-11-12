//! Git remote provider (ALIB-04) — a read-only [`AssetProvider`]
//! over immutable git objects.
//!
//! The provider keeps a lazy `--mirror --filter=blob:none` clone as a
//! derived cache and serves `browse`/`stat`/`pin`/`open_pinned`/
//! `open_preview` against fixed commit/tree/blob objects. Mutable
//! refs resolve to concrete commit SHAs per call; pins carry
//! [`PinProof::ImmutableObject`] (`commit:<sha>`) plus the blob or
//! LFS object identity, so later remote updates never change pinned
//! bytes. Git LFS pointers are metadata only — fetch verifies the
//! real object's SHA-256 (the LFS oid *is* the digest) and declared
//! size before any byte counts as content.
//!
//! Nothing here mutates a project, worktree, index, or HEAD: the
//! mirror is a separate bare directory owned by this provider.
//! Selective fetch is per-object through `cat-file` lazy fetch —
//! unselected objects are never downloaded, and a remote that cannot
//! honor `--filter` is rejected or explicitly mirrored whole per
//! [`UnsupportedFilterPolicy`], never silently checked out.

mod lfs;
mod locator;
mod mirror;
mod objects;
mod url;
mod vcs;

use std::io::Read;
use std::path::PathBuf;

pub use lfs::LfsBackendKind;
pub use mirror::{GitObjectFormat, MirrorMode, UnsupportedFilterPolicy};
pub use url::{GitRemoteScheme, GitRemoteUrl};
pub use vcs::{GitVcsConfig, GitVcsProvider};

use crate::browse::{
    BrowseEntry, BrowsePage, BrowseRequest, BrowseScope, Coverage, CursorPayload, EntryKind,
    FilterSpec, decode_cursor, encode_cursor, principal_fingerprint, query_fingerprint,
    scope_fingerprint, sort_entries,
};
use crate::capability::{
    AccessMode, Capability, PinCapability, ProviderCapabilities, UnsupportedReason,
};
use crate::context::ProviderContext;
use crate::digest::{completion_slot, io_provider_error};
use crate::error::{ProviderError, ProviderErrorKind};
use crate::git::lfs::LfsBackend;
use crate::git::locator::GitLocator;
use crate::git::mirror::GitMirror;
use crate::git::objects::{GitEntryMode, GitTreeEntry, is_pointer_sized, parse_lfs_pointer};
use crate::id::{CredentialKey, OpaqueLocator, ProviderId};
use crate::observation::{
    AssetKind, AssetObservation, AssetStat, Availability, EntryFacts, ObservationOrigin,
    UnavailableReason,
};
use crate::provider::{
    AssetProvider, ByteRange, FetchClaims, FetchCompletion, PinnedRead, VerificationPolicy,
};
use crate::refs::{
    ContentDigest, ObjectIdentity, PinProof, PinnedAssetRef, RemoteAssetRef, RevisionKey,
};
use crate::registry::ProviderKind;

/// The opened content stream plus the verification evidence to
/// apply — `(reader, claims-policy, measured-digest-hint)`.
type IdentityOpen = (
    Box<dyn Read + Send>,
    VerificationPolicy,
    Option<ContentDigest>,
);

/// Connection configuration for [`GitRemoteProvider`].
#[derive(Clone, Debug)]
pub struct GitRemoteConfig {
    /// The validated remote (file/http/https/ssh/git or local path).
    pub remote: GitRemoteUrl,
    /// Directory that owns the derived mirror clone. Must be outside
    /// any project worktree — the provider never writes project
    /// state, and the mirror is disposable derived data.
    pub mirror_dir: PathBuf,
    /// The revision expression used for `BrowseScope::Root`
    /// (`"HEAD"`, a branch, a tag, or a commit sha). `Locator`
    /// scopes carry their own rev.
    pub reference: String,
    /// Credential-store key resolved per git invocation for HTTP
    /// remotes. Never embedded in URLs, locators, or provenance.
    pub credential_key: Option<CredentialKey>,
    /// Behavior when the remote cannot serve `--filter=blob:none`.
    pub filter_policy: UnsupportedFilterPolicy,
}

impl GitRemoteConfig {
    /// A config for `remote` mirrored under `mirror_dir`, defaulting
    /// to `HEAD`, no credentials, and [`UnsupportedFilterPolicy::Reject`].
    #[must_use]
    pub fn new(remote: GitRemoteUrl, mirror_dir: impl Into<PathBuf>) -> Self {
        Self {
            remote,
            mirror_dir: mirror_dir.into(),
            reference: "HEAD".to_owned(),
            credential_key: None,
            filter_policy: UnsupportedFilterPolicy::Reject,
        }
    }
}

/// Honest runtime facts about a git remote connection.
#[derive(Clone, Debug)]
pub struct GitProviderInfo {
    /// The provider connection id.
    pub provider: ProviderId,
    /// The remote URL as configured (credential-free by validation).
    pub remote: String,
    /// The derived mirror directory.
    pub mirror_dir: PathBuf,
    /// The configured root reference expression.
    pub reference: String,
    /// Whether blobs are fetched lazily (partial) or were mirrored
    /// whole, as detected after clone.
    pub mirror_mode: MirrorMode,
    /// The repository's object id format, once ensured.
    pub object_format: Option<GitObjectFormat>,
    /// How Git LFS objects are reached.
    pub lfs_backend: LfsBackendKind,
    /// The configured filter fallback policy.
    pub filter_policy: UnsupportedFilterPolicy,
}

/// A read-only git remote [`AssetProvider`].
pub struct GitRemoteProvider {
    id: ProviderId,
    config: GitRemoteConfig,
    mirror: GitMirror,
    lfs: LfsBackend,
}

impl GitRemoteProvider {
    /// Builds the provider. The mirror is *not* cloned until the
    /// first operation that needs it.
    #[must_use]
    pub fn new(id: ProviderId, config: GitRemoteConfig) -> Self {
        let mirror = GitMirror::new(
            config.mirror_dir.clone(),
            config.remote.clone(),
            config.reference.clone(),
            config.credential_key.clone(),
            config.filter_policy,
        );
        let lfs = LfsBackend::for_remote(&config.remote, config.credential_key.clone());
        Self {
            id,
            config,
            mirror,
            lfs,
        }
    }

    /// How Git LFS objects are reached for this remote — declared
    /// without cloning (unlike [`Self::git_info`]).
    #[must_use]
    pub const fn lfs_backend_kind(&self) -> LfsBackendKind {
        self.lfs.kind()
    }

    /// Explicitly refreshes the mirror (`git fetch --prune`) so
    /// branch updates, force pushes, and deletions become visible.
    /// Never implicit — pinned objects stay fetchable regardless.
    ///
    /// # Errors
    /// [`ProviderError`] — transport/auth failures typed by git's
    /// stderr classification.
    pub fn refresh(&self, cx: &ProviderContext) -> Result<(), ProviderError> {
        self.mirror.refresh(cx)
    }

    /// Runtime facts about the connection (ensures the mirror).
    ///
    /// # Errors
    /// [`ProviderError`] — clone/transport failures.
    pub fn git_info(&self, cx: &ProviderContext) -> Result<GitProviderInfo, ProviderError> {
        self.mirror.ensure(cx)?;
        Ok(GitProviderInfo {
            provider: self.id,
            remote: self.config.remote.as_str().to_owned(),
            mirror_dir: self.config.mirror_dir.clone(),
            reference: self.config.reference.clone(),
            mirror_mode: self.mirror.mirror_mode(),
            object_format: self.mirror.object_format(),
            lfs_backend: self.lfs.kind(),
            filter_policy: self.config.filter_policy,
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

    /// `(rev, path)` for a browse scope: `Root` uses the configured
    /// reference; `Locator` carries its own `<rev>:<path>`.
    fn scope_target(&self, scope: &BrowseScope) -> Result<(String, String), ProviderError> {
        match scope {
            BrowseScope::Root => Ok((self.config.reference.clone(), String::new())),
            BrowseScope::Locator { locator } => {
                let gloc = GitLocator::parse(locator)?;
                Ok((gloc.rev, gloc.path))
            }
        }
    }

    /// Maps one `ls-tree` record to a browse entry. Returns `None`
    /// when the entry name cannot form a valid locator (the caller
    /// counts it as skipped → partial coverage).
    fn browse_entry(
        gloc: &GitLocator,
        entry: &GitTreeEntry,
        at: crate::observation::ObservedAt,
    ) -> Option<BrowseEntry> {
        let locator = gloc.child(&entry.name).ok()?;
        let (kind, asset_kind, availability, via_symlink, byte_len, oid_scheme) = match entry.mode {
            GitEntryMode::Tree => (
                EntryKind::Container,
                AssetKind::Directory,
                Availability::Available,
                false,
                AssetObservation::unknown(UnavailableReason::NotApplicable),
                "git-tree",
            ),
            GitEntryMode::Blob { .. } => (
                EntryKind::Asset,
                AssetKind::File,
                Availability::Available,
                false,
                entry.size.map_or_else(
                    || AssetObservation::unknown(UnavailableReason::NotProvided),
                    |s| AssetObservation::observed_at(s, ObservationOrigin::Provider, at),
                ),
                "git-blob",
            ),
            GitEntryMode::Symlink => (
                EntryKind::Asset,
                AssetKind::Symlink,
                Availability::Available,
                true,
                entry.size.map_or_else(
                    || AssetObservation::unknown(UnavailableReason::NotProvided),
                    |s| AssetObservation::observed_at(s, ObservationOrigin::Provider, at),
                ),
                "git-blob",
            ),
            GitEntryMode::Gitlink => (
                EntryKind::Asset,
                AssetKind::Other {
                    label: "git-submodule".to_owned(),
                },
                Availability::Unavailable {
                    reason: UnavailableReason::Other {
                        detail: "git submodule (gitlink) is not fetchable through this provider"
                            .to_owned(),
                    },
                },
                false,
                AssetObservation::unknown(UnavailableReason::NotApplicable),
                "git-commit",
            ),
        };
        Some(BrowseEntry {
            locator,
            display_name: entry.name.clone(),
            kind,
            availability,
            facts: EntryFacts {
                byte_len,
                modified: AssetObservation::unknown(UnavailableReason::NotProvided),
                kind: AssetObservation::observed_at(asset_kind, ObservationOrigin::Provider, at),
                checksum_hint: AssetObservation::observed_at(
                    ObjectIdentity::new(format!("{oid_scheme}:{}", entry.oid))
                        .unwrap_or_else(|_| ObjectIdentity::new("git:unknown").expect("non-empty")),
                    ObservationOrigin::Provider,
                    at,
                ),
                media: crate::observation::MediaFacts::all_unknown(),
            },
            via_symlink,
        })
    }

    fn matches_filter(name: &str, kind: EntryKind, filter: &FilterSpec) -> bool {
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
            let ext = name
                .rsplit_once('.')
                .map(|(_, e)| e.to_ascii_lowercase())
                .unwrap_or_default();
            if !exts.iter().any(|want| want.eq_ignore_ascii_case(&ext)) {
                return false;
            }
        }
        true
    }

    /// Resolves an asset locator to `(sha, GitLocator)`.
    fn resolve_asset(
        &self,
        locator: &OpaqueLocator,
        cx: &ProviderContext,
    ) -> Result<(String, GitLocator), ProviderError> {
        let gloc = GitLocator::parse(locator)?;
        let sha = self.mirror.resolve_commit(&gloc.rev, cx)?;
        Ok((sha, gloc))
    }

    /// Stats `path` inside commit `sha`; `None` → `NotFound`.
    fn stat_path(
        &self,
        sha: &str,
        path: &str,
        cx: &ProviderContext,
    ) -> Result<GitTreeEntry, ProviderError> {
        match self.mirror.stat_entry(sha, path, cx)? {
            Some(entry) => Ok(entry),
            None => Err(ProviderError::new(
                ProviderErrorKind::NotFound,
                format!("no such path {path:?} at resolved commit"),
            )),
        }
    }

    /// `AssetStat` facts for a resolved tree entry (or the synthetic
    /// root when `path` is empty).
    fn stat_facts(
        entry: Option<&GitTreeEntry>,
        commit_sha: &str,
        at: crate::observation::ObservedAt,
    ) -> EntryFacts {
        match entry {
            None => EntryFacts {
                byte_len: AssetObservation::unknown(UnavailableReason::NotApplicable),
                modified: AssetObservation::unknown(UnavailableReason::NotProvided),
                kind: AssetObservation::observed_at(
                    AssetKind::Directory,
                    ObservationOrigin::Provider,
                    at,
                ),
                checksum_hint: AssetObservation::observed_at(
                    ObjectIdentity::new(format!("git-commit:{commit_sha}"))
                        .expect("commit sha identity is non-empty"),
                    ObservationOrigin::Provider,
                    at,
                ),
                media: crate::observation::MediaFacts::all_unknown(),
            },
            Some(e) => {
                let (asset_kind, oid_scheme) = match e.mode {
                    GitEntryMode::Tree => (AssetKind::Directory, "git-tree"),
                    GitEntryMode::Blob { .. } => (AssetKind::File, "git-blob"),
                    GitEntryMode::Symlink => (AssetKind::Symlink, "git-blob"),
                    GitEntryMode::Gitlink => (
                        AssetKind::Other {
                            label: "git-submodule".to_owned(),
                        },
                        "git-commit",
                    ),
                };
                EntryFacts {
                    byte_len: e.size.map_or_else(
                        || AssetObservation::unknown(UnavailableReason::NotApplicable),
                        |s| AssetObservation::observed_at(s, ObservationOrigin::Provider, at),
                    ),
                    modified: AssetObservation::unknown(UnavailableReason::NotProvided),
                    kind: AssetObservation::observed_at(
                        asset_kind,
                        ObservationOrigin::Provider,
                        at,
                    ),
                    checksum_hint: AssetObservation::observed_at(
                        ObjectIdentity::new(format!("{oid_scheme}:{}", e.oid))
                            .expect("git oid identity is non-empty"),
                        ObservationOrigin::Provider,
                        at,
                    ),
                    media: crate::observation::MediaFacts::all_unknown(),
                }
            }
        }
    }

    /// Detects whether a small blob is a Git LFS pointer *and* is
    /// LFS-tracked at `sha:path`. Returns the parsed pointer.
    fn lfs_pointer_for(
        &self,
        sha: &str,
        path: &str,
        entry: &GitTreeEntry,
        cx: &ProviderContext,
    ) -> Result<Option<crate::git::objects::LfsPointer>, ProviderError> {
        let Some(size) = entry.size else {
            return Ok(None);
        };
        if !is_pointer_sized(size) {
            return Ok(None);
        }
        let head = self.mirror.blob_head(&entry.oid, size, cx)?;
        let Some(pointer) = parse_lfs_pointer(&head) else {
            return Ok(None);
        };
        // A pointer-shaped blob is only LFS when the path is
        // LFS-tracked at this commit; `check-attr` failure fails
        // safe toward LFS (never serve pointer text as content).
        if self.mirror.lfs_tracked(sha, path, cx) {
            Ok(Some(pointer))
        } else {
            Ok(None)
        }
    }

    /// Builds the pinned reference for a fetchable blob entry.
    fn pin_entry(
        &self,
        asset: &RemoteAssetRef,
        sha: &str,
        path: &str,
        entry: &GitTreeEntry,
        cx: &ProviderContext,
    ) -> Result<PinnedAssetRef, ProviderError> {
        let size = entry.size.unwrap_or(0);
        if size > cx.limits.max_bytes {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                format!(
                    "{} exceeds byte budget {}",
                    asset.locator, cx.limits.max_bytes
                ),
            ));
        }
        let revision = RevisionKey::new(format!("commit:{sha}"))?;
        if let Some(pointer) = self.lfs_pointer_for(sha, path, entry, cx)? {
            if pointer.size > cx.limits.max_bytes {
                return Err(ProviderError::new(
                    ProviderErrorKind::BudgetExceeded,
                    format!(
                        "git-lfs object for {} exceeds byte budget {}",
                        asset.locator, cx.limits.max_bytes
                    ),
                ));
            }
            let digest = ContentDigest::parse(&format!("sha256:{}", pointer.oid)).map_err(|e| {
                ProviderError::new(
                    ProviderErrorKind::Internal,
                    format!("git-lfs oid is not a valid sha256 digest: {e}"),
                )
            })?;
            return Ok(PinnedAssetRef {
                asset: asset.clone(),
                proof: PinProof::ImmutableObject { revision },
                object_identity: ObjectIdentity::new(format!("git-lfs:{}", pointer.oid))?,
                expected_size: Some(pointer.size),
                digest_hint: Some(digest),
            });
        }
        Ok(PinnedAssetRef {
            asset: asset.clone(),
            proof: PinProof::ImmutableObject { revision },
            object_identity: ObjectIdentity::new(format!("git-blob:{}", entry.oid))?,
            expected_size: entry.size,
            digest_hint: None,
        })
    }

    /// Opens the content stream for a `git-blob:`/`git-lfs:` identity.
    /// Returns the reader plus the verification evidence to apply.
    fn open_identity(
        &self,
        identity: &str,
        expected_size: Option<u64>,
        digest_hint: Option<&ContentDigest>,
        cx: &ProviderContext,
    ) -> Result<IdentityOpen, ProviderError> {
        if let Some(oid) = identity.strip_prefix("git-blob:") {
            // Existence + type precondition: `cat-file -t` resolves
            // the object header (lazily negotiating promisor fetch)
            // without streaming content; a gone object maps to
            // RevisionGone — the pin named something the remote no
            // longer has.
            let ty = self
                .mirror
                .runner()
                .run(Some(self.mirror.dir()), &["cat-file", "-t", oid], cx)
                .map_err(|e| match e.kind() {
                    ProviderErrorKind::NotFound => ProviderError::new(
                        ProviderErrorKind::RevisionGone,
                        format!("pinned git object {oid} is no longer on the remote"),
                    ),
                    _ => e,
                })?;
            if String::from_utf8_lossy(&ty.stdout).trim() != "blob" {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    format!("pinned identity {identity} is not a blob"),
                ));
            }
            let stream = self.mirror.open_blob(oid, cx)?;
            Ok((
                Box::new(stream),
                VerificationPolicy::IdentityBound {
                    scheme: "git-object-id".to_owned(),
                },
                digest_hint.cloned(),
            ))
        } else if let Some(oid) = identity.strip_prefix("git-lfs:") {
            let stream = self.lfs.open_object(oid, expected_size, cx)?;
            // The LFS oid *is* the SHA-256 of the content — the pin's
            // digest evidence is derived from the identity itself.
            // Without a derivable digest there is no verification
            // evidence, so the fetch must not proceed.
            let Some(digest) = digest_hint
                .cloned()
                .or_else(|| ContentDigest::parse(&format!("sha256:{oid}")).ok())
            else {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    format!("git-lfs identity {identity} has no sha256 digest evidence"),
                ));
            };
            Ok((
                stream,
                VerificationPolicy::StreamVerified {
                    digest: digest.clone(),
                },
                Some(digest),
            ))
        } else {
            Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("unsupported git object identity {identity:?}"),
            ))
        }
    }

    /// Wraps a content stream in the verifying reader + completion
    /// slot shared by `open_pinned`/`open_preview`.
    fn wrap_read(
        inner: Box<dyn Read + Send>,
        expected_digest: Option<ContentDigest>,
        expected_size: Option<u64>,
        verification: VerificationPolicy,
        object_identity: ObjectIdentity,
        cx: &ProviderContext,
    ) -> PinnedRead {
        let slot = completion_slot();
        let reader = crate::digest::VerifyingReader::new(
            inner,
            expected_digest,
            expected_size,
            cx.limits.max_bytes,
            cx.cancel.clone(),
            cx.limits.deadline,
            object_identity.clone(),
            slot.clone(),
        );
        PinnedRead {
            reader: Box::new(reader),
            claims: FetchClaims {
                object_identity,
                expected_size,
                verification,
            },
            completion: FetchCompletion::new(slot),
        }
    }
}

impl AssetProvider for GitRemoteProvider {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::GitRemote
    }

    fn capabilities(&self, _asset: Option<&RemoteAssetRef>) -> ProviderCapabilities {
        ProviderCapabilities {
            browse: Capability::Supported,
            stat: Capability::Supported,
            // No provider-side index exists — search must not fake
            // empty results.
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
        let scope_fp = scope_fingerprint(&req.scope);
        let query_fp = query_fingerprint(req.sort, &req.filter);
        let principal_fp = principal_fingerprint(&cx.principal);

        // Cursor path: the epoch pins the commit sha — later remote
        // updates cannot move an in-flight browse. The rev/path text
        // still comes from the current scope (scope_fp guarantees it
        // is identical to the cursor's), so child locators keep the
        // user's original rev spelling across pages.
        let (sha, gloc, offset, epoch) = if let Some(cursor) = &req.cursor {
            let payload =
                decode_cursor(cursor, self.id, &scope_fp, &query_fp, &principal_fp, |e| {
                    self.mirror.epoch_valid(e)
                })?;
            let (sha, _stored) = self.mirror.epoch_target(payload.epoch).ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorKind::CursorInvalid,
                    "cursor epoch target is no longer available",
                )
            })?;
            let (rev, path) = self.scope_target(&req.scope)?;
            (
                sha,
                GitLocator { rev, path },
                payload.offset,
                Some(payload.epoch),
            )
        } else {
            let (rev, path) = self.scope_target(&req.scope)?;
            let sha = self.mirror.resolve_commit(&rev, cx)?;
            let gloc = GitLocator {
                rev: rev.clone(),
                path: path.clone(),
            };
            (sha, gloc, 0, None)
        };

        let (entries, truncated, skipped_names) = self.mirror.list_tree(&sha, &gloc.path, cx)?;
        let at = cx.now();
        let mut mapped: Vec<BrowseEntry> = Vec::with_capacity(entries.len());
        let mut unaddressable = skipped_names;
        for entry in &entries {
            match Self::browse_entry(&gloc, entry, at) {
                Some(b) if Self::matches_filter(&b.display_name, b.kind, &req.filter) => {
                    mapped.push(b);
                }
                Some(_) => {}
                None => unaddressable += 1,
            }
        }
        sort_entries(&mut mapped, req.sort);
        let total = mapped.len() as u64;
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(mapped.len());
        let end = start
            .saturating_add(usize::try_from(req.page_limit).unwrap_or(usize::MAX))
            .min(mapped.len());
        let page_entries: Vec<BrowseEntry> = mapped[start..end].to_vec();
        let emitted = end as u64;

        let next_cursor = if emitted < total {
            let epoch = match epoch {
                Some(e) => e,
                None => self.mirror.issue_epoch(&sha, &gloc.path),
            };
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
        let coverage = if next_cursor.is_none() && !truncated && unaddressable == 0 {
            Coverage::Complete
        } else {
            Coverage::Partial
        };
        Ok(BrowsePage {
            entries: page_entries,
            next_cursor,
            coverage,
            observed_at: at,
        })
    }

    fn stat(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<AssetStat, ProviderError> {
        self.check_owner(asset)?;
        cx.check_interrupts()?;
        let (sha, gloc) = self.resolve_asset(&asset.locator, cx)?;
        let at = cx.now();
        let entry = if gloc.path.is_empty() {
            None
        } else {
            Some(self.stat_path(&sha, &gloc.path, cx)?)
        };
        let availability = match entry.as_ref().map(|e| e.mode) {
            Some(GitEntryMode::Gitlink) => Availability::Unavailable {
                reason: UnavailableReason::Other {
                    detail: "git submodule (gitlink) is not fetchable through this provider"
                        .to_owned(),
                },
            },
            _ => Availability::Available,
        };
        Ok(AssetStat {
            asset: asset.clone(),
            availability,
            facts: Self::stat_facts(entry.as_ref(), &sha, at),
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
        let (sha, gloc) = self.resolve_asset(&asset.locator, cx)?;
        if gloc.path.is_empty() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "cannot pin a repository root (container)",
            ));
        }
        let entry = self.stat_path(&sha, &gloc.path, cx)?;
        match entry.mode {
            GitEntryMode::Tree => Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "cannot pin a tree (container)",
            )),
            GitEntryMode::Gitlink => Err(ProviderError::new(
                ProviderErrorKind::UnsupportedCapability,
                "git submodule (gitlink) cannot be pinned",
            )),
            GitEntryMode::Blob { .. } | GitEntryMode::Symlink => {
                self.pin_entry(asset, &sha, &gloc.path, &entry, cx)
            }
        }
    }

    fn open_pinned(
        &self,
        pin: &PinnedAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        self.check_owner(&pin.asset)?;
        cx.check_interrupts()?;
        // Only ImmutableObject pins are meaningful here; the revision
        // must name a commit we can still resolve.
        let revision = match &pin.proof {
            PinProof::ImmutableObject { revision } => revision.as_str(),
            PinProof::ConditionalStrong { .. } => {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "git provider only opens ImmutableObject pins",
                ));
            }
        };
        let commit_sha = revision.strip_prefix("commit:").ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "git pin revision is not commit-scoped",
            )
        })?;
        // The pinned commit must still resolve — a force-push that
        // dropped it makes the pin unfetchable.
        let resolved = self.mirror.resolve_commit(commit_sha, cx).map_err(|e| {
            if e.kind() == ProviderErrorKind::NotFound {
                ProviderError::new(
                    ProviderErrorKind::RevisionGone,
                    "pinned commit is no longer reachable on the remote",
                )
            } else {
                e
            }
        })?;
        if resolved != commit_sha {
            return Err(ProviderError::new(
                ProviderErrorKind::SourceChanged,
                "pinned commit no longer resolves to the same object",
            ));
        }
        let identity = pin.object_identity.as_str();
        let (inner, verification, digest) =
            self.open_identity(identity, pin.expected_size, pin.digest_hint.as_ref(), cx)?;
        // Blob pins carry no strong digest (git oid ≠ sha256) —
        // IdentityBound is the honest claim; LFS pins verify sha256.
        let expected_digest = match &verification {
            VerificationPolicy::StreamVerified { digest } => Some(digest.clone()),
            _ => digest,
        };
        Ok(Self::wrap_read(
            inner,
            expected_digest,
            pin.expected_size,
            verification,
            pin.object_identity.clone(),
            cx,
        ))
    }

    fn open_preview(
        &self,
        asset: &RemoteAssetRef,
        range: Option<ByteRange>,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        self.check_owner(asset)?;
        cx.check_interrupts()?;
        let (sha, gloc) = self.resolve_asset(&asset.locator, cx)?;
        if gloc.path.is_empty() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "cannot preview a repository root (container)",
            ));
        }
        let entry = self.stat_path(&sha, &gloc.path, cx)?;
        match entry.mode {
            GitEntryMode::Tree => {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "cannot preview a tree (container)",
                ));
            }
            GitEntryMode::Gitlink => {
                return Err(ProviderError::new(
                    ProviderErrorKind::UnsupportedCapability,
                    "git submodule (gitlink) cannot be previewed",
                ));
            }
            GitEntryMode::Blob { .. } | GitEntryMode::Symlink => {}
        }
        // Preview must show real bytes: an LFS-tracked path resolves
        // to the LFS object, never the pointer text.
        let (identity, expected_size) =
            if let Some(pointer) = self.lfs_pointer_for(&sha, &gloc.path, &entry, cx)? {
                (format!("git-lfs:{}", pointer.oid), Some(pointer.size))
            } else {
                (format!("git-blob:{}", entry.oid), entry.size)
            };
        let object_identity = ObjectIdentity::new(identity)?;
        let (mut inner, _verification, _digest) =
            self.open_identity(object_identity.as_str(), expected_size, None, cx)?;
        // Apply the requested range. Discarding a blob stream's
        // prefix counts toward the byte budget; the LFS file seeks.
        if let Some(range) = range {
            if range.offset.checked_add(range.len).is_none() {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "preview range overflows",
                ));
            }
            if range.offset > cx.limits.max_bytes {
                return Err(ProviderError::new(
                    ProviderErrorKind::BudgetExceeded,
                    "preview offset exceeds byte budget",
                ));
            }
            // `inner` is either a GitObjectStream (Read) or a File —
            // both are Read; seek is only available on File, so use
            // bounded discard uniformly: honest accounting either way.
            let mut remaining = range.offset;
            let mut buf = [0u8; 8192];
            while remaining > 0 {
                cx.check_interrupts()?;
                let want = usize::try_from(remaining.min(8192)).unwrap_or(8192);
                match inner.read(&mut buf[..want]) {
                    Ok(0) => break,
                    Ok(n) => remaining -= n as u64,
                    Err(e) => {
                        return Err(io_provider_error("preview seek", &e));
                    }
                }
            }
            let cap = range.len.min(cx.limits.max_bytes.saturating_add(1));
            inner = Box::new(inner.take(cap));
        } else {
            inner = Box::new(inner.take(cx.limits.max_bytes.saturating_add(1)));
        }
        Ok(Self::wrap_read(
            inner,
            None,
            None,
            VerificationPolicy::Unverified,
            object_identity,
            cx,
        ))
    }
}
