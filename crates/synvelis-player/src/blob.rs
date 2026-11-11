//! Verified blob resolution — the single authority mapping a recorded
//! content-digest string to its blob file with byte-level
//! re-verification.
//!
//! The recorded digest is classified by
//! [`synvelis_project::project::AssetDigest`]:
//!
//! - [`DigestKind::Sha256`] (`"sha256:<64hex>"`) resolves under the
//!   algorithm-namespaced store `<media_root>/blobs/sha256/<lowercase
//!   hex>` — the same mapping [`synvelis_library::LocalBlobDir`]
//!   applies to object identities — and is re-verified with SHA-256.
//! - [`DigestKind::Crc32Fold`] (exactly 16 lowercase hex chars)
//!   resolves under the legacy flat `<media_root>/blobs/<digest>` and
//!   is re-verified with the forward+reverse CRC32 fold.
//! - [`DigestKind::Opaque`] never resolves: it is not a verified
//!   content address, so resolution fails closed with
//!   [`ResolveError::UnsupportedDigest`].
//!
//! Digest measurement runs on the same [`File`] later handed to the
//! demuxer — a path swap between verify and decode cannot put
//! unverified bytes under a verified digest. The returned file is
//! rewound to byte 0.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use sha2::Digest as _;
use synvelis_project::project::{AssetDigest, DigestKind};

use crate::error::ResolveError;
use crate::import::{blob_path, content_digest_stream};

/// A content blob opened and digest-verified.
#[derive(Debug)]
pub struct VerifiedBlob {
    /// The same open object that produced the digest measurement,
    /// positioned at byte 0 — handing it to the demuxer closes the
    /// verify-then-open gap.
    pub file: File,
    /// The resolved blob path (diagnostics).
    pub path: PathBuf,
}

/// Opens the blob backing `digest` under `media_root` and re-verifies
/// its bytes with the digest kind's scheme.
///
/// # Errors
/// [`ResolveError::MissingMedia`] when the blob is absent,
/// [`ResolveError::Io`] when it exists but cannot be read,
/// [`ResolveError::HashMismatch`] on byte drift,
/// [`ResolveError::UnsupportedDigest`] for an opaque (non
/// content-addressed) digest string — fail-closed.
pub fn open_blob_verified(media_root: &Path, digest: &str) -> Result<VerifiedBlob, ResolveError> {
    match AssetDigest::parse(digest).kind() {
        DigestKind::Sha256 => {
            // `AssetDigest` guarantees the `sha256:` tag and a 64-hex
            // payload; the blob layer lowercases the hex for the path.
            let hex = digest
                .strip_prefix("sha256:")
                .unwrap_or(digest)
                .to_ascii_lowercase();
            let path = media_root.join("blobs").join("sha256").join(&hex);
            let mut file = open_typed(&path, digest)?;
            let measured = sha256_stream(&mut file).map_err(|e| ResolveError::Io {
                detail: e.to_string(),
            })?;
            if measured != hex {
                return Err(ResolveError::HashMismatch {
                    expected: digest.to_owned(),
                    actual: format!("sha256:{measured}"),
                });
            }
            Ok(VerifiedBlob { file, path })
        }
        DigestKind::Crc32Fold => {
            let path = blob_path(media_root, digest);
            let mut file = open_typed(&path, digest)?;
            let actual = content_digest_stream(&mut file).map_err(|e| ResolveError::Io {
                detail: e.to_string(),
            })?;
            if actual != digest {
                return Err(ResolveError::HashMismatch {
                    expected: digest.to_owned(),
                    actual,
                });
            }
            Ok(VerifiedBlob { file, path })
        }
        DigestKind::Opaque => Err(ResolveError::UnsupportedDigest {
            digest: digest.to_owned(),
        }),
    }
}

/// Opens `path` keeping the missing/unreadable distinction honest:
/// absent means `MissingMedia`, present-but-unreadable means `Io`.
fn open_typed(path: &Path, digest: &str) -> Result<File, ResolveError> {
    File::open(path).map_err(|e| {
        if path.exists() {
            ResolveError::Io {
                detail: e.to_string(),
            }
        } else {
            ResolveError::MissingMedia {
                digest: digest.to_owned(),
            }
        }
    })
}

/// Streams `file` to EOF measuring SHA-256, then rewinds to byte 0 —
/// the same shape as [`content_digest_stream`], for the strong-digest
/// namespace. Returns the lowercase hex payload.
fn sha256_stream(file: &mut File) -> std::io::Result<String> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    file.seek(SeekFrom::Start(0))?;
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push(HEX[usize::from(b >> 4)] as char);
        out.push(HEX[usize::from(b & 0x0f)] as char);
    }
    Ok(out)
}
