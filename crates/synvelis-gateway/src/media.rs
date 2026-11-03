//! GW-07 verified-media domain — the staging/verification seam job
//! outputs flow through before they may be registered as project
//! Asset/Source (issue #17 contracts, GIA-002).
//!
//! A job output is *declared* media — claimed digest, format, length.
//! The [`MediaPort`] re-derives the evidence (hash/format/length) and
//! produces a verdict; the [`MediaBook`] records the lifecycle per
//! media ref. Only `Verified` records are registerable: the
//! [`MediaGate`] is the read seam GW-04's `synvelis.media.register`
//! lowering consults, and it is owner-scoped and existence-blind —
//! unknown, foreign, rejected, and still-validating media share one
//! `None`.
//!
//! A verified record is never deleted on placement: *verified media
//! without commit leaves the project revision unchanged*, and a denied
//! or never-requested apply leaves the verified record protected —
//! the book is the orphan-protection owner, not the provider.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use synvelis_project::ProjectId;

use crate::id::{JobRef, MediaRef};

use crate::job::{DeclaredOutput, OutputKind};
use crate::request::Principal;

/// The staged-media lifecycle (GIA-002 Staged Media states).
///
/// Forward evidence, not a state machine with rewrites: `Staged` is
/// the recorded landing state, `Validating` marks verification in
/// progress, `Verified`/`Rejected`/`Unavailable` are terminal verdicts
/// the media port produced.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MediaLifecycle {
    /// Landed from the job; not yet verified.
    Staged,
    /// Verification is in progress.
    Validating,
    /// Hash/format/length evidence re-derived and matched.
    Verified,
    /// Verification evidence contradicted the declaration — this
    /// media may never be registered into a project.
    Rejected,
    /// The content could not be verified at all (missing bytes,
    /// unreadable staging) — likewise never registerable.
    Unavailable,
}

impl MediaLifecycle {
    /// The wire status name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Staged => "staged",
            Self::Validating => "validating",
            Self::Verified => "verified",
            Self::Rejected => "rejected",
            Self::Unavailable => "unavailable",
        }
    }
}

/// The verification product — the re-derived evidence a registered
/// Source records as its `content_digest` (never the agent's or
/// provider's claim alone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedMedia {
    /// The staged-media reference.
    pub media: MediaRef,
    /// The re-derived content digest.
    pub content_digest: String,
    /// The verified media format tag.
    pub media_format: String,
    /// The verified byte length.
    pub byte_len: u64,
}

/// The media port's verdict for one declared output.
#[derive(Clone, Debug, PartialEq)]
pub enum MediaVerdict {
    /// Evidence re-derived and matched.
    Verified(VerifiedMedia),
    /// Evidence contradicted the declaration.
    Rejected {
        /// Bounded reason (never internal detail).
        reason: String,
    },
    /// The content could not be verified.
    Unavailable {
        /// Bounded reason.
        reason: String,
    },
}

/// The seam to the Synvelis staging/hash/format/length verification
/// stack (issue #17 — `synvelis-recording`'s staging/canonical lane).
///
/// The `PoC` wires [`crate::stub::MediaStub`], which re-derives the
/// evidence deterministically over the provider's declared content.
pub trait MediaPort: Send + Sync {
    /// Re-derives the verification evidence for one declared job
    /// output. Infallible at the seam level — failure modes are
    /// verdict variants, not errors.
    fn verify(&self, output: &DeclaredOutput) -> MediaVerdict;
}

/// One media-book record — owner-scoped verification truth.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaRecord {
    /// The producing job.
    pub job: JobRef,
    /// Owning principal — scoping, not authority.
    pub principal: Principal,
    /// Owning project.
    pub project: ProjectId,
    /// The declared output kind — the media gate only ever resolves
    /// `OutputKind::Media` records; a MIDI output is never a
    /// registerable staged-media ref.
    pub kind: OutputKind,
    /// The recorded lifecycle.
    pub lifecycle: MediaLifecycle,
    /// The verified descriptor (`Verified` only).
    pub verified: Option<VerifiedMedia>,
}

