//! GW-07 non-real-time job domain — the adapter-side records and the
//! [`JobPort`] seam to the Synvelis-owned job coordinator (issue #21).
//!
//! The adapter owns *linkage records*, not job state: the coordinator
//! behind [`JobPort`] is the sole authority for execution truth
//! ([`JobPhase`]). What the adapter records is the correlation an
//! authorized caller needs — operation ↔ job ↔ caller/task/grant, the
//! input snapshot bound at admission, the cancel linkage, and the
//! materialized result (media + provenance). Nothing here duplicates
//! the existing `synvelis-render` Export Job state, and no record is
//! tied to a connection or window lifetime.
//!
//! Cancel honesty (GIA-002): a cancel request is never reported as
//! `Cancelled` unless the provider confirms it. [`CancelLink`] keeps
//! the request's fate separate from the provider's execution truth —
//! `Requested` (accepted, unconfirmed), `Confirmed` (provider stopped
//! it), `Unconfirmed` (provider did not answer).

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::Value;
use synvelis_project::{ProjectId, ProjectRevision};

use crate::id::{GrantRef, JobRef, MediaRef, OperationId, RequestId, TaskRef};
use crate::outcome::ErrorEnvelope;
use crate::request::{OperationRequest, Principal, ResourceRef};

/// The canonical `"<kind>-<32hex>"` form of a resource ref — the same
/// tagged-id shape the wire and the changeset use.
#[must_use]
pub(crate) fn tagged(resource: &ResourceRef) -> String {
    match resource {
        ResourceRef::Project(id) => id.to_tagged(),
        ResourceRef::Track(id) => id.to_tagged(),
        ResourceRef::Lane(id) => id.to_tagged(),
        ResourceRef::Clip(id) => id.to_tagged(),
        ResourceRef::Source(id) => id.to_tagged(),
        ResourceRef::Asset(id) => id.to_tagged(),
        ResourceRef::Note(id) => id.to_tagged(),
        ResourceRef::TakeGroup(id) => id.to_tagged(),
        ResourceRef::Comp(id) => id.to_tagged(),
        ResourceRef::ParameterTarget(id) => id.to_tagged(),
        ResourceRef::AutomationLane(id) => id.to_tagged(),
        ResourceRef::Strip(id) => id.to_tagged(),
        ResourceRef::Send(id) => id.to_tagged(),
        ResourceRef::Processor(id) => id.to_tagged(),
    }
}

// ── Provider-owned truth ─────────────────────────────────────

/// The job coordinator's execution truth (GIA-002 Job Execution).
///
/// `Cancelled` exists only when the provider confirmed the stop — a
/// request alone never produces this phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobPhase {
    /// Admitted, not yet started.
    Pending,
    /// Executing.
    Running,
    /// Finished; `JobStatus::outputs` carries the declared outputs.
    Completed,
    /// Terminated on a provider-side failure.
    Failed {
        /// Bounded provider-reported reason.
        reason: String,
    },
    /// Provider-confirmed cancellation.
    Cancelled,
}

impl JobPhase {
    /// The wire status name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed { .. } => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Whether the phase admits no further provider-side transitions.
    #[must_use]
    pub const fn terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed { .. } | Self::Cancelled
        )
    }
}

/// The provider's status answer for one job.
#[derive(Clone, Debug, PartialEq)]
pub struct JobStatus {
    /// The execution phase.
    pub phase: JobPhase,
    /// Completion progress (0..=100), when the provider reports it.
    pub progress: Option<u64>,
    /// Outputs the provider declares — populated on `Completed`.
    pub outputs: Vec<DeclaredOutput>,
}

/// What a job kind is — the adapter-side tag on `JobSpec`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JobKind {
    /// AI/media generation producing derived outputs.
    Generation,
    /// Analysis over bound inputs producing metadata outputs.
    Analysis,
}

impl JobKind {
    /// The wire tag.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Generation => "generation",
            Self::Analysis => "analysis",
        }
    }
}

/// What a job produces.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OutputKind {
    /// Staged media (audio/image — the verified-media lane).
    Media,
    /// A MIDI payload (registered through the MIDI-source lane).
    Midi,
}

