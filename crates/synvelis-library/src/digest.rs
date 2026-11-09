//! Streaming strong-digest verification.
//!
//! [`VerifyingReader`] wraps any `Read` and computes SHA-256 as bytes
//! pass through. At EOF it compares the measured digest (and byte
//! count) with the pin's expectation and writes the typed outcome into
//! the shared [`FetchCompletion`] slot:
//!
//! - digest or length mismatch → the final `read` returns
//!   `io::Error(InvalidData)` and the slot records
//!   [`ProviderErrorKind::IntegrityMismatch`]. Mutated bytes are never
//!   silently delivered as "pinned".
//! - cancellation or deadline → `io::Error(Interrupted)` and the slot
//!   records `Cancelled` / `BudgetExceeded`.
//! - clean EOF → the slot records `Ok(`[`FetchReceipt`]`)`.
//!
//! The terminal slot is owned by [`crate::FetchCompletion`].

use std::io::{self, Read};
use std::sync::Arc;
use std::time::Instant;

use sha2::Digest as _;

use crate::context::CancellationToken;
use crate::error::{ProviderError, ProviderErrorKind};
use crate::provider::{FetchReceipt, FetchSlot};
use crate::refs::{ContentDigest, DigestAlgorithm, ObjectIdentity};

const CHUNK_LIMIT: usize = 1 << 16;

/// Short hex fingerprint (first 16 hex chars of SHA-256) for cursor
/// binding. Fingerprints never carry the hashed values themselves.
#[must_use]
pub(crate) fn short_fingerprint(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = sha2::Sha256::digest(bytes);
    let mut out = String::with_capacity(16);
    for &b in &digest[..8] {
        out.push(HEX[usize::from(b >> 4)] as char);
        out.push(HEX[usize::from(b & 0x0f)] as char);
    }
    out
}

/// Full SHA-256 hex fingerprint (64 chars). Used where a fingerprint
/// is a *security-relevant namespace separator* — e.g. the on-disk
/// principal tag of the ALIB-03 cache — and a 64-bit truncation
/// would be an unacceptable collision surface.
#[must_use]
pub(crate) fn full_fingerprint(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = sha2::Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for &b in &digest {
        out.push(HEX[usize::from(b >> 4)] as char);
        out.push(HEX[usize::from(b & 0x0f)] as char);
    }
    out
}

/// Streams `reader` to EOF computing SHA-256, honoring cancel,
/// deadline, and the byte budget.
///
/// Returns `(digest, bytes_read)`. A file larger than `max_bytes`
/// fails with [`ProviderErrorKind::BudgetExceeded`] rather than
/// silently truncating.
///
/// # Errors
/// [`ProviderError`] — cancel, deadline, budget, or read failure.
pub(crate) fn stream_digest(
    reader: &mut impl Read,
    cancel: &CancellationToken,
    deadline: Option<Instant>,
    max_bytes: u64,
) -> Result<(ContentDigest, u64), ProviderError> {
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; CHUNK_LIMIT];
    let mut total = 0u64;
    loop {
        check_interrupts(cancel, deadline)?;
        let n = reader
            .read(&mut buf)
            .map_err(|e| io_provider_error("read", &e))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > max_bytes {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                format!("content exceeds byte budget {max_bytes}"),
            ));
        }
        hasher.update(&buf[..n]);
    }
    Ok((
        ContentDigest::new(DigestAlgorithm::Sha256, &hasher.finalize()),
        total,
    ))
}

fn check_interrupts(
    cancel: &CancellationToken,
    deadline: Option<Instant>,
) -> Result<(), ProviderError> {
    if cancel.is_cancelled() {
        return Err(ProviderError::new(
            ProviderErrorKind::Cancelled,
            "operation cancelled",
        ));
    }
    if let Some(deadline) = deadline
        && Instant::now() >= deadline
    {
        return Err(ProviderError::new(
            ProviderErrorKind::BudgetExceeded,
            "provider deadline exceeded",
        ));
    }
    Ok(())
}

pub(crate) fn io_provider_error(op: &'static str, e: &io::Error) -> ProviderError {
    match e
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<ProviderError>())
    {
        Some(typed) => typed.clone(),
        None => ProviderError::new(ProviderErrorKind::TransientIo, format!("{op}: {e}")),
    }
}

