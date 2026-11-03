//! GW-07 deterministic provider/media stubs — the `PoC` fixtures for
//! the non-RT job and verified-media lanes (JM/JOB evidence).
//!
//! [`ProviderStub`] is the deterministic [`JobPort`]: minted `job-N`
//! refs, test-driven phase transitions, scriptable cancel behavior
//! (`Confirm`/`Pending`/`Unreachable`), and declared outputs written
//! into a shared [`StubContent`] byte store. [`MediaStub`] is the
//! deterministic [`MediaPort`]: it re-derives SHA-256 digest/format/
//! length evidence from the same byte store — a tampered declaration
//! deterministically rejects.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use sha2::Digest;

use crate::id::{JobRef, MediaRef};
use crate::job::{CancelResponse, DeclaredOutput, JobPhase, JobPort, JobSpec, JobStatus};
use crate::media::{MediaPort, MediaVerdict, VerifiedMedia};
use crate::outcome::{ErrorEnvelope, GatewayError};
use crate::provenance::ProviderClaim;

/// The stub's deterministic content store — provider-declared bytes
/// keyed by media ref. Shared between the provider (writes at submit)
/// and the media stub (re-reads at verify).
#[derive(Debug, Default)]
pub struct StubContent {
    bytes: RwLock<BTreeMap<MediaRef, Vec<u8>>>,
}

impl StubContent {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A shared empty store.
    #[must_use]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// The stub digest — `"sha256:<64 lowercase hex>"` over `bytes`.
    #[must_use]
    pub fn digest(bytes: &[u8]) -> String {
        let mut hasher = sha2::Sha256::new();
        hasher.update(bytes);
        format!("sha256:{:x}", hasher.finalize())
    }

    /// The stored bytes for `media`, when present.
    ///
    /// # Panics
    /// Panics if the store lock is poisoned.
    #[must_use]
    pub fn bytes(&self, media: &MediaRef) -> Option<Vec<u8>> {
        self.bytes
            .read()
            .expect("stub content poisoned")
            .get(media)
            .cloned()
    }

    fn write(&self, media: MediaRef, bytes: Vec<u8>) {
        self.bytes
            .write()
            .expect("stub content poisoned")
            .insert(media, bytes);
    }
}

/// How the stub answers a cancel request.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CancelMode {
    /// Stops a live job deterministically (`Confirmed`).
    Confirm,
    /// Accepts but never confirms (`Pending` — the job keeps running).
    Pending,
    /// Never answers (`Unreachable` — provider-unconfirmed evidence).
    Unreachable,
}

/// What the stub's status polls do.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DriveMode {
    /// Phases change only via the test's `set_phase`/`finish`.
    Manual,
    /// Each `status` poll advances `Pending → Running → Completed`.
    OnPoll,
}

/// One declared-output template — the stub mints a fresh `media-N-M`
/// per job from it.
#[derive(Clone, Debug)]
pub struct StubOutputSpec {
    /// The deterministic payload bytes.
    pub bytes: Vec<u8>,
    /// The declared format tag.
    pub media_format: String,
    /// Declare a digest that does not match `bytes` — the
    /// deterministic verification-failure fixture.
    pub tamper_digest: bool,
}

impl StubOutputSpec {
    /// A well-formed media output.
    #[must_use]
    pub fn media(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            media_format: "audio/wav".to_owned(),
            tamper_digest: false,
        }
    }

    /// The same output with a forged declared digest — the JOB-05
    /// fixture (completed job, unverifiable output).
    #[must_use]
    pub fn tampered(mut self) -> Self {
        self.tamper_digest = true;
        self
    }
}

struct StubJob {
    phase: JobPhase,
    progress: Option<u64>,
    outputs: Vec<DeclaredOutput>,
}

/// The deterministic provider stub — one instance stands in for one
/// registered provider key on [`crate::job_engine::JobEngine`].
pub struct ProviderStub {
    content: Arc<StubContent>,
    jobs: RwLock<BTreeMap<JobRef, StubJob>>,
    next: std::sync::Mutex<u64>,
    outputs: Vec<StubOutputSpec>,
    claim: ProviderClaim,
    cancel_mode: CancelMode,
    drive: DriveMode,
    submitted: RwLock<Vec<JobSpec>>,
}

