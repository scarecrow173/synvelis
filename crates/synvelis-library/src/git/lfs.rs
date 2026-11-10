//! Git LFS object access for local (`file://`) remotes.
//!
//! An LFS pointer names an object by SHA-256 oid + declared size; the
//! bytes live under `<remote>/lfs/objects/<oid[0..2]>/<oid[2..4]>/
//! <oid>` on the remote side. This slice reads that store directly —
//! the LFS batch API and remote download endpoints are out of scope
//! (declared `Unsupported` for non-file transports).
//!
//! The pointer itself is *never* returned as asset bytes: only a
//! file that passes oid-hex validation, declared-size pre-check, and
//! the downstream SHA-256 `VerifyingReader` becomes fetchable
//! content.

use std::fs::File;
use std::path::PathBuf;

use crate::context::ProviderContext;
use crate::error::{ProviderError, ProviderErrorKind};
use crate::git::url::GitRemoteUrl;

/// How LFS objects are reached for this remote.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LfsBackendKind {
    /// `<remote>/lfs/objects` on the local filesystem (file
    /// transport only).
    LocalObjects,
    /// No local object store — HTTP(S)/SSH remotes would need the
    /// LFS batch/download API, which this slice does not implement.
    Unsupported,
}

/// The LFS object backend selected for a remote.
pub(crate) enum LfsBackend {
    /// Local `lfs/objects` directory under the remote path.
    Local { root: PathBuf },
    /// Backend unavailable for this transport.
    Unsupported,
}

impl LfsBackend {
    /// Selects the backend for `remote`.
    #[must_use]
    pub(crate) fn for_remote(remote: &GitRemoteUrl) -> Self {
        match remote.lfs_objects_root() {
            Some(root) => Self::Local { root },
            None => Self::Unsupported,
        }
    }

    /// The declared backend kind for `GitProviderInfo`.
    #[must_use]
    pub const fn kind(&self) -> LfsBackendKind {
        match self {
            Self::Local { .. } => LfsBackendKind::LocalObjects,
            Self::Unsupported => LfsBackendKind::Unsupported,
        }
    }

    /// Opens the LFS object `oid` (64-hex SHA-256) for reading.
    ///
    /// `expected_size` is the pointer's declared size when known: a
    /// file whose length differs fails *before* streaming — a corrupt
    /// or swapped object never reaches the verifier as plausible
    /// bytes. `None` skips the pre-check (the SHA-256 verifier still
    /// applies downstream).
    ///
    /// # Errors
    /// [`ProviderErrorKind::UnsupportedCapability`] for non-local
    /// backends; `NotFound` for a missing object;
    /// `IntegrityMismatch` for a size mismatch;
    /// `InvalidRequest` for a malformed oid.
    pub(crate) fn open_object(
        &self,
        oid: &str,
        expected_size: Option<u64>,
        cx: &ProviderContext,
    ) -> Result<File, ProviderError> {
        cx.check_interrupts()?;
        let Self::Local { root } = self else {
            return Err(ProviderError::new(
                ProviderErrorKind::UnsupportedCapability,
                "git-lfs object download is unsupported for this remote transport",
            ));
        };
        if oid.len() != 64 || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "malformed git-lfs object id",
            ));
        }
        // LFS layout: objects/<oid[0..2]>/<oid[2..4]>/<oid> — the
        // file name is the full oid inside the two-level fan-out.
        let (a, rest) = oid.split_at(2);
        let (b, _) = rest.split_at(2);
        let path: PathBuf = root.join(a).join(b).join(oid);
        let file = File::open(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ProviderError::new(
                ProviderErrorKind::NotFound,
                format!("git-lfs object {oid} is not present on the remote"),
            ),
            std::io::ErrorKind::PermissionDenied => ProviderError::new(
                ProviderErrorKind::PermissionDenied,
                format!("git-lfs object {oid} is not readable"),
            ),
            _ => ProviderError::new(
                ProviderErrorKind::TransientIo,
                format!("git-lfs object open failed: {e}"),
            ),
        })?;
        let len = file
            .metadata()
            .map_err(|e| {
                ProviderError::new(
                    ProviderErrorKind::TransientIo,
                    format!("git-lfs object stat failed: {e}"),
                )
            })?
            .len();
        if let Some(expected) = expected_size
            && len != expected
        {
            return Err(ProviderError::new(
                ProviderErrorKind::IntegrityMismatch,
                format!("git-lfs object {oid} size {len} != pointer-declared {expected}"),
            ));
        }
        Ok(file)
    }
}