impl OutputKind {
    /// The wire tag.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Media => "media",
            Self::Midi => "midi",
        }
    }
}

/// One provider-declared job output — metadata only, never bytes.
///
/// The declared digest/format/length are *claims*: they become truth
/// only after the media verification seam re-derives them
/// (GIA-002 `Staged → Validating → Verified`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredOutput {
    /// The staged-media reference the provider assigned.
    pub media: MediaRef,
    /// Claimed content digest.
    pub content_digest: String,
    /// Claimed media format tag.
    pub media_format: String,
    /// Claimed byte length.
    pub byte_len: u64,
}

/// One input pinned at admission — the resource plus the evidence the
/// pinned base carried for it.
#[derive(Clone, Debug, PartialEq)]
pub struct InputPin {
    /// The bound input resource.
    pub resource: ResourceRef,
    /// The content digest the pinned source carried (`Source` inputs).
    pub content_digest: Option<String>,
    /// The source state the pinned base carried (`Source` inputs).
    pub source_state: Option<String>,
}

/// The admission-time input snapshot (GIA-002 Input Snapshot).
///
/// The revision is the committed base the inputs were resolved
/// against; members record what the job provably reads. `None`
/// revision means the job has no project inputs.
#[derive(Clone, Debug, PartialEq)]
pub struct InputSnapshot {
    /// The committed revision the inputs pinned.
    pub revision: Option<ProjectRevision>,
    /// The pinned inputs.
    pub members: Vec<InputPin>,
}

/// What the adapter hands the job coordinator at admission.
///
/// Carries the provider's work description — including `prompt`, which
/// is *job input*, not metadata: it must never be copied into
/// provenance records, media records, or project Asset/Source fields
/// (the leaked-secret bound, GIA-003).
#[derive(Clone, PartialEq)]
pub struct JobSpec {
    /// The job kind.
    pub kind: JobKind,
    /// The output kind.
    pub output: OutputKind,
    /// The provider registry key the caller named.
    pub provider: String,
    /// The bound inputs (already scope-checked at admission).
    pub inputs: Vec<ResourceRef>,
    /// Optional generation prompt — provider input, never disclosed
    /// back through `job.get` or provenance.
    pub prompt: Option<String>,
}

impl std::fmt::Debug for JobSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The prompt is a secret-class input: redact it so `{:?}` of a
        // spec or record can never spill it into logs.
        f.debug_struct("JobSpec")
            .field("kind", &self.kind)
            .field("output", &self.output)
            .field("provider", &self.provider)
            .field("inputs", &self.inputs)
            .field("prompt", &self.prompt.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// The cancel request's fate — the adapter's linkage axis, distinct
/// from the provider's [`JobPhase`] (GIA-002).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CancelLink {
    /// No cancel was ever requested.
    NotRequested,
    /// The provider accepted the request; confirmation is still
    /// pending.
    Requested,
    /// The provider confirmed the stop.
    Confirmed,
    /// The provider did not answer the request — the job's fate is
    /// unknown; reporting `cancelled` would be a lie.
    Unconfirmed,
}

impl CancelLink {
    /// The wire status name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::NotRequested => "none",
            Self::Requested => "requested",
            Self::Confirmed => "confirmed",
            Self::Unconfirmed => "unconfirmed",
        }
    }
}

/// What the provider says about a cancel request.
#[derive(Clone, Debug, PartialEq)]
pub enum CancelResponse {
    /// The provider stopped the job — the linkage may be marked
    /// `Confirmed`.
    Confirmed,
    /// The provider accepted the request; the stop is not yet
    /// confirmed.
    Pending,
    /// The provider did not answer — `Unconfirmed`, never a silent
    /// success.
    Unreachable,
    /// The job was already past cancellation — the settle race went to
    /// the job, not the request; the carried phase is the actual
    /// truth the caller must see.
    Settled {
        /// The provider's actual phase when the cancel arrived.
        phase: JobPhase,
    },
}