impl ProviderStub {
    /// A stub minting `job-N` refs over `content`, producing
    /// `outputs` per job with `claim` as its provenance claim.
    #[must_use]
    pub fn new(
        content: Arc<StubContent>,
        outputs: Vec<StubOutputSpec>,
        claim: ProviderClaim,
    ) -> Self {
        Self {
            content,
            jobs: RwLock::new(BTreeMap::new()),
            next: std::sync::Mutex::new(0),
            outputs,
            claim,
            cancel_mode: CancelMode::Confirm,
            drive: DriveMode::Manual,
            submitted: RwLock::new(Vec::new()),
        }
    }

    /// The default generation fixture — one well-formed media output,
    /// declared-but-unattested producer/model/rights.
    #[must_use]
    pub fn generation(content: Arc<StubContent>) -> Self {
        Self::new(
            content,
            vec![StubOutputSpec::media(b"stub-media-bytes".to_vec())],
            ProviderClaim {
                producer: Some("stub-provider".to_owned()),
                model: Some("stub-model".to_owned()),
                model_version: Some("1.0".to_owned()),
                rights: Some("stub-license".to_owned()),
                attested: std::collections::BTreeSet::new(),
            },
        )
    }

    /// The shared content store this stub writes declared outputs
    /// into — the fixture a [`MediaStub`] must read to verify them.
    #[must_use]
    pub fn content(&self) -> Arc<StubContent> {
        self.content.clone()
    }

    /// Sets the cancel answer mode.
    #[must_use]
    pub fn cancel_mode(mut self, mode: CancelMode) -> Self {
        self.cancel_mode = mode;
        self
    }

    /// Sets the status-poll drive mode.
    #[must_use]
    pub fn drive(mut self, mode: DriveMode) -> Self {
        self.drive = mode;
        self
    }

    /// Replaces the provenance claim.
    #[must_use]
    pub fn claim(mut self, claim: ProviderClaim) -> Self {
        self.claim = claim;
        self
    }

    /// Replaces the declared-output template.
    #[must_use]
    pub fn outputs(mut self, outputs: Vec<StubOutputSpec>) -> Self {
        self.outputs = outputs;
        self
    }

    /// Sets `job`'s phase (test-driven transitions).
    ///
    /// # Panics
    /// Panics when `job` is unknown to this stub.
    pub fn set_phase(&self, job: &JobRef, phase: JobPhase) {
        let mut jobs = self.jobs.write().expect("stub jobs poisoned");
        let record = jobs.get_mut(job).expect("unknown stub job");
        record.phase = phase;
    }

    /// Marks `job` completed (declares its outputs on the next poll —
    /// same shape a real completion reports).
    ///
    /// # Panics
    /// Panics when `job` is unknown to this stub.
    pub fn finish(&self, job: &JobRef) {
        self.set_phase(job, JobPhase::Completed);
    }

    /// Every spec the stub admitted — submission evidence.
    ///
    /// # Panics
    /// Panics if the submissions lock is poisoned.
    #[must_use]
    pub fn submissions(&self) -> Vec<JobSpec> {
        self.submitted
            .read()
            .expect("stub submissions poisoned")
            .clone()
    }

    /// The job ref for the `n`-th submission (1-based), when present.
    ///
    /// # Panics
    /// Panics if the jobs lock is poisoned.
    #[must_use]
    pub fn job_at(&self, n: u64) -> Option<JobRef> {
        JobRef::new(format!("job-{n}")).ok().filter(|j| {
            self.jobs
                .read()
                .expect("stub jobs poisoned")
                .contains_key(j)
        })
    }
}

