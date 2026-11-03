//! GW-10 deterministic scheduling and fault-injection seams (test/PoC
//! evidence only — production code paths use [`MutexFence`] and
//! [`NoHooks`]).
//!
//! The [`ScheduledFence`] parks labeled fence sections until the driver
//! releases them, so every Revoke/Commit ordering is *enumerated*, not
//! raced probabilistically. [`CrashHook`] injects process death at a
//! named commit-stage boundary through the production [`CommitHooks`]
//! seam. Both are the shared harness the GW-10 verification suites use
//! for concurrency and crash-recovery evidence (Issue #62).

use std::cell::RefCell;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::commit::{CommitFence, CommitHooks, FenceCtx, GateCrash};

/// A fence whose sections park under a caller-set label until released.
///
/// Cheaply cloneable — clones share one schedule so the gate and the
/// test driver coordinate on the same fence. Only *labeled* sections
/// park; unlabeled callers (setup, journaled policy admin on the main
/// thread) run free like production traffic.
#[derive(Clone, Default)]
pub struct ScheduledFence {
    inner: Arc<SchedInner>,
}

#[derive(Default)]
struct SchedInner {
    sched: Mutex<Sched>,
    cv: Condvar,
}

#[derive(Default)]
struct Sched {
    parked: Vec<String>,
    released: Vec<String>,
}

thread_local! {
    static SECTION_LABEL: RefCell<Option<String>> = const { RefCell::new(None) };
}

impl ScheduledFence {
    /// A fresh scheduled fence.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs `f` attributing every fence section it enters to `label`.
    #[allow(clippy::unused_self)] // thread-local label, self kept for call-site symmetry
    pub fn run<R>(&self, label: &str, f: impl FnOnce() -> R) -> R {
        SECTION_LABEL.with(|l| *l.borrow_mut() = Some(label.to_owned()));
        let out = f();
        SECTION_LABEL.with(|l| *l.borrow_mut() = None);
        out
    }

    /// The labels of the sections currently parked at the fence.
    ///
    /// # Panics
    /// The schedule lock is poisoned.
    #[must_use]
    pub fn parked(&self) -> Vec<String> {
        self.inner.sched.lock().unwrap().parked.clone()
    }

    /// Blocks until `n` sections are parked (bounded — a stuck test
    /// fails loudly instead of hanging).
    ///
    /// # Panics
    /// The deadline elapses before `n` sections arrive.
    pub fn wait_parked(&self, n: usize) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.parked().len() < n {
            assert!(Instant::now() < deadline, "fence sections never arrived");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Admits the section labeled `label` through the fence. Safe to
    /// call before the section arrives — the grant is queued.
    ///
    /// # Panics
    /// The schedule lock is poisoned.
    pub fn release(&self, label: &str) {
        let mut sched = self.inner.sched.lock().unwrap();
        sched.released.push(label.to_owned());
        self.inner.cv.notify_all();
    }
}

impl CommitFence for ScheduledFence {
    fn section<R>(&self, f: impl FnOnce() -> R) -> R {
        // Only *labeled* sections park — unlabeled callers (setup,
        // journaled policy admin on the main thread) run free like
        // production traffic.
        let Some(label) = SECTION_LABEL.with(|l| l.borrow().clone()) else {
            return f();
        };
        {
            let mut sched = self.inner.sched.lock().unwrap();
            sched.parked.push(label.clone());
            loop {
                if let Some(pos) = sched.released.iter().position(|l| *l == label) {
                    sched.released.remove(pos);
                    break;
                }
                sched = self.inner.cv.wait(sched).unwrap();
            }
            sched.parked.retain(|l| *l != label);
        }
        f()
    }
}

/// One-shot injected process death at a named fence stage boundary.
///
/// The stage name matches the [`CommitHooks`] method it arms:
/// `"authorize"`, `"apply"`, or `"journal"`.
pub struct CrashHook(pub &'static str);

impl CommitHooks for CrashHook {
    fn after_authorize(&mut self, _ctx: &FenceCtx<'_>) -> Result<(), GateCrash> {
        if self.0 == "authorize" {
            return Err(GateCrash("injected".to_owned()));
        }
        Ok(())
    }
    fn after_apply(&mut self, _ctx: &FenceCtx<'_>) -> Result<(), GateCrash> {
        if self.0 == "apply" {
            return Err(GateCrash("injected".to_owned()));
        }
        Ok(())
    }
    fn after_journal(&mut self, _ctx: &FenceCtx<'_>) -> Result<(), GateCrash> {
        if self.0 == "journal" {
            return Err(GateCrash("injected".to_owned()));
        }
        Ok(())
    }
}