/// The media book's record bound — like the job book, media
/// correlation data is finite by contract.
const MEDIA_BOOK_CAP: usize = 4096;

/// The adapter's media book — records keyed by [`MediaRef`].
///
/// A record is written once per `(job, media)` at materialization;
/// the lifecycle it records is the verdict's own truth — a `Rejected`
/// record can never be re-marked `Verified` by re-running the same
/// declaration.
#[derive(Debug, Default)]
pub struct MediaBook {
    records: RwLock<BTreeMap<MediaRef, MediaRecord>>,
}

impl MediaBook {
    /// An empty book.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A shared empty book.
    #[must_use]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Records the verdict for `output` produced by `job`. A record
    /// already present is immutable — re-materialization replays the
    /// recorded verdict, never re-decides it. The write is atomic
    /// with the presence check.
    ///
    /// # Panics
    /// Panics if the book lock is poisoned.
    pub fn record(
        &self,
        job: &JobRef,
        principal: Principal,
        project: ProjectId,
        kind: OutputKind,
        output: &DeclaredOutput,
        verdict: &MediaVerdict,
    ) -> MediaRecord {
        let mut records = self.records.write().expect("media book poisoned");
        if let Some(existing) = records.get(&output.media) {
            return existing.clone();
        }
        if records.len() >= MEDIA_BOOK_CAP {
            // Fail closed on saturation: the output reports
            // Unavailable and is never registrable (no record → the
            // gate finds nothing).
            return MediaRecord {
                job: job.clone(),
                principal,
                project,
                kind,
                lifecycle: MediaLifecycle::Unavailable,
                verified: None,
            };
        }
        let (lifecycle, verified) = match verdict {
            MediaVerdict::Verified(v) => (MediaLifecycle::Verified, Some(v.clone())),
            MediaVerdict::Rejected { .. } => (MediaLifecycle::Rejected, None),
            MediaVerdict::Unavailable { .. } => (MediaLifecycle::Unavailable, None),
        };
        let record = MediaRecord {
            job: job.clone(),
            principal,
            project,
            kind,
            lifecycle,
            verified,
        };
        records.insert(output.media.clone(), record.clone());
        record
    }

    /// The record for `media` — raw lookup (introspection/tests).
    ///
    /// # Panics
    /// Panics if the book lock is poisoned.
    #[must_use]
    pub fn get(&self, media: &MediaRef) -> Option<MediaRecord> {
        self.records
            .read()
            .expect("media book poisoned")
            .get(media)
            .cloned()
    }

    /// Number of records (test/introspection).
    ///
    /// # Panics
    /// Panics if the book lock is poisoned.
    #[must_use]
    pub fn count(&self) -> usize {
        self.records.read().expect("media book poisoned").len()
    }
}

/// The read seam `synvelis.media.register`'s gated lowering consults —
/// one honest question: "may this caller register this media ref as a
/// project source?"
///
/// `None` blinds everything non-registerable: unknown refs, foreign
/// callers, rejected/unavailable media, and still-staged records share
/// the same answer. Implemented by [`MediaBook`].
pub trait MediaGate: Send + Sync {
    /// The verified descriptor iff `media` is `Verified` *and* owned by
    /// `(principal, project)`.
    fn verified_media(
        &self,
        principal: &Principal,
        project: ProjectId,
        media: &MediaRef,
    ) -> Option<VerifiedMedia>;
}

impl MediaGate for MediaBook {
    fn verified_media(
        &self,
        principal: &Principal,
        project: ProjectId,
        media: &MediaRef,
    ) -> Option<VerifiedMedia> {
        let record = self.get(media)?;
        if record.lifecycle == MediaLifecycle::Verified
            && record.kind == OutputKind::Media
            && record.principal == *principal
            && record.project == project
        {
            record.verified.clone()
        } else {
            None
        }
    }
}