/// The seam to the Synvelis-owned non-RT job coordinator.
///
/// Implemented by the application-side coordinator in production and
/// by the deterministic [`crate::stub::ProviderStub`] in the `PoC`.
/// The adapter never fabricates job state — every answer comes
/// through this seam.
pub trait JobPort: Send + Sync {
    /// Admits a job. The coordinator owns the `JobRef`.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — the coordinator declined admission.
    #[allow(clippy::result_large_err)] // the typed envelope is the contract
    fn submit(&self, spec: &JobSpec) -> Result<JobRef, ErrorEnvelope>;

    /// The provider's current truth for `job`.
    fn status(&self, job: &JobRef) -> JobStatus;

    /// Requests cancellation. The response is honest about what the
    /// provider could confirm — a non-answer is [`CancelResponse::Unreachable`],
    /// an already-settled job is [`CancelResponse::Settled`].
    fn request_cancel(&self, job: &JobRef) -> CancelResponse;

    /// The provider's provenance claim for `job` — declared/attested
    /// producer/model/rights facts the adapter maps to evidence levels
    /// (default: an all-absent claim → all `Unknown`).
    fn claim(&self, job: &JobRef) -> crate::provenance::ProviderClaim {
        let _ = job;
        crate::provenance::ProviderClaim::default()
    }
}

// ── Adapter-side records ─────────────────────────────────────

/// One materialized output — the job's declared output resolved
/// through the media-verification seam.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputReport {
    /// The staged-media reference.
    pub media: MediaRef,
    /// The verification lifecycle the media book recorded.
    pub lifecycle: crate::media::MediaLifecycle,
    /// The verified descriptor — `Some` only for `Verified`.
    pub verified: Option<crate::media::VerifiedMedia>,
    /// A bounded rejection/unavailability reason, when the output did
    /// not verify.
    pub reason: Option<String>,
}

/// The materialized job result — computed once when the provider
/// reports `Completed`, idempotent thereafter.
#[derive(Clone, Debug, PartialEq)]
pub struct JobResult {
    /// Per-output verification reports.
    pub outputs: Vec<OutputReport>,
    /// The bounded generation-provenance record.
    pub provenance: crate::provenance::GenerationProvenance,
}

/// One adapter-side job record — correlation, never authority.
#[derive(Debug)]
pub struct JobRecord {
    /// The gateway operation the submit ran under (the caller-visible
    /// handle for `job.get`/`job.cancel`).
    pub operation: OperationId,
    /// The caller's request id that submitted the job.
    pub request: RequestId,
    /// The coordinator's job reference.
    pub job: JobRef,
    /// Owning principal — scoping, not authority.
    pub principal: Principal,
    /// Owning project.
    pub project: ProjectId,
    /// The task the submit was authorized under.
    pub task: Option<TaskRef>,
    /// The grant the submit was authorized under — revocation of this
    /// grant is what links to the job's cancel request.
    pub grant: Option<GrantRef>,
    /// The admitted spec (input bindings included).
    pub spec: JobSpec,
    /// The admission-time input snapshot.
    pub snapshot: InputSnapshot,
    /// The cancel linkage axis.
    pub cancel: CancelLink,
    /// Last provider-observed phase.
    pub last_phase: JobPhase,
    /// The materialized result, once the job completed.
    pub result: Option<JobResult>,
}

impl JobRecord {
    /// The ownership scope — one session's jobs are invisible to a
    /// caller bound on a different (principal, project, task, grant)
    /// tuple; foreign and unknown share one `None`.
    fn same_scope(&self, req: &OperationRequest) -> bool {
        self.principal == *req.caller().principal()
            && self.project == req.project()
            && self.task.as_ref() == req.task()
            && self.grant.as_ref() == req.grant()
    }

    /// Two records share an ownership scope — the same
    /// (principal, project, task, grant) tuple `same_scope` checks
    /// against a request.
    fn same_owner(&self, other: &Self) -> bool {
        self.principal == other.principal
            && self.project == other.project
            && self.task == other.task
            && self.grant == other.grant
    }