impl JobPort for ProviderStub {
    fn submit(&self, spec: &JobSpec) -> Result<JobRef, ErrorEnvelope> {
        let seq = {
            let mut next = self.next.lock().expect("stub counter poisoned");
            *next += 1;
            *next
        };
        let job = JobRef::new(format!("job-{seq}")).map_err(|e| GatewayError::Internal {
            reason: e.to_string(),
        })?;
        let outputs = self
            .outputs
            .iter()
            .enumerate()
            .map(|(i, out)| {
                let media = MediaRef::new(format!("media-{seq}-{}", i + 1))
                    .expect("fixed-width media ref is valid");
                self.content.write(media.clone(), out.bytes.clone());
                DeclaredOutput {
                    media,
                    content_digest: if out.tamper_digest {
                        // A forged declaration — deterministic JOB-05
                        // evidence: verification must reject it.
                        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                            .to_owned()
                    } else {
                        StubContent::digest(&out.bytes)
                    },
                    media_format: out.media_format.clone(),
                    byte_len: out.bytes.len() as u64,
                }
            })
            .collect();
        self.jobs.write().expect("stub jobs poisoned").insert(
            job.clone(),
            StubJob {
                phase: JobPhase::Pending,
                progress: Some(0),
                outputs,
            },
        );
        self.submitted
            .write()
            .expect("stub submissions poisoned")
            .push(spec.clone());
        Ok(job)
    }

    fn status(&self, job: &JobRef) -> JobStatus {
        let mut jobs = self.jobs.write().expect("stub jobs poisoned");
        let Some(record) = jobs.get_mut(job) else {
            // An unknown job is blind — the port answers for what it
            // owns, and what it does not own is not resolvable.
            return JobStatus {
                phase: JobPhase::Failed {
                    reason: "unknown job".to_owned(),
                },
                progress: None,
                outputs: Vec::new(),
            };
        };
        if self.drive == DriveMode::OnPoll
            && matches!(record.phase, JobPhase::Pending | JobPhase::Running)
        {
            record.phase = match record.phase {
                JobPhase::Pending => JobPhase::Running,
                _ => JobPhase::Completed,
            };
        }
        record.progress = match record.phase {
            JobPhase::Completed => Some(100),
            JobPhase::Pending => Some(0),
            _ => record.progress,
        };
        let outputs = if record.phase == JobPhase::Completed {
            record.outputs.clone()
        } else {
            Vec::new()
        };
        JobStatus {
            phase: record.phase.clone(),
            progress: record.progress,
            outputs,
        }
    }

    fn request_cancel(&self, job: &JobRef) -> CancelResponse {
        if self.cancel_mode == CancelMode::Unreachable {
            return CancelResponse::Unreachable;
        }
        let mut jobs = self.jobs.write().expect("stub jobs poisoned");
        let Some(record) = jobs.get_mut(job) else {
            return CancelResponse::Settled {
                phase: JobPhase::Failed {
                    reason: "unknown job".to_owned(),
                },
            };
        };
        if record.phase.terminal() {
            // The settle race resolved to the job — report the actual
            // phase, never a false cancelled.
            return CancelResponse::Settled {
                phase: record.phase.clone(),
            };
        }
        match self.cancel_mode {
            CancelMode::Confirm => {
                record.phase = JobPhase::Cancelled;
                CancelResponse::Confirmed
            }
            CancelMode::Pending => CancelResponse::Pending,
            CancelMode::Unreachable => unreachable!("early return above"),
        }
    }

    fn claim(&self, _job: &JobRef) -> ProviderClaim {
        self.claim.clone()
    }
}

/// The deterministic media stub — re-derives digest/format/length
/// evidence from the shared content store.
pub struct MediaStub {
    content: Arc<StubContent>,
}

impl MediaStub {
    /// A stub verifying against `content`.
    #[must_use]
    pub fn new(content: Arc<StubContent>) -> Self {
        Self { content }
    }
}

impl MediaPort for MediaStub {
    fn verify(&self, output: &DeclaredOutput) -> MediaVerdict {
        let Some(bytes) = self.content.bytes(&output.media) else {
            return MediaVerdict::Unavailable {
                reason: "no staged content for media ref".to_owned(),
            };
        };
        let actual = StubContent::digest(&bytes);
        if actual != output.content_digest {
            return MediaVerdict::Rejected {
                reason: "content digest mismatch".to_owned(),
            };
        }
        if output.media_format != "audio/wav" {
            return MediaVerdict::Rejected {
                reason: "unsupported media format".to_owned(),
            };
        }
        if output.byte_len != bytes.len() as u64 {
            return MediaVerdict::Rejected {
                reason: "byte length mismatch".to_owned(),
            };
        }
        MediaVerdict::Verified(VerifiedMedia {
            media: output.media.clone(),
            content_digest: actual,
            media_format: output.media_format.clone(),
            byte_len: output.byte_len,
        })
    }
}
