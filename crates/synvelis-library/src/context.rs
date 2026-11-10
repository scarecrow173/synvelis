//! Per-call provider context: principal, cancellation, limits,
//! credential access, and the observation clock.
//!
//! `ProviderContext` is passed explicitly to every provider call.
//! Cancellation and deadlines are *cooperative*: providers check them
//! at well-defined points (each page step, each read chunk) and fail
//! with `Cancelled` / `BudgetExceeded` rather than blocking forever.
//! Credential *keys* travel through the context; secret values never
//! appear in the Project snapshot.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use crate::error::{ProviderError, ProviderErrorKind};
use crate::id::{CredentialKey, OpaqueCredential, PrincipalId};

/// Cooperative cancellation token.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    flag: Arc<AtomicBool>,
}

impl CancellationToken {
    /// A fresh, unset token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Shared across clones.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// `true` once [`CancellationToken::cancel`] ran.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// A token aliasing an externally-owned shared flag.
    ///
    /// Adapter seam for foreign cancel types (e.g. the project
    /// crate's `VcsCancel`): cancelling either side is observed by
    /// both. The flag must only ever be used as a cancellation
    /// signal.
    #[doc(hidden)]
    #[must_use]
    pub fn from_shared(flag: Arc<AtomicBool>) -> Self {
        Self { flag }
    }
}

/// Bounded resource limits applied to provider operations.
#[derive(Clone, Debug)]
pub struct ProviderLimits {
    /// Maximum entries walked across a whole listing (all pages of a
    /// query share the scope but each call re-walks bounded).
    pub max_entries: u32,
    /// Maximum directory depth under the browse scope.
    pub max_depth: u32,
    /// Maximum bytes read by a single pin/fetch stream.
    pub max_bytes: u64,
    /// Optional wall-clock deadline for one call.
    pub deadline: Option<Instant>,
}

impl Default for ProviderLimits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_depth: 64,
            max_bytes: u64::MAX,
            deadline: None,
        }
    }
}

/// Observation clock — injected for deterministic tests.
pub trait Clock: Send + Sync {
    /// The current wall-clock time.
    fn now(&self) -> SystemTime;
}

/// The system clock.
#[derive(Copy, Clone, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// A fixed clock for deterministic tests.
#[derive(Copy, Clone, Debug)]
pub struct FixedClock(pub SystemTime);

impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

/// Resolves credential *keys* into opaque handles at use time.
///
/// Implementations live in the app layer (OS credential store,
/// session vault, ...). The library only passes keys through; baseline
/// providers never call this.
pub trait CredentialResolver: Send + Sync {
    /// Resolves `key` into an opaque credential handle.
    ///
    /// # Errors
    /// [`ProviderError`] — missing key, expired session, store failure.
    fn resolve(&self, key: &CredentialKey) -> Result<OpaqueCredential, ProviderError>;
}

/// The explicit per-call context.
#[derive(Clone)]
pub struct ProviderContext {
    /// Principal scope this call runs under. Cursors bind to it.
    pub principal: PrincipalId,
    /// Cooperative cancellation.
    pub cancel: CancellationToken,
    /// Resource limits.
    pub limits: ProviderLimits,
    /// Optional credential resolver (remote providers).
    pub credentials: Option<Arc<dyn CredentialResolver>>,
    /// Observation clock.
    pub clock: Arc<dyn Clock>,
}

impl ProviderContext {
    /// A context for `principal` with default limits and no
    /// credentials.
    #[must_use]
    pub fn new(principal: PrincipalId) -> Self {
        Self {
            principal,
            cancel: CancellationToken::new(),
            limits: ProviderLimits::default(),
            credentials: None,
            clock: Arc::new(SystemClock),
        }
    }

    /// Fails when the call was cancelled or its deadline passed.
    ///
    /// # Errors
    /// [`ProviderErrorKind::Cancelled`] or
    /// [`ProviderErrorKind::BudgetExceeded`].
    pub fn check_interrupts(&self) -> Result<(), ProviderError> {
        if self.cancel.is_cancelled() {
            return Err(ProviderError::new(
                ProviderErrorKind::Cancelled,
                "operation cancelled",
            ));
        }
        if let Some(deadline) = self.limits.deadline
            && Instant::now() >= deadline
        {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                "provider deadline exceeded",
            ));
        }
        Ok(())
    }

    /// The observation timestamp for facts produced now.
    #[must_use]
    pub fn now(&self) -> crate::ObservedAt {
        crate::ObservedAt(self.clock.now())
    }
}

/// `ProviderContext` is shareable across threads (`Send + Sync`).
#[allow(dead_code)]
fn assert_context_send_sync() {
    fn assert<T: Send + Sync>() {}
    assert::<ProviderContext>();
    // Silence unused warnings on the inner Arc<Mutex<()>> pattern
    // some embedders use to build custom clocks.
    let _ = Mutex::new(());
}