    /// Applies the provider's cancel answer to the linkage axis.
    ///
    /// `Settled { Cancelled }` upgrades the link to `Confirmed` — the
    /// provider's own settled report *is* the confirmation. Every
    /// other phase keeps the link honest: `Requested`/`Unconfirmed`
    /// never become `Confirmed` without provider evidence.
    pub fn note_cancel(&mut self, response: &CancelResponse) {
        match response {
            CancelResponse::Confirmed => {
                self.cancel = CancelLink::Confirmed;
                self.last_phase = JobPhase::Cancelled;
            }
            CancelResponse::Pending => self.cancel = CancelLink::Requested,
            CancelResponse::Unreachable => self.cancel = CancelLink::Unconfirmed,
            CancelResponse::Settled { phase } => {
                if *phase == JobPhase::Cancelled {
                    self.cancel = CancelLink::Confirmed;
                }
                self.last_phase = phase.clone();
            }
        }
    }
}

/// The adapter's job book — bounded, owner-scoped.
#[derive(Debug)]
pub struct JobBook {
    map: BTreeMap<OperationId, JobRecord>,
    order: std::collections::VecDeque<OperationId>,
    /// Cumulative admissions per grant — the execution budget's
    /// spend side. Never decremented: eviction drops correlation
    /// records, never the fact that a job was admitted.
    spent: BTreeMap<GrantRef, u64>,
    cap: usize,
}

impl JobBook {
    /// An empty book holding at most `cap` records.
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            map: BTreeMap::new(),
            order: std::collections::VecDeque::new(),
            spent: BTreeMap::new(),
            cap,
        }
    }

    /// Inserts the record; the oldest *same-scope* entry is evicted
    /// when the book is full — a flooding caller can never evict
    /// another caller's correlation data. The record itself carries
    /// the scope: the engine builds it from the request's own
    /// caller/task/grant tuple.
    ///
    /// Returns `false` when the record could not be admitted (id
    /// collision or the book saturated by other scopes) — the caller
    /// must not report admission then, and must not abandon the
    /// provider-side job silently.
    ///
    /// # Panics
    /// Panics on an internal order/map disagreement (a bug, not a
    /// runtime condition).
    pub fn insert(&mut self, record: JobRecord) -> bool {
        if self.map.contains_key(&record.operation) {
            return false;
        }
        while self.map.len() >= self.cap {
            let victim = self
                .order
                .iter()
                .position(|op| self.map.get(op).is_some_and(|r| r.same_owner(&record)))
                .map(|pos| self.order.remove(pos).expect("position found"));
            match victim {
                Some(oldest) => {
                    self.map.remove(&oldest);
                }
                None => return false,
            }
        }
        self.order.push_back(record.operation);
        self.map.insert(record.operation, record);
        true
    }

    /// Spends one unit of `grant`'s execution budget — atomically
    /// with every other admission under this lock, so concurrent
    /// submits can never both pass a check-and-spend. Returns `false`
    /// when the budget is exhausted; nothing is reserved then.
    pub fn try_spend(&mut self, grant: &GrantRef, limit: u64) -> bool {
        let spent = self.spent.entry(grant.clone()).or_insert(0);
        if *spent >= limit {
            return false;
        }
        *spent += 1;
        true
    }

    /// Refunds one budget unit — only when the provider admission the
    /// spend reserved never happened (submit error). A job admitted
    /// at the provider keeps its spend even if the record is later
    /// dropped or evicted: the budget bounds work dispatched, not
    /// records retained.
    pub fn unspend(&mut self, grant: &GrantRef) {
        if let Some(spent) = self.spent.get_mut(grant)
            && *spent > 0
        {
            *spent -= 1;
        }
    }

    /// The record `operation` names iff it belongs to this request's
    /// ownership scope — the same existence blinding the op book and
    /// proposal store use.
    #[must_use]
    pub fn scoped(&self, operation: OperationId, req: &OperationRequest) -> Option<&JobRecord> {
        let record = self.map.get(&operation)?;
        record.same_scope(req).then_some(record)
    }

    /// The mutable form of [`Self::scoped`].
    pub fn scoped_mut(
        &mut self,
        operation: OperationId,
        req: &OperationRequest,
    ) -> Option<&mut JobRecord> {
        let record = self.map.get_mut(&operation)?;
        record.same_scope(req).then_some(record)
    }

    /// How many jobs `grant` admitted — the execution-budget spend.
    /// Cumulative: eviction and terminal settlement never refund.
    #[must_use]
    pub fn count_for_grant(&self, grant: &GrantRef) -> u64 {
        self.spent.get(grant).copied().unwrap_or(0)
    }

    /// Every operation id recorded under `grant` (revocation sweep).
    #[must_use]
    pub fn operations_for_grant(&self, grant: &GrantRef) -> Vec<OperationId> {
        self.map
            .values()
            .filter(|r| r.grant.as_ref() == Some(grant))
            .map(|r| r.operation)
            .collect()
    }

    /// The record `operation` names by raw id — the admin-side sweep
    /// (`note_grant_revoked`) and introspection use this; it is *not*
    /// caller-scoped.
    #[must_use]
    pub fn get(&self, operation: OperationId) -> Option<&JobRecord> {
        self.map.get(&operation)
    }

    /// The mutable form of [`Self::get`].
    pub fn get_mut(&mut self, operation: OperationId) -> Option<&mut JobRecord> {
        self.map.get_mut(&operation)
    }

    /// Number of records (test/introspection).
    #[must_use]
    pub fn count(&self) -> usize {
        self.map.len()
    }
}