/// Shared terminal slot: the stream writes exactly one typed outcome
/// once it reaches EOF or fails terminally.
pub(crate) type CompletionSlot = Arc<FetchSlot>;

/// A `Read` wrapper that verifies fetched bytes against the pin's
/// expectation as they stream through.
pub(crate) struct VerifyingReader<R> {
    inner: R,
    hasher: sha2::Sha256,
    bytes: u64,
    expected_digest: Option<ContentDigest>,
    expected_size: Option<u64>,
    max_bytes: u64,
    cancel: CancellationToken,
    deadline: Option<Instant>,
    object_identity: ObjectIdentity,
    slot: CompletionSlot,
    finished: bool,
}

impl<R: Read> VerifyingReader<R> {
    /// Wraps `inner`. When `expected_digest` is `Some`, EOF verifies
    /// the measured digest; `expected_size` is checked as a fetch
    /// precondition at EOF.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        inner: R,
        expected_digest: Option<ContentDigest>,
        expected_size: Option<u64>,
        max_bytes: u64,
        cancel: CancellationToken,
        deadline: Option<Instant>,
        object_identity: ObjectIdentity,
        slot: CompletionSlot,
    ) -> Self {
        Self {
            inner,
            hasher: sha2::Sha256::new(),
            bytes: 0,
            expected_digest,
            expected_size,
            max_bytes,
            cancel,
            deadline,
            object_identity,
            slot,
            finished: false,
        }
    }

    fn finish(&mut self, outcome: Result<FetchReceipt, ProviderError>) {
        if !self.finished {
            self.finished = true;
            self.slot.record(outcome);
        }
    }

    fn fail(&mut self, err: ProviderError) -> io::Error {
        self.finish(Err(err.clone()));
        io::Error::from(err)
    }

    fn conclude(&mut self) -> io::Result<()> {
        // EOF reached: verify the measured state against the pin.
        let measured = ContentDigest::new(DigestAlgorithm::Sha256, &self.hasher.clone().finalize());
        if let Some(expected) = &self.expected_size
            && self.bytes != *expected
        {
            return Err(self.fail(ProviderError::new(
                ProviderErrorKind::IntegrityMismatch,
                format!(
                    "byte length {} != pinned expected_size {expected}",
                    self.bytes
                ),
            )));
        }
        if self.bytes > self.max_bytes {
            return Err(self.fail(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                format!("content exceeds byte budget {}", self.max_bytes),
            )));
        }
        if let Some(expected) = &self.expected_digest
            && measured != *expected
        {
            return Err(self.fail(ProviderError::new(
                ProviderErrorKind::IntegrityMismatch,
                format!("digest {measured} != pinned {expected}"),
            )));
        }
        self.finish(Ok(FetchReceipt {
            object_identity: self.object_identity.clone(),
            bytes_read: self.bytes,
            digest: Some(measured),
            verified: self.expected_digest.is_some(),
        }));
        Ok(())
    }
}

impl<R: Read> Read for VerifyingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // A zero-length read must be a no-op — letting it fall through
        // to `inner.read(&mut [])` would observe a spurious `Ok(0)` and
        // conclude the stream at a false EOF.
        if self.finished || buf.is_empty() {
            return Ok(0);
        }
        if let Err(err) = check_interrupts(&self.cancel, self.deadline) {
            return Err(self.fail(err));
        }
        match self.inner.read(buf) {
            Ok(0) => {
                self.conclude()?;
                Ok(0)
            }
            Ok(n) => {
                self.hasher.update(&buf[..n]);
                self.bytes += n as u64;
                if self.bytes > self.max_bytes {
                    return Err(self.fail(ProviderError::new(
                        ProviderErrorKind::BudgetExceeded,
                        format!("content exceeds byte budget {}", self.max_bytes),
                    )));
                }
                Ok(n)
            }
            Err(e) => {
                let typed = io_provider_error("read", &e);
                let _ = self.fail(typed);
                Err(e)
            }
        }
    }
}

/// Builds the shared completion slot.
#[must_use]
pub(crate) fn completion_slot() -> CompletionSlot {
    Arc::new(FetchSlot::new())
}