/// A thread-safe job book — the engine's record store.
#[derive(Debug)]
pub struct SharedJobBook {
    book: Mutex<JobBook>,
}

impl SharedJobBook {
    /// An empty shared book.
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            book: Mutex::new(JobBook::new(cap)),
        }
    }

    /// Runs `f` under the book lock — serialization is the budget and
    /// linkage consistency boundary.
    ///
    /// # Panics
    /// Panics if the book lock is poisoned.
    pub fn with<T>(&self, f: impl FnOnce(&mut JobBook) -> T) -> T {
        f(&mut self.book.lock().expect("job book poisoned"))
    }
}

/// The serialized-args `operation` field shared by `job.get` and
/// `job.cancel`.
#[allow(clippy::result_large_err)] // the typed envelope is the contract
pub(crate) fn operation_arg(req: &OperationRequest) -> Result<OperationId, ErrorEnvelope> {
    let value = req
        .args()
        .get("operation")
        .cloned()
        .ok_or_else(|| crate::prepare::violation("args.operation", "missing required field"))?;
    serde_json::from_value::<OperationId>(value)
        .map_err(|e| crate::prepare::violation("args.operation", e.to_string()))
}

/// Provider-supplied free text (failure/rejection reasons) is
/// bounded before it can leave the adapter — the same 256-char
/// bound provenance claims use.
pub(crate) fn bounded_reason(reason: &str) -> String {
    reason.chars().take(256).collect()
}

/// The disclosed job result `job.get` emits — bounded: no bytes, no
/// prompt, no grant credentials, only refs/digests/evidence.
#[must_use]
pub fn result_json(record: &JobRecord) -> Value {
    let snapshot = serde_json::json!({
        "revision": record.snapshot.revision.map(|r| r.0),
        "members": record
            .snapshot
            .members
            .iter()
            .map(|m| serde_json::json!({
                "resource": tagged(&m.resource),
                "content_digest": m.content_digest,
                "source_state": m.source_state,
            }))
            .collect::<Vec<_>>(),
    });
    let mut result = serde_json::json!({
        "operation": record.operation.to_tagged(),
        "job": record.job.as_str(),
        "provider": record.spec.provider,
        "kind": record.spec.kind.name(),
        "output": record.spec.output.name(),
        "phase": record.last_phase.name(),
        "cancel": record.cancel.name(),
        "snapshot": snapshot,
    });
    if let JobPhase::Failed { reason } = &record.last_phase {
        result["failure"] = serde_json::json!(bounded_reason(reason));
    }
    if let Some(res) = &record.result {
        result["outputs"] = serde_json::json!(
            res.outputs
                .iter()
                .map(|o| serde_json::json!({
                    "media": o.media.as_str(),
                    "lifecycle": o.lifecycle.name(),
                    "digest": o.verified.as_ref().map(|v| v.content_digest.clone()),
                    "format": o.verified.as_ref().map(|v| v.media_format.clone()),
                    "byte_len": o.verified.as_ref().map(|v| v.byte_len),
                    "reason": o.reason,
                }))
                .collect::<Vec<_>>()
        );
        result["provenance"] = res.provenance.disclosed();
    }
    result
}
